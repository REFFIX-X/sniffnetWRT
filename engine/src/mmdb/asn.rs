use crate::mmdb::types::mmdb_asn_entry::MmdbAsnEntry;
use crate::mmdb::types::mmdb_reader::MmdbReader;
use crate::networking::types::asn::Asn;
use std::net::IpAddr;

#[allow(clippy::module_name_repetitions)]
pub fn get_asn(address: &IpAddr, asn_db_reader: &MmdbReader) -> Asn {
    if let Some(res) = asn_db_reader.lookup::<MmdbAsnEntry>(*address) {
        return res.get_asn();
    }
    Asn::default()
}
