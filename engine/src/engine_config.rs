//! Minimal engine configuration (decoupled from Sniffnet's GUI `Conf`).

use serde::{Deserialize, Deserializer, Serialize};

/// BPF filter state, equivalent to upstream `gui::types::filters::Filters`.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct Filters {
    #[serde(deserialize_with = "deserialize_or_default")]
    pub expanded: bool,
    #[serde(deserialize_with = "deserialize_or_default")]
    pub bpf: String,
}

impl Filters {
    pub fn set_bpf(&mut self, bpf: String) {
        self.bpf = bpf;
    }

    pub fn bpf(&self) -> &str {
        &self.bpf
    }

    pub fn is_some_filter_active(&self) -> bool {
        self.expanded && !self.bpf.trim().is_empty()
    }
}

pub(crate) fn deserialize_or_default<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: Deserialize<'de> + Default,
    D: Deserializer<'de>,
{
    Ok(T::deserialize(deserializer).unwrap_or_default())
}
