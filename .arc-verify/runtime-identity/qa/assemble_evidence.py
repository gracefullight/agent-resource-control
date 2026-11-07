"""Assemble independent evidence from this run's exact-command logs."""
import datetime
import hashlib
import json
from pathlib import Path
import re

OUT = Path(__file__).resolve().parent
ROOT = Path.cwd()


def read(name):
    return json.loads((OUT / name).read_text())


if __name__ == "__main__":
    environment = read("environment.json")
    source = read("source.json")
    commands = read("commands.json")
    log = (OUT / "test.txt").read_text()
    passed = re.findall(r"^test (\S+) \.\.\. ok$", log, re.MULTILINE)
    summaries = re.findall(r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out", log)
    assert environment["system"] == "Darwin"
    assert source["unchangedDuringChecks"]
    assert all(item["exitCode"] == 0 for item in commands.values())
    assert len(passed) == 53 and all(sum(map(int, row[1:])) == 0 for row in summaries)
    cases = [
        ("Native confstr sizing, growth, byte preservation and failure handling", [
            "tests::confstr_preserves_native_bytes_and_retries_a_growing_buffer",
            "tests::confstr_rejects_errors_malformed_paths_and_unstable_lengths"]),
        ("TMPDIR-independent defaults and persistent database policy", ["native_default_is_independent_of_child_tmpdir"]),
        ("Socket override and empty-override policies, stable anchor", ["nonempty_socket_override_and_empty_override_policy_are_preserved"]),
        ("Actual default endpoint, proven-unwritable TMPDIR, 18 successful status/doctor combinations, 0700/0600 modes", [
            "native_default_and_cli_ignore_unset_changed_and_unwritable_tmpdir"]),
        ("Gated distinct socket/database/TMPDIR contenders; exactly one winner; no losing initialization; graceful and forced-death recovery", [
            "concurrent_distinct_overrides_admit_one_and_recover_after_shutdown_and_death"]),
        ("Unsafe directories, symlinks and nondirectories preserve contents and metadata", [
            "permissive_directories_and_their_contents_survive_daemon_and_cli_rejection",
            "symlink_targets_and_dangling_links_survive_daemon_and_cli_rejection",
            "nondirectory_parent_is_rejected_before_lock_or_database_initialization"]),
        ("Overlong and unsafe socket entries reject before anchor/DB/socket mutation", [
            "overlong_socket_paths_and_existing_socket_entries_reject_without_mutation",
            "server::tests::socket_length_validation_counts_native_bytes_and_preserves_entries"]),
        ("Unsafe lock entries and exact owner/mode policies", [
            "instance::tests::unsafe_anchor_entries_are_rejected_without_repair_or_writes",
            "server::tests::foreign_uid_and_nonprivate_modes_are_rejected_by_the_live_policy"]),
        ("Private new/existing directories and child umasks retain preservation", [
            "private_existing_and_new_nested_directories_support_override_startup",
            "server::tests::already_exists_is_validated_without_repair_or_following_links",
            "server::tests::a_directory_that_appears_between_walks_is_validated_before_nested_creation",
            "server::tests::a_shared_temporary_root_is_refused_instead_of_tightened"]),
        ("Separate cross-process daemon-fixture serialization", ["fixture_coordination_is_shared_across_processes"]),
        ("Live duplicate rejection, dead PID and PID reuse recovery, successor inode preservation", [
            "instance::tests::a_live_duplicate_is_rejected",
            "instance::tests::a_dead_pid_is_recovered_without_killing_anyone",
            "instance::tests::a_mismatched_start_time_is_recovered_without_killing_anyone",
            "instance::tests::dropping_a_stale_holder_preserves_its_successors_lock",
            "instance::tests::a_stale_inode_never_unlinks_the_successors_lock_file",
            "instance::tests::a_stale_inode_still_named_by_the_path_is_unlinked"]),
        ("Existing resident/CLI and storage failure contracts; frozen migration", [
            "resident_contract_holds_until_sigterm", "commands_report_daemon_unavailable_when_absent",
            "migration_failure_exits_nonzero_with_one_stderr_sentence",
            "nfr010_wal_migration_survives_close_and_reopen_without_cleanup",
            "mismatched_checksum_is_rejected_without_reapplying_migration",
            "migration_failure_rolls_back_all_new_tables"]),
    ]
    required = []
    for requirement, tests in cases:
        assert all(test in passed for test in tests), tests
        required.append({"requirement": requirement, "tests": tests, "executed": True,
                         "passed": True, "evidence": str((OUT / "test.txt").relative_to(ROOT))})
    artifacts = {}
    for path in sorted(OUT.iterdir()):
        if path.is_file():
            artifacts[str(path.relative_to(ROOT))] = hashlib.sha256(path.read_bytes()).hexdigest()
    result = {
        "baseline": environment["baseline"],
        "baselineTree": environment["commands"]["baselineTree"]["stdout"].strip(),
        "localHead": environment["commands"]["head"]["stdout"].strip(),
        "platform": "macos", "platformDetails": environment["commands"]["platform"]["stdout"].strip(),
        "collectedAt": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "sourceScope": "Integrated uncommitted working files; all tracked crates plus root Cargo files/toolchain and the two new Rust fixtures",
        "sourceFingerprint": source["sourceFingerprint"],
        "sourceManifest": str((OUT / "source.json").relative_to(ROOT)),
        "sourceEntryCount": len(source["sourceFiles"]),
        "unchangedDuringChecks": source["unchangedDuringChecks"],
        "matchesImplementationManifest": source["sourceFiles"] == json.loads((ROOT / ".arc-verify/runtime-identity/implementation/source.json").read_text())["sourceFiles"],
        "migrationSha256": environment["migration"]["finalSha256"],
        "migrationBaselineSha256": environment["migration"]["baselineSha256"],
        "migrationByteEqual": environment["migration"]["byteEqual"],
        "fixtureRoots": [{"path": environment["confstr"]["root"], "resolution": "confstr(_CS_DARWIN_USER_TEMP_DIR), independently checked against getconf",
                          "confstrAgreesWithGetconf": environment["confstr"]["agreesWithGetconf"],
                          "metadata": environment["rootMetadataBefore"], "writability": environment["fixtureWriteProbe"]}],
        "runtimeAfterTests": read("runtime-after.json"),
        "commands": commands, "requiredCases": required,
        "tests": {"passed": len(passed), "failed": 0, "ignored": 0, "filteredOut": 0,
                  "names": passed, "includesProbeEntrypoints": True,
                  "macosOnlyRequiredTestsObserved": ["tests::confstr_preserves_native_bytes_and_retries_a_growing_buffer",
                    "tests::confstr_rejects_errors_malformed_paths_and_unstable_lengths",
                    "native_default_is_independent_of_child_tmpdir",
                    "native_default_and_cli_ignore_unset_changed_and_unwritable_tmpdir"]},
        "additionalHarness": read("harness.json"),
        "harnessBaselineComparison": read("baseline-comparison.json"),
        "requiredFindings": [],
        "chiefFollowup": ["Additional repository validator failed: three oversized files and missing test:coverage. All three files exceeded 500 lines at baseline; two grew. This QA does not waive architecture conformance."],
        "containerTargets": {"suppliedInAssignment": [], "observations": [],
            "scope": "No target supplied by the saved assignment; av.example.yaml contains only a commented example. No container health claim is made."},
        "limits": [
            "Serena tools unavailable; read assignment-located files directly, without native repository code search.",
            "Mise tracking-symlink creation emitted Operation not permitted; all Cargo gates nevertheless exited 0. No permission bypass or GUI fallback.",
            "Actual foreign-UID ownership changes, real setuid file preservation and non-UTF-8 filesystem creation were not exercised by this QA; attribute/native-byte policy tests ran. Prior engineer reports sandbox restrictions for the latter two.",
            "Tests exercise owned disposable daemon children and isolated database overrides; no user production database or unrelated daemon was modified.",
            "Non-macOS execution and adversarial same-UID namespace replacement are outside the demonstrated evidence.",
            "Architecture conformance remains failed in the additional repository validator; Chief coordination is required outside this fixed runtime gate contract.",
            "No injected OMA claim/run/session identity was present; no secondary mission/OMA run or state mutation was created."
        ],
        "skillsUsed": ["oma-qa"], "report": "docs/runtime-identity-qa.md",
        "artifactSha256": artifacts,
    }
    assert result["migrationByteEqual"] and result["migrationSha256"] == "046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384"
    assert result["matchesImplementationManifest"]
    (ROOT / "docs/runtime-identity-evidence.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"sourceFingerprint": result["sourceFingerprint"], "requiredGroups": len(required),
                      "testsPassed": len(passed), "commands": {k: v["exitCode"] for k, v in commands.items()}}))
