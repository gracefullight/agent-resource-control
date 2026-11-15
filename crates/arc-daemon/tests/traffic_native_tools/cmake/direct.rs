//! Lane check: execute installed tools only, without requiring shared daemon wiring.
use super::*;
use serde_json::json;

#[test]
fn cmake_direct_native_success_and_intended_failure_offline() {
    let tools = Tools::installed();
    println!("CMAKE_NATIVE_PREREQUISITES {}", tools.receipt());
    for form in [Form::CmakeBuild, Form::Ctest] {
        for fail in [false, true] {
            let path = tools.scratch.join(format!(
                "{}-{} project",
                form.id(),
                if fail { "failure" } else { "success" }
            ));
            let prepared = project::prepare(&path, form, &tools, true, fail);
            let original = prepared.argv.clone();
            let baseline = execution::direct(&prepared, &tools);
            assert_eq!(prepared.argv, original, "no rewrite of native argv");
            assert_eq!(
                baseline.code,
                if !fail {
                    0
                } else if form == Form::CmakeBuild {
                    2
                } else {
                    8
                }
            );
            println!(
                "CMAKE_DIRECT_NATIVE {}",
                json!({"form":form.id(),"failure":fail,"evidence":baseline.facts})
            );
        }
    }
}
