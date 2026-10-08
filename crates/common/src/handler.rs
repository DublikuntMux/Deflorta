use num_traits::ToPrimitive;
use serde::{
    de::{self, Visitor},
    ser::{self, Serialize},
};
use std::fmt;
type Result<T, E> = std::result::Result<T, E>;

/// A JS function captured from a committed element tree. The function stays in
/// the script host; Rust holds this id and passes it back in events, where it
/// becomes the function again.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Handler {
    /// The `ui.commit` call that captured the function, modulo `GENERATIONS`.
    pub generation: u32,
    pub index: u32,
}

pub const HANDLER: &str = "$deflorta::Handler";

impl Handler {
    /// Generations wrap here so the bits stay exact as a JS number (< 2^53).
    pub const GENERATIONS: u32 = 1 << 20;

    #[must_use]
    pub const fn to_bits(self) -> u64 {
        ((self.generation as u64) << 32) | self.index as u64
    }

    #[must_use]
    pub fn from_bits(bits: u64) -> Self {
        Self {
            generation: (bits >> 32).to_u32().unwrap_or(u32::MAX),
            index: (bits & u64::from(u32::MAX)).to_u32().unwrap_or(u32::MAX),
        }
    }
}

impl Serialize for Handler {
    fn serialize<S: ser::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_newtype_struct(HANDLER, &self.to_bits())
    }
}

impl<'de> de::Deserialize<'de> for Handler {
    fn deserialize<D: de::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Bits;
        impl Visitor<'_> for Bits {
            type Value = Handler;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a function")
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Handler, E> {
                Ok(Handler::from_bits(v))
            }
        }
        d.deserialize_newtype_struct(HANDLER, Bits)
    }
}
