//! Fresh, bounded CI/remote-Git evidence capture. No subprocesses or network.
mod git;
mod github;

use arc_detector::{EndpointContext, EndpointFact, EndpointKind};
use std::{
    ffi::OsString,
    path::Path,
    sync::mpsc,
    time::{Duration, Instant},
};

pub(super) fn extend(argv: &[OsString], context: &mut EndpointContext) -> Option<()> {
    let now = arc_ipc::locality::now_ms()?;
    let age = now.checked_sub(context.captured_at_ms)?;
    let remaining = arc_detector::MAX_CONTEXT_AGE_MS.checked_sub(age)?;
    extend_with(
        argv,
        context,
        Duration::from_millis(remaining),
        capture_fact,
    )
}

fn extend_with(
    argv: &[OsString],
    context: &mut EndpointContext,
    remaining: Duration,
    capture: impl FnOnce(Vec<String>) -> Option<EndpointFact> + Send + 'static,
) -> Option<()> {
    let started = Instant::now();
    // An unrelated command or an oversized input is still classified by the
    // detector, which owns its precise INPUT_LIMIT/TOKEN_LIMIT reason.
    if !matches!(
        argv.first()
            .and_then(|s| Path::new(s).file_name())
            .and_then(|s| s.to_str()),
        Some("git" | "gh" | "gitlab")
    ) {
        return Some(());
    }
    if argv.len() > arc_detector::MAX_TOKENS
        || argv
            .iter()
            .try_fold(0usize, |n, word| {
                n.checked_add(word.as_encoded_bytes().len())
            })
            .is_none_or(|n| n > arc_detector::MAX_INPUT_BYTES)
    {
        return Some(());
    }
    let Some(words) = argv
        .iter()
        .map(|arg| arg.to_str().map(str::to_owned))
        .collect::<Option<Vec<_>>>()
    else {
        return Some(());
    };
    // Regular file and metadata operations can block even with O_NONBLOCK.
    // A timed-out one-shot worker cannot publish late facts to this context.
    let (send, receive) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("arc-ci-capture".into())
        .spawn(move || {
            let _ = send.send(capture(words));
        })
        .ok()?;
    let remaining = remaining.checked_sub(started.elapsed())?;
    if let Some(fact) = receive.recv_timeout(remaining).ok()? {
        context.facts.push(fact);
    }
    Some(())
}

fn capture_fact(words: Vec<String>) -> Option<EndpointFact> {
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    match words
        .first()
        .and_then(|s| Path::new(s).file_name())
        .and_then(|s| s.to_str())
    {
        Some("git") => {
            let (selector, endpoints) = git::push(&words)?;
            let kind = if selector == "@bare-push" {
                EndpointKind::GitPushDefault
            } else {
                EndpointKind::GitRemote
            };
            Some(EndpointFact {
                kind,
                selector,
                endpoints,
            })
        }
        Some("gh") if github::public_host(&words) => Some(EndpointFact {
            kind: EndpointKind::CiHost,
            selector: "gh".into(),
            endpoints: vec!["https://github.com".into()],
        }),
        // Require an explicit server URL. A named or missing configuration is
        // not sufficient to establish the effective endpoint; GITLAB_URL is
        // the documented server-url override of the gitlab CLI.
        Some("gitlab")
            if std::env::var_os("GITLAB_URL").as_deref()
                == Some(std::ffi::OsStr::new("https://gitlab.com")) =>
        {
            Some(EndpointFact {
                kind: EndpointKind::CiHost,
                selector: "gitlab".into(),
                endpoints: vec!["https://gitlab.com".into()],
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
