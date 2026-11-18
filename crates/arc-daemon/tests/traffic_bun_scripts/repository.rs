//! The repository's actual nested desktop scripts use unchanged native argv.
use super::{projects::*, *};
#[path = "repository_modules.rs"]
mod repository_modules;

pub fn run(read: Read) {
    let case = Case::new("repository-desktop", read, false);
    // Isolated worktrees may link installed dependencies from a read-only
    // workspace. Keep package resolution unchanged while giving Vite its own
    // temporary config directory, and restore the original link on unwind.
    let _modules =
        repository_modules::writable_vite_temp(&root().join("apps/desktop/node_modules"));
    let mut observations = Vec::new();
    let scripts = [
        ("desktop:build", "BUILD", "BUN_SCRIPT_BUILD"),
        ("desktop:typecheck", "TYPECHECK", "BUN_SCRIPT_TYPECHECK"),
        ("desktop:check", "LINT", "BUN_SCRIPT_LINT"),
    ];
    // The build takes the full CPU budget. Start the two read-only checks
    // after it; their reservations can overlap. Observe each shim before
    // dispatching the next command so admission requests are not raced.
    for batch in [&scripts[..1], &scripts[1..]] {
        let mut running = Vec::new();
        for &(name, class, reason) in batch {
            let p = project(&case, name.replace(':', "-").as_str(), Kind::Test, false);
            let mut run = Run::start_with(&case, &p, &["bun", "run", name], |cmd| {
                cmd.current_dir(root());
            });
            eventually(|| {
                let count = run.count("bun");
                if count >= 1 {
                    true
                } else if run.wrapper.child.try_wait().unwrap().is_some() {
                    assert!(run.count("bun") >= 1, "Bun exited without an entry witness");
                    true
                } else {
                    false
                }
            });
            running.push((name, class, reason, p, run));
        }
        for (name, class, reason, p, mut run) in running {
            run.finish(0);
            let record = case.terminal(&run, class, reason, 0);
            let output = format!("{}{}", log(&p.join("stdout")), log(&p.join("stderr")));
            if name == "desktop:build" {
                let index = root().join("apps/desktop/dist/index.html");
                assert!(log(&index).contains("/assets/"));
                let js: Vec<_> = fs::read_dir(root().join("apps/desktop/dist/assets"))
                    .unwrap()
                    .map(|p| p.unwrap().path())
                    .filter(|p| p.extension().is_some_and(|x| x == "js"))
                    .collect();
                assert!(!js.is_empty());
                assert!(js.iter().any(|p| log(p).contains("Typecheck")));
            }
            if name == "desktop:typecheck" {
                assert!(output.contains(
                    "tsc --noEmit -p tsconfig.json && tsc --noEmit -p tsconfig.test.json"
                ));
            }
            if name == "desktop:check" {
                assert!(output.contains("Checked"));
            }
            observations.push(json!({"argv":["bun","run",name],"cwd":root(),"record":record,"outcome":0,"originalArcctlChildStarts":1,"interpreterGraphEntries":run.count("bun"),"output":output}));
        }
    }
    case.save(
        "repository-desktop",
        json!({"scripts":observations,"noInvocationRewrite":true}),
    );
    case.done();
}
