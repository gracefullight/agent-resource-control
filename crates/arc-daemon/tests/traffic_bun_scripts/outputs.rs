//! Tool outcomes are independently checked, including expected negative diagnostics.
use super::{projects::*, *};
pub fn run(read: Read) {
    let case = Case::new("real-outputs", read, false);
    let mut observations = Vec::new();
    for (name, kind) in [
        ("bun-build", Kind::BunBuild),
        ("vite-build", Kind::Vite),
        ("bun-test", Kind::Test),
        ("biome-check", Kind::Lint),
        ("tsc-no-emit", Kind::Typecheck),
    ] {
        let p = project(&case, name, kind, false);
        let argv = kind.argv();
        let mut run = Run::start(&case, &p, &argv);
        run.finish(0);
        assert_eq!(run.count("bun"), 1, "original bun run must execute once");
        let check = validate_artifact(&case, &p, kind);
        let record = case.terminal(&run, kind.class(), kind.reason(), 0);
        observations.push(json!({"name":name,"argv":argv,"cwd":p,"originalStarts":1,"independentOutput":check,"record":record,"stdout":log(&p.join("stdout")),"stderr":log(&p.join("stderr"))}));
    }
    for (name, kind, body, code, expected) in [
        (
            "bad-lint",
            Kind::Lint,
            "debugger;\nexport const fixture = 42;\n",
            1,
            "lint/suspicious/noDebugger",
        ),
        (
            "bad-types",
            Kind::Typecheck,
            "export const fixture: number = 'wrong';\n",
            1,
            "TS2322",
        ),
        ("bad-test", Kind::Test, "", 1, "1 fail"),
    ] {
        let p = project(&case, name, kind, false);
        if kind == Kind::Test {
            write(&p.join("evidence.test.ts"),"import {test,expect} from 'bun:test'; test('expected failure',()=>expect(1).toBe(2));\n");
        } else {
            write(&p.join("good.ts"), body);
        }
        let mut run = Run::start(&case, &p, &kind.argv());
        run.finish(code);
        let out = format!("{}{}", log(&p.join("stdout")), log(&p.join("stderr")));
        assert!(
            out.contains(expected),
            "independent diagnostic {expected}: {out}"
        );
        assert_eq!(run.count("bun"), 1);
        if kind == Kind::Typecheck {
            validate_artifact(&case, &p, kind);
        }
        observations.push(json!({"name":name,"argv":kind.argv(),"outcome":code,"expectedDiagnostic":expected,"output":out,"record":case.terminal(&run,kind.class(),kind.reason(),code)}));
    }
    case.save(
        "real-tools",
        json!({"realTools":true,"injectedPressure":false,"jobs":observations}),
    );
    case.done();
}
