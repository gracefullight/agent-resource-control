//! Reject duplicate keys before preserving the complete vendor input.
//!
//! Preserve the original raw tool-input values when constructing a response,
//! so unrelated decimal and large-integer fields are never rounded.
use super::Vendor;
use serde::de::{self, Deserialize, Deserializer, MapAccess, Visitor};
use serde_json::{value::RawValue, Value};
use std::{collections::BTreeMap, fmt};

struct UniqueObject(BTreeMap<String, Box<RawValue>>);

impl<'de> Deserialize<'de> for UniqueObject {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(UniqueObjectVisitor)
    }
}

struct UniqueObjectVisitor;

impl<'de> Visitor<'de> for UniqueObjectVisitor {
    type Value = UniqueObject;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON with unique object keys")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut values = BTreeMap::new();
        while let Some((key, value)) = map.next_entry::<String, Box<RawValue>>()? {
            if values.insert(key, value).is_some() {
                return Err(de::Error::custom("duplicate object key"));
            }
        }
        Ok(UniqueObject(values))
    }
}

// RawValue validates number syntax without forcing it through f64. Build only
// the adapter's temporary view here; wire_response restores all original raw
// tool-input values. A numeric value too large for Value cannot be a string
// field used for routing, so it can safely be represented by null in the view.
fn view(raw: &RawValue, depth: usize) -> Option<Value> {
    if depth > 128 {
        return None;
    }
    let text = raw.get();
    match text.as_bytes().first()? {
        b'{' => {
            let object: UniqueObject = serde_json::from_str(text).ok()?;
            let mut result = serde_json::Map::new();
            for (key, value) in object.0 {
                result.insert(key, view(&value, depth + 1)?);
            }
            Some(Value::Object(result))
        }
        b'[' => {
            let values: Vec<Box<RawValue>> = serde_json::from_str(text).ok()?;
            values
                .iter()
                .map(|value| view(value, depth + 1))
                .collect::<Option<Vec<_>>>()
                .map(Value::Array)
        }
        b'-' | b'0'..=b'9' => Some(serde_json::from_str(text).unwrap_or(Value::Null)),
        _ => serde_json::from_str(text).ok(),
    }
}

pub(super) fn decode(bytes: &[u8]) -> Option<Value> {
    let raw: Box<RawValue> = serde_json::from_slice(bytes).ok()?;
    view(&raw, 0)
}

#[derive(serde::Deserialize)]
struct RawEvent {
    tool_input: Box<RawValue>,
}

#[derive(serde::Serialize)]
struct ClaudeOutput {
    #[serde(rename = "hookSpecificOutput")]
    specific: ClaudeSpecific,
}

#[derive(serde::Serialize)]
struct ClaudeSpecific {
    #[serde(rename = "hookEventName")]
    event: &'static str,
    #[serde(rename = "updatedInput")]
    input: BTreeMap<String, Box<RawValue>>,
}

#[derive(serde::Serialize)]
struct CursorOutput {
    updated_input: BTreeMap<String, Box<RawValue>>,
}

/// Called only after `decode` has rejected duplicate keys throughout the event.
pub(super) fn wire_response(vendor: Vendor, bytes: &[u8], response: &Value) -> Option<Vec<u8>> {
    let raw: RawEvent = serde_json::from_slice(bytes).ok()?;
    let mut input: BTreeMap<String, Box<RawValue>> =
        serde_json::from_str(raw.tool_input.get()).ok()?;
    let command = match vendor {
        Vendor::Claude => response["hookSpecificOutput"]["updatedInput"]["command"].as_str()?,
        Vendor::Cursor => response["updated_input"]["command"].as_str()?,
    };
    input.insert(
        "command".into(),
        RawValue::from_string(serde_json::to_string(command).ok()?).ok()?,
    );
    match vendor {
        Vendor::Claude => serde_json::to_vec(&ClaudeOutput {
            specific: ClaudeSpecific {
                event: "PreToolUse",
                input,
            },
        })
        .ok(),
        Vendor::Cursor => serde_json::to_vec(&CursorOutput {
            updated_input: input,
        })
        .ok(),
    }
}
