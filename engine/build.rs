use std::borrow::Cow;
use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::Path;

use sniffnet_packet_parser::Protocol;

// ServiceQuery is textually included so `get_valid_service_query` can build keys.
include!("./src/networking/types/service_query.rs");

const SERVICES_LIST_PATH: &str = "./resources/services/services.txt";
const SERVICE_CATEGORIES_LIST_PATH: &str = "./resources/services/categories.txt";

fn main() {
    println!("cargo:rerun-if-changed={SERVICES_LIST_PATH}");
    println!("cargo:rerun-if-changed={SERVICE_CATEGORIES_LIST_PATH}");

    let service_names = build_services_phf();
    build_service_categories_phf(service_names);
}

fn build_services_phf() -> HashSet<String> {
    let out_path = Path::new(&env::var("OUT_DIR").unwrap()).join("services.rs");
    let mut output = BufWriter::new(File::create(out_path).unwrap());

    let mut services_map = phf_codegen::Map::new();
    let mut service_names = HashSet::new();

    let input = BufReader::new(File::open(SERVICES_LIST_PATH).unwrap());
    let mut num_entries = 0;
    for line_res in input.lines() {
        let line = line_res.unwrap();
        if line.trim().is_empty() || line.trim().starts_with('#') {
            continue;
        }
        let mut parts = line.split('\t');
        let name = parts.next().unwrap().trim();
        let val = Cow::Owned(get_valid_service_fmt_const(name));
        service_names.insert(name.to_owned());
        let key = get_valid_service_query(parts.next().unwrap());
        assert!(parts.next().is_none());
        services_map.entry(key, val);
        num_entries += 1;
    }
    assert_eq!(num_entries, 12093);

    writeln!(
        &mut output,
        "#[allow(clippy::unreadable_literal)]\n\
         static SERVICES: phf::Map<ServiceQuery, Service> = {};",
        services_map.build()
    )
    .unwrap();
    service_names
}

fn build_service_categories_phf(mut service_names: HashSet<String>) {
    let out_path = Path::new(&env::var("OUT_DIR").unwrap()).join("service_categories.rs");
    let mut output = BufWriter::new(File::create(out_path).unwrap());

    let mut categories_map = phf_codegen::Map::new();

    let input = BufReader::new(File::open(SERVICE_CATEGORIES_LIST_PATH).unwrap());
    let mut num_entries = 0;
    for line_res in input.lines() {
        let line = line_res.unwrap();
        if line.trim().is_empty() || line.trim().starts_with('#') {
            continue;
        }
        let mut parts = line.split('\t');
        let name = parts.next().unwrap().trim();
        let val = Cow::Owned(get_valid_service_category_fmt_const(
            parts.next().unwrap().trim(),
        ));
        assert!(
            service_names.remove(name),
            "Duplicate or unexpected service: {name}"
        );
        assert!(parts.next().is_none());
        categories_map.entry(name.to_owned(), val);
        num_entries += 1;
    }
    assert!(
        service_names.is_empty(),
        "Services without categories: {service_names:?}"
    );
    assert_eq!(num_entries, 6466);

    writeln!(
        &mut output,
        "#[allow(clippy::unreadable_literal)]\n\
         static SERVICE_CATEGORIES: phf::Map<&'static str, ServiceCategory> = {};",
        categories_map.build()
    )
    .unwrap();
}

fn get_valid_service_category_fmt_const(s: &str) -> String {
    let category = match s {
        "web" => "Web",
        "email" => "Email",
        "chat" => "Chat",
        "media" => "Media",
        "files" => "Files",
        "storage" => "Storage",
        "database" => "Database",
        "remote" => "Remote",
        "management" => "Management",
        "discovery" => "Discovery",
        "network" => "Network",
        "vpn" => "Vpn",
        "identity" => "Identity",
        "printing" => "Printing",
        "gaming" => "Gaming",
        "industrial" => "Industrial",
        "development" => "Development",
        "middleware" => "Middleware",
        "security" => "Security",
        "licensing" => "Licensing",
        "other" => "Other",
        invalid => panic!("Invalid service category found: {invalid}"),
    };
    format!("ServiceCategory::{category}")
}

fn get_valid_service_fmt_const(s: &str) -> String {
    match s.trim() {
        invalid
            if ["", "unknown", "-"].contains(&invalid)
                || !invalid.is_ascii()
                || invalid.starts_with('#')
                || invalid.contains(' ')
                || invalid.contains('?') =>
        {
            panic!("Invalid service name found: {invalid}")
        }
        name => format!("Service::Name(\"{name}\")"),
    }
}

fn get_valid_service_query(s: &str) -> ServiceQuery {
    let mut parts = s.split('/');
    let port = parts.next().unwrap().parse::<u16>().unwrap();
    let protocol = match parts.next().unwrap() {
        "tcp" => Protocol::Tcp,
        "udp" => Protocol::Udp,
        invalid => panic!("Invalid protocol found: {invalid}"),
    };
    assert!(parts.next().is_none());
    ServiceQuery(port, protocol)
}
