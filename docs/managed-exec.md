# Direct execution and lifecycle tracking

Run a program directly:

```sh
arcctl exec -- cargo test
arcctl exec -- bun test
arcctl exec -- printf '%s\n' 'two words'
arcctl exec -- '/path/with spaces/tool' '' '--help' '|'
```

The daemon is optional for execution. ARC attempts the command once when an outage leaves lifecycle tracking unavailable or unconfirmed. Cancellation, invalid arguments, explicit authenticated rejection, invalid protocol replies and local preparation failures do not start a child; a native spawn failure is reported separately.

Pass the executable and each argument separately after `--`. Your shell handles quoting. ARC passes native arguments directly to the process API, without interpreting pipes, redirections or substitutions; the one exception is the Bash source of the two mixed compounds described in [Mixed command stages](#mixed-command-stages). An executable name containing spaces stays one name. Unsupported classification, including native non-UTF-8 arguments, becomes UNKNOWN and still executes.

The CLI owns one child and inherits the caller's working directory, environment and standard descriptors. Scheduled Go, SwiftPM and CMake/CTest commands receive `ARC_TRAFFIC_LINEAGE` exactly as supplied by the caller, including when it is absent or empty. For other scheduled families ARC retains the legacy behavior of adding an opaque value only when the variable is absent; an existing value is preserved. ARC records scheduled lineage in a private sidecar. A new nested CLI prefers a live ancestor's sidecar and may fall back to an inherited environment value when no live sidecar is found. A nested eligible job can reuse its ancestor's permit only after the daemon verifies its process ancestry; the value alone does not grant admission. Child exit codes, including 17, are preserved; native signal termination is re-raised after bookkeeping. With a controlling terminal, ARC keeps its child in the caller's process group and does not change foreground ownership. Terminal signals reach the whole job without duplicate forwarding. Without a controlling terminal, the child gets a separate process group. Wrapper-directed nonterminal signals, including SIGTERM and SIGHUP, are forwarded to the child. A stopped child suspends the wrapper; the shell's background and foreground controls apply to the whole terminal job. Inherited ignored signal dispositions and terminal modes are preserved.

## Remote endpoint classification

For supported invocations, `arcctl exec` captures bounded, fresh facts from the command, environment and local configuration before deciding whether work uses a local permit. It resolves the selected public GitHub host for `gh`, an explicit Git push destination or the configured destination of a bare `git push`, SSH destination, Docker context and Buildx builder, and kubectl context. Git selection accounts for branch and push defaults, push URLs and supported URL rewrites. Explicit options and supported environment overrides are considered in their respective selection order. Endpoint capture does not run Git, SSH, Docker, kubectl or project code, make network requests, or start a background refresh. Raw endpoint facts are not saved in job records.

An endpoint can be unknown when configuration is missing, stale, conflicting, unsupported or cannot be verified within the capture bounds. A Unix socket alone does not prove where Docker computes. On macOS, ARC can establish local Docker placement without contacting the socket when the socket ownership and operating-system listener inventory identify exactly one supported local engine. A Kubernetes API endpoint does not establish where a pod runs; classification does not launch `kubectl` or contact a cluster. Such unresolved commands remain `UNKNOWN`/`OBSERVE`: this expresses uncertainty, not proof of a remote target. UNKNOWN commands bypass the local Traffic queue and reservations, so an actually local command whose location ARC cannot establish may also run without a local permit.

The finite `act -j build` form can use an explicitly selected, proven-local Docker socket for local admission; an unproved endpoint remains unknown. A same-PC runner can submit its local workload through `arcctl exec`, so that workload is classified and admitted according to the actual managed child. ARC does not infer a runner's placement from its name. The bounded mixed examples include the selected literal `make test` recipe and `pnpm run deploy` script; ARC revalidates their selected files and observes unsupported or changed inputs instead of guessing. Bash and zsh `-c` execution uses captured facts only when the selected system shell's startup cannot change them. Startup-dependent Bash, login-shell and zsh cases remain UNKNOWN/OBSERVE.

`arcctl classify -- 'gh workflow run build.yml'` is an optional inspection of literal shell text using the same endpoint capture. It does not run the command or provide a receipt for an execution. The actual command may have different arguments, environment, working directory or configuration. The inspection route can report `UNRESOLVED_COMMAND` where native execution records `ENDPOINT_AMBIGUOUS`; both retain `UNKNOWN`/`OBSERVE`. For a supported remote execution, inspect an available job status or Meter summary for recorded results; missing tracking does not prove the command did or did not run.

The endpoint-aware classify and lifecycle operations are additive to protocol 1. Older peers retain their existing requests and responses; when they cannot establish endpoint-aware classification, ARC falls back conservatively. A build on a proven local Docker engine or Buildx builder, including the proven-local `act -j build` form, is recorded BUILD with compute location `LOCAL_CONTAINER` and takes a BUILD reservation. That location is additive vocabulary as well: `arcctl classify` and the negotiated `job.status.container.v1` reader name it, every other reader of the same record, including Traffic diagnostics and the desktop, receives `HOST_LOCAL`, and a protocol-1 reader keeps its UNKNOWN result. The desktop therefore shows such a build as an ordinary local build. The CLI's own native CPU accounting can be read separately with `arcctl meter summary --job <JOB_ID> --json` when a report was recorded; it is not a measurement of remote-builder computation.

## Agent hook setup and recovery

Claude Code Bash and Cursor Shell hooks can route the exact literals `cargo build`, `bun run build`, `gh workflow run build.yml`, `docker build .` and `docker buildx build .`, and the two exact mixed compounds `bun test && gh workflow run ci.yml` and `make test && gh workflow run ci.yml`, through ARC on the supported local macOS execution surface. The gh rewrite preserves shell-owned functions and aliases and passes through when its executable resolution cannot be established. It also leaves the command to zsh when a script or nested evaluation makes the shell's execution context uncertain. Non-default tracing can add wrapper trace lines on stderr. Other commands, disabled or recognized remote events, and hook errors pass through unchanged. For a no-change hook run the CLI writes no response: Claude exits 0 and Cursor exits 1. Cursor's documented default treats that nonzero exit as fail-open; the ARC installer does not set `failClosed`.

The operator recorded the vendors accepting permission-free rewrite responses on macOS. Those sessions did not exercise the ARC hook or an interactive approval prompt. Installing a hook alone does not prove invocation or scheduling.
Choose one vendor and scope. Workspace scope requires an existing absolute directory; replace the example path with the workspace root. User scope does not accept `--workspace`.

```sh
arcctl hooks install claude --scope user
arcctl hooks install claude --scope workspace --workspace "/absolute/path/to/workspace"
arcctl hooks install cursor --scope user
arcctl hooks install cursor --scope workspace --workspace "/absolute/path/to/workspace"
```

Use the matching command and scope to remove an installation:

```sh
arcctl hooks uninstall claude --scope user
arcctl hooks uninstall claude --scope workspace --workspace "/absolute/path/to/workspace"
arcctl hooks uninstall cursor --scope user
arcctl hooks uninstall cursor --scope workspace --workspace "/absolute/path/to/workspace"
```

The Claude installer targets `~/.claude/settings.json` for user scope, or `settings.json` under `CLAUDE_CONFIG_DIR` when that variable is set and nonempty. Workspace scope targets `.claude/settings.json` under the supplied directory. Cursor uses `~/.cursor/hooks.json` for user scope and `.cursor/hooks.json` under the supplied workspace; its user scope requires an existing absolute `HOME`. Installing requires an available absolute `arcctl` executable. The installer modules preserve unrelated settings and hook order, update an existing owned entry instead of adding a duplicate, and remove only the entry whose ownership they can establish. These properties were exercised in isolated settings fixtures.

For Claude, ownership is recorded in `settings.json.arc-claude-id` beside the settings file; if settings is a symlink, the record is beside its resolved target. If the identity cannot be read, the installer stops; restore access to the existing record before retrying. If the identity is invalid, a marked candidate has no matching identity, or a candidate is ambiguous, the installer stops and leaves the settings and identity unchanged. Restore the matching identity from a trustworthy backup and review any extra candidates. Otherwise, review the settings and manually remove only entries you can establish as yours along with the stale identity before retrying. Do not treat a copied command or marker alone as proof of ownership; leave an entry intact if its ownership remains unclear. Cursor similarly stops on an unproven, changed or duplicated `ARC_CURSOR_HOOK_V1` entry; review the marked entry and its ownership record in `.hooks.json.arc.lock` beside `hooks.json` before resolving it and retrying. If settings changed during an update, retry after reviewing the intervening change.

### Bounded hook behavior

The production transformation accepts only the exact literal text `cargo build`, `bun run build`, `gh workflow run build.yml`, `docker build .` and `docker buildx build .`, and the exact text of the two compounds in [Mixed command stages](#mixed-command-stages). The two Docker literals become one `hook-exec` child; the hook resolves no Docker endpoint and contacts no socket. The later `hook-exec` process captures the context and builder, so only a proven local endpoint is recorded BUILD with compute location `LOCAL_CONTAINER` and takes a BUILD reservation, while a remote, mixed-node, missing or conflicting endpoint keeps its REMOTE_EXEC or UNKNOWN/OBSERVE pass-through. Extra whitespace, options, quotes, assignments, any other shell operator, redirections, substitutions, path-qualified commands and already wrapped commands receive no update. The hook does not execute the command. Its response preserves the complete tool input except for the command and does not make a permission decision; the vendor's normal permission evaluation remains in place. An interactive approval prompt has not been observed. A rewritten Bun command is admitted as BUILD only when the target workspace's build script is resolved by the bounded script capability. The installed-hook gh execution has been exercised with a local daemon and offline stub; this is simulated agent execution, not a live vendor or GitHub acceptance result.

The supported surface is the recorded local macOS tool shell, which starts `arcctl hook-exec` in the project cwd with the tool shell's PATH. ARC resolves the program through that inherited PATH after the hook returns. A shell alias or function named `cargo`, `bun` or `docker` is bypassed by the wrapped command; in a stage rewrite the same holds for `bun`, `make` and `gh`. The gh route has isolated sh, Bash and zsh fixture coverage; live vendor execution, remote and cloud agents, and Windows remain unverified. Recognized remote markers `CLAUDE_CODE_REMOTE=true`, `CURSOR_CODE_REMOTE=true`, or `SSH_CONNECTION` cause pass-through. `ARC_HOOKS_DISABLED=1` disables both adapters; `ARC_HOOKS_CLAUDE_DISABLED=1` and `ARC_HOOKS_CURSOR_DISABLED=1` disable the named adapter. If the installed `arcctl` path disappears, restore the executable and update the installation.

An executable text file without an interpreter line can run directly in zsh but fails as a native spawn through `hook-exec`.

The child inherits cwd, streams and environment, including `ARC_TRAFFIC_LINEAGE` without legacy injection, and retains its exit status; ARC may add queue or outage diagnostics to stderr. For `_`, the hook route substitutes the PATH-resolved tool spelling only when the inherited value exactly names the current wrapper executable and the tool can be resolved; otherwise it leaves the inherited value unchanged. A tracking outage uses the bounded single-attempt behavior described in [Outages and recovery](#outages-and-recovery). Use the vendor's observed result and, when available, the job status guidance below. A missing record or an unavailable status does not establish whether a command ran; do not start another copy merely to recover tracking.

### Mixed command stages

Exactly two compounds are separated into stages, on both routes:

```sh
bun test && gh workflow run ci.yml
make test && gh workflow run ci.yml
```

The hook rewrites that exact text to `<arcctl> hook-exec --arc-request <ID> -- <test stage> && <arcctl> hook-exec --arc-request <ID> -- gh workflow run ci.yml`, with one fresh request ID per rewrite. `arcctl exec -- bash -c '<compound>'` does the same to the Bash source when it records the command as MIXED with no uncertainty, which requires a proven remote CI endpoint and, for `make test`, the selected literal recipe `cargo test`; the Bash child stays the request record and its job ID is the request ID. `hook-exec` accepts exactly the three stage commands `bun test`, `make test` and `gh workflow run ci.yml` in addition to the five literals above. `--arc-request` is consumed by `arcctl`: it never reaches the stage's arguments or environment, and a malformed value exits 2 without starting a child.

The shell that received the command keeps `&&`. ARC schedules nothing between the stages: a failing test stage prevents the gh stage, and the compound's exit status is the shell's. Each stage is one ordinary managed child with the caller's arguments, working directory, environment, streams and exit status. The test stage is recorded TEST and waits for a TEST permit like a directly executed test; while it waits its `hook-exec` writes one `QUEUED` notice and, once started, one `STARTED` notice to stderr. The notice's `Reference:` line is the waiting stage's job ID, and `arcctl job status <JOB_ID>` shows that record as `State: queued`. The gh stage is classified on its own when it starts. With a proven remote CI endpoint it is recorded REMOTE_CI, takes no queue entry or reservation, writes no notice and does not wait for local permits. Both stage records are linked to the request; the additive `job.request.v1` operation returns them in order. A stage is linked when its admission returns, so a stage that is still waiting is read through job status. With no daemon each stage is still attempted once and writes its own outage notice.

Every other spelling keeps the earlier single MIXED or UNKNOWN/OBSERVE record and runs unchanged in the shell that received it. That includes a command substitution, a redirection, a dynamic workflow name, a nested shell, `sh -c` and `zsh -c` on the `arcctl exec` route, and a Bash whose startup is altered by `BASH_ENV`, `SHELLOPTS`, `BASHOPTS` or an exported function. `arcctl classify` still reports the two compounds as MIXED and starts nothing.

The Traffic CPU budget is shared by all local classes. With the default budget of two cores a held BUILD reservation is the whole budget, so a test stage also waits for a running build; the daemon's `ARC_TRAFFIC_CPU_BUDGET_MILLIS` setting raises the budget. The stage tests ran with a private daemon, a four-core budget and offline stubs; no live GitHub dispatch, vendor runtime or Bun or Cargo toolchain was exercised.

## Scheduling

The daemon records lifecycle metadata and opaque scheduling capabilities; it does not record command text or other environment values. With a healthy daemon that supports the relevant Traffic capability, directly executed local `cargo build`, `cargo test`, `cargo clippy`, `cargo fmt`, one-shot `bun test`, the Python forms below and the seven supported native-tool forms below normally enter a durable FIFO queue. A verified nested job can reuse its ancestor's permit. For direct Cargo/Bun commands, the detector checks for the program token `cargo` or `bun` followed by a supported subcommand; it does not resolve the executable. An explicit Cargo/Bun path such as `/path/to/bun` or a wrapper invoked under an unrecognized name such as `bun-wrapper` is unresolved and bypasses Traffic admission. Bounded `bun run` script graphs use the existing separately negotiated script capability. A PATH shim invoked as `bun` or `cargo` can still be eligible.

For Bun, test paths and recognized one-shot options can be scheduled. Watch or hot mode, such as `bun test --watch` or `bun test --hot`, and unresolved forms such as unknown or ambiguous options bypass the Traffic queue and reservations. Remote, UNKNOWN and other unsupported classifications also bypass them; they do not wait for a Traffic permit. When started, Bun receives the original arguments, including for bypassed forms.

`arcctl classify` analyzes literal command text rather than the native argument vector used by `arcctl exec`. Its conservative Bun result can be unresolved even when a direct invocation is eligible. Python and Go/SwiftPM/CMake/CTest recognition applies only to native argv: literal Python text, including `arcctl classify 'python -m pytest'` and shell compounds, retains UNKNOWN/OBSERVE. Literal native-tool text such as `arcctl classify 'go build'` also remains unresolved. A classification alone does not show whether a particular execution was admitted.

### Native Go, SwiftPM and CMake/CTest commands

This macOS slice recognizes exactly seven native argv forms. It shares the existing BUILD, TEST and LINT windows, global reservations, Governor, SQLite writer and Meter. No tool, manifest, target discovery or project code runs during classification. Bare executables and relative/absolute paths whose basename is exactly `go`, `swift`, `cmake` or `ctest` are accepted, with no trailing slash. The original executable, arguments, cwd, environment, streams, signals and exit status pass through unchanged.

| Supported example | Class | Unsupported example (executes with admission bypassed) |
| --- | --- | --- |
| `go -C ./project build -p 2 -o ./out ./cmd/app` | BUILD | `go build -toolexec custom ./...` |
| `go test -run TestSmoke -count=1 ./...` | TEST | `go test -exec custom ./...` |
| `go vet -printf ./...` | LINT | `go vet -vettool custom ./...` |
| `swift build --package-path ./Package -c release -j 2` | BUILD | `swift build --target Custom` |
| `swift test --scratch-path ./cache --parallel --num-workers 2 --filter Smoke` | TEST | `swift test --list-tests` |
| `cmake --build ./build --config Debug -j2 --target all` | BUILD | `cmake --build ./build --target custom` |
| `ctest --test-dir ./build -C Debug -R Smoke --output-on-failure` | TEST | `ctest -S dashboard.cmake` |

The grammar is finite; unlisted options bypass admission. Paths and opaque values are nonempty UTF-8 arguments without a leading `-`. Integers are ASCII decimal 1–1024. Options occur once (aliases share a key) unless repetition is explicitly allowed. Empty, NUL, non-UTF-8, missing, malformed or conflicting arguments bypass the whole invocation. Analysis uses existing byte/token/invocation/depth/work budgets (16384/1024/128/8/65536); exhaustion observes rather than blocks the original execution.

- Go: optional `-C PATH` or `-C=PATH` precedes `build`, `test` or `vet`. Flags precede selectors. Empty selection means native default package. Selectors are all `.go` paths or all ASCII package words using letters, digits and `_./~+-`, including `.`, `./...`, `../pkg`, `example.org/pkg`, `all`, `std`, `cmd`. Version suffixes and source/package mixtures bypass. Values accept separate or `=value` syntax; no attached short values.
- Go common flags: `-a`, `-v`, `-x`, `-race`, `-trimpath`; values: `-p INTEGER`, `-tags VALUE`, `-mod readonly|vendor|mod`, `-modfile PATH`. Build adds `-o PATH`. Test adds `-short`, `-failfast`, `-json`; values: `-count INTEGER`, `-parallel INTEGER`, `-run VALUE`, `-skip VALUE`, `-bench VALUE`, `-timeout DURATION`, `-benchtime DURATION`. Duration is a positive u32 followed by `ns`, `us`, `ms`, `s`, `m` or `h`. Vet adds `-all`, `-composites`, `-printf`, `-structtag`, `-unusedresult`.
- Swift: `swift build|test FLAGS`, no positionals. Common flags: `-v|--verbose`, `-q|--quiet`; values: `-c|--configuration debug|release`, `--package-path PATH`, `--scratch-path PATH`, `-j|--jobs INTEGER`. Long values accept separate or `=value`; short values are separate. Build adds `--build-tests`. Test adds `--parallel`, `--skip-build`, `--enable-code-coverage`, `--num-workers INTEGER` (requires `--parallel`) and repeatable `--filter VALUE`/`--skip VALUE`. `run`, `package`, product/target selection, test-list/debugger modes and `-X*` forwarding bypass.
- CMake: `cmake --build PATH FLAGS`, path first and separate. Flags: `--clean-first`, `-v|--verbose`; values: `--config Debug|Release|RelWithDebInfo|MinSizeRel`, `-j|--parallel INTEGER`, `-t|--target all`. Only `-jINTEGER` accepts an attached value; no equals. `all` is the only resolved aggregate target. Configure, presets, `-P`, `-E`, custom targets and payload after `--` bypass.
- CTest: flags only: `--output-on-failure`, `--stop-on-failure`; one verbosity choice `-V|--verbose`, `-VV|--extra-verbose` or `-Q|--quiet`. Values: `--test-dir PATH`, `-C|--build-config Debug|Release|RelWithDebInfo|MinSizeRel`, `-j|--parallel INTEGER`, `-R|--tests-regex VALUE`, `-E|--exclude-regex VALUE`, repeatable `-L|--label-regex VALUE`/`-LE|--label-exclude VALUE`. Only `-jINTEGER` is attached; no equals or positionals. Show-only, dashboard/script `-D/-M/-T/-S/-SP` and `--build-and-test` bypass.

The daemon defaults remain a 2000-millicore shared CPU budget and the existing per-class reservations/windows. An explicit `ARC_TRAFFIC_CPU_BUDGET_MILLIS` setting at daemon startup accepts decimal millicores that fit every class reservation, exact JSON integers (at most 9007199254740991) and SQLite integers; malformed or impossible values reject startup. Admission and diagnostics share that immutable resident setting. Native contention fixtures set 4000 millicores, retain the one-slot class windows and prove both estimates fit the CPU and memory budgets before asserting a class-window wait. This configures only owned fixture daemons during verification.

Wrappers such as `env`, `sh` and `xcrun`, custom execution flags, help/version/dry-run modes and unresolved targets bypass. Classification never rewrites a rejected invocation or prevents native execution. Broader DET-003, other vendor integration and native Windows acceptance remain open.

### Python TEST commands

This macOS Python slice recognizes `pytest`, `python -m pytest` and `python -m unittest`. Interpreter basenames are exactly `python`, `python3`, and `python3.8` through `python3.15`; the separate arguments `-m pytest` or `-m unittest` must immediately follow the executable, with no interpreter flags before them.

The executable can be a bare name or a relative or absolute path with basename `pytest` or a listed interpreter basename and no trailing slash. Virtual-environment examples include `.venv/bin/pytest`, `.venv/bin/python`, `../env/bin/python3` and `/work tree/.venv/bin/python3.13`. Quote paths containing spaces as one shell argument:

```sh
arcctl exec -- pytest tests/test_native.py::test_one -q
arcctl exec -- python -m pytest tests -k 'fast or smoke'
arcctl exec -- python -m unittest discover -s tests -p 'test_*.py' -t .
arcctl exec -- python -m unittest tests.test_native.TestNative.test_one -v
arcctl exec -- .venv/bin/python -m pytest
arcctl exec -- .venv/bin/pytest tests
arcctl exec -- '/work tree/.venv/bin/python3.13' -m unittest discover --start-directory=tests
```

Recognition is lexical and bounded: classification does not look up or execute an interpreter, read project configuration, or import project code. It does not activate a virtual environment or verify that a named runner is installed. ARC preserves the original native executable and arguments; it does not substitute console-script `pytest` for `python -m pytest`, or vice versa. Their native interpreter selection and module-import behavior remain intact, together with the working directory, environment, streams and signals described above.

The finite selector and discovery grammar is:

| Runner | Selectors and discovery |
| --- | --- |
| `pytest` or `python -m pytest` | No selectors retains native discovery. Accepts non-option file/directory paths, optionally followed by `::`-separated node names, such as `tests/test_native.py::TestNative::test_one`. Path and node components must be nonempty. Node parameter text in `[parameters]` is opaque, including embedded `::`; an opened parameter must close. Brackets in the path are literal. |
| `python -m unittest` | No selectors retains native discovery. Accepts dotted module/class/method names whose components match `[A-Za-z_][A-Za-z0-9_]*`, or non-option relative/absolute file paths ending in `.py`. |
| `python -m unittest discover` | `discover` must be the first argument after `unittest`. Only the options below may follow it; positional discovery shorthand is unresolved. Omitted discovery options retain native defaults. |

Only these options are recognized:

| Runner | Options |
| --- | --- |
| Pytest flags | `-q`, `--quiet`, `-qq`, `-v`, `--verbose`, `-vv`, `-x`, `--exitfirst`, `-s`, `--disable-warnings`, `--collect-only`, `--co` |
| Pytest expressions | `-k EXPR`, `-m EXPR`; each expression is one separate argument, preserved without interpretation by ARC |
| Pytest failure limit | `--maxfail N`, where `N` contains decimal digits only, with a value from 1 through 4294967295 |
| Pytest traceback | `--tb STYLE`, where `STYLE` is `auto`, `long`, `short`, `line`, `native` or `no` |
| Pytest capture | `--capture MODE`, where `MODE` is `fd`, `sys`, `no` or `tee-sys` |
| Pytest color | `--color WHEN`, where `WHEN` is `yes`, `no` or `auto` |
| Unittest, with or without `discover` | `-v`, `--verbose`, `-q`, `--quiet`, `-f`, `--failfast`, `-b`, `--buffer`, `-c`, `--catch`, and the separate pair `-k PATTERN` |
| Unittest discovery only | `-s DIR` / `--start-directory DIR`, `-p GLOB` / `--pattern GLOB`, `-t DIR` / `--top-level-directory DIR` |

Long options taking values also accept `--name=value`. Short options taking values require separate arguments. Every value must be nonempty and must not begin with `-`; quote spaces and glob patterns so the shell passes each as one argument. Options may repeat and may interleave with selectors, except that explicit unittest discovery accepts only options after its leading `discover`.

Other forms make the whole classification UNKNOWN/OBSERVE and bypass the Traffic queue and TEST reservation. Examples include interpreter flags (`python -I -m pytest`), other modules, `--help`, `--version`, a runner's `--` separator, unlisted flags/clusters such as `-qv`, attached short values such as `-kfast`, malformed selectors and missing option values. Empty or non-UTF-8 tokens and inputs exceeding the detector's byte, token or work budget are also unresolved. Bypassed commands retain their original executable and arguments and follow the same native execution and preparation-error behavior as other commands.

### Shared admission

The current daemon uses these fixed class windows and reservation estimates for supported commands:

| Class | Commands | Window | CPU reservation | Memory reservation |
| --- | --- | --- | --- | --- |
| Build | `cargo build`, supported Bun build scripts, `go build`, `swift build`, `cmake --build` | 1 | 2 cores | 2 GiB |
| Test | `cargo test`, one-shot `bun test`, supported Bun test scripts and Python tests, `go test`, `swift test`, `ctest` | 1 | 1 core | 1 GiB |
| Lint | `cargo clippy`, `cargo fmt`, supported Bun lint scripts, `go vet` | 1 | 1 core | 1 GiB |
| Typecheck | supported Bun typecheck scripts | 1 | 1 core | 1 GiB |

Commands in each class share its one-slot window and the global budget, which defaults to 2 CPU cores and 4 GiB. Admission requires both a class window and enough shared budget. These are estimates for admission, not operating-system resource limits. `ARC_TRAFFIC_CPU_BUDGET_MILLIS` can configure the daemon's CPU budget at startup as described above; the class windows, reservation estimates and memory budget have no corresponding setting in this slice.

Python jobs use the existing Governor controls. `arcctl daemon pause` lets supported waiting jobs proceed without resubmitting; running children and usage measurement continue. `arcctl daemon resume` restores admission rules, so waiting work starts when those rules permit.

When the daemon confirms that a scheduled command is waiting, ARC attempts this notice once on stderr before starting the child:

```text
QUEUED
Waiting to start.
Reference: <JOB_ID>
```

While the daemon is healthy, the command waits until admission without a promised start time or test-percentage estimate. A shared Test-window wait means waiting for another test, whether that job uses Python, Cargo or Bun. If it was queued and then starts under a confirmed grant, ARC prints `STARTED` and `Command started.` after the native spawn succeeds. An immediately admitted command normally has no queue or start notice. ARC diagnostics are best effort when stderr is blocked or broken and can interleave with the child's output.

## Status and cancellation

Result inspection is optional after the single `arcctl exec -- ...` invocation. A queue notice's `Reference` is the waiting job's ID and can be passed directly to `arcctl job status <JOB_ID>`. Immediately admitted commands normally print no reference; find their ID through `arcctl job status`, which lists up to 50 recent last-known records, and copy it into `arcctl job status <JOB_ID>` for details. When recorded exit evidence is available, both views show `succeeded` for exit 0 and `failed` for a nonzero exit. Historical `COMPLETED` records use the same mapping when an exit code is recorded. Signal termination, spawn failure, cancellation and unknown outcomes remain distinct. The by-ID view also shows queued, starting or running state, queue wait and exit or signal details when recorded. An unconfirmed grant reports `outcome unknown` with its reservation retained.

If the Traffic status query is unavailable or expires, lifecycle evidence can still supply a recorded exit, signal or spawn failure. Without a known outcome, status reports `unknown` or `outcome unknown`. The recent list shares a bounded lookup budget across its records, so an unresolved exit can remain `outcome unknown`; inspect its ID for a separate lookup. If no usable status reply can be obtained, ARC reports `STATUS_UNAVAILABLE`. Status does not print command text, and timestamps are Unix milliseconds.

A successful status query means ARC retrieved the status record or list. Its exit 0 does not mean the child succeeded: a successful query can show `State: failed` and `Exit code: 17`. Read `State` and the recorded details for the child outcome. An unavailable outcome is never inferred from a process disappearing or a query succeeding.

To cancel while waiting, interrupt the invocation as usual. If cancellation is confirmed before the spawn attempt, ARC reports `NOT_STARTED` and that no child was started. If queue cleanup cannot be confirmed, it also reports `QUEUE_CLEANUP_UNCONFIRMED` when diagnostics can be delivered; this does not confirm that the queue entry or permit was removed. After the child starts, signals follow the native process behavior described above.

The new status query opts into the unconfirmed-grant state; the original Traffic status reply keeps its existing shape. When an older daemon does not support the new query, the CLI falls back to the protocol-1 lifecycle status. If a protocol-1 daemon does not support Bun Traffic admission, the new CLI uses the legacy unresolved classification and lifecycle tracking for `bun test`, without a Traffic reservation. Older protocol-1 daemons without Traffic use lifecycle tracking only. If an earlier enqueue may have committed before a daemon change or lost reply, ARC retains the recovery witness and treats tracking as uncertain rather than assuming there was no reservation.

Python admission is negotiated separately through `traffic.python-tests.v1` within protocol 1. An explicitly unsupported Python capability falls back to the legacy unresolved classification and lifecycle tracking without a TEST reservation, provided no earlier enqueue could have committed. Python-aware diagnostics negotiate `traffic.python-tests.diagnostics.v1`. Legacy lifecycle/classification readers receive `UNKNOWN` class and locality, `UNRESOLVED_COMMAND` reason and `OBSERVE` uncertainty for Python; legacy diagnostics also omit Python admission details. These reply projections do not change stored Python facts or release an existing permit. A missing admission detail is not proof that no reservation exists.

Native tools negotiate `traffic.native-tools.v1` and `traffic.native-tools.diagnostics.v1` through existing authenticated envelopes and deadlines. Explicit unsupported capability can fall back only when no earlier enqueue could have committed. Malformed replies stop preparation; uncertain acknowledgments retain the same job identity and witness and never replay the command. Older lifecycle and diagnostic surfaces project native reasons to valid UNKNOWN/OBSERVE facts and omit admission without altering stored classifications or permits. Cold and retained desktop reads with hidden admission say “Scheduling details unavailable; reservations unconfirmed”; a missing detail cannot prove bypass or release.

Normal-exit protocol-1 replies still use `COMPLETED` with the recorded outcome. Database compatibility is separate: older daemons whose decoder lacks `SUCCEEDED` and `FAILED` cannot read those newly stored terminal states.

An old nested CLI cannot read the lineage sidecar. A native-tool parent does not add a missing environment value for it, and a caller-supplied value can be unrelated to the parent's permit. In either case an old nested client may be unable to find the outer permit for reuse. A new nested CLI uses a live sidecar; if it falls back to an unrelated environment value after the wrapper exits, that value likewise does not establish permit reuse.

## Outages and recovery

Metadata retries never launch a child. A healthy queue can wait beyond the outage deadline because each bounded exchange releases its IPC and database tickets before polling again. If the daemon is absent, unreachable, stalled or disconnects during preparation, ARC continues to its single native spawn attempt. This includes requests that may have committed before their acknowledgements were lost. Fallback reports `TRACKING_UNAVAILABLE` or `TRACKING_UNCERTAIN` on stderr only after successful spawn. Neither message confirms lifecycle persistence. ARC waits for the same child and preserves its locally observed outcome. `COMPLETION_UNCONFIRMED` means the completion recording was not confirmed; when a child has exited, ARC also reports its locally observed exit code or signal. This does not authorize another invocation. Traffic fallback attempts bounded metadata updates to record the child and outcome or reconcile an uncertain queue entry.

Stopping or crashing the daemon does not stop the CLI's child. When using Traffic, the CLI writes a private witness containing the owner's PID and start token before requesting admission. Immediately before executing the target, the child adds its own PID and start token. If the witness cannot be created or updated before execution, the command fails with `SPAWN_FAILED` and the target does not run. A file lock prevents recovery from reading a partial pre-exec update. After a failed spawn or cancellation, the CLI attempts to record a no-spawn outcome in the witness before sending the completion or cancellation request. If that evidence cannot be written, it reports `SPAWN_EVIDENCE_UNAVAILABLE` when diagnostics can be delivered. The witness contains an opaque capability and process identities or a no-spawn outcome, but no command text or other environment values.

When the owner is verified dead, recovery releases a grant whose witness confirms that no child started or that its recorded child has exited. A verified live child retains the grant. If the child's liveness is unknown, status reports an unknown outcome and the grant is held for a grace period starting at the first verified dead-owner observation. The default is 120 seconds; `ARC_TRAFFIC_RECOVERY_GRACE_MS` configures a positive duration in milliseconds on the daemon. After the grace period, reconciliation releases the grant and records an audit event. Restart and later status or admission requests perform reconciliation without replaying work.

Recorded exit outcomes survive daemon restart; status uses the saved exit code when available. Recovery checks native PID, start token and boot identity for records that need reconciliation. A missing or mismatched process has an unknown outcome unless the launch owner recorded its exit. A historical completion without an exit code also remains unknown. Status never replays work or invents an exit code.

Tracking recovery is observational. A by-ID status check may update the lifecycle record with a fresh observation, but never replays the command. If ARC printed a reference, it is a lookup key, not proof that a record was saved. A missing lifecycle record does not establish whether the command ran. Daemon recovery never authorizes replay; do not launch another copy to recover tracking. A local exit code or signal describes the observed child outcome, independently of whether that outcome was persisted.

`job status` identifies truncation. `job status <JOB_ID>` refreshes the selected record's identity observation. Existing `status` and `doctor` retain their protocol-1 behavior; `classify` negotiates the endpoint-aware operation described above and preserves the older protocol-1 fallback.

Transport and SQLite admission remain bounded. Desktop integration, native Windows acceptance, broader command detection and remaining scheduling policy remain outstanding.

## Job CPU accounting

For optional result inspection, use the queue reference when available, or find a durable job ID in the recent list. Read that job's recorded outcome, scheduling facts and accounting:

```sh
arcctl job status
arcctl job status <JOB_ID>
arcctl job diagnostics <JOB_ID> --json
arcctl meter summary --job <JOB_ID> --json
```

An immediately admitted command normally prints no ID. The recent list contains at most 50 records and identifies truncation; an earlier known ID can still be queried directly. The summary is a point lookup through the authenticated daemon, with a two-second read budget. It never launches work or retries execution. `meter snapshot --json` retains the separate host snapshot contract.

`job diagnostics` reads last-recorded classification, admission and permit facts without refreshing process liveness. Its last recorded queue cause can describe an earlier wait rather than a current condition. An unsupported or unavailable diagnostics read leaves those facts unconfirmed. Optional desktop job detail presents the supported scheduling and Meter reads; refreshing it does not launch or replay a command.

`cpu_core_seconds` records cumulative user CPU plus system CPU, in CPU core-seconds. Parallel workers can consume more core-seconds than the wall duration. This is CPU used, independent of the host CPU utilization percentage. `run_elapsed.seconds` is the CLI's monotonic interval immediately around its sole native spawn attempt through terminal observation. It includes stopped time and excludes queue wait and post-terminal completion/report traffic. It is never derived by subtracting wall timestamps. Native terminal observation can lag actual exit while the supervisor is handling metadata or scheduling.

`queue_wait` is separate: scheduled jobs use durable QUEUED-to-RESERVED wall timestamps, with their clock and basis identified. Affirmatively measurement-aware lifecycle-only jobs have an established zero with basis `lifecycle_only`. Missing, reversed or legacy queue facts stay null/Unknown; an absent measurement is never converted to zero.

The existing sole waiter captures the owned child's native terminal user/system usage with PID-specific `wait4`. The `native_waited_descendants` scope includes usage propagated through explicit child waits, including short-lived reaped descendants. It excludes unrelated pipeline/process-group peers. It does not establish coverage of escaped, detached or unwaited work. Generic commands therefore retain Partial quality with `unobserved_descendants` even when they exit successfully. CPU-unavailable reports retain null values and Unknown quality; zero is a valid native reading. Exit 17 and signal termination retain their actual CPU measurements and original command status.

Reports identify the observer and child by PID, process birth token and boot identity, alongside attribution basis, scope, completeness and missing-coverage reasons. Private identity-keyed native ancestry records cover scheduled and lifecycle-only wrappers; inherited environment values alone cannot establish accounting ownership. A verified nested report is `included_in_root` with its canonical root job ID and is non-additive. Uncertain ancestry remains `unverified` and non-additive. Do not sum inclusive nested reports, add samples to a terminal total, or interpret Partial as a complete process-tree total.

`accounting.state` distinguishes a recorded report from Unknown legacy/interrupted/unavailable/not-terminated data, missing records and confirmed expiry. Missing data proves neither expiry nor nonexecution. A read failure or unsupported older daemon returns a static error without inventing a successful summary. Retrying this read cannot repair lost execution accounting by running the command again. The selected-job desktop detail consumes the same typed summary when supported.

After terminal observation the CLI freezes one 500 ms accounting-report deadline. Existing bounded lifecycle completion runs first and consumes that window; a slow completion can leave no report budget. Accounting sends only additive `meter.report` metadata, preserving protocol-1 start/complete payloads. Lost acknowledgements use the same immutable report identity and bytes, with a bounded summary read and identical retry inside the original window. Unsupported peers, outages, conflicts and exhausted budgets do not change child streams, exit status, signals or the number of executions. One accepted summary per job remains immutable and is subject to the SRS 90-day metadata retention policy; client wall time and retries cannot extend retention.

Vendor/project aggregation, live process-tree metering, Windows and full Meter acceptance remain outstanding.

## Timing and protocol errors

Initial preparation begins with a 1,800 ms absolute metadata deadline. Capability probes, initial admission, legacy REGISTER, CLAIM and retries share that deadline, including fallback after an explicitly unsupported Bun, Python or native-tools admission request or a confirmed Traffic bypass. Those fallback paths do not renew the outage budget. The engineering target allows up to 200 ms more for native handoff. Traffic queue polls receive a fresh bounded deadline after each healthy response; time spent waiting in a healthy queue is not an outage. Each transport attempt uses at most 600 ms and checks cancellation at intervals of at most 25 ms. Native tests measure the larger parent-spawn-to-child-handshake interval; host suspension is not a hard real-time guarantee.

DB_BUSY is retryable metadata uncertainty: the response can follow a commit. Exhaustion permits fallback. Other matching protocol-1 error responses reject preparation. Complete malformed, empty, oversized or mismatched replies stop with IPC_ERROR (or PROTOCOL_INCOMPATIBLE for a version mismatch), rather than becoming outage fallback. A disconnected or truncated stream is an outage and may leave tracking uncertain. Invalid local socket configuration stops preparation. Managed ATTACH, COMPLETE and Traffic fallback updates remain bounded best effort.
