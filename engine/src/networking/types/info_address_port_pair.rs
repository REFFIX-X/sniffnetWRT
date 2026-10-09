//! Per-address:port-pair statistics (program identification decoupled out).

use crate::Service;
use crate::networking::types::data_info::DataInfo;
use crate::networking::types::data_representation::DataRepr;
use crate::networking::types::message_type::MessageType;
use crate::networking::types::traffic_direction::TrafficDirection;
use crate::sort_type::SortType;
use crate::utils::types::timestamp::Timestamp;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::time::Instant;

/// Statistics about a single address:port pair.
#[derive(Clone, Debug)]
pub struct InfoAddressPortPair {
    /// Source MAC address (last seen), if available.
    pub src_mac: Option<[u8; 6]>,
    /// Destination MAC address (last seen), if available.
    pub dst_mac: Option<[u8; 6]>,
    /// Amount of bytes transmitted between the pair.
    pub bytes: u128,
    /// Amount of packets transmitted between the pair.
    pub packets: u128,
    /// First occurrence of the pair.
    pub initial_timestamp: Timestamp,
    /// Last occurrence of the pair.
    pub final_timestamp: Timestamp,
    /// Final instant of occurrence.
    pub final_instant: Instant,
    /// Upper layer service carried by the pair.
    pub service: Service,
    /// Whether the connection is incoming or outgoing.
    pub traffic_direction: TrafficDirection,
    /// Message types exchanged, with their count.
    pub message_types: HashMap<MessageType, usize>,
    /// Latest VLAN ID, if any.
    pub vlan_id: Option<u16>,
    /// Whether the remote address is blacklisted.
    pub is_blacklisted: bool,
}

impl InfoAddressPortPair {
    pub fn refresh(&mut self, other: &Self) {
        let Self {
            src_mac,
            dst_mac,
            bytes,
            packets,
            initial_timestamp,
            final_timestamp,
            final_instant,
            service,
            traffic_direction,
            message_types,
            vlan_id,
            is_blacklisted,
        } = other;

        self.bytes += bytes;
        self.packets += packets;
        self.src_mac = *src_mac;
        self.dst_mac = *dst_mac;
        self.vlan_id = *vlan_id;
        if *initial_timestamp < self.initial_timestamp {
            self.initial_timestamp = *initial_timestamp;
        }
        if *final_timestamp > self.final_timestamp {
            self.final_timestamp = *final_timestamp;
        }
        if *final_instant > self.final_instant {
            self.final_instant = *final_instant;
        }
        self.service = *service;
        self.is_blacklisted = *is_blacklisted;
        self.traffic_direction = *traffic_direction;
        for (message_type, count) in message_types {
            self.message_types
                .entry(*message_type)
                .and_modify(|v| *v += count)
                .or_insert(*count);
        }
    }

    pub fn transmitted_data(&self, data_repr: DataRepr) -> u128 {
        match data_repr {
            DataRepr::Packets => self.packets,
            DataRepr::Bytes => self.bytes,
            DataRepr::Bits => self.bytes * 8,
        }
    }

    pub fn compare(&self, other: &Self, sort_type: SortType, data_repr: DataRepr) -> Ordering {
        match sort_type {
            SortType::Ascending => self
                .transmitted_data(data_repr)
                .cmp(&other.transmitted_data(data_repr)),
            SortType::Descending => other
                .transmitted_data(data_repr)
                .cmp(&self.transmitted_data(data_repr)),
            SortType::Neutral => other.final_timestamp.cmp(&self.final_timestamp),
        }
    }

    pub fn data_info(&self) -> DataInfo {
        let mut data_info = DataInfo::default();
        data_info.add_packets(
            self.packets,
            self.bytes,
            self.traffic_direction,
            self.final_instant,
        );
        data_info
    }
}

impl Default for InfoAddressPortPair {
    fn default() -> Self {
        Self {
            src_mac: None,
            dst_mac: None,
            bytes: 0,
            packets: 0,
            initial_timestamp: Timestamp::default(),
            final_timestamp: Timestamp::default(),
            final_instant: Instant::now(),
            service: Service::default(),
            traffic_direction: TrafficDirection::default(),
            message_types: HashMap::new(),
            vlan_id: None,
            is_blacklisted: false,
        }
    }
}
