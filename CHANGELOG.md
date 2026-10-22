# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
### Added �?" 2026-10-21

- ci: add `.github/workflows/release.yml` to build the token, vesting, and
  multisig WASM artifacts on `v*.*.*` tags and attach them plus
  `sha256sums.txt` to a GitHub Release (#87).

- multisig: `get_confirmations(proposal_id) -> Vec<Address>` read-only view
  returning the owners who have confirmed a proposal (#104).

### Fixed — 2026-10-21

- backend/src/routes/vesting.js: `GET /api/vesting/claimable/:id` now simulates
  `get_claimable` and returns the live on-chain amount when
  `VESTING_CONTRACT_ID` and `SOURCE_SECRET_KEY` are configured. It falls back
  to the existing stub if either setting is absent; `docs/api.md` documents
  both response shapes.

- Makefile: `setup` target now installs `wasm32v1-none` instead of the
  outdated `wasm32-unknown-unknown` target (#59).

- Makefile: `build` target now uses `stellar contract build` instead of
  `cargo build --all --target wasm32-unknown-unknown --release` (#59).

- Makefile: `build-optimized` target now references
  `target/wasm32v1-none/release/` instead of
  `target/wasm32-unknown-unknown/release/` (#59).

- Makefile: `lint` target now runs
  `cargo clippy --all --target wasm32v1-none -- -D warnings` to match the
  exact CI invocation, replacing `--all-targets` which diverged from CI (#59).


### Added — 2026-10-19

- docs/api.md: REST API reference for the backend (#60). One
  section per route group (health, vesting, token, multisig) documenting
  all 29 endpoints (the 28 listed in the issue plus `GET /api/health/ready`,
  which exists in `health.js`) with method, path, request fields with types
  and required flags, success status and response shape, and error responses —
  derived directly from `backend/src/routes/*.js`. Covers the shared
  `contractId` resolution
  (body → query → env var), 200/202 success conventions, the `{ error }` /
  `{ error, code }` error shapes, rate limiting, and stub-vs-live response
  differences (including the `balance` string/number distinction). Includes
  request/response examples per group and a full endpoint index table.

- README.md: "Backend API — live data note" section now links to
  docs/api.md.

### Fixed — 2026-10-18

- backend/src/routes/vesting.js: reject invalid schedule IDs before converting
  `req.params.id` with `BigInt()`. The GET `/api/vesting/schedule/:id` route now
  returns HTTP 400 with `{ error: 'scheduleId must be a positive integer' }` for
  non-numeric or negative IDs instead of crashing with a 500. Added regression
  coverage in `backend/src/routes/vesting.test.js` for `abc` and `-1` inputs.

### Added — 2026-10-17

- backend/src/services/stellar.js: `validateStellarAddress(addr)` helper (#132).
  Wraps `StrKey.isValidEd25519PublicKey()` from `@stellar/stellar-sdk` and
  returns `true` only for valid 56-character `G...` Stellar public keys.
  Applied to every address field on the mutating routes: `POST
  /api/vesting/schedule` (`from`, `beneficiary`, `tokenAddress`), `POST
  /api/vesting/claim` (`beneficiary`), `POST /api/vesting/revoke`
  (`recipient`), `POST /api/token/mint` (`to`), `POST /api/token/transfer`
  (`from`, `to`), `POST /api/multisig/proposal` (`proposer`), and `POST
  /api/multisig/confirm` (`owner`). Invalid addresses now return HTTP 400
  with `{ error: 'invalid Stellar address: <field>' }` instead of a 500
  from the SDK. New tests in `vesting.test.js`, `token.test.js`, and
  `multisig.test.js` cover invalid address inputs.

### Tests — 2026-10-17

- contracts/vesting/src/lib.rs: `test_revoke_after_full_claim_no_beneficiary_transfer`
  (#131). Covers the previously untested zero branch of `revoke()`'s
  `if unclaimed_vested > 0` guard. Creates a 100-unit schedule, advances to
  50% vested, claims all 50, then revokes immediately. Asserts the returned
  unvested amount is 50, the beneficiary balance stays at 50 (no duplicate
  payment), the treasury receives exactly 50, and the vesting contract holds
  0. Complements the existing `test_revoke_pays_vested_to_beneficiary`
  (non-zero path) and `test_e2e_revoke_after_partial_claim`. No contract
  behaviour changed; the test verifies the CEI ordering assumption that a
  fully-claimed beneficiary is not paid twice on revoke.


### Changed — 2026-10-17

- contracts/vesting/src/lib.rs: consolidate duplicate test setup helpers into
  `mod test_helpers` (#115). Previously each of the three test modules
  (`tests`, `e2e_token_tests`, `prop_tests`) defined its own private helpers
  (`setup`, `new_token`, `deploy_token`, `deploy_vesting`, `dummy`). These are
  now unified in a single `#[cfg(test)] pub(crate) mod test_helpers` at the
  bottom of the file with clearly named functions: `deploy_vesting_with_admin`,
  `deploy_stellar_asset_token`, `deploy_real_token`, `dummy`. All three test
  modules import from `test_helpers`. No test logic changed; all 45 existing
  tests continue to pass.

### Added — 2026-10-17

- .github/dependabot.yml: Dependabot configuration for automated dependency
  updates (#84). Configures two ecosystems: `cargo` at the repo root (`/`) and
  `npm` at `/backend`, both on a weekly schedule with `open-pull-requests-limit`
  set to 5. Keeps `soroban-sdk`, `@stellar/stellar-sdk`, and all other
  dependencies up to date automatically without manual monitoring.

- contracts/vesting/src/lib.rs: `time_until_fully_vested(env, schedule_id) -> u64`
  view function (#127). Returns `(start_time + total_duration).saturating_sub(now)`
  — the number of seconds remaining until the schedule fully vests. Returns `0`
  when the schedule has already reached or passed its end time. Useful for
  client-side countdown timers and dashboards that need "time remaining" without
  off-chain arithmetic. Read-only: no TTL bump. Four new tests:
  `test_time_until_fully_vested_mid_schedule` (positive value before end),
  `test_time_until_fully_vested_at_end` (0 at exactly `total_duration`),
  `test_time_until_fully_vested_after_end` (0 past end),
  `test_time_until_fully_vested_not_found_panics` (panics for unknown ID).
  README Vesting API table updated.

### Changed — 2026-10-17

- contracts/vesting/src/lib.rs: consolidate duplicate test setup helpers into
  `mod test_helpers` (#115). Previously each of the three test modules
  (`tests`, `e2e_token_tests`, `prop_tests`) defined its own private helpers
  (`setup`, `new_token`, `deploy_token`, `deploy_vesting`, `dummy`). These are
  now unified in a single `#[cfg(test)] pub(crate) mod test_helpers` at the
  bottom of the file with clearly named functions:
  `deploy_vesting_with_admin`, `deploy_stellar_asset_token`, `deploy_real_token`,
  `dummy`. All three test modules import from `test_helpers`. No test logic
  changed; all 49 existing tests continue to pass.

- backend/src/services/stellar.js: removed unused `Keypair` import (#80).
  `Keypair` was destructured from `@stellar/stellar-sdk` on line 10 but never
  referenced anywhere in the file, causing an ESLint `no-unused-vars` warning
  in CI. Removed from the destructured import. All 70 backend tests pass;
  `npm run lint` reports zero warnings for `stellar.js`.

### Added — 2026-10-17

- contracts/vesting/src/lib.rs: `time_until_fully_vested(env, schedule_id) -> u64`
  view function (#127). Returns `(start_time + total_duration).saturating_sub(now)`
  — the number of seconds remaining until the schedule fully vests. Returns `0`
  when the schedule has already reached or passed its end time. Useful for
  client-side countdown timers and dashboards that need "time remaining" without
  off-chain arithmetic. Read-only: no TTL bump. Four new tests:
  `test_time_until_fully_vested_mid_schedule` (positive value before end),
  `test_time_until_fully_vested_at_end` (0 at exactly `total_duration`),
  `test_time_until_fully_vested_after_end` (0 past end),
  `test_time_until_fully_vested_not_found_panics` (panics for unknown ID).
  README Vesting API table updated.

- contracts/vesting/README.md: "Panic conditions" reference table (#91).
  Documents every `panic_with_error!` call in the vesting contract source with
  the function name, `VestingError` variant, numeric error code, and the
  condition that triggers it. Covers `create_schedule`,
  `batch_create_schedule`, `claim`, `revoke`, `transfer_beneficiary`,
  `pause`/`unpause`/`transfer_admin`, and the read-only getters. Based
  strictly on the source — no messages were invented. All 10 `VestingError`
  variants (`AmountMustBePositive`, `DurationMustBePositive`,
  `CliffExceedsDuration`, `StartPlusCliffOverflow`, `StartPlusDurationOverflow`,
  `ScheduleNotFound`, `ScheduleNotActive`, `NothingToClaim`,
  `ContractIsPaused`, `BatchEmpty`) are listed.

### Tests — 2026-10-16

- contracts/vesting/src/lib.rs: `test_create_schedule_future_start_time` (#63).
  Verifies that when `start_time` is set to a future ledger timestamp, nothing
  is claimable before `start_time` and the correct linear amount is claimable
  once the ledger advances past `start_time`. Asserts `get_claimable() == 0`
  at `start_time - 1` and `get_claimable() == 500` (50% of 1 000) at
  `start_time + 50` with `total_duration = 100`. Calls `claim()` and asserts
  the beneficiary balance matches. Also fixes a pre-existing duplicate test
  definition that prevented the vesting test suite from compiling.

- contracts/vesting/src/lib.rs: two new tests for `get_claimable()` on
  terminal schedule statuses (#61).
  - `test_get_claimable_on_completed_schedule`: fully vests and claims a
    schedule, asserts status is `Completed`, then asserts
    `get_claimable() == 0` (vested - claimed = 0).
  - `test_get_claimable_on_revoked_schedule`: revokes a schedule mid-vest,
    asserts status is `Revoked`, then asserts `get_claimable() == 0`
    (revoke sets `claimed_amount = vested_amount` before transferring).



  event with payload `(old_admin: Address, new_admin: Address)` (#101).
  Added `EVT_ADM_XFER` constant (`symbol_short!("admXfer")`), matching the
  vesting contract's pattern for consistency. Off-chain systems monitoring admin
  changes now receive an on-chain signal without polling `admin()`. New test:
  `test_event_transfer_admin` asserts the event is emitted with the correct
  old and new admin addresses and that `admin()` returns the new admin.
  `contracts/token/README.md` events table and `transfer_admin` doc updated.

- contracts/multisig/src/lib.rs: `is_executable(env, proposal_id: u64) -> bool`
  view function (#103). Returns `true` when the proposal's status is `Pending`
  and `confirmation_count >= threshold`. Encapsulates the two-step
  `get_proposal` + `get_threshold` check, making it easy for UIs and bots to
  poll readiness without making two RPC calls or implementing the logic
  themselves. Read-only — no TTL bump. Five new tests:
  `test_is_executable_false_before_threshold`,
  `test_is_executable_true_at_threshold`,
  `test_is_executable_false_after_execution`,
  `test_is_executable_false_after_cancel`,
  `test_is_executable_nonexistent_panics`.
  `contracts/multisig/README.md` updated with function entry.

- scripts/create-schedule-example.js: Node.js example script that sends a
  `POST /api/vesting/schedule` request to the backend API (#110). Uses the
  built-in Node 18+ `fetch` — no external npm dependencies. Reads `API_URL`
  from the environment (defaults to `http://localhost:3001`). Uses placeholder
  Stellar addresses matching `.env.example` pattern. Exits non-zero on HTTP
  error or network failure. Usage comment at the top of the file.

- .github/CODEOWNERS: GitHub CODEOWNERS file assigning `@unajiogenyi05-hub`
  as the reviewer for all files, with explicit patterns for `contracts/`,
  `.github/`, `backend/`, `frontend/`, and `scripts/` (#116). Includes an
  explanatory comment on how to add new reviewers.

### Changed — 2026-10-16

- .github/workflows/ci.yml: bumped `node-version` from `'20'` to `'24'` in the
  backend and frontend jobs (#85). GitHub Actions was already forcing Node 20
  jobs to run on Node 24 and emitting a deprecation annotation on every CI run.
  Explicitly setting `'24'` eliminates the annotation and makes the CI
  configuration match the runtime in use. The backend `package.json` engine
  constraint (`>=18.0.0`) is fully compatible with Node 24.

### Tests — 2026-10-16

- backend/src/routes/vesting.test.js: expanded test coverage for
  `GET /api/vesting/beneficiary/:addr` and `GET /api/vesting/count` (#76).
  Added 9 new tests across the two describe blocks:
  - `GET /beneficiary/:addr`: correct beneficiary address reflected in
    response, `scheduleIds` is an array, `contractId` present, different
    address values work, `contractId` query-param override honoured.
  - `GET /count`: `success: true`, `count` is a number, `contractId` present,
    `contractId` query-param override honoured.
  All 61 backend tests pass.

### Changed — 2026-10-16

- backend/src/routes/vesting.js: removed unused imports `nativeToScVal`,
  `Address`, `xdr` (from `@stellar/stellar-sdk`) and `simulateContractCall`
  (from `../services/stellar`) (#79). These were dead-code left over from an
  incomplete wiring of live contract calls and produced four ESLint
  `no-unused-vars` warnings in CI. No behaviour changes; all 54 backend tests
  still pass and `npm run lint` reports zero warnings for `vesting.js`.

### Changed — 2026-10-16

- contracts/token/src/lib.rs: added `///` doc comments to all 18 public
  functions (#95). Each comment states purpose, auth requirement, panic
  conditions, and return value. Covers `__constructor`, `mint`, `burn`,
  `transfer`, `transfer_from`, `approve`, `pause`, `unpause`,
  `transfer_admin`, `name`, `symbol`, `decimals`, `total_supply`, `balance`,
  `allowance`, `is_paused`, `admin`, and `extend_ttl`. No behaviour changes;
  all existing tests continue to pass.

### Added — 2026-10-16

- contracts/vesting/src/lib.rs: `get_unvested_amount(env, schedule_id) -> i128`
  view function (#99). Returns `total_amount - vested_amount(now)`, clamped to
  `0`. Replaces client-side computation for treasury dashboards that need to
  display how much remains unvested. Read-only (no TTL bump). Four new tests:
  `test_get_unvested_amount_mid_schedule_and_at_end`,
  `test_get_unvested_amount_before_cliff`, and a conservation invariant test
  `test_unvested_plus_claimable_plus_claimed_equals_total`. README updated.

### Added — 2026-10-16 (earlier)

- CODE_OF_CONDUCT.md: Contributor Covenant 2.1 added at repository root (#117).
  Enforcement contact points to the GitHub private security reporting URL.
  CONTRIBUTING.md updated with a "Code of Conduct" section linking to it.

- docs/testnet-demo.md: "Multisig governance demo" section (#90). Covers all
  six steps: deploy multisig with `--owners` and `--threshold 1`, transfer
  vesting admin via `transfer_admin`, submit a `ProposalAction::Call` proposal
  for `create_schedule`, confirm (threshold 1), execute, and verify the schedule
  was created. CLI commands match the real function signatures in
  `contracts/multisig/src/lib.rs`.

- contracts/multisig/src/lib.rs: `test_cancel_executed_proposal_panics` (#68).
  Submits a proposal, confirms to threshold, executes it, then asserts that
  calling `cancel()` on the now-Executed proposal panics with
  `"proposal not pending"`. Mirrors the existing `test_execute_twice_panics`
  pattern for the cancel path.

- backend/src/routes/health.js: `GET /api/health/ready` endpoint (#119).
  Checks `VESTING_CONTRACT_ID`, `TOKEN_CONTRACT_ID`, and
  `MULTISIG_CONTRACT_ID` environment variables. Returns 200 with
  `{ ready: true, contracts: { vesting, token, multisig } }` when at least
  one contract ID is configured; returns 503 with `ready: false` when none
  are set. Four new tests in `health.test.js` cover the 503 (none set), 200
  (one set), 200 (all set), and per-contract boolean shape cases.

### Added — 2026-10-16 (earlier)

- contracts/vesting/README.md: "Rounding behavior" section (#96). Documents
  that `vested_amount` uses integer (floor) division
  (`elapsed * total_amount / total_duration`), explains dust accumulation until
  schedule completion, includes a worked example (99-token / 100-second
  schedule), and adds integrator notes. Linked from the `vested_amount` source
  comment.

- contracts/vesting/src/lib.rs: inline `// Note:` comment in `vested_amount()`
  (#96). Points to the README "Rounding behavior" section and explains that the
  fractional remainder (dust) is released only when `elapsed >= total_duration`.

### Added — 2026-10-15

- contracts/multisig: `confirm()` now includes `confirmation_count` in its
  event payload (#129).  The `("confirmed",)` event data tuple changes from
  `(proposal_id, owner)` to `(proposal_id, owner, confirmation_count)`.
  Off-chain monitors can now track confirmation progress from the event stream
  alone without making a separate `get_proposal()` call.  New test:
  `test_confirmed_event_includes_count` asserts count = 1 after the first
  confirmation and count = 2 after the second.  Updated
  `contracts/multisig/README.md` events table and `confirm()` function docs.

- contracts/vesting: `get_claimed_by_beneficiary(env, beneficiary) -> i128`
  view function (#128).  Sums `claimed_amount` across all schedules for the
  given beneficiary in one call, useful for dashboards.  Returns 0 for
  addresses with no schedules.  Uses `saturating_add` to avoid overflow.
  Read-only; does not extend any TTL.  Three new tests:
  `test_total_claimed_no_schedules`, `test_total_claimed_single_schedule`,
  `test_total_claimed_multiple_schedules`.  Updated `contracts/vesting/README.md`
  with function reference.

- contracts/vesting: `transfer_beneficiary(env, schedule_id, new_beneficiary)`
  function (#123).  Lets the current beneficiary atomically reassign a vesting
  schedule to a new address, preserving all vesting parameters and
  `claimed_amount`.  Requires: current beneficiary auth, schedule `Active`,
  contract not paused.  Updates both `BeneficiarySchedules` index entries
  (removes from old, appends to new) and emits `("benXfer",)` →
  `(schedule_id, old_beneficiary, new_beneficiary)`.  Five new tests:
  `test_transfer_beneficiary_success` (new beneficiary claims, old gets
  nothing), `test_transfer_beneficiary_preserves_claimed_amount` (history
  preserved), `test_transfer_beneficiary_revoked_panics`,
  `test_transfer_beneficiary_auth_failure`,
  `test_transfer_beneficiary_index_consistency_multiple` (index correct after
  mid-list transfer).  Updated `contracts/vesting/README.md` functions, events
  table, and persistent storage TTL table.

- backend/src/routes/token.js: `GET /api/token/balance/:addr` now calls
  `simulateContractCall` with method `balance` when `TOKEN_CONTRACT_ID` and
  `SOURCE_SECRET_KEY` are set (#130).  Encodes the address argument via
  `nativeToScVal(addr, { type: 'address' })`, decodes the `i128` result via
  `scValToNative`, and returns `balance` as a string.  Validates the address
  with the `G[A-Z2-7]{55}` regex and returns 400 for bad inputs.  Falls back
  to the existing stub (balance: 0, note field) when either env var is absent.
  Updated `backend/src/routes/token.test.js` with five new assertions: stub
  when `SOURCE_SECRET_KEY` absent, stub has `note` field, live call returns
  parsed balance as string, invalid address returns 400, RPC error propagates
  to 500.

- contracts/vesting: `("benXfer",)` event constant `EVT_BEN_XFER` added.

### Added — 2026-10-15

- contracts/vesting: `batch_create_schedule(env, params: Vec<CreateScheduleParams>) -> Vec<u64>`
  (#122). Creates multiple vesting schedules atomically in a single transaction.
  Requires admin auth; contract must not be paused; `params` must be non-empty
  (`VestingError::BatchEmpty` otherwise). Delegates to the existing
  `create_schedule_internal` helper so per-schedule validation (amount > 0,
  duration > 0, cliff ≤ duration, overflow guards) applies to every entry.
  Because the function runs inside a single Soroban transaction, any validation
  failure rolls back all state changes — no partial batches are possible.
  Returns a `Vec<u64>` of newly created schedule IDs in input order.
  New tests in `contracts/vesting/src/lib.rs`:
  `test_batch_create_schedule_three` (3-schedule batch, ID order, token
  conservation, beneficiary schedule lists), `test_batch_token_conservation`
  (deposits, full-vest claim, conservation), `test_batch_empty_panics`,
  `test_batch_all_or_nothing_invalid_amount`, `test_batch_blocked_when_paused`.

- contracts/vesting: `VestingError` typed error enum (#124).
  `#[contracterror]` enum replaces all bare `panic!()` strings in the vesting
  contract. Variants: `AmountMustBePositive = 1`, `DurationMustBePositive = 2`,
  `CliffExceedsDuration = 3`, `StartPlusCliffOverflow = 4`,
  `StartPlusDurationOverflow = 5`, `ScheduleNotFound = 6`,
  `ScheduleNotActive = 7`, `NothingToClaim = 8`, `ContractIsPaused = 9`,
  `BatchEmpty = 10`. All `panic!()` calls replaced with `panic_with_error!`.
  Callers receive a stable `u32` error code instead of matching on string
  messages. All existing `#[should_panic]` tests updated to match; all pass.

- docs/security-model.md (#120). Per-function authorization model and threat
  analysis covering all three contracts. Six sections: (1) trust hierarchy
  (admin, multisig owners, beneficiary, anyone); (2) per-function auth table
  for vesting, token, and multisig contracts; (3) pause semantics with
  rationale for each gated/ungated function; (4) CEI compliance summary for
  `claim`, `revoke`, and multisig `execute`; (5) known limitations (no upgrade
  mechanism, floor-division rounding, no audit, no mainnet deployment,
  unbounded beneficiary list); (6) attack surface summary cross-referenced with
  `SECURITY.md`. Written strictly from source code and existing documentation.

- backend/src/error-handler.test.js (#114): Jest tests asserting that all error
  responses have the standardised `{ error: string, code: string }` envelope on
  both 400 (route-level validation) and 500 (unhandled error) paths.

### Changed — 2026-10-15

- backend/src/index.js (#114): centralised error handler middleware now always
  returns `{ error: string, code: string }`. `code` is taken from `err.code`
  if set, otherwise defaults to `'INTERNAL_ERROR'` for 500 responses or
  `'REQUEST_ERROR'` for other status codes. The raw stack trace continues to be
  logged server-side only and is never sent to the client.

- README.md: "Security" section now links to the new
  `docs/security-model.md` document (#120).

### Added — 2026-10-07

- Testnet deployment: token contract `CBFVQMSOIMRWLZGKY4MMBASEJLKBLGBIYWT76MH47M5B2ODRDT264KKH` and
  vesting contract `CDBSJEAWCMVWQQDVPY6I7Y4TAHWXFLGE2MEYG2UIOC2A6SQMJ5V3NMJF` deployed to Stellar Testnet
  using the `stellar contract deploy` CLI with constructor arguments passed at
  deploy time. The multisig contract is not deployed.

- deployments.json: records the Testnet contract IDs
  (`token_testnet`, `vesting_testnet`) in the format `scripts/deploy.sh`
  writes.

- README: "Deploy to Testnet" section with Codespaces-friendly setup (Rust,
  `wasm32v1-none` target, stellar-cli), build, account funding, token and
  vesting deploy commands, optional multisig deploy, and a table of the
  current Testnet contract IDs with Stellar Expert links. Added a "Deployed on
  Testnet" contract ID table near the top of the README.

### Changed — 2026-10-07

- README: description changed from "testnet-ready" to "deployed on Testnet".
  Prerequisites now install stellar-cli from the GitHub install script
  (`install.stellar.org` did not resolve from the Codespace). Quick start now
  uses `stellar contract build` and `cargo test --all`, and no longer lists
  `make deploy-testnet`. "Status and limitations" Testnet row updated to
  reflect the deployed token and vesting contracts. Backend section links to
  the deployment IDs.

- README: added a note that `make deploy-testnet` and `scripts/deploy.sh` are
  out of date (they build for `wasm32-unknown-unknown` and deploy without
  constructor arguments); the scripts themselves are unchanged.

### Fixed — 2026-10-06

- CHANGELOG.md: added missing PR #37 reference to the five e2e vesting tests
  entry; removed one meta-doc bullet that described an earlier documentation
  correction (PR #36 entry in Fixed — 2026-10-04).

### Added — 2026-10-06

- contracts/vesting: proptest 1.11.0 as dev-dependency; three property-based
  tests in a new `prop_tests` module (F5):
  `prop_vested_never_decreases_and_never_exceeds_total` (P1 + P2),
  `prop_zero_before_cliff` (P3), `prop_equals_total_at_end` (P4).
  Tests call `VestingContract::vested_amount` directly (pure math, no Env
  required). Vesting test count: 42 → 45.

- .gitignore: added `**/test_snapshots/` pattern; removed 118 tracked snapshot
  files with `git rm -r --cached`. The `test_snapshots/` directories contain
  JSON files auto-generated by the soroban-sdk test harness on every `cargo
  test` run (auth traces and ledger state); they are not hand-authored and
  regenerate deterministically, so tracking them adds churn without benefit (F4).

- contracts/vesting/README.md, contracts/token/README.md,
  contracts/multisig/README.md: function reference (name, arguments,
  authorization required, panic messages), storage keys with TTL behaviour,
  and events. Written from source only (F3).

- contracts/multisig, vesting, token: remove `bump_instance()` from pure
  getters; add permissionless `extend_ttl()` (E8, PR #46, merged 2026-10-06).
  Pure read-only functions (`get_*`, `is_*`, `name`, `symbol`, `decimals`,
  `total_supply`, `balance`, `allowance`, `admin`, `proposal_count`,
  `has_confirmed`, `is_owner`, `schedule_count`) no longer carry a write
  footprint, so the CLI submits them as simulations rather than transactions.
  Each contract gains a permissionless `extend_ttl(env)` entry point for
  off-chain keep-alive bots.  Existing TTL tests unchanged; new tests:
  `test_extend_ttl` in multisig, token, and vesting (one per contract).
  Updated docs/ttl.md with "Read-only getters and extend_ttl()" section;
  updated README function tables.
  Counts: multisig 36, token 33, vesting 42 (111 total).

- README: "Pause semantics" section (E5, PR #45, merged 2026-10-06). Documents which functions are
  pause-gated in vesting and token, and why `revoke()` and `transfer_admin()`
  are intentionally not pause-gated in vesting (admin must be able to recover
  funds and hand off control even during a freeze). Token `transfer_admin` is
  also not pause-gated for the same reason. Fixed stale TTL constant values
  in the README "Storage TTL constants" table (3 110 400 / ~180 days).

- contracts/vesting, token: overflow guards (E4, PR #44, merged 2026-10-06).
  `create_schedule` now rejects schedules where `start_time + cliff_duration`
  or `start_time + total_duration` would overflow `u64` using `checked_add`
  with a clear panic message; without this guard either call would trap inside
  `vested_amount()` on every `claim()` / `revoke()`, permanently locking funds.
  `token::_mint` uses `checked_add` for both per-account balance and total
  supply instead of wrapping arithmetic (`+`).
  New tests: `test_create_schedule_start_plus_duration_overflow`,
  `test_create_schedule_start_plus_cliff_overflow` (vesting);
  `test_mint_balance_overflow`, `test_mint_total_supply_overflow` (token).
  Counts: multisig 35, token 32, vesting 41 (108 total).

- contracts/multisig, vesting, token: lower `INSTANCE/PERSISTENT_BUMP_LEDGERS`
  from 6 307 200 to 3 110 400 (E2-CHECK, PR #43, merged 2026-10-06). The previous value exceeded the
  network's `max_entry_ttl` (3 110 400 ledgers ≈ 180 days, protocol 29,
  source: stellar-core soroban-settings). Values above `max_entry_ttl` are
  silently clamped by the host, so using the exact ceiling is both correct and
  maximally protective. Threshold (518 400, ~30 days) is unchanged.
  Updated docs/ttl.md with explanation and source reference.
  All existing TTL tests continue to pass; no bump call was missing.

- contracts/vesting: reorder `revoke()` to checks-effects-interactions (E3, PR #42, merged 2026-10-06).
  State mutation (`claimed_amount`, `status = Revoked`, persistent write + TTL
  bump) now happens before both token transfers, matching the pattern already
  used in `claim()`. New test: `test_revoke_cei_state_and_balances` verifies
  final status, `claimed_amount`, beneficiary balance, and treasury balance
  with exact values. Count: vesting 39.

### Added — 2026-10-05

- contracts/multisig, vesting, token: replace `initialize()` with
  `__constructor` (soroban-sdk 27, E1, PR #40, merged 2026-10-05). Constructor args are supplied once at
  deploy time; the initialization front-running window is eliminated. New
  multisig tests: `test_constructor_empty_owners_panics`,
  `test_constructor_zero_threshold_panics`,
  `test_constructor_threshold_exceeds_owners_panics`,
  `test_owner_list_after_add_remove_cycle`. Vesting: replaced
  `test_double_initialize_panics` with `test_constructor_sets_state`. Counts:
  multisig 32, token 27, vesting 35 (94 total). Scripts, docs and README
  updated to single-step deploy with constructor args.

- contracts/multisig, vesting, token: storage TTL management (E2, PR #41, merged 2026-10-05).
  Every public entry point calls `bump_instance()` so instance storage
  (OWNERS, THRESHOLD, PROP\_COUNT, ADMIN, PAUSED, SCHED\_ID, NAME, SYMBOL,
  DECIMALS, TOTAL) never expires while the contract is in active use.
  Persistent entries are bumped on every write: Balance and Allowance (token);
  Proposal and Confirm (multisig); Schedule and BeneficiarySchedules (vesting,
  already bumped, now uses shared public constants).
  Constants: `INSTANCE_BUMP_LEDGERS = 6_307_200` (~1 year),
  `INSTANCE_BUMP_THRESHOLD = 518_400` (~30 days), same for persistent.
  New tests: `test_ttl_instance_bumped_on_submit`,
  `test_ttl_proposal_bumped_on_confirm`, `test_ttl_confirm_entry_bumped`
  (multisig); `test_ttl_instance_bumped_on_mint`,
  `test_ttl_balance_bumped_on_mint`, `test_ttl_allowance_bumped_on_approve`
  (token); `test_ttl_instance_bumped_on_create_schedule`,
  `test_ttl_schedule_bumped_on_create`, `test_ttl_instance_bumped_on_claim`
  (vesting). Counts: multisig 35, token 30, vesting 38 (103 total).
  Added docs/ttl.md and README "Storage TTL constants" section.

### Added — 2026-10-04

- contracts/multisig/src/lib.rs: 15 new tests — 19 total (PR #30):
  `test_update_threshold`, `test_update_threshold_zero_panics`,
  `test_update_threshold_exceeds_owners_panics`, `test_add_owner`,
  `test_add_owner_duplicate_panics`, `test_remove_owner`,
  `test_remove_owner_below_threshold_panics`, `test_double_confirm_panics`,
  `test_non_owner_submit_panics`, `test_non_owner_confirm_panics`,
  `test_execute_by_non_owner_succeeds_when_threshold_met`,
  `test_execute_twice_panics`, `test_cancel_by_proposer`,
  `test_cancel_by_non_proposer_panics`,
  `test_revoke_drops_below_threshold_then_reconfirm_executes`.
- contracts/token/src/lib.rs: 22 new tests — 27 total (PR #32):
  `test_event_mint`, `test_event_burn`, `test_event_transfer`,
  `test_event_transfer_from`, `test_event_approve`, `test_event_pause`,
  `test_event_unpause`, `test_allowance_exhaustion`,
  `test_transfer_from_over_allowance`, `test_approve_zero_resets_allowance`,
  `test_unpause_restores_transfer`, `test_approve_while_paused`,
  `test_transfer_from_while_paused`, `test_burn_while_paused`,
  `test_mint_while_paused`, `test_burn_more_than_balance`,
  `test_mint_zero_amount`, `test_mint_negative_amount`,
  `test_transfer_zero_amount`, `test_approve_negative_amount`,
  `test_burn_zero_amount`, `test_unauthorized_mint`.
- contracts/multisig/src/lib.rs: `execute()` dispatches cross-contract calls
  via `env.invoke_contract`; 5 new tests (PR #33, PR #34):
  `test_execute_real_cross_contract_call`, `test_execute_failing_target_reverts`,
  `test_execute_below_threshold_panics`, `test_execute_twice_panics_t3`,
  `test_execute_proposal_stored_fields`.
- contracts/multisig/src/lib.rs: `ProposalAction` enum (`Call(CallData)`,
  `AddOwner(Address)`, `RemoveOwner(Address)`, `UpdateThreshold(u32)`)
  replacing flat `(target, function, args)` fields on `ProposalData` (PR #35).
  Owner management variants are handled directly inside `execute()` — no public
  `add_owner`, `remove_owner`, or `update_threshold` entry points exist.
  `RemoveOwner` clears the removed owner's confirmations on pending proposals
  and decrements `confirmation_count`. 28 tests total; new tests (PR #35):
  `test_add_owner_requires_threshold`, `test_owner_management_only_via_proposal`,
  `test_remove_owner_only_via_proposal`, `test_update_threshold_only_via_proposal`,
  `test_self_call_reentrance_fails`, `test_remove_owner_clears_confirmations`.
- contracts/vesting/src/lib.rs: multisig-as-admin end-to-end tests (PR #34, PR #35):
  `test_multisig_admin_flow` — multisig (2-of-3) calls `vesting.pause()` and the
  contract is paused; `test_multisig_below_threshold_panics` — below threshold
  panics; `test_multisig_create_schedule` — multisig calls `create_schedule` with
  a real token, asserts funder balance goes to zero, vesting contract receives
  tokens, beneficiary claims the full amount after duration.
- contracts/vesting/src/lib.rs: manual parameterised property tests (PR #33):
  `test_prop_vested_never_decreases`, `test_prop_zero_before_cliff`,
  `test_prop_equals_total_at_end`, `test_no_arithmetic_overflow_large_amount`.
- contracts/vesting/src/lib.rs: event-emission tests (PR #33):
  `test_event_created`, `test_event_claimed`, `test_event_revoked`,
  `test_event_pause_unpause`, `test_event_transfer_admin`.
- backend/src/routes/*.test.js: Jest tests for vesting, token, multisig, and
  health routes with Stellar service mocked (PR #31).
- SECURITY.md: threat model and signing model documentation (PR #31).
- .github/workflows/ci.yml: html-validate and eslint steps for frontend (PR #31).
- contracts/vesting/src/lib.rs: 5 end-to-end tests using the real token
  contract (PR #37, PR #38, merged 2026-10-04):
  `test_e2e_create_and_claim_partial_then_full`,
  `test_e2e_cliff_gates_claim`,
  `test_e2e_revoke_splits_correctly`,
  `test_e2e_revoke_after_partial_claim`,
  `test_e2e_total_supply_conservation`.
  Each test deploys a real `TokenContract`, mints to a funder, calls
  `create_schedule`, advances ledger time, and asserts exact balances and
  total-supply conservation.

### Changed — 2026-10-04

- README.md: architecture diagram, contract API tables, "Multisig as vesting
  admin" section, "Status and limitations" table updated to reflect actual
  behaviour (PRs #31, #35).
- docs/architecture.md: contract interactions section updated; multisig execute
  described accurately (PR #35).
- CONTRIBUTING.md, SECURITY.md: rewritten with repo-specific commands and
  threat model (PR #31).
- backend/package.json: removed `--passWithNoTests` flag (PR #31).

### Fixed — 2026-10-04

- contracts/multisig/src/lib.rs: critical — `add_owner`, `remove_owner`, and
  `update_threshold` had no `require_auth` and no caller check, allowing any
  account to take over the multisig and any contract it administers (PR #35).
- .github/workflows/ci.yml: YAML parse error (inline JSON in `--rule` flag);
  `|| true` removed from lint step; ESLint config extracted to
  `frontend/.eslintrc.json` (PR #31).
- frontend/index.html: 37 html-validate errors fixed — void-element
  self-closing slashes, redundant ARIA landmark roles, missing `type=` on
  buttons, missing submit button on `confirmForm` (PR #31).
- contracts/multisig/src/lib.rs: test name `test_outsider_cannot_add_owner`
  renamed to `test_add_owner_requires_threshold`; contradictory comment fixed
  (PR #36).

### Removed — 2026-10-04

- Cargo.lock removed from `.gitignore` — lockfile is now tracked by git (PR #31).

### Added — 2026-09-20

- contracts/vesting/src/lib.rs: 29 tests covering create/claim (partial and
  full), zero cliff, cliff gate, revoke (unvested returned, vested paid to
  beneficiary), claim-after-revoke panic, claim-before-cliff panic, pause
  blocks create/claim, unpause restores, transfer_admin, schedule_count,
  multiple beneficiaries, beneficiary_schedules, double-revoke panic,
  double-initialize panic, is_paused state, events (PR #29).
- scripts/demo-testnet.sh: keygen, Friendbot funding, build, deploy token +
  vesting, mint, create schedule, claim (PR #28).
- docs/testnet-demo.md: step-by-step walkthrough with 1-year cliff / 4-year
  DAO example (PR #28).

### Changed — 2026-09-20

- README.md: architecture diagram, "What this platform does" DAO example,
  contract API tables, schedule lifecycle diagram, repository layout,
  prerequisites, link to testnet demo docs (PR #27).

## [0.1.0] - 2026-09-02

### Added

#### Smart Contracts
- `contracts/vesting/` — Linear token vesting contract with cliff support, `create_schedule`, `claim`, `revoke`, `pause`/`unpause`, `transfer_admin`, and full unit test suite
- `contracts/token/` — SEP-0041 compatible fungible token with `mint`, `burn`, `transfer`, `approve`, `transfer_from`, and `pause`/`unpause`
- `contracts/multisig/` — N-of-M multisig governance contract with `submit`, `confirm`, `revoke_confirmation`, `execute`, `add_owner`, `remove_owner`, `update_threshold`
- Soroban SDK `v27.0.6` across all contracts
- Workspace `Cargo.toml` with shared `soroban-sdk` dependency and optimized release profile

#### Backend
- `backend/src/index.js` — Express.js REST API server with Helmet security headers, CORS, rate limiting, and Winston structured logging
- `backend/src/routes/vesting.js` — Full vesting API: `POST /schedule`, `GET /schedule/:id`, `GET /claimable/:id`, `POST /claim`, `POST /revoke`, `POST /pause`, `POST /unpause`, `GET /beneficiary/:addr`, `GET /count`
- `backend/src/routes/token.js` — Token API routes
- `backend/src/routes/multisig.js` — Multisig API routes
- `backend/src/routes/health.js` — Health check endpoint
- `backend/src/services/stellar.js` — Stellar SDK integration service for contract simulation
- `backend/src/logger.js` — Winston logger configuration
- `backend/.eslintrc.json` — ESLint configuration

#### Frontend
- `frontend/index.html` — Single-page vesting dashboard UI
- `frontend/js/app.js` — Frontend JavaScript for schedule management
- `frontend/css/styles.css` — Dashboard styles

#### Infrastructure
- `.github/workflows/ci.yml` — Three-job CI pipeline: `contracts` (fmt, clippy, test, WASM build), `backend` (lint, test), `frontend` (file validation)
- `scripts/deploy.sh`, `scripts/fund.sh`, `scripts/invoke.sh` — Deployment helper scripts
- `docs/architecture.md` — Platform architecture documentation
- `.env.example` — Environment variable template

[Unreleased]: https://github.com/unajiogenyi05-hub/soroban-vesting-platform/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/unajiogenyi05-hub/soroban-vesting-platform/releases/tag/v0.1.0
