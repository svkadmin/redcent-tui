// src/scripts.rs

use crate::{MenuNode, OsDistribution};
use std::{cell::RefCell, rc::Rc};

// Helper macro to create a leaf node (an item)
macro_rules! item {
    ($name:expr, $func:expr) => {
        Rc::new(RefCell::new(MenuNode::Item {
            name: $name.to_string(),
            script_fn: $func,
            selected: false,
        }))
    };
}

// Helper macro to create a branch node (a sub-menu)
macro_rules! menu {
    ($name:expr, $($child:expr),*) => {
        Rc::new(RefCell::new(MenuNode::Menu {
            name: $name.to_string(),
            children: vec![$($child),*],
        }))
    };
}

pub fn build_menu_tree(os: OsDistribution) -> Rc<RefCell<MenuNode>> {
    menu!("Main Menu",
        menu!("Graphical Environments",
            menu!("Gnome DE",
                menu!("Environment Installation",
                    item!("Minimal Installation", scripts_gnome::minimal_install),
                    item!("Full Installation", scripts_gnome::full_install)
                ),
                menu!("Customization",
                    menu!("Extensions",
                        menu!("Tiling WM",
                        ), // Placeholder for Forge, PaperWM, Tiling, etc.
                        menu!("Top Bar",
                        ), // Placeholder for Vitals, Status area horizontal spacing, etc.
                        menu!("Desktop Functions",
                        ), // Placeholder for Just Perfection, etc.
                        menu!("Search", 
                        ) // Placeholder for Search Light
                    )
                )   
            ),
            menu!("Sway WM",
                menu!("Environment Installation",
                    item!("Compile from Source", scripts_sway::compile_from_source)
                ),
                menu!("Customization",
                    item!("Wofi", scripts_sway::install_wofi)
                )
            )
        ),
        menu!("Repositories",
            // The "view installed" action is not a script, so it's not included here.
            // This would require a different kind of action handling.
            menu!("Add Repositories",
                item!("CEPH", scripts_repos::add_ceph),
                item!(if os == OsDistribution::Rhel { "CodeReady Builder" } else { "CRB" }, scripts_repos::add_crb),
                item!("EPEL", scripts_repos::add_epel),
                item!("Flathub", scripts_repos::add_flathub),
                item!("Real-Time (RT)", scripts_repos::add_rt),
                item!("High Availability (HA)", scripts_repos::add_ha)
            )
        ),
        menu!("Virtualization",
            item!("KVM (Core & Tools)", scripts_virt::install_kvm),
            menu!("Cockpit",
                item!("Minimal Install", scripts_virt::install_cockpit_minimal),
                item!("Full Install (with Machines)", scripts_virt::install_cockpit_full)
            )
        ),
        menu!("Networking",
            menu!("NetworkManager",
                item!("OpenVPN", scripts_net::install_vpn_ovpn),
                item!("OpenConnect", scripts_net::install_vpn_oconn),
                item!("L2TP", scripts_net::install_vpn_l2tp),
                item!("LibreSwan", scripts_net::install_vpn_lswan),
                item!("StrongSwan", scripts_net::install_vpn_sswan),
                item!("PPTP", scripts_net::install_vpn_pptp)

                // Placeholders for VPN scripts
            ),
            menu!("KVM (libvirt networks)",
                // Placeholders for libvirt network scripts
            )
        ),
        menu!("Hardening",
            // Placeholders for hardening scripts
        )
    )
}

// --- Script Functions ---

mod scripts_gnome {
    pub fn minimal_install() -> &'static str {
        "sudo dnf install -y gdm gnome-browser-connector\nsudo systemctl set-default graphical.target"
    }
    pub fn full_install() -> &'static str {
        "sudo dnf groupinstall -y 'Workstation'\nsudo systemctl set-default graphical.target"
    }
}

mod scripts_sway {
    pub fn compile_from_source() -> &'static str {
        "# This is a complex process and requires many dependencies.\n# This script is a placeholder for the required commands.\nsudo dnf install -y ninja-build meson gcc wayland-devel wayland-protocols-devel libinput-devel libxcb-devel libxkbcommon-devel pixman-devel"
    }
    pub fn install_wofi() -> &'static str {
        "sudo dnf install -y wofi"
    }
}

mod scripts_repos {
    pub fn add_ceph() -> &'static str {
        "sudo dnf install -y ceph-common"
    }
    pub fn add_crb() -> &'static str {
        // The command depends on the OS, which is handled by the script generation logic,
        // but we can provide a generic placeholder or the RHEL version.
        "sudo dnf config-manager --set-enabled codeready-builder-for-rhel-10-rhui-rpms || sudo dnf config-manager --set-enabled crb"
    }
    pub fn add_epel() -> &'static str {
        "sudo dnf install -y epel-release"
    }
    pub fn add_flathub() -> &'static str {
        "sudo flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo"
    }
    pub fn add_rt() -> &'static str {
        "sudo dnf config-manager --set-enabled rt"
    }
    pub fn add_ha() -> &'static str {
        "sudo dnf config-manager --set-enabled ha"
    }
}

mod scripts_virt {
    pub fn install_kvm() -> &'static str {
        "sudo dnf install -y @virtualization\nsudo systemctl enable --now libvirtd"
    }
    pub fn install_cockpit_minimal() -> &'static str {
        "sudo dnf install -y cockpit\nsudo systemctl enable --now cockpit.socket\nsudo firewall-cmd --add-service=cockpit --permanent\nsudo firewall-cmd --reload"
    }
    pub fn install_cockpit_full() -> &'static str {
        "sudo dnf install -y cockpit cockpit-machines\nsudo systemctl enable --now cockpit.socket\nsudo firewall-cmd --add-service=cockpit --permanent\nsudo firewall-cmd --reload"
    }
}
mod scripts_net {
    pub fn install_vpn_ovpn() -> &'static str {
        "sudo dnf install -y NetworkManager-openvpn NetworkManager-openvpn-gnome"
    }
    pub fn install_vpn_l2tp() -> &'static str {
        "sudo dnf install -y NetworkManager-l2tp NetworkManager-l2tp-gnome"
    }
    pub fn install_vpn_sswan() -> &'static str {
        "sudo dnf install -y strongswan strongswan-charon-nm"
    }
    pub fn install_vpn_lswan() -> &'static str {
        "sudo dnf install -y NetworkManager-libreswan NetworkManager-libreswan-gnome"
    }
    pub fn install_vpn_pptp() -> &'static str {
        "sudo dnf install -y NetworkManager-pptp NetworkManager-pptp-gnome"
    }
    pub fn install_vpn_oconn() -> &'static str {
        "sudo dnf install -y NetworkManager-openconnect NetworkManager-openconnect-gnome"
    }

}
