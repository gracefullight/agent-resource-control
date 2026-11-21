use super::*;
use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Dir(PathBuf);
impl Dir {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "arc-kube-capture-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        Self(dir)
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn selected_cluster_is_captured_but_never_attests_a_pod() {
    let dir = Dir::new();
    let file = dir.0.join("fixture.yaml");
    fs::write(&file, "current-context: first\ncontexts:\n- name: first\n  context:\n    cluster: one\n- name: second\n  context:\n    cluster: two\nclusters:\n- name: one\n  cluster:\n    server: https://192.0.2.41:6443\n- name: two\n  cluster:\n    server: https://192.0.2.42:6443\n").unwrap();
    for (selected, expected) in [
        (None, "https://192.0.2.41:6443"),
        (Some("second"), "https://192.0.2.42:6443"),
    ] {
        let mut args = vec!["--kubeconfig".to_owned(), file.to_str().unwrap().to_owned()];
        if let Some(selected) = selected {
            args.extend(["--context".into(), selected.into()]);
        }
        let mut context = EndpointContext {
            captured_at_ms: 10_000,
            ..Default::default()
        };
        capture(&args, &mut context, &mut Reads::new()).unwrap();
        assert_eq!(context.facts.len(), 4);
        assert!(context.facts[0].kind == EndpointKind::KubernetesContext);
        assert_eq!(context.facts[0].endpoints, [expected]);
        let mut argv = vec!["kubectl".into()];
        argv.extend(args.iter().cloned().map(Into::into));
        argv.extend(["exec", "pod", "--", "make"].map(Into::into));
        let classification = arc_detector::classify_argv_with_context(&argv, &context, 10_000);
        assert_eq!(
            classification.classification,
            arc_detector::JobClass::Unknown
        );
        assert_eq!(
            classification.uncertainty,
            arc_detector::Uncertainty::Observe
        );
    }
    fs::write(
        &file,
        "current-context: missing\ncontexts: []\nclusters: []\n",
    )
    .unwrap();
    let args = vec!["--kubeconfig".to_owned(), file.to_str().unwrap().to_owned()];
    let mut context = EndpointContext {
        captured_at_ms: 10_000,
        ..Default::default()
    };
    assert!(capture(&args, &mut context, &mut Reads::new()).is_none());
    fs::write(
        &file,
        "current-context: &anchor x\ncontexts: []\nclusters: []\n",
    )
    .unwrap();
    assert!(capture(&args, &mut context, &mut Reads::new()).is_none());
}

#[test]
fn cluster_and_server_overrides_never_emit_the_context_server() {
    for options in [
        vec!["--cluster", "other"],
        vec!["--cluster=other"],
        vec!["--server", "https://192.0.2.43:6443"],
        vec!["--server=https://192.0.2.43:6443"],
        vec!["-s", "https://192.0.2.43:6443"],
        vec!["--cluster", "one", "--cluster", "other"],
    ] {
        let mut args = vec!["--context".to_owned(), "one".to_owned()];
        args.extend(options.into_iter().map(str::to_owned));
        args.extend(["exec", "pod", "--", "make"].map(str::to_owned));
        let mut context = EndpointContext {
            captured_at_ms: 10_000,
            ..Default::default()
        };
        assert!(capture(&args, &mut context, &mut Reads::new()).is_none());
        assert!(context.facts.is_empty());
    }
}

#[test]
fn kubeconfig_list_uses_first_current_context_and_first_named_objects() {
    let dir = Dir::new();
    let one = dir.0.join("one.yaml");
    let two = dir.0.join("two.yaml");
    fs::write(&one, "current-context: first\ncontexts:\n- name: first\n  context:\n    cluster: alpha\nclusters: []\n").unwrap();
    fs::write(&two, "current-context: second\ncontexts:\n- name: first\n  context:\n    cluster: beta\n- name: second\n  context:\n    cluster: beta\nclusters:\n- name: alpha\n  cluster:\n    server: https://192.0.2.41:6443\n- name: beta\n  cluster:\n    server: https://192.0.2.42:6443\n").unwrap();
    let mut context = EndpointContext {
        captured_at_ms: 10_000,
        ..Default::default()
    };
    resolve(vec![one, two], None, None, &mut context, &mut Reads::new()).unwrap();
    assert_eq!(context.facts[0].endpoints, ["https://192.0.2.41:6443"]);
}

#[test]
fn selected_context_cluster_and_effective_namespace_are_captured_as_facts() {
    let dir = Dir::new();
    let config = dir.0.join("kube.yaml");
    fs::write(&config, "current-context: current\ncontexts:\n- name: current\n  context:\n    cluster: alpha\n    namespace: from-config\n- name: other\n  context:\n    cluster: beta\n    namespace: other-config\nclusters:\n- name: alpha\n  cluster:\n    server: https://192.0.2.41:6443\n- name: beta\n  cluster:\n    server: https://192.0.2.42:6443\n").unwrap();
    for (extra, expected) in [
        (
            vec![],
            [
                "@current",
                "@selected-context/current",
                "@selected-cluster/alpha",
                "@selected-namespace/from-config",
            ],
        ),
        (
            vec!["--context", "other", "--namespace", "flag"],
            [
                "other",
                "@selected-context/other",
                "@selected-cluster/beta",
                "@selected-namespace/flag",
            ],
        ),
        (
            vec!["-n=short"],
            [
                "@current",
                "@selected-context/current",
                "@selected-cluster/alpha",
                "@selected-namespace/short",
            ],
        ),
        (
            vec!["-njoined"],
            [
                "@current",
                "@selected-context/current",
                "@selected-cluster/alpha",
                "@selected-namespace/joined",
            ],
        ),
        (
            vec!["exec", "pod", "--", "make", "--namespace=child"],
            [
                "@current",
                "@selected-context/current",
                "@selected-cluster/alpha",
                "@selected-namespace/from-config",
            ],
        ),
    ] {
        let mut args = vec![
            "--kubeconfig".to_owned(),
            config.to_str().unwrap().to_owned(),
        ];
        args.extend(extra.into_iter().map(str::to_owned));
        let mut context = EndpointContext {
            captured_at_ms: 10_000,
            ..Default::default()
        };
        capture(&args, &mut context, &mut Reads::new()).unwrap();
        assert_eq!(
            context
                .facts
                .iter()
                .map(|item| item.selector.as_str())
                .collect::<Vec<_>>(),
            expected
        );
        assert!(context
            .facts
            .iter()
            .all(|item| item.kind == EndpointKind::KubernetesContext));
        assert!(context
            .facts
            .iter()
            .all(|item| item.endpoints == context.facts[0].endpoints));
    }
    let mut args = vec![
        "--kubeconfig".to_owned(),
        config.to_str().unwrap().to_owned(),
    ];
    args.extend(["-n", "one", "--namespace=two"].map(str::to_owned));
    let mut context = EndpointContext {
        captured_at_ms: 10_000,
        ..Default::default()
    };
    assert!(capture(&args, &mut context, &mut Reads::new()).is_none());
    assert!(context.facts.is_empty());
}
