//! Contract-derived vendor events through installed hooks; no live vendor session.
use crate::{
    journey::{self, output_with_input, Sandbox},
    ledger,
    runtime::{child_watchdog, Fixture},
    stage::{self, Run, Tool},
};
use serde_json::{json, Value};
use std::fs;

#[path = "codex_journey/helpers.rs"]
mod helpers;
use helpers::{advertise, event, install, reply, verify_finish};

pub fn run() {
    let f = Fixture::new();
    // Provisioned binaries only: no nested build and no compiler.
    let s = Sandbox::provisioned(&f);
    let project = s.project("codex-journey");
    fs::write(
        project.join("package.json"),
        b"{\"scripts\":{\"build\":\"vite build\"}}",
    )
    .unwrap();
    let cargo = Tool::cargo(&f, &s);
    let bun = Tool::named(&f, &s, "bun");
    let mut daemon = f.spawn_isolated("");
    let db = journey::database(&f);
    db.busy_timeout(std::time::Duration::from_secs(60)).unwrap();
    ledger::watch(&db);
    let claude_hook = install(&f, &s, &project, "claude");
    let codex_hook = install(&f, &s, &project, "codex");

    let mut events = Vec::new();
    for (round, first_vendor) in ["claude", "codex"].into_iter().enumerate() {
        let second_vendor = if first_vendor == "claude" {
            "codex"
        } else {
            "claude"
        };
        let tool = |vendor| if vendor == "claude" { &cargo } else { &bun };
        let hook = |vendor| {
            if vendor == "claude" {
                &claude_hook
            } else {
                &codex_hook
            }
        };
        let first_event = event(&project, first_vendor, &format!("{round}-first"));
        let second_event = event(&project, second_vendor, &format!("{round}-second"));
        events.push(first_event.clone());
        events.push(second_event.clone());
        let first_command = reply(
            &f,
            &s,
            &project,
            hook(first_vendor),
            first_vendor,
            &first_event,
        );
        let second_command = reply(
            &f,
            &s,
            &project,
            hook(second_vendor),
            second_vendor,
            &second_event,
        );
        let first_before = tool(first_vendor).starts();
        let second_before = tool(second_vendor).starts();
        let mut first_shell = s.shell(&f, &project, &first_command);
        tool(first_vendor).arm(&mut first_shell);
        let mut first = Run::start(&mut first_shell, &f.root, &format!("first-{round}"));
        first.until("first Build starts and holds its permit", || {
            tool(first_vendor).starts() == first_before + 1 && ledger::permits(&db).1 == 1
        });
        first.assert_stdin_cloexec();
        let mut second_shell = s.shell(&f, &project, &second_command);
        tool(second_vendor).arm(&mut second_shell);
        let mut second = Run::start(&mut second_shell, &f.root, &format!("second-{round}"));
        second.until("second Build is queued", || {
            ledger::entries(&db)
                .iter()
                .filter(|e| e.state == "QUEUED")
                .count()
                == 1
        });
        assert_eq!(
            tool(second_vendor).starts(),
            second_before,
            "queued child started before release"
        );
        assert_eq!(tool(first_vendor).starts(), first_before + 1);
        assert_eq!(ledger::permits(&db).1, 1);
        let queued_job = ledger::entries(&db)
            .into_iter()
            .find(|entry| entry.state == "QUEUED")
            .unwrap()
            .job;
        first.input(stage::INPUT);
        let finished = first.finish("first Build");
        verify_finish(
            &finished,
            &tool(first_vendor).take(),
            &first_shell,
            &project,
            first_vendor,
        );
        assert_eq!(finished.stderr, stage::TOOL_STDERR);
        second.until("second Build starts after release", || {
            tool(second_vendor).starts() == second_before + 1
        });
        second.input(stage::INPUT);
        let finished = second.finish("second Build");
        verify_finish(
            &finished,
            &tool(second_vendor).take(),
            &second_shell,
            &project,
            second_vendor,
        );
        let queued = format!("QUEUED\nWaiting to start.\nReference: {queued_job}\n");
        let started = b"STARTED\nCommand started.\n";
        let first_order = [queued.as_bytes(), started, stage::TOOL_STDERR].concat();
        let second_order = [queued.as_bytes(), stage::TOOL_STDERR, started].concat();
        assert!(
            finished.stderr == first_order || finished.stderr == second_order,
            "unexpected stderr: {:?}",
            finished.stderr
        );
        journey::eventually("all Build permits released", || ledger::permits(&db).1 == 0);
        assert_eq!(tool(first_vendor).starts(), first_before + 1);
        assert_eq!(tool(second_vendor).starts(), second_before + 1);
    }
    let vendors: Vec<String> = db.prepare("SELECT COALESCE(j.vendor_codex,j.vendor) FROM jobs j JOIN traffic_entries t ON t.job_id=j.id ORDER BY t.sequence").unwrap().query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect();
    assert_eq!(vendors, ["claude-code", "codex", "codex", "claude-code"]);
    let keys: Vec<(String, String)> = db.prepare("SELECT l.event_key,j.session_key FROM jobs j JOIN job_lifecycle l ON l.job_id=j.id ORDER BY l.registered_at_ms").unwrap().query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(Result::unwrap).collect();
    assert_eq!(keys.len(), 4);
    assert!(keys
        .iter()
        .all(|(event, session)| event.len() == 64 && session.len() == 64 && event != session));
    // Old v1/v2 clients must not receive the new vocabulary; the negotiated
    // v3 route and the current built CLI must retain Codex's group.
    let query = json!({"by":"vendor","from_ms":0,"to_ms":9007199254740991_u64,"bucket":"day"});
    let summary = |operation: &str| {
        let mut request = arc_ipc::Request::new("compat-summary", operation);
        request.payload = query.clone();
        let response =
            arc_ipc::exchange(&mut crate::runtime::connect(&f.socket), &request).unwrap();
        assert_eq!(response.status, "ok", "{response:?}");
        response.result.unwrap()
    };
    for operation in ["meter.usage_summary", "meter.usage_summary.v2"] {
        let old = summary(operation);
        assert!(
            !serde_json::to_string(&old).unwrap().contains("\"codex\""),
            "{old}"
        );
        assert!(
            !old["groups"]
                .as_array()
                .unwrap()
                .iter()
                .any(|group| group["vendor"] == "codex"),
            "{old}"
        );
    }
    let modern = summary("meter.usage_summary.v3");
    assert!(
        modern["groups"]
            .as_array()
            .unwrap()
            .iter()
            .any(|group| group["vendor"] == "codex"),
        "{modern}"
    );
    let cli = child_watchdog::output(
        s.arcctl(&f, &project).args([
            "meter",
            "summary",
            "--by",
            "vendor",
            "--from",
            "0",
            "--to",
            "9007199254740991",
            "--bucket",
            "day",
            "--json",
        ]),
        "negotiated Codex summary",
    )
    .unwrap();
    assert!(cli.status.success(), "{cli:?}");
    let cli: Value = serde_json::from_slice(&cli.stdout).unwrap();
    assert!(cli["groups"]
        .as_array()
        .unwrap()
        .iter()
        .any(|group| group["vendor"] == "codex"));
    assert_eq!(ledger::held(&db).iter().max(), Some(&1));
    assert_eq!(ledger::held(&db).last(), Some(&0));

    // Malformed and unsupported events emit no decision and leave the original command to the vendor.
    let malformed = output_with_input(
        &mut s.shell(&f, &project, &codex_hook),
        b"{",
        "malformed Codex hook",
    );
    assert!(malformed.status.success() && malformed.stdout.is_empty());
    let mut unsupported = event(&project, "codex", "unsupported");
    unsupported["tool_input"]["command"] = json!("echo unmanaged");
    let out = output_with_input(
        &mut s.shell(&f, &project, &codex_hook),
        &serde_json::to_vec(&unsupported).unwrap(),
        "unsupported Codex hook",
    );
    assert!(out.status.success() && out.stdout.is_empty());

    // A redelivered event names one job. The second returned command can run
    // untracked while the first holds the permit; it is not a second admission.
    let duplicate_event = event(&project, "codex", "duplicate");
    events.push(duplicate_event.clone());
    let first_command = reply(&f, &s, &project, &codex_hook, "codex", &duplicate_event);
    let second_command = reply(&f, &s, &project, &codex_hook, "codex", &duplicate_event);
    let starts = bun.starts();
    let jobs_before = ledger::entries(&db).len();
    let mut first_shell = s.shell(&f, &project, &first_command);
    bun.arm(&mut first_shell);
    let mut first = Run::start(&mut first_shell, &f.root, "duplicate-first");
    first.until("duplicate original starts", || {
        bun.starts() == starts + 1 && ledger::permits(&db).1 == 1
    });
    first.assert_stdin_cloexec();
    let mut duplicate_shell = s.shell(&f, &project, &second_command);
    bun.arm(&mut duplicate_shell);
    let mut duplicate = Run::start(&mut duplicate_shell, &f.root, "duplicate-second");
    duplicate.until("redelivery starts", || bun.starts() == starts + 2);
    duplicate.input(stage::INPUT);
    let duplicate_done = duplicate.finish("redelivery");
    verify_finish(
        &duplicate_done,
        &bun.take(),
        &duplicate_shell,
        &project,
        "codex",
    );
    assert_eq!(duplicate_done.stderr, stage::TOOL_STDERR);
    assert_eq!(ledger::entries(&db).len(), jobs_before + 1);
    let audit: i64 = db
        .query_row(
            "SELECT count(*) FROM audit_events WHERE event_type='HOOK_EVENT_DUPLICATE'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(audit, 1);
    assert!(first.alive());
    first.input(stage::INPUT);
    let done = first.finish("duplicate original");
    verify_finish(&done, &bun.take(), &first_shell, &project, "codex");
    journey::eventually("duplicate original releases", || {
        ledger::permits(&db).1 == 0
    });
    assert_eq!(ledger::held(&db).iter().max(), Some(&1));
    ledger::unwatch(&db);
    drop(db);
    daemon.stop(libc::SIGTERM);
    f.report();
    crate::hook_record::assert_no_raw_ids_of_any(&f, &events);

    // A peer holding only the old usage advertisements models the old
    // capability vocabulary: the production hook writes no Codex fields. The
    // same locks with the Codex advertisement added are the positive control.
    let old_event = event(&project, "codex", "old-peer");
    let old_peer = advertise(
        &f,
        &[
            arc_ipc::meter::usage::ADVERTISEMENT,
            arc_ipc::meter::usage::ADVERTISEMENT_V2,
        ],
    );
    let old_command = reply(&f, &s, &project, &codex_hook, "codex", &old_event);
    assert!(!old_command.contains("--arc-vendor"), "{old_command}");
    assert!(!old_command.contains("--arc-session"), "{old_command}");
    drop(old_peer);
    let new_peer = advertise(
        &f,
        &[
            arc_ipc::meter::usage::ADVERTISEMENT,
            arc_ipc::meter::usage::ADVERTISEMENT_V2,
            arc_ipc::meter::usage::CODEX_ADVERTISEMENT,
        ],
    );
    let new_command = reply(&f, &s, &project, &codex_hook, "codex", &old_event);
    assert!(
        new_command.contains(" --arc-vendor codex "),
        "{new_command}"
    );
    assert!(new_command.contains(" --arc-session "), "{new_command}");
    drop(new_peer);

    // A disconnected daemon still runs the returned command exactly once.
    let offline_event = event(&project, "codex", "offline");
    let offline = reply(&f, &s, &project, &codex_hook, "codex", &offline_event);
    let before = bun.starts();
    let mut shell = s.shell(&f, &project, &offline);
    bun.arm(&mut shell);
    let mut run = Run::start(&mut shell, &f.root, "offline");
    run.input(stage::INPUT);
    let done = run.finish("offline Codex command");
    verify_finish(&done, &bun.take(), &shell, &project, "codex");
    assert_eq!(
        done.stderr,
        [stage::TRACKING_UNAVAILABLE, stage::TOOL_STDERR].concat()
    );
    assert_eq!(bun.starts(), before + 1);

    // Workspace scope modifies only its own trusted-project hooks file.
    let workspace_dir = project.join(".codex");
    fs::create_dir_all(&workspace_dir).unwrap();
    fs::write(
        workspace_dir.join("hooks.json"),
        br#"{"hooks":{"PostToolUse":[{"hooks":[{"type":"command","command":"keep"}]}]}}"#,
    )
    .unwrap();
    let out = child_watchdog::output(
        s.arcctl(&f, &project)
            .args([
                "hooks",
                "install",
                "codex",
                "--scope",
                "workspace",
                "--workspace",
            ])
            .arg(&project),
        "install workspace Codex hook",
    )
    .unwrap();
    assert!(out.status.success(), "{out:?}");
    let workspace: Value =
        serde_json::from_slice(&fs::read(workspace_dir.join("hooks.json")).unwrap()).unwrap();
    assert_eq!(
        workspace["hooks"]["PostToolUse"][0]["hooks"][0]["command"],
        "keep"
    );
    let out = child_watchdog::output(
        s.arcctl(&f, &project)
            .args([
                "hooks",
                "uninstall",
                "codex",
                "--scope",
                "workspace",
                "--workspace",
            ])
            .arg(&project),
        "uninstall workspace Codex hook",
    )
    .unwrap();
    assert!(out.status.success(), "{out:?}");
    let workspace: Value =
        serde_json::from_slice(&fs::read(workspace_dir.join("hooks.json")).unwrap()).unwrap();
    assert_eq!(
        workspace["hooks"]["PostToolUse"][0]["hooks"][0]["command"],
        "keep"
    );
    assert!(workspace["hooks"]["PreToolUse"]
        .as_array()
        .unwrap()
        .is_empty());

    // An explicit CODEX_HOME is isolated from the default user scope.
    let alternate_home = f.root.join("alternate-codex-home");
    fs::create_dir(&alternate_home).unwrap();
    let default_before = fs::read(s.home.join(".codex/hooks.json")).unwrap();
    let out = child_watchdog::output(
        s.arcctl(&f, &project)
            .env("CODEX_HOME", &alternate_home)
            .args(["hooks", "install", "codex", "--scope", "user"]),
        "install explicit CODEX_HOME hook",
    )
    .unwrap();
    assert!(out.status.success(), "{out:?}");
    assert!(alternate_home.join("hooks.json").is_file());
    assert_eq!(
        fs::read(s.home.join(".codex/hooks.json")).unwrap(),
        default_before
    );
    let out = child_watchdog::output(
        s.arcctl(&f, &project)
            .env("CODEX_HOME", &alternate_home)
            .args(["hooks", "uninstall", "codex", "--scope", "user"]),
        "uninstall explicit CODEX_HOME hook",
    )
    .unwrap();
    assert!(out.status.success(), "{out:?}");

    // Uninstall removes only the owned definition.
    for (vendor, file) in [
        ("claude", ".claude/settings.json"),
        ("codex", ".codex/hooks.json"),
    ] {
        let out = child_watchdog::output(
            s.arcctl(&f, &project)
                .args(["hooks", "uninstall", vendor, "--scope", "user"]),
            "uninstall hook",
        )
        .unwrap();
        assert!(out.status.success(), "{out:?}");
        let config: Value = serde_json::from_slice(&fs::read(s.home.join(file)).unwrap()).unwrap();
        assert_eq!(config["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);
        assert_eq!(
            config["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
            "existing"
        );
        assert_eq!(
            config["hooks"]["PostToolUse"][0]["hooks"][0]["command"],
            "other"
        );
    }
}
