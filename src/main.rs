// src/main.rs
mod scripts;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use std::{cell::RefCell, error::Error, io, fs, process::Command, os::unix::fs::PermissionsExt, rc::Rc};

/// Represents a node in the menu tree. It can be a selectable item or a sub-menu.
pub enum MenuNode {
    Item {
        name: String,
        script_fn: fn() -> &'static str,
        selected: bool,
    },
    Menu {
        name: String,
        children: Vec<Rc<RefCell<MenuNode>>>,
    },
}

impl MenuNode {
    /// Recursively collects all selected script functions.
    fn get_selected_scripts(&self, scripts: &mut Vec<fn() -> &'static str>) {
        match self {
            MenuNode::Item { selected, script_fn, .. } => {
                if *selected {
                    scripts.push(*script_fn);
                }
            }
            MenuNode::Menu { children, .. } => {
                for child in children {
                    child.borrow().get_selected_scripts(scripts);
                }
            }
        }
    }
    
    /// Recursively collects the names of all selected items.
    fn get_selected_item_names(&self, names: &mut Vec<String>) {
        match self {
            MenuNode::Item { name, selected, .. } => {
                if *selected {
                    names.push(name.clone());
                }
            }
            MenuNode::Menu { children, .. } => {
                for child in children {
                    child.borrow().get_selected_item_names(names);
                }
            }
        }
    }
}


/// Enum to represent the detected Linux distribution.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum OsDistribution {
    Rhel,
    Centos,
    Unknown,
}

/// Enum to manage the overall state of the application.
enum AppState {
    Running,
    Finished,
    Saving,
}

/// Enum to tell the main function what to do after the TUI exits.
pub enum ActionAfterExit {
    Quit,
    RunScript(String),
}

/// Holds the application's state.
struct App {
    state: AppState,
    menu_tree: Rc<RefCell<MenuNode>>,
    nav_path: Vec<Rc<RefCell<MenuNode>>>,
    selected_index: usize,
    os_distro: OsDistribution,
    reboot_requested: bool,
    filename_input: String,
    save_status_message: Option<String>,
}

fn detect_os() -> OsDistribution {
    if let Ok(content) = fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if line.starts_with("ID=") {
                let id = line.trim_start_matches("ID=").trim_matches('"');
                return match id {
                    "rhel" => OsDistribution::Rhel,
                    "centos" => OsDistribution::Centos,
                    _ => OsDistribution::Unknown,
                };
            }
        }
    }
    OsDistribution::Unknown
}

impl App {
    /// Creates a new App instance with default values.
    fn new() -> App {
        let os_distro = detect_os();
        let menu_tree = scripts::build_menu_tree(os_distro);
        let nav_path = vec![menu_tree.clone()];

        App {
            state: AppState::Running,
            menu_tree,
            nav_path,
            selected_index: 0,
            os_distro,
            reboot_requested: false,
            filename_input: String::new(),
            save_status_message: None,
        }
    }

    /// Generates the shell commands based on the user's selections.
    fn generate_commands(&self, reboot: bool) -> String {
        let mut command_text = String::new();
        command_text.push_str("#!/bin/bash\n");
        command_text.push_str(&format!("# Commands generated for {:?} by RHEL/CentOS TUI Manager\n", self.os_distro));
        command_text.push_str("# Save this script and run it with sudo: sudo bash ./script.sh\n\n");

        let mut scripts = Vec::new();
        self.menu_tree.borrow().get_selected_scripts(&mut scripts);
        
        if scripts.is_empty() {
             command_text.push_str("\n# No options selected.\n");
        } else {
            for script_fn in scripts {
                command_text.push_str(script_fn());
                command_text.push('\n');
            }
        }

        if reboot {
            command_text.push_str("\necho 'Installation complete. Rebooting now...'\n");
            command_text.push_str("sudo reboot\n");
        }

        command_text
    }

    fn get_current_menu_view(&self) -> &Rc<RefCell<MenuNode>> {
        self.nav_path.last().unwrap()
    }

