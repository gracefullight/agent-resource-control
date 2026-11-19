# Remote locality contract and handoff

Dispatch is REMOTE_CI; a local runner's `cargo build` is HOST_LOCAL/BUILD.

## Ownership

`crates/` paths; task 1 owns
`arc-detector/src/{lib.rs,parser.rs,types.rs,locality.rs,locality/tests.rs}`,
`arc-detector/src/scripts/resolver.rs`, `arc-ipc/src/{lib.rs,locality.rs}`,
`arc-ipc/tests/remote_locality.rs`,
`arc-cli/src/{exec.rs,exec/metadata.rs,exec/metadata/{tests.rs,clock_tests.rs}}`,
`arc-cli/tests/{fixtures/child.rs,traffic_scripts/{mod.rs,compatibility.rs,endpoint_compatibility.rs}}`,
`arc-daemon/src/{connection.rs,connection/classification.rs}`,
`arc-daemon/tests/managed_exec.rs`,
`arc-daemon/tests/managed_exec/remote_contract.rs`, manifests, lockfiles,
registrations and contract.

* Task 2: `arc-detector/{src/remote_ci.rs,tests/remote_ci.rs}`,
  `arc-cli/src/exec/ci_context.rs`, `arc-daemon/tests/managed_exec/remote_ci.rs`.
* Task 3: `arc-detector/{src/remote_endpoints.rs,tests/remote_endpoints.rs}`,
  `arc-cli/src/exec/endpoint_context.rs`, `arc-daemon/tests/managed_exec/remote_endpoints.rs`.
* Task 4: `arc-daemon/tests/managed_exec/{remote_unknown.rs,retries.rs}` and,
  if needed, `arc-cli/src/exec/{governor.rs,traffic.rs,run.rs,preparation.rs}`
  Report any other paths to Chief.

## Interfaces and safety

`classify_with_context` and `classify_argv_with_context` (also `analyze_*`)
take `EndpointContext` and a Unix-ms clock. Shell uses the existing grammar;
argv is not shell expanded. Bun endpoint leaves in a different package cwd are
UNKNOWN; facts bind to the capture cwd. Lanes implement
`recognize(words, context, budget) -> Checked<Option<Classification>>`:
`None` allows fallback; `Some(UNKNOWN)` is terminal. Charge scans to the budget.
Never execute tools or project code for classification.

Never log or store context values. `EndpointFact {kind, selector, endpoints}`
is data. Bind objects to compute endpoints through fresh, bounded, revalidated config;
unreadable, changing, conflicting, incomplete or unsupported config yields
UNKNOWN. `ci_context::extend(argv, context)` adds CI/Git facts;
`endpoint_context::capture(argv)` collects relevant environment. Capture before reads; validate at classification. For gh, `--repo` selects a repo;
`GH_REPO` can replace local-repository selection, while
`GH_HOST` is only a fallback if a host is not provided or inferred. A `CiHost`
fact with selector `gh` attests the effective host after fresh, revalidated
repository/config discovery. GH_HOST alone is insufficient; absent or
conflicting facts are UNKNOWN. The scaffold accepts three legacy gh forms with one `https://github.com` fact,
no GH_REPO and no conflicting GH_HOST. Task 2 owns the remaining grammar and discovery. Explicit options
override their environment/config when their effective target is proven. Resolve
the builder or pod compute endpoint, not merely an API server or context name.
Forwarding, aliases, mixed endpoints and missing facts are UNKNOWN/OBSERVE.
Values: at most 1,024 bytes; total: 8,192 bytes and 32
environment/fact/endpoint entries. Max age: 5,000 ms; future timestamps and
duplicate kind/selector facts are invalid. Input and work budgets apply.
Invalid context is rejected; independent local workloads need no endpoint data.
On expiry, revalidate local Bun graphs; remote/mixed remain UNKNOWN/OBSERVE.

Protocol-1 `job.classify` is unchanged. The additive `job.classify.locality.v1`
accepts `{command:{kind:"SHELL",command:"...",shell_kind:"sh"},context:...}`
or `{command:{kind:"ARGV",argv:[...]},context:...}`. Its operation is the
capability: old peers return `UNSUPPORTED_CAPABILITY`, malformed payloads
`INVALID_REQUEST`; stale/inconsistent context is UNKNOWN/OBSERVE. Reasons:
`REMOTE_CI_DISPATCH`,
`REMOTE_ENDPOINT_EXEC`, `LOCAL_ENDPOINT_BUILD`, `ENDPOINT_AMBIGUOUS`; use the
existing `GH_*` and `SSH_REMOTE_COMMAND` when accurate. New reasons project to
`UNKNOWN/UNRESOLVED_COMMAND` for protocol-1 readers and CLI registration: local
Docker admission and new-reason persistence need a negotiated job route.
Preserve both hook rewrites and vendor approval. Mixed execution stays unsplit;
AT-TRF-12 is not complete.

