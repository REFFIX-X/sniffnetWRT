//! Entry point for starting the `pcap`-based capture backend (IPFIX decoupled out).

use async_channel::Sender;
use dns_lookup::lookup_addr;
use pcap::Address;
use std::collections::HashMap;
use std::net::IpAddr;
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::broadcast::Receiver;

use crate::engine_config::Filters;
use crate::location;
use crate::mmdb::asn::get_asn;
use crate::mmdb::country::get_country;
use crate::mmdb::types::mmdb_reader::MmdbReaders;
use crate::networking::manage_packets::{
    get_address_to_lookup, get_traffic_type, is_local_connection,
};
use crate::networking::parse_packets::parse_packets;
use crate::networking::types::address_port_pair::AddressPortPair;
use crate::networking::types::bogon::is_bogon;
use crate::networking::types::capture_context::{CaptureContext, CaptureSource};
use crate::networking::types::data_info::DataInfo;
use crate::networking::types::data_info_host::DataInfoHost;
use crate::networking::types::host::{Host, HostMessage};
use crate::networking::types::info_traffic::InfoTraffic;
use crate::networking::types::ip_blacklist::IpBlacklist;
use crate::networking::types::traffic_direction::TrafficDirection;
use crate::utils::error_logger::{ErrorLogger, Location};
use crate::utils::formatted_strings::get_domain_from_r_dns;

const REVERSE_DNS_LOOKUP_THREADS: usize = 5;

/// Spawns the backend thread that feeds `tx` with traffic updates.
#[allow(clippy::too_many_arguments)]
pub fn spawn_capture_thread(
    cap_id: usize,
    capture_source: CaptureSource,
    capture_context: CaptureContext,
    mmdb_readers: MmdbReaders,
    ip_blacklist: IpBlacklist,
    filters: Filters,
    tx: Sender<BackendTrafficMessage>,
    freeze_rxs: (Receiver<()>, Receiver<()>),
) {
    let _ = thread::Builder::new()
        .name("thread_parse_packets".to_string())
        .spawn(move || {
            parse_packets(
                cap_id,
                capture_source,
                &mmdb_readers,
                &ip_blacklist,
                capture_context,
                filters,
                &tx,
                freeze_rxs,
            );
        })
        .log_err(location!());
}

/// Spawns the pool of reverse-DNS lookup threads and returns the resolution state wired to it.
pub fn spawn_reverse_dns_pool(mmdb_readers: &MmdbReaders) -> AddressesResolutionState {
    let (lookup_request_tx, lookup_request_rx) = async_channel::unbounded();
    let (lookup_result_tx, lookup_result_rx) = std::sync::mpsc::channel();
    for i in 0..REVERSE_DNS_LOOKUP_THREADS {
        let lookup_request_rx = lookup_request_rx.clone();
        let lookup_result_tx = lookup_result_tx.clone();
        let mmdb_readers = mmdb_readers.clone();
        let _ = thread::Builder::new()
            .name(format!("thread_reverse_dns_lookups_{i}"))
            .spawn(move || {
                reverse_dns_lookups(&lookup_request_rx, &lookup_result_tx, &mmdb_readers);
            })
            .log_err(location!());
    }
    AddressesResolutionState::new(lookup_request_tx, lookup_result_rx)
}

/// Returns true if a tick was sent, false otherwise.
pub fn maybe_send_tick(
    cap_id: usize,
    info_traffic_msg: &mut InfoTraffic,
    first_packet_ticks: &mut Option<Instant>,
    tx: &Sender<BackendTrafficMessage>,
    resolutions_state: &mut AddressesResolutionState,
) -> bool {
    if first_packet_ticks.is_some_and(|i| i.elapsed() >= Duration::from_secs(1)) {
        *first_packet_ticks =
            first_packet_ticks.and_then(|i| i.checked_add(Duration::from_secs(1)));
        let _ = tx.send_blocking(BackendTrafficMessage::TickRun(
            cap_id,
            info_traffic_msg.take_but_leave_something(),
            resolutions_state.new_hosts_to_send(),
            false,
        ));
        true
    } else {
        false
    }
}

fn reverse_dns_lookups(
    lookup_request_rx: &async_channel::Receiver<(AddressPortPair, TrafficDirection, Vec<Address>)>,
    lookup_result_tx: &std::sync::mpsc::Sender<HostMessage>,
    mmdb_readers: &MmdbReaders,
) {
    while let Ok((key, traffic_direction, interface_addresses)) = lookup_request_rx.recv_blocking()
    {
        let address_to_lookup = get_address_to_lookup(&key, traffic_direction);

        let lookup_result = lookup_addr(&address_to_lookup);

        let traffic_type =
            get_traffic_type(&address_to_lookup, &interface_addresses, traffic_direction);
        let is_loopback = address_to_lookup.is_loopback();
        let is_local = is_local_connection(&address_to_lookup, &interface_addresses);
        let is_bogon = is_bogon(&address_to_lookup);
        let country = get_country(&address_to_lookup, &mmdb_readers.country);
        let asn = get_asn(&address_to_lookup, &mmdb_readers.asn);
        let rdns = if let Ok(result) = lookup_result {
            if result.is_empty() {
                address_to_lookup.to_string()
            } else {
                result
            }
        } else {
            address_to_lookup.to_string()
        };
        let new_host = Host {
            domain: get_domain_from_r_dns(rdns.clone()),
            asn,
            country,
        };

        let data_info_host = DataInfoHost {
            data_info: DataInfo::default(),
            is_local,
            is_bogon,
            is_loopback,
            traffic_type,
        };

        let msg_data = HostMessage {
            host: new_host,
            data_info_host,
            address_to_lookup,
            rdns,
        };

        let _ = lookup_result_tx.send(msg_data);
    }
}

pub struct AddressesResolutionState {
    pub(crate) lookup_request_tx:
        async_channel::Sender<(AddressPortPair, TrafficDirection, Vec<Address>)>,
    lookup_result_rx: std::sync::mpsc::Receiver<HostMessage>,
    pub(crate) addresses_waiting_resolution: HashMap<IpAddr, DataInfo>,
    pub(crate) addresses_resolved: HashMap<IpAddr, Host>,
}

impl AddressesResolutionState {
    fn new(
        lookup_request_tx: async_channel::Sender<(AddressPortPair, TrafficDirection, Vec<Address>)>,
        lookup_result_rx: std::sync::mpsc::Receiver<HostMessage>,
    ) -> Self {
        Self {
            lookup_request_tx,
            lookup_result_rx,
            addresses_waiting_resolution: HashMap::new(),
            addresses_resolved: HashMap::new(),
        }
    }

    pub(crate) fn new_hosts_to_send(&mut self) -> Vec<HostMessage> {
        let mut new_hosts = Vec::new();
        while let Ok(mut host_msg) = self.lookup_result_rx.try_recv() {
            let address_to_lookup = host_msg.address_to_lookup;
            let other_data = self
                .addresses_waiting_resolution
                .remove(&address_to_lookup)
                .unwrap_or_default();
            host_msg.data_info_host.data_info = other_data;
            self.addresses_resolved
                .insert(address_to_lookup, host_msg.host.clone());
            new_hosts.push(host_msg);
        }
        new_hosts
    }
}

pub enum BackendTrafficMessage {
    TickRun(usize, InfoTraffic, Vec<HostMessage>, bool),
    PendingHosts(usize, Vec<HostMessage>),
    OfflineGap(usize, u32),
}
