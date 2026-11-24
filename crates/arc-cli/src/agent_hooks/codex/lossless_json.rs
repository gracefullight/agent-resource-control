//! Preserve number lexemes while editing Codex settings through serde_json.
use serde_json::Value;
use std::collections::{HashMap, HashSet};

pub(super) struct Settings {
    pub value: Value,
    numbers: HashMap<String, String>,
}

impl Settings {
    pub fn parse(bytes: &[u8]) -> Result<Self, &'static str> {
        let id = arc_core::process_identity::random_job_id()
            .map_err(|_| "Cannot allocate Codex settings identity.")?;
        let prefix = format!("__arc_json_number_{id}_");
        let mut sanitized = Vec::with_capacity(bytes.len());
        let mut masked = Vec::with_capacity(bytes.len());
        let mut numbers = HashMap::new();
        let mut objects: Vec<Option<HashSet<String>>> = Vec::new();
        let mut expect_key = false;
        let mut index = 0;
        while index < bytes.len() {
            let start = index;
            match bytes[index] {
                b'"' => {
                    index += 1;
                    let mut escaped = false;
                    while index < bytes.len() {
                        let byte = bytes[index];
                        index += 1;
                        if escaped {
                            escaped = false;
                        } else if byte == b'\\' {
                            escaped = true;
                        } else if byte == b'"' {
                            break;
                        }
                    }
                    let token = &bytes[start..index];
                    let string: String = serde_json::from_slice(token)
                        .map_err(|_| "Invalid Codex settings JSON.")?;
                    // A collision must not turn a user's string into a number.
                    if string.starts_with(&prefix) {
                        return Err("Codex settings number identity collision.");
                    }
                    if expect_key {
                        if let Some(Some(keys)) = objects.last_mut() {
                            if !keys.insert(string) {
                                return Err("Duplicate keys in Codex settings.");
                            }
                        }
                        expect_key = false;
                    }
                    sanitized.extend_from_slice(token);
                    masked.extend_from_slice(token);
                }
                b'-' | b'0'..=b'9' => {
                    index += 1;
                    while index < bytes.len()
                        && !matches!(
                            bytes[index],
                            b',' | b']' | b'}' | b':' | b' ' | b'\n' | b'\r' | b'\t'
                        )
                    {
                        index += 1;
                    }
                    let token = std::str::from_utf8(&bytes[start..index])
                        .map_err(|_| "Invalid Codex settings JSON.")?;
                    if !valid_number(token.as_bytes()) {
                        return Err("Invalid Codex settings JSON.");
                    }
                    let marker = format!("{prefix}{}", numbers.len());
                    numbers.insert(marker.clone(), token.to_owned());
                    sanitized.push(b'0');
                    masked.extend_from_slice(format!("\"{marker}\"").as_bytes());
                }
                byte => {
                    index += 1;
                    match byte {
                        b'{' => {
                            objects.push(Some(HashSet::new()));
                            expect_key = true;
                        }
                        b'[' => {
                            objects.push(None);
                            expect_key = false;
                        }
                        b'}' | b']' => {
                            objects.pop();
                            expect_key = false;
                        }
                        b',' => expect_key = matches!(objects.last(), Some(Some(_))),
                        b':' => expect_key = false,
                        _ => {}
                    }
                    sanitized.push(byte);
                    masked.push(byte);
                }
            }
        }
        // Validate the original structure with numbers replaced by zero; this
        // also rejects numeric object keys and malformed literals.
        serde_json::from_slice::<Value>(&sanitized).map_err(|_| "Invalid Codex settings JSON.")?;
        let value = serde_json::from_slice(&masked).map_err(|_| "Invalid Codex settings JSON.")?;
        Ok(Self { value, numbers })
    }

    pub fn encode(&self) -> Result<Vec<u8>, &'static str> {
        let text = serde_json::to_string_pretty(&self.value)
            .map_err(|_| "Cannot encode Codex settings.")?;
        let bytes = text.as_bytes();
        let mut result = Vec::with_capacity(bytes.len());
        let mut index = 0;
        while index < bytes.len() {
            let start = index;
            if bytes[index] != b'"' {
                result.push(bytes[index]);
                index += 1;
                continue;
            }
            index += 1;
            let mut escaped = false;
            while index < bytes.len() {
                let byte = bytes[index];
                index += 1;
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == b'"' {
                    break;
                }
            }
            let token = &text[start + 1..index - 1];
            if let Some(original) = self.numbers.get(token) {
                result.extend_from_slice(original.as_bytes());
            } else {
                result.extend_from_slice(&bytes[start..index]);
            }
        }
        result.push(b'\n');
        Ok(result)
    }
}

fn valid_number(bytes: &[u8]) -> bool {
    let mut index = usize::from(bytes.first() == Some(&b'-'));
    match bytes.get(index) {
        Some(b'0') => index += 1,
        Some(b'1'..=b'9') => {
            index += 1;
            while bytes.get(index).is_some_and(u8::is_ascii_digit) {
                index += 1;
            }
        }
        _ => return false,
    }
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        let start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if index == start {
            return false;
        }
    }
    if matches!(bytes.get(index), Some(b'e' | b'E')) {
        index += 1;
        if matches!(bytes.get(index), Some(b'+' | b'-')) {
            index += 1;
        }
        let start = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if index == start {
            return false;
        }
    }
    index == bytes.len()
}