## SRS remote fixture denominator

**42 target fixtures**; assert shell/argv classes where representable and report
`false-local / 42`. Facts are simulated.

`GH`: CI host `gh -> https://github.com`. `GL`: CI host
`gitlab -> https://gitlab.com`. `GR`: Git `origin -> https://github.com/org/repo.git`.
`S`: effective SSH selector resolves to `ssh://192.0.2.30:22`, without proxy.
`D`: Docker context `prod` and effective builder resolve
to `tcp://192.0.2.40:2376`. `B`: Buildx builder `remote` resolves there. `K`:
context `prod` and pod `prod/default/remote-pod` resolve to `192.0.2.50`.
Facts include effective config and placement.

CI cases expect REMOTE_CI; EP cases expect REMOTE_EXEC. CI01–12 use GH; CI13–16 GR; CI19–20 GL; EP01–07 S; EP08–09 and EP13 D; EP15–19 B; EP20–22 K. Exceptions carry facts below.

CI01 `gh workflow run build.yml`  
CI02 `gh workflow run build.yml --ref main`  
CI03 `gh workflow run build.yml --repo org/repo`  
CI04 `gh workflow run build.yml -f mode=release`  
CI05 `gh workflow run 42`  
CI06 `gh run watch 123`  
CI07 `gh run watch 123 --exit-status`  
CI08 `gh run view 123`  
CI09 `gh run view 123 --json status`  
CI10 `gh run list --workflow build.yml`  
CI11 `gh workflow view build.yml`  
CI12 `gh workflow list`  
CI13 `git push origin main`  
CI14 `git push --set-upstream origin main`  
CI15 `git push --force-with-lease origin main`  
CI16 `git push origin --tags`  
CI17 `git push https://github.com/org/repo.git main` (explicit URL)  
CI18 `git push ssh://git@github.com/org/repo.git main` (verified SSH target)  
CI19 `gitlab pipeline run`  
CI20 `gitlab pipeline run --branch main`  
EP01 `ssh build01 'make test'`  
EP02 `ssh user@build01 make test`  
EP03 `ssh -- build01 cargo build`  
EP04 `ssh -p 22 build01 make`  
EP05 `ssh 192.0.2.30 make`  
EP06 `ssh build01 -- make test`  
EP07 `ssh build01 'cargo test --locked'`  
EP08 `docker --context prod build .`  
EP09 `docker --context prod build -t demo .`  
EP10 `docker build .` (`DOCKER_CONTEXT=prod`, D)  
EP11 `docker build -f Containerfile .` (`DOCKER_CONTEXT=prod`, D)  
EP12 `docker --host tcp://192.0.2.40:2376 build .` (selected builder also remote)  
EP13 `docker --context prod build --no-cache .`  
EP14 `docker buildx build .` (`BUILDX_BUILDER=remote`, B)  
EP15 `docker buildx build --builder remote .`  
EP16 `docker buildx build --builder remote -t demo .`  
EP17 `docker buildx build --builder remote --platform linux/arm64 .`  
EP18 `docker buildx build --builder remote --push .`  
EP19 `docker buildx build --builder remote --no-cache .`  
EP20 `kubectl --context prod exec remote-pod -- make`  
EP21 `kubectl --context prod exec remote-pod -- make test`  
EP22 `kubectl --context prod -n default exec remote-pod -- cargo build`  

Controls: local runner `cargo build` is BUILD. Local Docker/Buildx yield
`LOCAL_ENDPOINT_BUILD` only with established compute. Local Git, SSH loopback
or forwarding, missing alias/config, custom programs, missing pod placement,
mixed builders and invalid or stale facts are UNKNOWN/OBSERVE. Explicit remote/local options beat
contrary environment/config, or yield UNKNOWN if selection is unprovable.
`cargo test && gh workflow run build.yml` is MIXED only when both stages are
established; execution remains unsplit. `act -j build` is not declared supported.

Derive both payloads from `gh workflow run build.yml`; test built-hook
pass-through, classify via the daemon, and run an offline `gh` stub once
with a Build permit held. Check queue and reservations. The hook does not call the daemon. Run a custom shell program via `arcctl exec`; verify
UNKNOWN, once-only execution, argv/cwd/environment, streams, exit status and
fail-open. No live GitHub or vendor acceptance. Child watchdogs: at least 60s.
Runner owns workspace, coverage, desktop and harness gates.
