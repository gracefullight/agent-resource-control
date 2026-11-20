//! Kubeconfig proves an API selection only, never pod placement.
use super::files::{fact, path_env, Reads};
use arc_detector::{EndpointContext, EndpointKind};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

#[derive(Deserialize, Default)]
struct Config {
    #[serde(rename = "current-context")]
    current: Option<String>,
    #[serde(default)]
    contexts: Vec<NamedContext>,
    #[serde(default)]
    clusters: Vec<NamedCluster>,
}
#[derive(Deserialize)]
struct NamedContext {
    name: String,
    context: Context,
}
#[derive(Deserialize)]
struct Context {
    cluster: String,
}
#[derive(Deserialize)]
struct NamedCluster {
    name: String,
    cluster: Cluster,
}
#[derive(Deserialize)]
struct Cluster {
    server: String,
}

pub(super) fn capture(
    args: &[String],
    context: &mut EndpointContext,
    reads: &mut Reads,
) -> Option<()> {
    let (mut selected, mut config, mut i) = (None, None, 0);
    while let Some(arg) = args.get(i) {
        if arg == "--" {
            break;
        }
        // These options override the cluster or its server, so a fact for
        // the context's configured server would describe the wrong target.
        // Until those overrides are supported, do not read or emit a fact.
        if arg == "--cluster"
            || arg.starts_with("--cluster=")
            || arg == "--server"
            || arg.starts_with("--server=")
            || arg == "-s"
            || (arg.starts_with("-s") && !arg.starts_with("--"))
        {
            return None;
        }
        let mut found = None;
        for name in ["--context", "--kubeconfig"] {
            if arg == name {
                i += 1;
                found = Some((name, args.get(i)?.as_str()));
                break;
            }
            if let Some(v) = arg.strip_prefix(name).and_then(|s| s.strip_prefix('=')) {
                found = Some((name, v));
                break;
            }
        }
        if let Some((name, value)) = found {
            if value.is_empty() {
                return None;
            }
            let slot = if name == "--context" {
                &mut selected
            } else {
                &mut config
            };
            if slot.replace(value).is_some() {
                return None;
            }
        }
        i += 1;
    }
    let paths: Vec<PathBuf> = if let Some(file) = config {
        vec![file.into()]
    } else if let Some(value) = std::env::var_os("KUBECONFIG").filter(|v| !v.is_empty()) {
        if value.as_encoded_bytes().len() > arc_detector::MAX_CONTEXT_VALUE_BYTES {
            return None;
        }
        std::env::split_paths(&value)
            .filter(|p| !p.as_os_str().is_empty())
            .collect()
    } else {
        vec![path_env("HOME")??.join(".kube/config")]
    };
    if paths.is_empty() || paths.len() > 8 {
        return None;
    }
    resolve(paths, selected, context, reads)
}

fn resolve(
    paths: Vec<PathBuf>,
    selected: Option<&str>,
    context: &mut EndpointContext,
    reads: &mut Reads,
) -> Option<()> {
    let mut current = None;
    let mut contexts = BTreeMap::new();
    let mut clusters = BTreeMap::new();
    for path in paths {
        let Some(bytes) = reads.read(&path)? else {
            continue;
        };
        let input = std::str::from_utf8(&bytes).ok()?;
        // Aliases, tags, merges and multi-document streams require expansion
        // semantics outside this bounded capture contract.
        if input.contains(['&', '*', '!'])
            || input.contains("<<:")
            || input.lines().filter(|l| l.trim() == "---").count() > 1
            || input.lines().any(|l| l.trim() == "...")
        {
            return None;
        }
        let config: Config = serde_norway::from_slice(&bytes).ok()?;
        if current.is_none() {
            current = config.current.filter(|s| !s.is_empty());
        }
        let mut local = BTreeSet::new();
        for item in config.contexts {
            if item.name.is_empty() || !local.insert(item.name.clone()) {
                return None;
            }
            contexts.entry(item.name).or_insert(item.context.cluster);
        }
        local.clear();
        for item in config.clusters {
            if item.name.is_empty() || !local.insert(item.name.clone()) {
                return None;
            }
            clusters.entry(item.name).or_insert(item.cluster.server);
        }
    }
    let name = selected.or(current.as_deref())?;
    let server = clusters.get(contexts.get(name)?)?;
    fact(
        context,
        EndpointKind::KubernetesContext,
        selected.unwrap_or("@current"),
        server.clone(),
    )
}

#[cfg(test)]
mod tests {
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
            assert_eq!(context.facts.len(), 1);
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
        resolve(vec![one, two], None, &mut context, &mut Reads::new()).unwrap();
        assert_eq!(context.facts[0].endpoints, ["https://192.0.2.41:6443"]);
    }
}
