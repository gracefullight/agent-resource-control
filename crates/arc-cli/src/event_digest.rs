//! The digest behind a vendor hook event key (AT-TRF-09, ADR-014).
//!
//! `hooks/event_key.rs` frames the event tuple and asks the module beside
//! `hooks` for its SHA-256. The hook source is also compiled, by path, into
//! test targets of crates that link no digest crate; each crate that
//! compiles it supplies this module, and only `arcctl` and its own tests
//! supply this one. A stand-in that returns `None` names no event, so such a
//! copy of the hook returns exactly the command it returned before.
use sha2::{Digest, Sha256};

/// SHA-256 of the concatenation of `parts`.
pub(crate) fn sha256(parts: &[&[u8]]) -> Option<[u8; 32]> {
    let mut digest = Sha256::new();
    for part in parts {
        digest.update(part);
    }
    Some(digest.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::sha256;

    fn hex(parts: &[&[u8]]) -> String {
        sha256(parts)
            .unwrap()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    /// FIPS 180-4 examples, and the split of the input does not matter.
    #[test]
    fn sha256_matches_the_published_examples_however_the_input_is_split() {
        assert_eq!(
            hex(&[]),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        let abc = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        assert_eq!(hex(&[b"abc"]), abc);
        assert_eq!(hex(&[b"a", b"", b"bc"]), abc);
        assert_eq!(
            hex(&[
                b"abcdbcdecdefdefgefghfghighij",
                b"hijkijkljklmklmnlmnomnopnopq"
            ]),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }
}