    /// Returns a cloned Vec of nodes that should be visible in the menu view.
    /// Behavior:
    /// - If we're at the root (nav_path.len()==1) -> show all top-level nodes and, for any Menu node,
    ///   show its immediate children inline (one level deep).
    /// - If we're inside a submenu -> show only that menu's immediate children.
    fn visible_nodes(&self) -> Vec<Rc<RefCell<MenuNode>>> {
        let current_rc = self.get_current_menu_view();
        let current = current_rc.borrow();

        let mut v = Vec::new();
        // If at root, flatten one level deep for convenience (show submenus & their children).
        if self.nav_path.len() == 1 {
            if let MenuNode::Menu { children, .. } = &*current {
                for child in children {
                    // push the top-level child
                    v.push(child.clone());
                    // if it's a menu, push its immediate children (indented logically by UI)
                    if let MenuNode::Menu { children: subchildren, .. } = &*child.borrow() {
                        for sub in subchildren {
                            v.push(sub.clone());
                        }
                    }
                }
            }
        } else {
            // Not root: show only the current menu's immediate children (original behavior).
            if let MenuNode::Menu { children, .. } = &*current {
                for child in children {
                    v.push(child.clone());
                }
            }
        }

        v
    }
    
    fn get_selected_items(&self) -> Vec<String> {
        let mut names = Vec::new();
        self.menu_tree.borrow().get_selected_item_names(&mut names);
        names
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app = App::new();
    let res = run_app(&mut terminal, app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    if let Ok(ActionAfterExit::RunScript(script_content)) = res {
        let script_path = "/tmp/tui_install_script.sh";
        println!("Saving temporary script to {}...", script_path);
        fs::write(script_path, &script_content)?;
        fs::set_permissions(script_path, fs::Permissions::from_mode(0o755))?;

        println!("Exited TUI. Now attempting to run the script with sudo...");
        println!("--- SCRIPT ---");
        println!("{}", script_content);
        println!("--------------");
        
        let status = Command::new("sudo").arg("bash").arg(script_path).status()?;

        if status.success() {
            println!("\nScript executed successfully.");
        } else {
            println!("\nScript execution failed. Please check the output above.");
        }
        fs::remove_file(script_path)?;
    } else if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<ActionAfterExit> {
    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        if let Event::Key(key) = event::read()? {
            match app.state {
                AppState::Running => {
                    // Build a non-borrowing view of visible nodes
                    let visible = app.visible_nodes();
                    // clamp selected index safely (no Ref borrow is held here)
                    if !visible.is_empty() {
                        app.selected_index = app.selected_index.min(visible.len() - 1);
                    } else {
                        app.selected_index = 0;
                    }

                    match key.code {
                        KeyCode::Char('q') => return Ok(ActionAfterExit::Quit),
                        KeyCode::Char('i') => { app.state = AppState::Finished; app.reboot_requested = false; },
                        KeyCode::Char('r') => { app.state = AppState::Finished; app.reboot_requested = true; },
                        KeyCode::Down => {
                            if !visible.is_empty() {
                                app.selected_index = (app.selected_index + 1) % visible.len();
                            }
                        }
                        KeyCode::Up => {
                            if !visible.is_empty() {
                                app.selected_index = (app.selected_index + visible.len() - 1) % visible.len();
                            }
                        }
                        KeyCode::Right | KeyCode::Enter => {
                            if let Some(selected_rc) = visible.get(app.selected_index).cloned() {
                                // borrow mutably only for the brief section that updates the node / nav_path
                                let mut node_mut = selected_rc.borrow_mut();
                                match &mut *node_mut {
                                    MenuNode::Menu { .. } => {
                                        // Navigate into the chosen menu (preserve the nav path)
                                        drop(node_mut);
                                        app.nav_path.push(selected_rc.clone());
                                        app.selected_index = 0;
                                    }
                                    MenuNode::Item { selected, .. } => {
                                        *selected = !*selected;
                                    }
                                }
                            }
                        }
                        KeyCode::Left | KeyCode::Backspace => {
                            if app.nav_path.len() > 1 {
                                app.nav_path.pop();
                                app.selected_index = 0;
                            }
                        }
                        _ => {}
                    }
                },
                AppState::Finished => match key.code {
                    KeyCode::Char('q') => return Ok(ActionAfterExit::Quit),
                    KeyCode::Char('s') => app.state = AppState::Saving,
                    KeyCode::Char('r') => return Ok(ActionAfterExit::RunScript(app.generate_commands(app.reboot_requested))),
                    KeyCode::Esc | KeyCode::Backspace => app.state = AppState::Running,
                    _ => {}
                },
                AppState::Saving => match key.code {
                    KeyCode::Char(c) => app.filename_input.push(c),
                    KeyCode::Backspace => { app.filename_input.pop(); },
                    KeyCode::Esc => { app.state = AppState::Finished; app.filename_input.clear(); app.save_status_message = None; },
                    KeyCode::Enter => {
                        let script = app.generate_commands(app.reboot_requested);
                        match fs::write(&app.filename_input, script) {
                            Ok(_) => app.save_status_message = Some(format!("Saved to {}", app.filename_input)),
                            Err(e) => app.save_status_message = Some(format!("Error: {}", e)),
                        }
                        app.state = AppState::Finished;
                        app.filename_input.clear();
                    }
                    _ => {}
                }
            }
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    match app.state {
        AppState::Finished | AppState::Saving => {
            draw_finished_screen(f, app);
            if let AppState::Saving = app.state {
                draw_saving_popup(f, &app.filename_input);
            }
        },
        AppState::Running => {
            draw_main_ui(f, app);
        }
    }
}

fn draw_main_ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Min(0), // Main content area
            Constraint::Percentage(40), // Script preview
            Constraint::Length(3), // Footer
        ].as_ref())
        .split(f.size());

    // compute path string (short lived borrow)
    let path_str = {
        app.nav_path.iter().map(|node_rc| {
            let node = node_rc.borrow();
            match &*node {
                MenuNode::Menu { name, .. } => name.clone(),
                MenuNode::Item { name, .. } => name.clone(),
            }
        }).collect::<Vec<_>>().join(" > ")
    };

    let title_text = format!("RHEL/CentOS 10 TUI Manager (Detected: {:?})", app.os_distro);
    let title = Paragraph::new(title_text).style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    let main_chunks = Layout::default().direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(chunks[1]);

    // Acquire visible nodes (cloned Rcs) without holding long borrows
    let visible = app.visible_nodes();

    // Build display strings inside a short block so any Ref borrows drop before mutation
    let menu_strings: Vec<String> = {
        let mut out = Vec::new();
        // When at root, visible contains (top-level node, then its children)
        // We want to display indentation for children of menus at root.
        if app.nav_path.len() == 1 {
            // iterate pairs: top-level then its possible inline children
            let mut i = 0usize;
            while i < visible.len() {
                let top_rc = visible[i].clone();
                let top_b = top_rc.borrow();
                match &*top_b {
                    MenuNode::Menu { name: tname, .. } => {
                        // top-level menu header
                        out.push(format!("{} >", tname));
                        i += 1;
                        // push following items that belong to this submenu (one-level deep),
                        // they were appended in visible_nodes directly after their parent.
                        while i < visible.len() {
                            // We peek at next element and check whether it's actually a child: we cannot
                            // distinguish strictly by ownership here, but our visible_nodes created the order top, child, child...
                            // We'll render them indented until we reach another top-level Menu node in the original sequence.
                            let maybe_rc = visible[i].clone();
                            // To decide if it's a child, we check if the parent (top_rc) actually contains it.
                            // This is somewhat costly but safe: check whether parent's children contains maybe_rc by comparing pointer addresses.
                            let mut is_child_of_top = false;
                            if let MenuNode::Menu { children: subchildren, .. } = &*top_b {
                                for s in subchildren {
                                    // compare pointer equality of Rc (pointer address of inner)
                                    if Rc::ptr_eq(&s, &maybe_rc) {
                                        is_child_of_top = true;
                                        break;
                                    }
                                }
                            }
                            if !is_child_of_top {
                                break;
                            }
                            // Render child
                            let child_b = maybe_rc.borrow();
                            match &*child_b {
                                MenuNode::Menu { name: cname, .. } => out.push(format!("  {} >", cname)),
                                MenuNode::Item { name, selected, .. } => {
                                    let prefix = if *selected { "[x]" } else { "[ ]" };
                                    out.push(format!("  {} {}", prefix, name));
                                }
                            }
                            i += 1;
                        }
                    }
                    MenuNode::Item { name, selected, .. } => {
                        let prefix = if *selected { "[x]" } else { "[ ]" };
                        out.push(format!("{} {}", prefix, name));
                        i += 1;
                    }
                }
            }
        } else {
            // not root: visible contains only immediate children of the current menu
            for n in &visible {
                let nb = n.borrow();
                match &*nb {
                    MenuNode::Menu { name, .. } => out.push(format!("{} >", name)),
                    MenuNode::Item { name, selected, .. } => {
                        let prefix = if *selected { "[x]" } else { "[ ]" };
                        out.push(format!("{} {}", prefix, name));
                    }
                }
            }
        }
        out
    };

    // clamp again after we built strings (protect against empty)
    if !menu_strings.is_empty() {
        app.selected_index = app.selected_index.min(menu_strings.len() - 1);
    } else {
        app.selected_index = 0;
    }

    // --- Menu Panel ---
    let menu_block = Block::default().title(path_str).borders(Borders::ALL).style(Style::default().fg(Color::Yellow));
    
    let menu_items: Vec<ListItem> = menu_strings.iter().map(|s| ListItem::new(s.clone())).collect();

    let list = List::new(menu_items)
        .block(menu_block)
        .highlight_style(Style::default().add_modifier(Modifier::BOLD).bg(Color::DarkGray))
        .highlight_symbol(">> ");
    
    let mut list_state = ratatui::widgets::ListState::default();
    if !menu_strings.is_empty() {
        list_state.select(Some(app.selected_index));
    }
    f.render_stateful_widget(list, main_chunks[0], &mut list_state);

    // --- Side Panel (Selected Items) ---
    let selected_items: Vec<ListItem> = app.get_selected_items().iter().map(|s| ListItem::new(s.clone())).collect();
    let selected_list = List::new(selected_items).block(Block::default().borders(Borders::ALL).title("Selected Components"));
    f.render_widget(selected_list, main_chunks[1]);

    // --- Script Preview ---
    let script_content = app.generate_commands(false); // Preview without reboot
    let script_preview = Paragraph::new(script_content)
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title("Generated Script Preview"));
    f.render_widget(script_preview, chunks[2]);

