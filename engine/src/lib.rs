//! Headless extraction of Sniffnet's packet-capture engine.
//!
//! This crate reuses upstream's `networking` / `mmdb` / `countries` / `utils`
//! modules, with the `iced` GUI, IPFIX collector, program identification, and
//! translations decoupled out. See `PORTING.md` at the repo root.

pub mod countries;
pub mod engine_config;
pub mod mmdb;
pub mod networking;
pub mod sort_type;
pub mod utils;

// Crate-root re-exports that upstream's networking modules rely on.
pub use networking::types::service::Service;
pub use sniffnet_packet_parser::Protocol;
