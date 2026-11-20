use super::*;
fn config(s: &str) -> Config {
    let mut c = Config::default();
    c.parse(s.as_bytes()).unwrap();
    c
}
#[test]
fn selection_push_urls_rewrites_and_ambiguity() {
    let c = config("[branch \"topic\"]\n remote = fetch\n pushRemote = publish\n[remote]\n pushDefault = other\n[remote \"publish\"]\n url = https://fetch.invalid/o/r\n pushurl = https://github.com/a/b\n[url \"https://gitlab.com/\"]\n pushInsteadOf = https://github.com/\n");
    assert_eq!(c.default_remote(Some("topic")).as_deref(), Some("publish"));
    assert_eq!(c.default_remote(None).as_deref(), Some("other"));
    assert_eq!(c.endpoints("publish").unwrap(), ["https://github.com/a/b"]);
    let c = config("[remote \"origin\"]\n url = alias:repo\n[url \"https://github.com/a/\"]\n pushInsteadOf = alias:\n[url \"https://gitlab.com/b/\"]\n pushInsteadOf = alias:repo\n");
    assert_eq!(c.endpoints("origin").unwrap(), ["https://gitlab.com/b/"]);
    let c = config("[remote \"origin\"]\n url = https://github.com/a/b\n pushurl = https://github.com/a/b\n pushurl = file:///fixture\n");
    assert_eq!(c.endpoints("origin").unwrap().len(), 2);
    assert!(Config::default()
        .parse(b"[include]\n path = other\n")
        .is_none());
}

#[test]
fn identity_push_rewrite_suppresses_insteadof_even_when_url_is_unchanged() {
    let c = config("[remote \"origin\"]\n url = file:///fixture/repo\n[url \"file:///fixture/\"]\n pushInsteadOf = file:///fixture/\n[url \"https://github.com/example/\"]\n insteadOf = file:///fixture/\n");
    assert_eq!(c.endpoints("origin").unwrap(), ["file:///fixture/repo"]);
}

#[test]
fn single_remote_push_alias_excludes_unmatched_fetch_url() {
    let c = config("[remote \"origin\"]\n url = https://github.com/example/app.git\n url = file:///fixture/unmatched\n[url \"https://github.com/\"]\n pushInsteadOf = https://github.com/\n");
    assert_eq!(
        c.endpoints("origin").unwrap(),
        ["https://github.com/example/app.git"]
    );
}

#[test]
fn single_local_push_alias_excludes_unmatched_fetch_url() {
    let c = config("[remote \"origin\"]\n url = https://github.com/example/app.git\n url = file:///fixture/local\n[url \"file:///fixture/\"]\n pushInsteadOf = file:///fixture/\n");
    assert_eq!(c.endpoints("origin").unwrap(), ["file:///fixture/local"]);
}

#[test]
fn multiple_push_aliases_exclude_all_unmatched_fetch_urls() {
    let c = config("[remote \"origin\"]\n url = https://github.com/example/app.git\n url = file:///fixture/local\n url = https://gitlab.com/example/unmatched.git\n[url \"https://github.com/\"]\n pushInsteadOf = https://github.com/\n[url \"file:///fixture/\"]\n pushInsteadOf = file:///fixture/\n");
    assert_eq!(
        c.endpoints("origin").unwrap(),
        [
            "https://github.com/example/app.git",
            "file:///fixture/local"
        ]
    );
}

#[test]
fn no_push_alias_rewrites_all_fetch_urls() {
    let c = config("[remote \"origin\"]\n url = alias:app.git\n url = file:///fixture/local\n[url \"https://github.com/example/\"]\n insteadOf = alias:\n");
    assert_eq!(
        c.endpoints("origin").unwrap(),
        [
            "https://github.com/example/app.git",
            "file:///fixture/local"
        ]
    );
}

#[test]
fn longer_rewrite_wins_over_conflicting_shorter_prefixes() {
    for key in ["pushInsteadOf", "insteadOf"] {
        let c = config(&format!(
            "[remote \"origin\"]\n url = alias:repo.git\n[url \"https://github.com/other/\"]\n {key} = alias:\n[url \"file:///fixture/\"]\n {key} = alias:\n[url \"https://github.com/example/repo\"]\n {key} = alias:repo\n"
        ));
        assert_eq!(
            c.endpoints("origin"),
            Some(vec!["https://github.com/example/repo.git".to_owned()]),
            "{key}"
        );
    }
}

#[test]
fn conflicting_longest_rewrites_remain_unproven() {
    for key in ["pushInsteadOf", "insteadOf"] {
        let c = config(&format!(
            "[remote \"origin\"]\n url = alias:repo.git\n[url \"https://github.com/example/\"]\n {key} = alias:\n[url \"file:///fixture/\"]\n {key} = alias:\n"
        ));
        assert!(c.endpoints("origin").is_none(), "{key}");
    }
}
