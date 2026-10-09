//! Aggregated traffic statistics (program lookup decoupled out).

use crate::Service;
use crate::networking::types::address_port_pair::AddressPortPair;
use crate::networking::types::data_info::DataInfo;
use crate::networking::types::data_info_host::DataInfoHost;
use crate::networking::types::dropped_packets::DroppedPackets;
use crate::networking::types::host::Host;
use crate::networking::types::info_address_port_pair::InfoAddressPortPair;
use crate::utils::types::timestamp::Timestamp;
use std::collections::HashMap;
use std::collections::hash_map::Entry;

/// Struct containing overall traffic statistics and data.
#[derive(Debug, Default, Clone)]
pub struct InfoTraffic {
    /// Total amount of exchanged data.
    pub tot_data_info: DataInfo,
    /// Number of dropped packets, if applicable.
    pub dropped_packets: Option<DroppedPackets>,
    /// Timestamp of the latest parsed packet.
    pub last_packet_timestamp: Timestamp,
    /// Map of the traffic.
    pub map: HashMap<AddressPortPair, InfoAddressPortPair>,
    /// Map of the upper layer services with their data info.
    pub services: HashMap<Service, DataInfo>,
    /// Map of the hosts with their data info.
    pub hosts: HashMap<Host, DataInfoHost>,
}

impl InfoTraffic {
    /// Merge a per-interval snapshot (`msg`) into the accumulated state.
    pub fn refresh(&mut self, msg: &mut Self) {
        let Self {
            tot_data_info,
            dropped_packets,
            last_packet_timestamp,
            map,
            services,
            hosts,
        } = msg;

        self.tot_data_info.refresh(*tot_data_info);
        self.dropped_packets = *dropped_packets;

        if self.last_packet_timestamp.secs() == last_packet_timestamp.secs() {
            last_packet_timestamp.add_secs(1);
        }
        self.last_packet_timestamp = *last_packet_timestamp;

        for (key, value) in &mut *map {
            match self.map.entry(*key) {
                Entry::Occupied(mut o) => o.get_mut().refresh(value),
                Entry::Vacant(v) => {
                    v.insert(value.clone());
                }
            }
        }

        for (key, value) in &*services {
            self.services
                .entry(*key)
                .and_modify(|x| x.refresh(*value))
                .or_insert(*value);
        }

        for (key, value) in &*hosts {
            self.hosts
                .entry(key.clone())
                .and_modify(|x| x.refresh(value))
                .or_insert(*value);
        }
    }

    /// Take the accumulated state and reset `self` to an empty snapshot
    /// (preserving only the timestamp and dropped-packet counters).
    pub fn take_but_leave_something(&mut self) -> Self {
        let info_traffic = Self {
            last_packet_timestamp: self.last_packet_timestamp,
            dropped_packets: self.dropped_packets,
            ..Self::default()
        };
        std::mem::replace(self, info_traffic)
    }
}
