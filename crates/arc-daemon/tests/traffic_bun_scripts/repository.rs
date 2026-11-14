//! The repository's actual nested desktop scripts use unchanged native argv.
use super::{projects::*, *};
pub fn run(read: Read) {
    let case = Case::new("repository-desktop", read, false);
    let mut observations = Vec::new();
    for (name, class, reason) in [
        ("desktop:build", "BUILD", "BUN_SCRIPT_BUILD"),
        ("desktop:typecheck", "TYPECHECK", "BUN_SCRIPT_TYPECHECK"),
        ("desktop:check", "LINT", "BUN_SCRIPT_LINT"),
    ] {
        let p = project(&case, name.replace(':', "-").as_str(), Kind::Test, false);
        let mut run = Run::start_with(&case, &p, &["bun", "run", name], |cmd| {
            cmd.current_dir(root());
        });
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
            assert!(output
                .contains("tsc --noEmit -p tsconfig.json && tsc --noEmit -p tsconfig.test.json"));
        }
        if name == "desktop:check" {
            assert!(output.contains("Checked"));
        }
        observations.push(json!({"argv":["bun","run",name],"cwd":root(),"record":record,"outcome":0,"originalArcctlChildStarts":1,"interpreterGraphEntries":run.count("bun"),"output":output}));
    }
    case.save(
        "repository-desktop",
        json!({"scripts":observations,"noInvocationRewrite":true}),
    );
    case.done();
}
