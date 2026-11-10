# Root profile hygiene: historical implementation

For the cleanup's authoritative recorded verification status and scope, see the [independent QA review](qa.md), the [harness command record](../rust-coverage/tooling/acceptance/runs/harness-0049875b-a0d0-4bf2-98fc-3973be1bffeb/00-command.json) and its [source binding](../rust-coverage/tooling/acceptance/runs/harness-0049875b-a0d0-4bf2-98fc-3973be1bffeb/binding.json), the [harness gate records](../rust-coverage/tooling/acceptance/runs/), the [coverage gate records](../rust-coverage/baseline/runs/), and the [coverage pointer](../rust-coverage/baseline/latest.json). The gate records and pointer are ignored, workspace-local artifacts and may be absent from another checkout.

The operator explicitly authorized compilation for this mission, including the recorded harness and coverage runs. Consult the QA review and gate records above for their recorded outcomes and verification scope.

## Previous change

The previous cycle was based on accepted HEAD `e0875e7449c4e5d6752dfd1b6508c408ebd42ed3`. Its reports record removal of four root coverage profiles from the Git index while retaining local copies, three root-anchored ignore rules for `.profraw`, `.profdata` and `.lcov`, and checks for tracked profiles and existing ignore behavior.

The [historical receipt](gate-results.json) is authoritative for that cycle, not the cleanup's current acceptance. It records a failed coverage collection when a saturation test reported OS error 22 while setting a socket timeout; the cause was not established. It also records harness passes with coverage enabled.

Historical log SHA-256: `da5f1f6f1d7256fa52f22f28fe0b8cdd5e0d90c5b591acae77f3d0549cf612eb`.

## Cleanup verification

The hygiene checks passed at the integration snapshot. Consult the QA review and gate records linked above for the cleanup's verification records and scope.
