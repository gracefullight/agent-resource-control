//! Compare the controlled Swift fixture output, not independent vendor progress.
use super::Form;

const OUT: &str =
    "<ARC_SWIFT_STDOUT>\nSWIFT_NATIVE_STDOUT\nliteral $HOME ' value 日本語\n</ARC_SWIFT_STDOUT>";
const ERR: &str =
    "<ARC_SWIFT_STDERR>\nSWIFT_NATIVE_STDERR\nliteral $HOME ' value 日本語\n</ARC_SWIFT_STDERR>";
const START: &str = "<ARC_SWIFT_STDOUT>";
const END: &str = "</ARC_SWIFT_STDERR>";
const PAYLOAD: &str = concat!(
    "ordinary swift stdout before\n[1/2] Fixture\n[2/2] FixtureTests-product\n",
    "<ARC_SWIFT_STDOUT>\nSWIFT_NATIVE_STDOUT\nliteral $HOME ' value 日本語\n</ARC_SWIFT_STDOUT>\n",
    "ordinary swift stdout after\nordinary swift stderr before\n",
    "[3/4] Fixture\n[4/4] FixtureTests-product\n",
    "<ARC_SWIFT_STDERR>\nSWIFT_NATIVE_STDERR\nliteral $HOME ' value 日本語\n</ARC_SWIFT_STDERR>\n",
    "ordinary swift stderr after"
);

pub fn validate(stdout: &[u8], stderr: &[u8]) {
    let streams = [
        std::str::from_utf8(stdout).expect("native stdout must be UTF-8"),
        std::str::from_utf8(stderr).expect("native stderr must be UTF-8"),
    ];
    let count = |payload: &str| {
        streams
            .iter()
            .map(|s| s.matches(payload).count())
            .sum::<usize>()
    };
    assert_eq!(count(OUT), 1, "complete stdout payload is required");
    assert_eq!(count(ERR), 1, "complete stderr payload is required");
    assert_eq!(count(PAYLOAD), 1, "ordered fixture bytes are required");
    for prefix in ["stdout", "stderr"] {
        for edge in ["before", "after"] {
            assert_eq!(count(&format!("ordinary swift {prefix} {edge}")), 1);
        }
        let marker = prefix.to_ascii_uppercase();
        assert_eq!(count(&format!("<ARC_SWIFT_{marker}>")), 1);
        assert_eq!(count(&format!("</ARC_SWIFT_{marker}>")), 1);
    }
}

fn fixture_channel<'a>(form: Form, stdout: &'a [u8], stderr: &'a [u8]) -> &'a str {
    validate(stdout, stderr);
    let (fixture, other) = match form {
        Form::SwiftBuild => (stderr, stdout),
        Form::SwiftTest => (stdout, stderr),
        _ => panic!("not a Swift fixture"),
    };
    let fixture = std::str::from_utf8(fixture).unwrap();
    let other = std::str::from_utf8(other).unwrap();
    assert!(
        fixture.contains(PAYLOAD),
        "fixture payload on {form:?} channel"
    );
    assert!(!other.contains(PAYLOAD), "fixture payload moved channels");
    fixture
}

pub fn validate_form(form: Form, stdout: &[u8], stderr: &[u8]) {
    fixture_channel(form, stdout, stderr);
}

fn framed(fixture: &str) -> &str {
    let start = fixture.find(START).expect("stdout frame");
    let end = fixture.find(END).expect("stderr frame") + END.len();
    assert!(start < end, "fixture frames must remain ordered");
    &fixture[start..end]
}

pub fn compare(form: Form, expected: [&[u8]; 2], actual: [&[u8]; 2]) {
    let expected_fixture = fixture_channel(form, expected[0], expected[1]);
    let actual_fixture = fixture_channel(form, actual[0], actual[1]);
    assert_eq!(
        framed(expected_fixture).as_bytes(),
        framed(actual_fixture).as_bytes(),
        "native Swift controlled frame bytes changed"
    );
    // Real SwiftPM runs have independent vendor progress and timing on either
    // stream. Exact forwarding outside these frames is tested in forwarding.rs
    // with deterministic bytes through the same managed execution path.
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(form: Form, progress: &str) -> (String, String) {
        let body = format!("fixture header\n{progress}\n{PAYLOAD}\nfixture footer\n");
        let other = format!("vendor header\n{progress}\nvendor footer\n");
        match form {
            Form::SwiftBuild => (other, body),
            Form::SwiftTest => (body, other),
            _ => unreachable!(),
        }
    }

    #[test]
    fn swift_stream_compares_controlled_bytes_and_accepts_variable_vendor_progress() {
        for form in [Form::SwiftBuild, Form::SwiftTest] {
            let (out, err) = fixture(form, "[Provisioning 6 / 6]\n[Planning 135 / 152]");
            let (other_out, other_err) = fixture(form, "[Provisioning 4 / 6]");
            let expected = [out.as_bytes(), err.as_bytes()];
            compare(form, expected, [other_out.as_bytes(), other_err.as_bytes()]);
            let fixture = if form == Form::SwiftBuild { &err } else { &out };
            for changed in [
                fixture.replace("[3/4] Fixture\n", ""),
                fixture.replace("SWIFT_NATIVE_STDOUT", "SWIFT_NATIVE_STDOUt"),
                fixture.replace("ordinary swift stdout after", "ordinary swift stdout afTer"),
                fixture
                    .replace(OUT, "<TEMP>")
                    .replace(ERR, OUT)
                    .replace("<TEMP>", ERR),
                format!("{fixture}{PAYLOAD}"),
            ] {
                let actual = if form == Form::SwiftBuild {
                    [out.as_bytes(), changed.as_bytes()]
                } else {
                    [changed.as_bytes(), err.as_bytes()]
                };
                assert!(std::panic::catch_unwind(|| compare(form, expected, actual)).is_err());
            }
            assert!(std::panic::catch_unwind(|| compare(
                form,
                expected,
                [err.as_bytes(), out.as_bytes()]
            ))
            .is_err());
            let invalid = [0xff];
            let actual = if form == Form::SwiftBuild {
                [&invalid[..], err.as_bytes()]
            } else {
                [out.as_bytes(), &invalid[..]]
            };
            assert!(std::panic::catch_unwind(|| compare(form, expected, actual)).is_err());
        }
    }
}
