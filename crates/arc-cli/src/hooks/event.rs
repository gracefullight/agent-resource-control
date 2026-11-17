//! Reject input that `Value` cannot round-trip without changing its meaning.
//!
//! Vendor updates replace the whole tool input. Duplicate object keys and
//! numbers outside the exact integer representation are therefore declined
//! before deserializing to `Value`, rather than silently collapsed or rounded.
use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::Value;
use std::{collections::HashSet, fmt};

struct Exact;

impl<'de> Deserialize<'de> for Exact {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(ExactVisitor)
    }
}

struct ExactVisitor;

impl<'de> Visitor<'de> for ExactVisitor {
    type Value = Exact;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON with unique object keys and exact integer numbers")
    }

    fn visit_bool<E: de::Error>(self, _: bool) -> Result<Exact, E> {
        Ok(Exact)
    }
    fn visit_i64<E: de::Error>(self, _: i64) -> Result<Exact, E> {
        Ok(Exact)
    }
    fn visit_u64<E: de::Error>(self, _: u64) -> Result<Exact, E> {
        Ok(Exact)
    }
    fn visit_str<E: de::Error>(self, _: &str) -> Result<Exact, E> {
        Ok(Exact)
    }
    fn visit_unit<E: de::Error>(self) -> Result<Exact, E> {
        Ok(Exact)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Exact, A::Error> {
        while seq.next_element::<Exact>()?.is_some() {}
        Ok(Exact)
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Exact, A::Error> {
        let mut keys = HashSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key) {
                return Err(de::Error::custom("duplicate object key"));
            }
            map.next_value::<Exact>()?;
        }
        Ok(Exact)
    }
}

pub(super) fn decode(bytes: &[u8]) -> Option<Value> {
    serde_json::from_slice::<Exact>(bytes).ok()?;
    let value = serde_json::from_slice(bytes).ok()?;
    exact_numbers(&value).then_some(value)
}

// The typed fixture seam can be called without raw JSON. It cannot detect
// duplicate keys, so raw events must pass through `decode` first.
pub(super) fn exact_numbers(value: &Value) -> bool {
    match value {
        Value::Number(number) => number.as_i64().is_some() || number.as_u64().is_some(),
        Value::Array(values) => values.iter().all(exact_numbers),
        Value::Object(values) => values.values().all(exact_numbers),
        _ => true,
    }
}
