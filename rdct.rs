// main.rs

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use std::{error::Error, io, fs};

/// Enum to represent the detected Linux distribution.
#[derive(Debug, PartialEq, Clone, Copy)]
enum OsDistribution {
    Rhel,
    Centos,
    Unknown,
}

/// Enum to represent the current active menu.
#[derive(Clone, Copy)]
enum MenuItem {
    Home,
    Repos,
    Virt,
    Desktop,
}

/// Enum to represent the currently focused UI panel.
enum ActivePanel {
    Menu,
    Content,
}

/// Holds the application's state.
struct App<'a> {
    active_menu_item: MenuItem,
    repo_list: Vec<(&'a str, bool)>,
    virt_list: Vec<(&'a str, bool)>,
    desktop_list: Vec<(&'a str, bool)>,
    active_panel: ActivePanel,
    menu_index: usize,
    selected_index: usize,
    os_distro: OsDistribution,
}

/// Detects the running OS distribution by parsing /etc/os-release.
fn detect_os() -> OsDistribution {
    if let Ok(content) = fs::read_to_string("/etc/os-release") {
        for line in content.lines() {
            if line.starts_with("ID=") {
                // Extracts the value from ID="value"
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


impl<'a> App<'a> {
    /// Creates a new App instance with default values.
    fn new() -> App<'a> {
        let os_distro = detect_os();
        
        let repo_list = match os_distro {
            OsDistribution::Rhel => vec![
                ("EPEL (Extra Packages for Enterprise Linux)", false),
                ("CodeReady Linux Builder", false),
                ("Enable Real-Time (RT) repository", false),
                ("Enable High Availability (HA) repository", false),
            ],
            OsDistribution::Centos => vec![
                ("EPEL (Extra Packages for Enterprise Linux)", false),
                ("CRB (CodeReady Builder)", false),
                ("Enable Real-Time (RT) repository", false),
                ("Enable High Availability (HA) repository", false),
            ],
            OsDistribution::Unknown => vec![], // No options if OS is unknown
        };

        App {
            active_menu_item: MenuItem::Home,
            repo_list,
            virt_list: vec![
                ("Install KVM & Tools (@virtualization)", false),
                ("Install & Enable Cockpit Web Console", false),
                ("Install virt-manager (GUI Client)", false),
                ("Install Guest Tools (libguestfs-tools)", false),
            ],
            desktop_list: vec![
                ("Minimal GNOME Install", false),
                ("Full GNOME Desktop Environment", false),
            ],
            active_panel: ActivePanel::Menu,
            menu_index: 0,
            selected_index: 0,
            os_distro,
        }
    }

    /// Toggles the selection state of the currently highlighted item.
    fn toggle_selection(&mut self) {
        // Read the index before creating a mutable borrow of a list inside self.
        let selected_index = self.selected_index;
        let list = self.get_active_list_mut();
        if let Some(item) = list.get_mut(selected_index) {
            item.1 = !item.1;
        }
    }

    /// Returns a mutable reference to the list corresponding to the active menu.
    fn get_active_list_mut(&mut self) -> &mut Vec<(&'a str, bool)> {
        match self.active_menu_item {
            MenuItem::Repos => &mut self.repo_list,
            MenuItem::Virt => &mut self.virt_list,
            MenuItem::Desktop => &mut self.desktop_list,
            MenuItem::Home => &mut self.repo_list, // Default, shouldn't be used
        }
    }

    /// Gathers all selected items into a single list.
    fn get_selected_items(&self) -> Vec<String> {
        let mut selected = vec![];
        self.repo_list.iter().filter(|(_, s)| *s).for_each(|(n, _)| selected.push(n.to_string()));
        self.virt_list.iter().filter(|(_, s)| *s).for_each(|(n, _)| selected.push(n.to_string()));
        self.desktop_list.iter().filter(|(_, s)| *s).for_each(|(n, _)| selected.push(n.to_string()));
        selected
    }

    /// Generates the shell commands based on the user's selections.
    fn generate_commands(&self, reboot: bool) -> String {
        let mut commands = String::new();
        commands.push_str("#!/bin/bash\n");
        commands.push_str(&format!("# Commands generated for {:?} by RHEL/CentOS TUI Manager\n", self.os_distro));
        commands.push_str("# Run this script with sudo: sudo bash ./commands.sh\n\n");

        if !self.repo_list.is_empty() {
             // EPEL Repo
            if self.repo_list[0].1 {
                commands.push_str("sudo dnf install -y epel-release\n");
            }
            // Builder Repo
            if self.repo_list[1].1 {
                match self.os_distro {
                    OsDistribution::Rhel => commands.push_str("sudo dnf config-manager --set-enabled codeready-builder-for-rhel-10-rhui-rpms\n"),
                    OsDistribution::Centos => commands.push_str("sudo dnf config-manager --set-enabled crb\n"),
                    OsDistribution::Unknown => commands.push_str("# SKIPPING BUILDER REPO: Unknown OS\n"),
                }
            }
            // RT Repo
            if self.repo_list[2].1 {
                commands.push_str("sudo dnf config-manager --set-enabled rt\n");
            }
            // HA Repo
            if self.repo_list[3].1 {
                commands.push_str("sudo dnf config-manager --set-enabled ha\n");
            }
        }

        // Virtualization Commands
        if self.virt_list[0].1 {
            commands.push_str("sudo dnf install -y @virtualization\n");
            commands.push_str("sudo systemctl enable --now libvirtd\n");
        }
        if self.virt_list[1].1 {
            commands.push_str("sudo dnf install -y cockpit\n");
            commands.push_str("sudo systemctl enable --now cockpit.socket\n");
            commands.push_str("sudo firewall-cmd --add-service=cockpit --permanent\n");
            commands.push_str("sudo firewall-cmd --reload\n");
        }
        if self.virt_list[2].1 {
            commands.push_str("sudo dnf install -y virt-manager\n");
        }
        if self.virt_list[3].1 {
            commands.push_str("sudo dnf install -y libguestfs-tools\n");
        }

        // Desktop Environment Commands
        if self.desktop_list[0].1 {
             commands.push_str("sudo dnf groupinstall -y 'Server with GUI' --exclude=gnome-software,gnome-tour\n");
             commands.push_str("sudo systemctl set-default graphical.target\n");
        }
        if self.desktop_list[1].1 {
            commands.push_str("sudo dnf groupinstall -y 'Workstation'\n");
            commands.push_str("sudo systemctl set-default graphical.target\n");
        }

        if commands.lines().count() <= 3 {
            commands.push_str("\n# No options selected.\n");
        }

        if reboot {
            commands.push_str("\necho 'Installation complete. Rebooting now...'\n");
            commands.push_str("sudo reboot\n");
        }

        commands
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run it
    let app = App::new();
    let res = run_app(&mut terminal, app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        if let Event::Key(key) = event::read()? {
             match key.code {
                KeyCode::Char('q') => return Ok(()),
                KeyCode::Char('i') => {
                    let commands = app.generate_commands(false);
                    execute!(io::stdout(), LeaveAlternateScreen)?;
                    disable_raw_mode()?;
                    println!("--- Generated Commands (Install Only) ---");
                    println!("\n{}", commands);
                    println!("--- End of Commands ---");
                    println!("\nSave the script above and run it with sudo.");
                    return Ok(());
                }
                KeyCode::Char('r') => {
                    let commands = app.generate_commands(true);
                    execute!(io::stdout(), LeaveAlternateScreen)?;
                    disable_raw_mode()?;
                    println!("--- Generated Commands (Install & Reboot) ---");
                    println!("\n{}", commands);
                    println!("--- End of Commands ---");
                    println!("\nSave the script above and run it with sudo.");
                    return Ok(());
                }
                _ => {}
            }

            match app.active_panel {
                ActivePanel::Menu => match key.code {
                    KeyCode::Down => {
                        app.menu_index = (app.menu_index + 1) % 3; // 3 menu items
                    }
                    KeyCode::Up => {
                        app.menu_index = (app.menu_index + 3 - 1) % 3;
                    }
                    KeyCode::Enter | KeyCode::Right | KeyCode::Tab => {
                        app.active_panel = ActivePanel::Content;
                        app.active_menu_item = match app.menu_index {
                            0 => MenuItem::Repos,
                            1 => MenuItem::Virt,
                            2 => MenuItem::Desktop,
                            _ => unreachable!(),
                        };
                        app.selected_index = 0; // Reset content index
                    },
                    _ => {}
                },
                ActivePanel::Content => match key.code {
                    KeyCode::Left | KeyCode::Tab => app.active_panel = ActivePanel::Menu,
                    KeyCode::Up => {
                        if app.selected_index > 0 {
                            app.selected_index -= 1;
                        }
                    }
                    KeyCode::Down => {
                        let list_len = match app.active_menu_item {
                            MenuItem::Repos => app.repo_list.len(),
                            MenuItem::Virt => app.virt_list.len(),
                            MenuItem::Desktop => app.desktop_list.len(),
                            _ => 0,
                        };
                        if list_len > 0 && app.selected_index < list_len - 1 {
                            app.selected_index += 1;
                        }
                    }
                    KeyCode::Enter | KeyCode::Char(' ') => app.toggle_selection(),
                    _ => {}
                },
            }
        }
    }
}

// The Frame does not need a generic Backend parameter.
fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(3)].as_ref())
        .split(f.size());

    let title_text = format!("RHEL/CentOS 10 TUI Manager (Detected: {:?})", app.os_distro);
    let title = Paragraph::new(title_text)
        .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL).style(Style::default()));
    
    f.render_widget(title, chunks[0]);

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(45), Constraint::Percentage(30)].as_ref())
        .split(chunks[1]);

    // --- Menu Panel ---
    let menu_style = match app.active_panel {
        ActivePanel::Menu => Style::default().fg(Color::Yellow),
        _ => Style::default(),
    };
    let menu_block = Block::default().title("Menu").borders(Borders::ALL).style(menu_style);
    let menu_items = vec![
        ListItem::new("Repositories"),
        ListItem::new("Virtualization"),
        ListItem::new("Desktop Environments"),
    ];
    let menu = List::new(menu_items)
        .block(menu_block)
        .highlight_style(Style::default().add_modifier(Modifier::BOLD).bg(Color::DarkGray))
        .highlight_symbol(">> ");
    
    let mut menu_state = ratatui::widgets::ListState::default();
    menu_state.select(Some(app.menu_index));
    f.render_stateful_widget(menu, main_chunks[0], &mut menu_state);


    // --- Content Panel ---
    let content_style = match app.active_panel {
        ActivePanel::Content => Style::default().fg(Color::Yellow),
        _ => Style::default(),
    };
    
    if app.os_distro == OsDistribution::Unknown {
         let warning_text = "WARNING: Could not determine OS distribution.\n\n\
            Please ensure you are running on RHEL 10 or CentOS Stream 10.\n\n\
            Repository options are disabled. Other generated commands may not be correct.";
        let warning_p = Paragraph::new(warning_text)
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::Red))
            .block(Block::default().title("OS Detection Failed").borders(Borders::ALL).style(content_style));
        f.render_widget(warning_p, main_chunks[1]);
    } else {
        let (content_title, list_items) = match app.active_menu_item {
            MenuItem::Home => ("Welcome", vec![ListItem::new("Select a category from the menu.")]),
            MenuItem::Repos => ("Repositories", app.repo_list.iter().map(|(name, selected)| {
                ListItem::new(format!("[{}] {}", if *selected { "x" } else { " " }, name))
            }).collect()),
            MenuItem::Virt => ("Virtualization", app.virt_list.iter().map(|(name, selected)| {
                ListItem::new(format!("[{}] {}", if *selected { "x" } else { " " }, name))
            }).collect()),
            MenuItem::Desktop => ("Desktop Environments", app.desktop_list.iter().map(|(name, selected)| {
                ListItem::new(format!("[{}] {}", if *selected { "x" } else { " " }, name))
            }).collect()),
        };
        
        let content_block = Block::default().title(content_title).borders(Borders::ALL).style(content_style);
        
        if !matches!(app.active_menu_item, MenuItem::Home) {
            let list = List::new(list_items)
                .block(content_block)
                .highlight_style(Style::default().add_modifier(Modifier::BOLD).bg(Color::DarkGray))
                .highlight_symbol(">> ");
            
            let mut list_state = ratatui::widgets::ListState::default();
            if !app.get_active_list_mut().is_empty() {
                 list_state.select(Some(app.selected_index));
            }
            
            f.render_stateful_widget(list, main_chunks[1], &mut list_state);
        } else {
            let welcome_text = Paragraph::new(
                "Welcome to the RHEL/CentOS 10 TUI Manager!\n\n\
                Use the arrow keys to navigate the menu.\n\
                Press Enter to select a category.\n\
                Use Tab or Arrow Keys to switch between panels.\n\
                Use Up/Down arrows to navigate lists.\n\
                Use Space or Enter to toggle selections."
            )
            .wrap(Wrap { trim: true })
            .block(content_block);
            f.render_widget(welcome_text, main_chunks[1]);
        }
    }


    // --- Side Panel (Selected Items & Actions) ---
    let side_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(75), Constraint::Percentage(25)].as_ref())
        .split(main_chunks[2]);

    // Selected items box
    let selected_items = app.get_selected_items();
    let selected_list_items: Vec<ListItem> = selected_items.iter().map(|s| ListItem::new(s.as_str())).collect();
    let selected_list = List::new(selected_list_items)
        .block(Block::default().borders(Borders::ALL).title("Selected Components"));
    f.render_widget(selected_list, side_chunks[0]);

    // Actions box
    let actions_text = "[i] Install\n[r] Install & Reboot";
    let actions_block = Block::default().borders(Borders::ALL).title("Actions");
    let actions = Paragraph::new(actions_text)
        .style(Style::default().fg(Color::Green))
        .block(actions_block);
    f.render_widget(actions, side_chunks[1]);


    // --- Footer ---
    let footer_text = "Navigate [←→↑↓] | Select [Space/Enter] | [i] Install | [r] Install & Reboot | [q] Quit";
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::Cyan))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

