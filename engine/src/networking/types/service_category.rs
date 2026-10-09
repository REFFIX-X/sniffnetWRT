//! Upper-layer service categories (icon rendering decoupled out).

use std::fmt::Display;

/// General purpose of an upper-layer service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ServiceCategory {
    Web,
    Email,
    Chat,
    Media,
    Files,
    Storage,
    Database,
    Remote,
    Management,
    Discovery,
    Network,
    Vpn,
    Identity,
    Printing,
    Gaming,
    Industrial,
    Development,
    Middleware,
    Security,
    Licensing,
    #[default]
    Other,
}

impl Display for ServiceCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ServiceCategory::Web => "Web",
            ServiceCategory::Email => "Email",
            ServiceCategory::Chat => "Chat & Messaging",
            ServiceCategory::Media => "Voice & Media",
            ServiceCategory::Files => "File Transfer & Sharing",
            ServiceCategory::Storage => "Storage & Backup",
            ServiceCategory::Database => "Databases & Caches",
            ServiceCategory::Remote => "Remote Access & Execution",
            ServiceCategory::Management => "Monitoring & Administration",
            ServiceCategory::Discovery => "Naming & Service Discovery",
            ServiceCategory::Network => "Network Infrastructure",
            ServiceCategory::Vpn => "VPNs, Tunnels & Proxies",
            ServiceCategory::Identity => "Identity & Directory",
            ServiceCategory::Printing => "Printing & Scanning",
            ServiceCategory::Gaming => "Gaming",
            ServiceCategory::Industrial => "Industrial & Automation",
            ServiceCategory::Development => "Software Development",
            ServiceCategory::Middleware => "Middleware",
            ServiceCategory::Security => "Security",
            ServiceCategory::Licensing => "Software Licensing",
            ServiceCategory::Other => "",
        };
        write!(f, "{s}")
    }
}
