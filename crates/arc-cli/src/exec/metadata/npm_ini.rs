//! `.npmrc` lines as the `ini` package npm uses decodes them (lane C).
//!
//! Lines are split on runs of CR and LF. A blank line, or one whose first
//! non-space character is `;` or `#`, is a comment. A key is the text before
//! the first `=`, a value the text after it; a line without `=` assigns
//! `true`, and a line `ini`'s pattern does not match (a leading `=`, or a
//! U+2028/U+2029 in the value) is skipped. Keys and values are both decoded
//! by `ini`'s `unsafe`: trimmed of JavaScript white space, then either a
//! quoted token (single quotes removed, then `JSON.parse` attempted) or an
//! unquoted one cut at the first unescaped `;` or `#`, where `\\`, `\;` and
//! `\#` are escapes and any other backslash is kept. A decoded value `true`,
//! `false` or `null` becomes that JSON value; a key ending in `[]` is an
//! array element.
//!
//! What cannot be reproduced exactly is reported, never guessed: a quoted
//! token that `JSON.parse` might accept where this parser does not is
//! [`Decoded::Uncertain`], and a line that could be a section header is
//! [`Line::Section`]. [`nests`] reports the assignments whose value `ini`'s
//! dotted-key post-processing could move onto another key.
use serde_json::Value;

/// One decoded key or value.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Decoded {
    /// A string after quotes, comments and escapes.
    Text(String),
    /// A non-string `JSON.parse` result, or `true`, `false` or `null`.
    Json(Value),
    /// A quoted token whose `JSON.parse` result is not reproduced here.
    Uncertain,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Line {
    /// A comment, a blank line, or a line `ini` does not match.
    Skip,
    /// A section header, or a line that could be read as one.
    Section,
    Assign {
        key: Decoded,
        /// The key ended in `[]`, which `ini` removed.
        array: bool,
        value: Decoded,
    },
}

/// JavaScript's `String.prototype.trim` white space and line terminators.
fn js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}
pub(super) fn js_trim(text: &str) -> &str {
    text.trim_matches(js_space)
}

/// Whether `JSON.parse` could accept `text` where `serde_json` did not:
/// a string (lone surrogates), a number (out of range) or a container.
fn json_like(text: &str) -> bool {
    let text = text.trim_start_matches([' ', '\t', '\n', '\r']);
    text.starts_with(['"', '-', '[', '{']) || text.starts_with(|c: char| c.is_ascii_digit())
}

/// `ini`'s `unsafe`.
pub(super) fn decode(raw: &str) -> Decoded {
    let val = js_trim(raw);
    let quoted = (val.starts_with('"') && val.ends_with('"'))
        || (val.starts_with('\'') && val.ends_with('\''));
    if quoted {
        let inner = match val.strip_prefix('\'') {
            Some(rest) => rest.strip_suffix('\'').unwrap_or(""),
            None => val,
        };
        return match serde_json::from_str::<Value>(inner) {
            Ok(Value::String(text)) => Decoded::Text(text),
            Ok(value) => Decoded::Json(value),
            Err(_) if json_like(inner) => Decoded::Uncertain,
            Err(_) => Decoded::Text(inner.to_owned()),
        };
    }
    let mut escaped = false;
    let mut out = String::with_capacity(val.len());
    for c in val.chars() {
        if escaped {
            if !matches!(c, '\\' | ';' | '#') {
                out.push('\\');
            }
            out.push(c);
            escaped = false;
        } else if c == ';' || c == '#' {
            break;
        } else if c == '\\' {
            escaped = true;
        } else {
            out.push(c);
        }
    }
    if escaped {
        out.push('\\');
    }
    Decoded::Text(js_trim(&out).to_owned())
}

fn value(raw: &str) -> Decoded {
    match decode(raw) {
        Decoded::Text(text) if text == "true" => Decoded::Json(Value::Bool(true)),
        Decoded::Text(text) if text == "false" => Decoded::Json(Value::Bool(false)),
        Decoded::Text(text) if text == "null" => Decoded::Json(Value::Null),
        other => other,
    }
}

fn line(line: &str) -> Line {
    let start = line.trim_start_matches(js_space);
    if start.is_empty() || start.starts_with([';', '#']) {
        return Line::Skip;
    }
    if start.starts_with('[') {
        return Line::Section;
    }
    let (key, value) = match line.split_once('=') {
        Some(("", _)) => return Line::Skip,
        // `.` in `ini`'s pattern does not match these line terminators.
        Some((_, value)) if value.contains(['\u{2028}', '\u{2029}']) => return Line::Skip,
        Some((key, value)) => (key, self::value(value)),
        None => (line, Decoded::Json(Value::Bool(true))),
    };
    let (key, array) = match decode(key) {
        // `keyRaw.length > 2`, in UTF-16 code units.
        Decoded::Text(text) if text.encode_utf16().count() > 2 && text.ends_with("[]") => {
            (Decoded::Text(text[..text.len() - 2].to_owned()), true)
        }
        Decoded::Text(text) if text == "__proto__" => return Line::Skip,
        other => (other, false),
    };
    Line::Assign { key, array, value }
}

/// The decoded lines of one `.npmrc`.
pub(super) fn lines(text: &str) -> impl Iterator<Item = Line> + '_ {
    text.split(['\r', '\n']).filter(|l| !l.is_empty()).map(line)
}

/// Whether `ini`'s post-processing could split a dotted key and set the
/// value, an object or `null`, under another key, overwriting that key.
pub(super) fn nests(key: &Decoded, value: &Decoded) -> bool {
    let objectish = matches!(
        value,
        Decoded::Json(Value::Null | Value::Object(_)) | Decoded::Uncertain
    );
    let dotted = match key {
        Decoded::Text(text) => text.contains('.'),
        _ => true,
    };
    objectish && dotted
}
