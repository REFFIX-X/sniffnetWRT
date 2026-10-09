use crate::countries::types::country::Country;
use crate::mmdb::types::mmdb_country_entry::MmdbCountryEntry;
use crate::mmdb::types::mmdb_reader::MmdbReader;
use std::net::IpAddr;

#[allow(clippy::module_name_repetitions)]
pub fn get_country(address: &IpAddr, country_db_reader: &MmdbReader) -> Country {
    if let Some(res) = country_db_reader.lookup::<MmdbCountryEntry>(*address) {
        return res.get_country();
    }
    Country::ZZ
}
