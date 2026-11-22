use super::*;

const KEY: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

#[test]
fn only_an_absent_or_well_formed_value_is_accepted() {
    assert_eq!(parse(None), Ok(None));
    let key = parse(Some(KEY.to_owned())).unwrap().unwrap();
    assert_eq!(key.as_str(), KEY);
    for malformed in [
        String::new(),
        KEY[..63].to_owned(),
        format!("{KEY}0"),
        KEY.to_uppercase(),
        "session-1:toolu_01".to_owned(),
        format!("{}/", &KEY[..63]),
    ] {
        assert_eq!(parse(Some(malformed.clone())), Err(()), "{malformed}");
    }
}

#[test]
fn the_key_is_scoped_to_one_run_and_restored_afterwards() {
    let key = |digit: char| parse(Some(digit.to_string().repeat(64))).unwrap();
    assert_eq!(scoped(), None);
    let seen = with_event(key('a'), || {
        let outer = scoped();
        let inner = with_event(key('b'), scoped);
        assert_eq!(scoped(), outer, "the outer run's key was not restored");
        let none = with_event(None, scoped);
        (outer, inner, none)
    });
    assert_eq!(seen, (key('a'), key('b'), None));
    assert_eq!(scoped(), None);
}

/// The key is sent only to the daemon that answered this run and holds the
/// hook-event advertisement: an older daemon, a scripted peer or a file left
/// behind receives the bytes it received before.
#[test]
fn the_key_is_sent_only_to_the_daemon_that_holds_the_advertisement() {
    use super::super::project_key::tests::{peer, Holder};
    use arc_ipc::traffic::fairness;

    let directory = std::env::temp_dir().join(format!("arc-event-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    let socket = directory.join("arc.sock");
    let this = std::process::id();
    let key = parse(Some(KEY.to_owned())).unwrap();
    let sent = |daemon: &Continuity| with_event(key.clone(), || negotiated(&socket, daemon));

    // No file, then a file that nobody locks.
    assert_eq!(sent(&peer(&[this])), None);
    std::fs::write(directory.join(ADVERTISEMENT), "").unwrap();
    assert_eq!(sent(&peer(&[this])), None);
    // A daemon that advertises project fair queuing only is an older daemon.
    let older = Holder::lock_named(&socket, fairness::ADVERTISEMENT);
    assert_eq!(sent(&peer(&[older.pid as u32])), None);
    drop(older);

    let holder = Holder::lock_named(&socket, ADVERTISEMENT);
    let daemon = holder.pid as u32;
    assert_eq!(sent(&peer(&[daemon])), key);
    // A run that delivers no event sends none, whatever is advertised.
    assert_eq!(negotiated(&socket, &peer(&[daemon])), None);
    // The lock is held, but not by the process that answered this run; and
    // no authenticated peer, or two different ones, names no daemon.
    assert_eq!(sent(&peer(&[this])), None);
    assert_eq!(sent(&peer(&[])), None);
    assert_eq!(sent(&peer(&[daemon, this])), None);
    // The lock ends with its holder.
    let answered = peer(&[daemon]);
    drop(holder);
    assert_eq!(sent(&answered), None);
    std::fs::remove_dir_all(&directory).unwrap();
}
