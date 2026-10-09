//! Data representation (bytes/packets/bits) — translations decoupled out.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum DataRepr {
    Packets,
    #[default]
    Bytes,
    Bits,
}

impl DataRepr {
    pub(crate) const ALL: [DataRepr; 3] = [DataRepr::Bits, DataRepr::Bytes, DataRepr::Packets];

    /// Human-readable string for an amount of traffic.
    pub fn formatted_string(self, amount: u128) -> String {
        if self == DataRepr::Packets {
            return amount.to_string();
        }

        #[allow(clippy::cast_precision_loss)]
        let mut n = amount as f32;

        let byte_multiple = ByteMultiple::from_amount(amount);

        #[allow(clippy::cast_precision_loss)]
        let multiplier = byte_multiple.multiplier() as f32;
        n /= multiplier;
        if n > 999.0 && byte_multiple != ByteMultiple::PB {
            n = 999.0;
        }
        let precision = usize::from(byte_multiple != ByteMultiple::B && n <= 9.95);
        format!("{n:.precision$} {}", byte_multiple.pretty_print(self))
    }
}

/// Byte/bit multiple for human-readable display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ByteMultiple {
    #[default]
    B,
    KB,
    MB,
    GB,
    TB,
    PB,
}

impl ByteMultiple {
    pub fn multiplier(self) -> u64 {
        match self {
            ByteMultiple::B => 1,
            ByteMultiple::KB => 1_000,
            ByteMultiple::MB => 1_000_000,
            ByteMultiple::GB => 1_000_000_000,
            ByteMultiple::TB => 1_000_000_000_000,
            ByteMultiple::PB => 1_000_000_000_000_000,
        }
    }

    fn from_amount(bytes: u128) -> Self {
        match bytes {
            x if (u128::MIN..u128::from(ByteMultiple::KB.multiplier())).contains(&x) => {
                ByteMultiple::B
            }
            x if (u128::from(ByteMultiple::KB.multiplier())
                ..u128::from(ByteMultiple::MB.multiplier()))
                .contains(&x) =>
            {
                ByteMultiple::KB
            }
            x if (u128::from(ByteMultiple::MB.multiplier())
                ..u128::from(ByteMultiple::GB.multiplier()))
                .contains(&x) =>
            {
                ByteMultiple::MB
            }
            x if (u128::from(ByteMultiple::GB.multiplier())
                ..u128::from(ByteMultiple::TB.multiplier()))
                .contains(&x) =>
            {
                ByteMultiple::GB
            }
            x if (u128::from(ByteMultiple::TB.multiplier())
                ..u128::from(ByteMultiple::PB.multiplier()))
                .contains(&x) =>
            {
                ByteMultiple::TB
            }
            _ => ByteMultiple::PB,
        }
    }

    pub fn get_char(self) -> &'static str {
        match self {
            Self::B => "",
            Self::KB => "K",
            Self::MB => "M",
            Self::GB => "G",
            Self::TB => "T",
            Self::PB => "P",
        }
    }

    pub fn from_char(ch: char) -> Self {
        match ch.to_ascii_uppercase() {
            'K' => ByteMultiple::KB,
            'M' => ByteMultiple::MB,
            'G' => ByteMultiple::GB,
            'T' => ByteMultiple::TB,
            'P' => ByteMultiple::PB,
            _ => ByteMultiple::B,
        }
    }

    fn pretty_print(self, repr: DataRepr) -> String {
        match repr {
            DataRepr::Packets => String::new(),
            DataRepr::Bytes => format!("{}B", self.get_char()),
            DataRepr::Bits => format!("{}b", self.get_char()),
        }
    }
}