    // --- Footer ---
    let footer_text = "Navigate [←→↑↓] | Select [Enter] | [i] Generate Script | [q] Quit";
    let footer = Paragraph::new(footer_text).style(Style::default().fg(Color::Cyan))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[3]);
}


fn draw_finished_screen(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default().direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref()).split(f.size());
    let script_content = app.generate_commands(app.reboot_requested);
    let title = if app.reboot_requested { "Installation Script (with Reboot)" } else { "Installation Script" };
    let paragraph = Paragraph::new(script_content).wrap(Wrap { trim: true })
        .block(Block::default().title(title).borders(Borders::ALL));
    f.render_widget(paragraph, chunks[0]);

    if let Some(msg) = &app.save_status_message {
        let msg_p = Paragraph::new(msg.as_str()).style(Style::default().fg(Color::Yellow));
        let area = centered_rect(50, 10, f.size());
        f.render_widget(Clear, area);
        f.render_widget(msg_p.block(Block::default().borders(Borders::ALL).title("Status")), area);
        if app.filename_input.is_empty() { 
             app.save_status_message = None;
        }
    }

    let footer_text = "Review Script | [s] Save to File | [r] Run Directly | [q] Quit | [Esc/Backspace] Go Back";
    let footer = Paragraph::new(footer_text).style(Style::default().fg(Color::Cyan))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[1]);
}

fn draw_saving_popup(f: &mut Frame, input: &str) {
    let area = centered_rect(60, 20, f.size());
    let block = Block::default().title("Save Script").borders(Borders::ALL);
    f.render_widget(Clear, area);
    f.render_widget(block, area);

    let popup_chunks = Layout::default().direction(Direction::Vertical).margin(2)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Min(1)].as_ref()).split(area);
    
    let p1 = Paragraph::new("Enter filename (press Enter to save, Esc to cancel):");
    let p2 = Paragraph::new(input).block(Block::default().borders(Borders::ALL));
    f.render_widget(p1, popup_chunks[0]);
    f.render_widget(p2, popup_chunks[1]);
}

/// Helper function to create a centered rectangle for popups
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default().direction(Direction::Vertical)
        .constraints([Constraint::Percentage((100 - percent_y) / 2), Constraint::Percentage(percent_y), Constraint::Percentage((100 - percent_y) / 2)].as_ref())
        .split(r);
    Layout::default().direction(Direction::Horizontal)
        .constraints([Constraint::Percentage((100 - percent_x) / 2), Constraint::Percentage(percent_x), Constraint::Percentage((100 - percent_x) / 2)].as_ref())
        .split(popup_layout[1])[1]
}

