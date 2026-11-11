#[test]
fn registered_job_validation_rejects_invalid_inputs_before_socket_io() {
    let peer = Peer::new(0, |_, _| panic!("invalid input performed I/O"));
    let app = App::new(peer.bridge());
    for job_id in [
        String::new(),
        "a".repeat(31),
        "a".repeat(33),
        "A".repeat(32),
        SENTINEL.to_owned(),
        "x".repeat(1_048_577),
        "0".repeat(31) + "\0",
    ] {
        assert_eq!(
            app.invoke("job_by_id", json!({"job_id":job_id})),
            Err(error("invalid_request", false))
        );
    }
    assert_eq!(peer.requests.load(Ordering::Relaxed), 0);
    for body in [
        json!({}),
        json!({"job_id":[SENTINEL]}),
        json!({"jobId":JOB}),
    ] {
        let rejected = app.invoke("job_by_id", body).unwrap_err();
        assert!(!rejected.to_string().contains(SENTINEL));
    }
    assert!(app.invoke("execute_job", json!({})).is_err());
    assert!(app.invoke("daemon_stop", json!({})).is_err());
    assert!(app
        .invoke("exchange", json!({"operation":"job.start"}))
        .is_err());
}

#[test]
fn registered_opener_accepts_exactly_the_three_enum_destinations() {
    let app = App::new(DesktopBridge::new());
    for (link, url) in [
        ("github", "https://github.com/gracefullight"),
        ("linkedin", "https://www.linkedin.com/in/gracefullight"),
        ("oma", "https://github.com/first-fluke/oh-my-agent"),
    ] {
        assert_eq!(CreditLink::parse(link).unwrap().url(), url);
        assert_eq!(
            app.invoke("open_credit_link", json!({"link":link})),
            Ok(Value::Null)
        );
    }
    for link in [
        SENTINEL,
        "https://github.com/gracefullight",
        "file:///tmp/private",
        "Github",
        "",
    ] {
        assert_eq!(
            app.invoke("open_credit_link", json!({"link":link})),
            Err(error("invalid_request", false))
        );
    }
    assert_eq!(
        *app.ports.links.lock().unwrap(),
        [CreditLink::Github, CreditLink::Linkedin, CreditLink::Oma]
    );
    app.ports.opener_fails.store(true, Ordering::Relaxed);
    assert_eq!(
        app.invoke("open_credit_link", json!({"link":"github"})),
        Err(error("opener_failed", false))
    );
    assert!(app
        .invoke(
            "plugin:opener|open_url",
            json!({"url":"https://example.invalid"})
        )
        .is_err());
}
