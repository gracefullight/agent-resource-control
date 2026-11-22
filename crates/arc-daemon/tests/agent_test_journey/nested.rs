//! The `bun run test` tree of AT-TRF-05, with offline `bun` and `vitest`.
//!
//! `bun` runs the package script `vitest`. `vitest` stands for a test that
//! runs an agent command of its own: it passes a PreToolUse event for the
//! nested command to the hook the test names, stops at a gate while the test
//! takes the vendor's part, and then executes the resulting command. `bun`
//! stops at a second gate after `vitest` has exited, so the root is alive
//! with the nested run finished until the test lets it go.
use crate::{
    journey::Sandbox,
    runtime::Fixture,
    stage::{self, Finished, Observed, Run, Tool},
};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub struct Tree {
    directory: PathBuf,
}

/// One run of the tree, as the test and the offline cargo observed it.
pub struct Journey {
    pub finished: Finished,
    pub cargo: Observed,
    /// The command `vitest` executed for its nested agent command.
    pub nested: String,
}

fn quoted(path: &Path) -> String {
    let text = path.to_str().unwrap();
    assert!(!text.contains('\''), "{text}");
    format!("'{text}'")
}

impl Tree {
    pub fn install(f: &Fixture, s: &Sandbox, project: &Path) -> Self {
        let directory = f.root.join("tree");
        fs::create_dir(&directory).unwrap();
        fs::write(
            project.join("package.json"),
            r#"{"name":"journey","scripts":{"test":"vitest"}}"#,
        )
        .unwrap();
        let at = |name: &str| quoted(&directory.join(name));
        stage::executable(
            &s.bin.join("bun"),
            &format!(
                "#!/bin/sh\n\
                 echo \"$*\" >> {starts}\n\
                 vitest\n\
                 status=$?\n\
                 echo \"$status\" > {pending}\n\
                 /bin/mv {pending} {exit}\n\
                 IFS= read -r go < {gate}\n\
                 exit \"$status\"\n",
                starts = at("bun-starts"),
                pending = at("vitest-exit.pending"),
                exit = at("vitest-exit"),
                gate = at("root-gate"),
            ),
        );
        stage::executable(
            &s.bin.join("vitest"),
            &format!(
                "#!/bin/sh\n\
                 echo \"$*\" >> {starts}\n\
                 /bin/sh -c \"$(/bin/cat {hook})\" < {event} > {reply} 2> {errors}\n\
                 echo \"$?\" > {pending}\n\
                 /bin/mv {pending} {exit}\n\
                 IFS= read -r go < {gate}\n\
                 /bin/sh -c \"$(/bin/cat {command})\"\n",
                starts = at("vitest-starts"),
                hook = at("hook"),
                event = at("nested-event"),
                reply = at("nested-reply"),
                errors = at("nested-hook-stderr"),
                pending = at("nested-hook-exit.pending"),
                exit = at("nested-hook-exit"),
                gate = at("nested-gate"),
                command = at("nested-command"),
            ),
        );
        stage::fifo(&directory.join("root-gate"));
        stage::fifo(&directory.join("nested-gate"));
        Self { directory }
    }

    fn lines(&self, name: &str) -> Vec<String> {
        fs::read_to_string(self.directory.join(name))
            .map(|text| text.lines().map(str::to_owned).collect())
            .unwrap_or_default()
    }

    /// Arguments of each `bun` and each `vitest` execution so far.
    pub fn starts(&self) -> (Vec<String>, Vec<String>) {
        (self.lines("bun-starts"), self.lines("vitest-starts"))
    }

    /// Execute `root` in the vendor's shell. `hook` is the hook command the
    /// nested event passes through. `before_nested` runs once that hook has
    /// replied and before the nested command starts; `after_nested` runs when
    /// the nested command and `vitest` have exited and `bun` is still alive.
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &self,
        f: &Fixture,
        s: &Sandbox,
        project: &Path,
        cargo: &Tool,
        name: &str,
        root: &str,
        hook: &str,
        nested: &Value,
        before_nested: impl FnOnce(&mut Run),
        after_nested: impl FnOnce(&mut Run),
    ) -> Journey {
        for stale in ["nested-hook-exit", "vitest-exit", "nested-command"] {
            let _ = fs::remove_file(self.directory.join(stale));
        }
        fs::write(self.directory.join("hook"), hook).unwrap();
        fs::write(
            self.directory.join("nested-event"),
            serde_json::to_vec(nested).unwrap(),
        )
        .unwrap();
        let mut shell = s.shell(f, project, root);
        cargo.arm(&mut shell);
        let mut run = Run::start(&mut shell, &self.directory, name);
        run.input(stage::INPUT);

        let hooked = self.directory.join("nested-hook-exit");
        run.until("the nested hook's reply", || hooked.exists());
        assert_eq!(fs::read_to_string(&hooked).unwrap(), "0\n");
        assert_eq!(
            fs::read(self.directory.join("nested-hook-stderr")).unwrap(),
            b""
        );
        let reply = fs::read(self.directory.join("nested-reply")).unwrap();
        let command = stage::vendor_command(&reply, nested);
        before_nested(&mut run);
        fs::write(self.directory.join("nested-command"), &command).unwrap();
        stage::release(&mut run, &self.directory.join("nested-gate"));

        let exited = self.directory.join("vitest-exit");
        run.until("the nested command's exit", || exited.exists());
        assert_eq!(
            fs::read_to_string(&exited).unwrap(),
            format!("{}\n", stage::TOOL_EXIT)
        );
        assert!(run.alive(), "the root exited with its nested command");
        after_nested(&mut run);
        stage::release(&mut run, &self.directory.join("root-gate"));
        let finished = run.finish("root agent command");
        let cargo = cargo.take();
        stage::assert_environment(&cargo, &shell);
        Journey {
            finished,
            cargo,
            nested: command,
        }
    }
}
