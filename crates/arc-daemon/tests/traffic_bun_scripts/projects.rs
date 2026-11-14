//! Real installed tools and independent fixture sources. Shims observe then exec.
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
pub struct Tools {
    pub bun: PathBuf,
    pub cargo: PathBuf,
    pub vite: PathBuf,
    pub tsc: PathBuf,
    pub biome: PathBuf,
    pub shims: PathBuf,
}
fn output(program: &Path, args: &[&str]) -> String {
    let o = Command::new(program)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("required real tool {}: {e}", program.display()));
    assert!(
        o.status.success(),
        "required real tool {}: {o:?}",
        program.display()
    );
    String::from_utf8(o.stdout).unwrap().trim().to_owned()
}
fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
impl Tools {
    pub fn installed() -> Self {
        assert_eq!(output(Path::new("bun"), &["--version"]), "1.4.4");
        let bun = PathBuf::from(output(Path::new("bun"), &["-p", "process.execPath"]));
        let cargo = PathBuf::from(env!("CARGO"));
        assert!(output(&cargo, &["--version"]).contains("1.95.0"));
        let vite = root()
            .join("apps/desktop/node_modules/.bin/vite")
            .canonicalize()
            .expect("installed pinned Vite");
        let tsc = root()
            .join("apps/desktop/node_modules/.bin/tsc")
            .canonicalize()
            .expect("installed pinned TypeScript");
        let biome = root()
            .join("node_modules/.bin/biome")
            .canonicalize()
            .expect("installed pinned Biome");
        assert!(output(&vite, &["--version"]).contains("8.3.8"));
        assert!(output(&tsc, &["--version"]).contains("7.0.2"));
        assert!(output(&biome, &["--version"]).contains("2.6.6"));
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let shims = root().join(format!(
            "target/traffic-bun-scripts/shims/{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&shims).unwrap();
        let tools = Self {
            bun: bun.canonicalize().unwrap(),
            cargo: cargo.canonicalize().unwrap(),
            vite,
            tsc,
            biome,
            shims,
        };
        for (name, real) in [
            ("bun", &tools.bun),
            ("cargo", &tools.cargo),
            ("vite", &tools.vite),
            ("tsc", &tools.tsc),
            ("biome", &tools.biome),
        ] {
            let script = format!(
                r##"#!/bin/sh
label={name}
if [ "$label" = bun ] && [ "$1" != run ]; then label=bun-leaf; fi
printf '%s %s %s\n' "$label" "$$" "$PPID" >> "$TBS_PROJECT/launches" || exit 125
if [ -n "${{TBS_PREDECESSOR:-}}" ] && {{ [ "$label" = bun ] || [ "$label" = cargo ]; }}; then
  case "$TBS_PREDECESSOR" in *[!0123456789abcdef]*|'') exit 125;; esac
  [ "${{#TBS_PREDECESSOR}}" -eq 32 ] || exit 125
  if kill -0 "$TBS_PREDECESSOR_PID" 2>/dev/null; then exit 125; fi
  state=$(/usr/bin/sqlite3 -readonly -cmd '.timeout 2000' -cmd ".parameter set :first \"'$TBS_PREDECESSOR'\"" -cmd ".parameter set :observer $PPID" "$ARC_DB_PATH" "SELECT p.job_id || ' ' || p.state || ' ' || p.released_at || ' ' || t.job_id || ' ' || q.state || ' ' || t.granted_at_ms FROM permits p JOIN traffic_entries t JOIN job_lifecycle l ON l.job_id=t.job_id JOIN permits q ON q.job_id=t.job_id WHERE p.job_id=:first AND l.observer_pid=:observer AND p.state='RELEASED' AND p.released_at IS NOT NULL AND q.state='ACTIVE' AND t.granted_at_ms>=p.released_at;") || exit 125
  [ -n "$state" ] || exit 125
  printf '%s\n' "$state" > "$TBS_PROJECT/release-witness" || exit 125
fi
exec {real} "$@"
"##,
                real = quote(real.to_str().unwrap())
            );
            let path = tools.shims.join(name);
            write(&path, script);
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        tools
    }
    pub fn receipt(&self) -> Value {
        json!([
            (&self.bun, "1.4.4"),
            (&self.cargo, "1.95.0"),
            (&self.vite, "8.3.8"),
            (&self.tsc, "7.0.2"),
            (&self.biome, "2.6.6")
        ]
        .map(|(p, v)| json!({"path":p,"version":v,"sha256":sha(p)})))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Cargo,
    BunBuild,
    Vite,
    Test,
    Lint,
    Typecheck,
}
impl Kind {
    pub fn class(self) -> &'static str {
        match self {
            Self::Cargo | Self::BunBuild | Self::Vite => "BUILD",
            Self::Test => "TEST",
            Self::Lint => "LINT",
            Self::Typecheck => "TYPECHECK",
        }
    }
    pub fn reason(self) -> &'static str {
        match self {
            Self::Cargo => "CARGO_BUILD",
            Self::BunBuild | Self::Vite => "BUN_SCRIPT_BUILD",
            Self::Test => "BUN_SCRIPT_TEST",
            Self::Lint => "BUN_SCRIPT_LINT",
            Self::Typecheck => "BUN_SCRIPT_TYPECHECK",
        }
    }
    pub fn argv(self) -> Vec<&'static str> {
        if self == Self::Cargo {
            vec!["cargo", "build", "--offline"]
        } else {
            vec!["bun", "run", "unexpected"]
        }
    }
}
pub fn project(case: &Case, name: &str, kind: Kind, hold: bool) -> PathBuf {
    let path = case.artifacts.join(name);
    fs::create_dir_all(&path).unwrap();
    if kind == Kind::Cargo {
        fs::create_dir(path.join("src")).unwrap();
        write(&path.join("Cargo.toml"),"[package]\nname=\"bun_scripts_build_fixture\"\nversion=\"0.1.0\"\nedition=\"2021\"\n[workspace]\n");
        write(
            &path.join("src/lib.rs"),
            "pub fn independent_artifact() -> u32 { 42 }\n",
        );
        write(&path.join("build.rs"), include_str!("sources/build.rs.txt"));
        return path;
    }
    // The installed workspace dependencies are reused, never installed by a test.
    fs::create_dir_all(path.join("node_modules/.bin")).unwrap();
    for tool in ["bun", "vite", "tsc", "biome"] {
        symlink(
            case.tools.shims.join(tool),
            path.join("node_modules/.bin").join(tool),
        )
        .unwrap();
    }
    let script = match kind {
        Kind::BunBuild => "bun build entry.ts --target bun --outfile bundle.js",
        Kind::Vite => "vite build",
        Kind::Test => "bun test evidence.test.ts --timeout 120000",
        Kind::Lint => "biome check --config-path . --error-on-warnings good.ts",
        Kind::Typecheck => "tsc --noEmit -p tsconfig.json",
        Kind::Cargo => unreachable!(),
    };
    write(
        &path.join("package.json"),
        json!({"type":"module","scripts":{"unexpected":script}}).to_string(),
    );
    write(
        &path.join("entry.ts"),
        "console.log('ARC_BUN_BUNDLE_42');\n",
    );
    write(&path.join("good.ts"), "export const fixture = 42;\n");
    write(
        &path.join("biome.json"),
        json!({"root":true,"vcs":{"enabled":false},"files":{"includes":["**/*.ts"]}}).to_string(),
    );
    match kind {
        Kind::Vite => {
            write(&path.join("index.html"),"<html><head></head><body><script type=\"module\" src=\"/entry.ts\"></script></body></html>\n");
            write(
                &path.join("vite.config.mjs"),
                include_str!("sources/vite.config.mjs"),
            );
        }
        Kind::Test => write(
            &path.join("evidence.test.ts"),
            include_str!("sources/evidence.test.ts"),
        ),
        Kind::Typecheck => {
            let files = if hold {
                vec!["good.ts", "gate.ts"]
            } else {
                vec!["good.ts"]
            };
            write(&path.join("tsconfig.json"),json!({"compilerOptions":{"target":"esnext","types":[],"strict":true},"files":files}).to_string());
            if hold {
                let p =
                    std::ffi::CString::new(path.join("gate.ts").as_os_str().as_bytes()).unwrap();
                assert_eq!(unsafe { libc::mkfifo(p.as_ptr(), 0o600) }, 0);
            }
        }
        _ => (),
    }
    if !hold {
        write(&path.join("release"), "ready");
    }
    path
}
pub fn validate_artifact(case: &Case, path: &Path, kind: Kind) -> Value {
    match kind {
        Kind::Cargo => {
            let p = path.join("target/debug/libbun_scripts_build_fixture.rlib");
            assert!(fs::metadata(&p).unwrap().len() > 0);
            json!({"artifact":p,"sha256":sha(&p)})
        }
        Kind::BunBuild => {
            let p = path.join("bundle.js");
            assert!(log(&p).contains("ARC_BUN_BUNDLE_42"));
            assert_eq!(
                output(&case.tools.bun, &[p.to_str().unwrap()]),
                "ARC_BUN_BUNDLE_42"
            );
            json!({"bundleSha256":sha(&p),"evaluated":"ARC_BUN_BUNDLE_42"})
        }
        Kind::Vite => {
            let html = log(&path.join("dist/index.html"));
            assert!(html.contains("/assets/"));
            let js: Vec<_> = fs::read_dir(path.join("dist/assets"))
                .unwrap()
                .map(|p| p.unwrap().path())
                .filter(|p| p.extension().is_some_and(|x| x == "js"))
                .collect();
            assert_eq!(js.len(), 1);
            assert!(log(&js[0]).contains("ARC_BUN_BUNDLE_42"));
            json!({"htmlSha256":sha(&path.join("dist/index.html")),"bundleSha256":sha(&js[0])})
        }
        Kind::Test => {
            let stderr = log(&path.join("stderr"));
            assert!(
                stderr.contains("1 pass") && stderr.contains("0 fail"),
                "{stderr}"
            );
            assert!(log(&path.join("stdout")).contains("TBS_TEST_OK"));
            json!({"passes":1,"failures":0,"bodyCount":log(&path.join("bodies")).lines().count()})
        }
        Kind::Lint => {
            let out = format!("{}{}", log(&path.join("stdout")), log(&path.join("stderr")));
            assert!(out.contains("Checked 1 file"), "{out}");
            json!({"checkedFiles":1})
        }
        Kind::Typecheck => {
            for p in ["good.js", "gate.js", "good.d.ts", "tsconfig.tsbuildinfo"] {
                assert!(!path.join(p).exists(), "--noEmit emitted {p}");
            }
            json!({"noEmit":true})
        }
    }
}
