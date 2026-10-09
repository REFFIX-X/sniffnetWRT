//! Capture source / context (live device + offline PCAP; IPFIX decoupled out).

use crate::engine_config::Filters;
use crate::location;
use crate::networking::types::my_device::MyDevice;
use crate::utils::error_logger::{ErrorLogger, Location};
use pcap::{Active, Address, Capture, Device, Error, Packet, Savefile, Stat};
use sniffnet_packet_parser::LinkType;

pub enum CaptureContext {
    Live(Live),
    LiveWithSavefile(LiveWithSavefile),
    Offline(Offline),
    Error(String),
}

impl CaptureContext {
    pub fn new(source: &CaptureSource, pcap_out_path: Option<&String>, filters: &Filters) -> Self {
        let mut cap_type = match source {
            CaptureSource::Device(device) => match CaptureType::from_device(device, pcap_out_path) {
                Ok(c) => c,
                Err(e) => return Self::Error(e),
            },
            CaptureSource::File(file) => match CaptureType::from_file(file) {
                Ok(c) => c,
                Err(e) => return Self::Error(e),
            },
        };

        if filters.is_some_filter_active()
            && let Err(e) = cap_type.set_bpf(filters.bpf())
        {
            return Self::Error(e.to_string());
        }

        let cap = match cap_type {
            CaptureType::Live(cap) => cap,
            CaptureType::Offline(cap) => return Self::new_offline(cap),
        };

        if let Some(out_path) = pcap_out_path {
            match cap.savefile(out_path) {
                Ok(s) => Self::new_live_with_savefile(cap, s),
                Err(e) => Self::Error(e.to_string()),
            }
        } else {
            Self::new_live(cap)
        }
    }

    fn new_live(cap: Capture<Active>) -> Self {
        Self::Live(Live { cap })
    }

    fn new_live_with_savefile(cap: Capture<Active>, savefile: Savefile) -> Self {
        Self::LiveWithSavefile(LiveWithSavefile {
            live: Live { cap },
            savefile,
        })
    }

    fn new_offline(cap: Capture<pcap::Offline>) -> Self {
        Self::Offline(Offline { cap })
    }

    pub fn error(&self) -> Option<&str> {
        match self {
            Self::Error(e) => Some(e),
            _ => None,
        }
    }

    pub fn consume(self) -> (Option<CaptureType>, Option<Savefile>) {
        match self {
            Self::Live(on) => (Some(CaptureType::Live(on.cap)), None),
            Self::LiveWithSavefile(onws) => {
                (Some(CaptureType::Live(onws.live.cap)), Some(onws.savefile))
            }
            Self::Offline(off) => (Some(CaptureType::Offline(off.cap)), None),
            Self::Error(_) => (None, None),
        }
    }

    pub fn link_type(&self) -> Option<LinkType> {
        match self {
            Self::Live(on) => Some(LinkType::from_pcap(on.cap.get_datalink())),
            Self::LiveWithSavefile(onws) => Some(LinkType::from_pcap(onws.live.cap.get_datalink())),
            Self::Offline(off) => Some(LinkType::from_pcap(off.cap.get_datalink())),
            Self::Error(_) => None,
        }
    }
}

pub struct Live {
    cap: Capture<Active>,
}

pub struct LiveWithSavefile {
    live: Live,
    savefile: Savefile,
}

pub struct Offline {
    cap: Capture<pcap::Offline>,
}

pub enum CaptureType {
    Live(Capture<Active>),
    Offline(Capture<pcap::Offline>),
}

impl CaptureType {
    pub fn next_packet(&mut self) -> Result<Packet<'_>, Error> {
        match self {
            Self::Live(on) => on.next_packet(),
            Self::Offline(off) => off.next_packet(),
        }
    }

    pub fn stats(&mut self) -> Result<Stat, Error> {
        match self {
            Self::Live(on) => on.stats(),
            Self::Offline(off) => off.stats(),
        }
    }

    fn from_device(device: &MyDevice, pcap_out_path: Option<&String>) -> Result<Self, String> {
        let inactive = Capture::from_device(device.to_pcap_device()).map_err(|e| e.to_string())?;
        let cap = inactive
            .promisc(false)
            .buffer_size(2_000_000)
            .snaplen(if pcap_out_path.is_some() {
                i32::from(u16::MAX)
            } else {
                200
            })
            .immediate_mode(false)
            .timeout(150)
            .open()
            .map_err(|e| e.to_string())?;
        Ok(Self::Live(cap))
    }

    fn from_file(file: &MyPcapImport) -> Result<Self, String> {
        Ok(Self::Offline(
            Capture::from_file(&file.path).map_err(|e| e.to_string())?,
        ))
    }

    fn set_bpf(&mut self, bpf: &str) -> Result<(), Error> {
        match self {
            Self::Live(cap) => cap.filter(bpf, true),
            Self::Offline(cap) => cap.filter(bpf, true),
        }
    }

    pub fn pause(&mut self) {
        if let Self::Live(cap) = self {
            let _ = cap.filter("less 2", true).log_err(location!());
        }
    }

    pub fn resume(&mut self, filters: &Filters) {
        if let Self::Live(cap) = self {
            if filters.is_some_filter_active() {
                let _ = cap.filter(filters.bpf(), true).log_err(location!());
            } else if cap.filter("", true).log_err(location!()).is_err() {
                let _ = cap.filter("greater 0", true).log_err(location!());
            }
        }
    }
}

#[derive(Clone)]
pub enum CaptureSource {
    Device(MyDevice),
    File(MyPcapImport),
}

impl CaptureSource {
    /// Build a live device source from an interface name (e.g. `wan`).
    pub fn device_from_name(name: &str) -> Self {
        Self::Device(MyDevice::from_name(name))
    }

    pub fn get_addresses(&self) -> &[Address] {
        match self {
            Self::Device(device) => device.get_addresses(),
            Self::File(_) => &[],
        }
    }

    pub fn set_addresses(&mut self) {
        match self {
            Self::Device(my_device) => {
                let mut addresses = Vec::new();
                for dev in Device::list().log_err(location!()).unwrap_or_default() {
                    if matches!(
                        my_device.get_link_type(),
                        Some(LinkType::LinuxSll(_) | LinkType::LinuxSll2(_))
                    ) {
                        addresses.extend(dev.addresses);
                    } else if dev.name.eq(my_device.get_name()) {
                        addresses.extend(dev.addresses);
                        break;
                    }
                }
                my_device.set_addresses(addresses);
            }
            Self::File(_) => {}
        }
    }
}

#[derive(Clone)]
pub struct MyPcapImport {
    path: String,
    link_type: Option<LinkType>,
}

impl MyPcapImport {
    pub fn new(path: String) -> Self {
        Self {
            path,
            link_type: None,
        }
    }
}
