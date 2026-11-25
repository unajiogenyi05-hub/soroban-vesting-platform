# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]
### Added
- README.md: added a `## Glossary` section defining cliff period, linear
  vesting, vesting schedule statuses (`Active`, `Completed`, `Revoked`,
  `Cancelled`), multisig threshold (N-of-M), unvested / vested / claimable
  balances, Soroban storage TTL, and the CEI pattern (#255).
### Changed
- contracts/multisig/src/lib.rs: added `///` API documentation to every public
  function, including parameters, authorization requirements, panics, and
  return values. The `execute()` documentation now explicitly describes its
  permissionless semantics and threshold-only authorization (#258).
### Fixed
- backend/src/routes/vesting.test.js: fixed test suite crash caused by the
  ESM-only `uint8array-extras` transitive dependency of `@stellar/stellar-sdk`
  breaking Jest's CommonJS transform (#108). Added a full `jest.mock` for
  `@stellar/stellar-sdk` (matching the token test pattern) so the SDK is never
  loaded by Jest directly. Replaced the orphaned `mockSimulate` jest.fn() with
  the actual `simulateContractCall` mock obtained from `jest.mock('../services/stellar')`
  in all live-call assertions — the old `mockSimulate` was never connected to
  the route and those tests were silently vacuous. All 42 vesting tests now pass.
### Tests
- backend/src/routes/token.test.js: added "returns 400 when amount is zero or
  negative" test case to `describe('POST /api/token/burn')` (#213). Also
  added the matching validation guard (`Number(amount) <= 0` → 400) to the
  `/burn` route in `backend/src/routes/token.js`, since `!amount` only catches
  falsy values and would have allowed a negative amount through.
### Tests
- contracts/multisig/src/lib.rs: `test_confirm_on_cancelled_proposal_panics` (#228).
  Verifies that `confirm()` panics with `"proposal not pending"` when called on a
  Cancelled proposal. The guard inside `confirm()` covers both Executed and
  Cancelled statuses; the Executed path was already covered by
  `test_revoke_confirmation_after_execute_panics`; this test closes the gap for
  the Cancelled path.
### Added
- contracts/vesting/src/lib.rs: `get_active_schedule_count(env) -> u64` (#222).
  Returns the count of schedules with `status == Active` by scanning IDs
  `1..=schedule_count()`. Documented as O(n); no write footprint (no TTL bump).
  Closes the gap where `schedule_count()` returned the total ever created
  (including revoked/completed), forcing dashboards to filter client-side.
### Tests
- contracts/vesting/src/lib.rs: `test_active_schedule_count_empty` and
  `test_active_schedule_count_mixed_statuses` (#222). Cover: zero schedules,
  correct count after three creates, decrement after a revoke, and decrement
  after a completed full claim. Also asserts that `schedule_count()` (total
  ever created) is unaffected by status changes.
### Added
- scripts/deploy-multisig.sh: standalone script to deploy the multisig
  contract to any Stellar network with configurable `OWNERS` (JSON array),
  `THRESHOLD`, `SOURCE_ACCOUNT`, `NETWORK`, and `SKIP_BUILD` environment
  variables (#264). Saves the deployed contract ID to `deployments.json`
  when `jq` is available and prints next-step commands for transferring the
  vesting admin role. Linked from docs/testnet-demo.md "Multisig governance
  demo" step 1.
### Tests
- contracts/multisig/src/lib.rs: `test_revoke_confirmation_after_execute_panics`
  (#227). Verifies that `revoke_confirmation()` panics with "proposal not
  pending" when called on an already-executed proposal. Exercises the
  executed-proposal branch of the `proposal.status != ProposalStatus::Pending`
  guard that was previously covered only by the happy-path
  `test_revoke_and_reconfirm`.
### Tests
- backend/src/routes/token.test.js: `describe('POST /api/token/burn')` block
  with three test cases (#236): 202 with valid `from` and `amount`, 400 when
  `from` is missing, and 400 when `amount` is missing. Exercises the
  `if (!from || !amount)` validation guard in the `/burn` route handler that
  had no negative-path coverage.
### Tests
- contracts/token/src/lib.rs: `test_transfer_admin_emits_exactly_admxfer_event`
  (#224). Asserts that `transfer_admin()` emits exactly one event using
  `env.events().all().len() == 1`, catching any future change that accidentally
  adds or removes an event emission. Complements the existing
  `test_event_transfer_admin` which checks the full event payload.
  Also updates the `transfer_admin` doc comment to note the intentional event
  and explain its purpose for off-chain indexers.
### Tests
- contracts/vesting/src/lib.rs: `test_transfer_admin_old_admin_loses_create_schedule` —
  verifies that after `transfer_admin()`, the old admin's auth no longer satisfies
  `require_admin()` and a `create_schedule` call with only the old admin's auth
  panics. Closes the gap left by the existing `test_transfer_admin` which only
  checked `get_admin()` (#245).

### Added — 2026-11-19

- docs/events.md: reference for every event the token, vesting and multisig
  contracts publish, with topic tuple, typed data payload and emitting
  function (#248). It includes `benXfer`, notes that `token::transfer_admin`
  emits `admXfer`, explains the 9-character `symbol_short!` limit and shows a
  `getEvents` RPC filter. Linked from the README and each contract README.
- docs/testnet-demo.md: "Common pitfalls" section after the manual
  walkthrough (#290). It covers unfunded accounts, the `wasm32v1-none` target,
  the `--` argument separator, identity and network mismatches, schedule
  parameters and timestamps, and archived storage (linking `docs/ttl.md`). Each
  entry quotes the real error text captured on testnet with stellar-cli 28.0.0
  and gives the fix command.

- contracts/vesting/src/lib.rs: `batch_revoke(env, schedule_ids, recipient)
  -> Vec<i128>` revokes multiple schedules for a beneficiary in a single
  transaction with a single admin auth, reusing the internal revoke logic
  (#285).

### Fixed — 2026-11-19

- contracts/vesting/README.md: the Events table now lists `benXfer`, emitted
  by `transfer_beneficiary` (#248).
- docs/testnet-demo.md: the step 6 `create_schedule` example now passes
  `total_amount` as a JSON string. stellar-cli 28 rejects a bare number for
  `i128` with "invalid type: number, expected string or map" (#290).

### Tests — 2026-11-19

- contracts/vesting/src/lib.rs: tests for `get_schedules_paginated()` — first
  page returns the correct schedules, offset skips the expected number of
  schedules, offset beyond total is empty, and an oversized limit returns all
  schedules (#288).

### Added �?" 2026-10-21

- multisig: `get_pending_proposals(env) -> Vec<u64>` read-only view
  returning the IDs of all proposals whose status is `ProposalStatus::Pending`
  (#105). Scans IDs `1..=proposal_count()` and collects matching proposals,
  so governance UIs and monitoring bots can list pending proposals in a single
  RPC call instead of iterating `get_proposal()` per ID. Note: O(n) scan over
  all proposals; excludes Executed and Cancelled proposals; no TTL bump.

- ci: add `.github/workflows/release.yml` to build the token, vesting, and
  multisig WASM artifacts on `v*.*.*` tags and attach them plus
  `sha256sums.txt` to a GitHub Release (#87).

- multisig: `get_confirmations(proposal_id) -> Vec<Address>` read-only view
  returning the owners who have confirmed a proposal (#104).

### Tests — 2026-10-21

- contracts/multisig/src/lib.rs: tests for `get_pending_proposals()` (#105):
  empty list with no proposals, correct list after submits, and exclusion of
  Executed and Cancelled proposals.

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

- contracts/multisig/README.md: added `get_pending_proposals()` to the
  function reference table, noting the O(n) scan cost and that it is
  read-only (no TTL bump).


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

### Added

- scripts/examples/query-claimable.js: Node.js example script that queries
  `get_claimable(schedule_id)` on the vesting contract directly via
  `@stellar/stellar-sdk` v12.3.0 without the backend REST API (#265).
  Builds an unsigned transaction and calls `server.simulateTransaction()` for a
  read-only preflight — no signing or XLM fees required.  Decodes the `i128`
  return value with `scValToNative` and prints it alongside the CPU/memory cost
  breakdown.  Reads `VESTING_CONTRACT_ID`, `SCHEDULE_ID`, `SOURCE_PUBLIC_KEY`,
  `STELLAR_RPC_URL`, and `STELLAR_NETWORK` from the environment with safe
  testnet defaults.  Usage instructions in the file header and in
  `docs/testnet-demo.md`.

### Added

- scripts/query-schedule.sh: CLI helper that reads a vesting schedule from
  testnet by ID using `stellar contract invoke` and pretty-prints the result
  (#263).  Calls `get_schedule(schedule_id)` as a read-only simulation — no
  signing, secret key, or XLM fees required.  Accepts the contract ID and
  schedule ID as positional arguments (or via `VESTING_CONTRACT_ID` env var).
  Respects `STELLAR_NETWORK` and sources `.env` if present.  Usage instructions
  in the script header and in `docs/testnet-demo.md` (new step 7).

### Added

- Makefile: `lint-all` target that runs `lint` (cargo fmt --check + cargo
  clippy) followed by `cd backend && npm run lint` (#261).  Contributors can
  now run a single command from the repo root to check both the Rust contracts
  and the Node.js backend in sequence.  `.PHONY` and `help` output updated.
  CONTRIBUTING.md updated with a note about `make lint-all` in the PR
  checklist section.

### Added

- scripts/check-ttl.sh: helper script that queries the remaining TTL for all
  deployed contracts (token, vesting, multisig) using `stellar contract read`
  and prints a warning when any contract's instance TTL falls below
  `INSTANCE_BUMP_THRESHOLD` (518 400 ledgers, ~30 days) (#262).  Reads
  contract IDs from `TOKEN_ID`, `VESTING_ID`, and `MULTISIG_ID` env vars,
  falling back to `deployments.json` testnet values when unset.  Shows
  remaining ledgers, approximate days, and the exact `extend_ttl` command to
  run for any contract below the threshold.  Respects `STELLAR_NETWORK`;
  sources `.env` if present.  Usage instructions in the script header; full
  TTL reference in `docs/ttl.md`.

### Fixed — 2026-11-05

- contracts/vesting/src/lib.rs: removed the accidental duplicate definition
  of `test_create_schedule_minimum_amount` introduced in #164, which made the
  vesting test binary fail to compile (E0428: name defined multiple times).

### Tests — 2026-11-05

- backend/src/routes/token.test.js: three new tests for the
  GET /api/token/allowance validation (#241):
  - 200 with an `allowance` field when both `owner` and `spender`
    query params are present
  - 400 with an `error` body when `owner` is missing
  - 400 with an `error` body when `spender` is missing

### Tests — 2026-11-05

- backend/src/app.test.js: new test verifying that unknown routes
  return a 404 with a JSON error body (#235). The catch-all handler
  in src/index.js responds with { error: 'Not found' }; the test
  asserts both the 404 status and that res.body.error is defined,
  across GET /api, GET non-/api, and POST cases.

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

### Added

- contracts/vesting: `cancel_schedule(env, schedule_id, recipient)` (#282). Cancels a vesting schedule that has not yet started (now < start_time), refunds total_amount to recipient, and sets status to Cancelled. Panics with ScheduleNotStarted if the schedule has already started. New ScheduleStatus::Cancelled variant and VestingError::ScheduleNotStarted = 11 added. Three new tests: test_cancel_schedule_success, test_cancel_schedule_already_started_panics, test_cancel_already_cancelled_panics.

- contracts/vesting: `get_vesting_progress(env, schedule_id) -> VestingProgress` (#284). Returns vested, claimed, claimable, unvested, and status in a single read-only call. Eliminates the two-round-trip pattern (get_schedule + get_claimable) previously needed to build a progress display. New VestingProgress contracttype struct. Three new tests: test_get_vesting_progress_mid_schedule, test_get_vesting_progress_at_completion, test_get_vesting_progress_revoked. README updated.

### Added

- contracts/vesting: `cancel_schedule(env, schedule_id, recipient)` (#282). Cancels a vesting schedule that has not yet started (now < start_time), refunds total_amount to recipient, and sets status to Cancelled. Panics with ScheduleNotStarted if the schedule has already started. New ScheduleStatus::Cancelled variant and VestingError::ScheduleNotStarted = 11 added. Three new tests: test_cancel_schedule_success, test_cancel_schedule_already_started_panics, test_cancel_already_cancelled_panics.

### Added — 2026-11-22

- contracts/token/src/lib.rs: `test_allowance_of_unapproved_pair_returns_zero` (#233).
  Asserts that `allowance()` returns exactly `0` for a fresh owner-spender pair
  where `approve()` was never called, exercising the `.unwrap_or(0)` fallback
  in `TokenContract::allowance`.

### Added — 2026-11-22

- backend/src/routes/multisig.test.js: three test cases for `POST /api/multisig/proposal` (#237).
  Covers the happy path (202 with valid body), missing `proposer` (400), and
  missing `description` (400), exercising the validation guard in
  `multisig.js` that returns 400 when either field is absent.

### Added — docs/254-disaster-recovery

- docs/disaster-recovery.md (#254): guide for restoring archived contract
  entries using `stellar contract restore`. Covers detection (empty
  `stellar contract read` output), instance-storage restore, persistent-entry
  restore for all three contracts (schedules, proposals, balances, allowances),
  post-restore verification via `stellar contract read` and read-only getters,
  and prevention by tuning keep-alive frequency. Linked from docs/ttl.md and
  README "Storage TTL constants" section.

### Added — ci/260-validate-deployments-json

- .github/workflows/ci.yml (#260): added "Validate deployments.json" step to
  the contracts CI job. Runs `python3 -m json.tool deployments.json > /dev/null`
  which exits non-zero on malformed JSON (merge conflicts, truncation, etc.).
  `python3` is available on all GitHub-hosted Ubuntu runners with no extra
  installation step.

### Added — ci/259-cargo-deny

- deny.toml (#259): cargo-deny configuration at the workspace root. Allows
  MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception, Unicode-DFS-2016, ISC,
  BSD-2-Clause, BSD-3-Clause, 0BSD, and CC0-1.0 licenses (all licenses
  present in the Stellar/Soroban dependency tree). Denies unlicensed and
  copyleft crates. Warns on duplicate crate versions. Restricts sources to
  crates.io only.

- .github/workflows/ci.yml (#259): two new steps in the contracts job —
  "Install cargo-deny" (cargo install cargo-deny --locked) and "Dependency
  check (licenses + advisories)" (cargo deny check). The check runs after
  the WASM build and fails the job on license violations or unpatched
  security advisories.

### Added — 2026-10-16

- contracts/vesting/README.md: "Error Codes" section (#283). Documents all
  `VestingError` variants with their `u32` codes, trigger conditions, and
  the functions that emit each error. Updates all per-function **Panics**
  entries in the README from bare string literals to typed
  `VestingError::Variant (N)` references, matching the `panic_with_error!`
  calls already in `lib.rs`. No code changes — all `panic!` strings were
  already replaced by `VestingError` in the `#[contracterror]` enum (PR #124).

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

### Added — 2026-10-17

- docs/postman-collection.json: Postman Collection v2.1 covering all 28
  backend REST API endpoints (#266). Four folders — Health (2), Vesting (9),
  Token (9), Multisig (9) — each request uses `{{baseUrl}}` defaulting to
  `http://localhost:3001` and includes an example request body and example
  response. README updated with import instructions under a new "Postman
  Collection" subsection inside "Backend API".

### Changed — 2026-10-17

- .github/workflows/ci.yml: backend CI test step changed from `npm test
  --if-present` to `npx jest --coverage --forceExit` so coverage data is
  collected on every CI run (#275). The coverage report is uploaded as a
  GitHub Actions artifact (`backend-coverage`, 14-day retention).
- backend/package.json: added `jest` configuration block with
  `testEnvironment: "node"`, `coverageDirectory: "coverage"`, and
  `collectCoverageFrom: ["src/**/*.js"]` (#275).

### Added — 2026-10-17 (openapi)

- docs/openapi.yaml: OpenAPI 3.0.3 specification covering all 28 REST
  endpoints across Health, Vesting, Token, and Multisig routers (#276).
  Includes component schemas for `VestingScheduleStub`, `ProposalStub`,
  `TokenInfoResponse`, `SuccessResponse`, `ErrorResponse`, and
  `AcceptedResponse`. Server set to `http://localhost:3001`. Paths carry
  full request body schemas with required-field annotations and 202/400/503
  response shapes matching the actual route implementations.

- .github/workflows/ci.yml: OpenAPI lint step added to the backend job,
  after the existing test step, using `npx @redocly/cli@latest lint` (#276).

- README.md: "Backend API" section now links to `docs/openapi.yaml` for
  Swagger UI / Redoc import (#276).

### Added — 2026-10-17

- docs/postman-collection.json: Postman Collection v2.1 covering all 28
  backend REST API endpoints (#266). Four folders — Health (2), Vesting (9),
  Token (9), Multisig (9) — each request uses `{{baseUrl}}` defaulting to
  `http://localhost:3001` and includes an example request body and example
  response. README updated with import instructions under a new "Postman
  Collection" subsection inside "Backend API".

### Changed — 2026-10-17

- .github/workflows/ci.yml: backend CI test step changed from `npm test
  --if-present` to `npx jest --coverage --forceExit` so coverage data is
  collected on every CI run (#275). The coverage report is uploaded as a
  GitHub Actions artifact (`backend-coverage`, 14-day retention).
- backend/package.json: added `jest` configuration block with
  `testEnvironment: "node"`, `coverageDirectory: "coverage"`, and
  `collectCoverageFrom: ["src/**/*.js"]` (#275).

### Changed — 2026-10-17

- .github/workflows/ci.yml: backend CI test step changed from `npm test
  --if-present` to `npx jest --coverage --forceExit` so coverage data is
  collected on every CI run (#275). The coverage report is uploaded as a
  GitHub Actions artifact (`backend-coverage`, 14-day retention).
- backend/package.json: added `jest` configuration block with
  `testEnvironment: "node"`, `coverageDirectory: "coverage"`, and
  `collectCoverageFrom: ["src/**/*.js"]` (#275).

### Added — 2026-10-17

- contracts/vesting/src/lib.rs: `test_multisig_revoke_schedule` e2e test (#277).
  Deploys a 2-of-3 multisig as the vesting admin, creates a schedule via a
  multisig `create_schedule` proposal, then submits a second proposal for
  `ProposalAction::Call { target: vesting, function: "revoke", args: [schedule_id, treasury] }`,
  confirms ×2, and executes. Asserts `schedule.status == Revoked`, treasury
  received all 100 000 unvested tokens, beneficiary received 0 (nothing vested
  at t=0), and vesting contract holds 0 tokens after revoke. This covers the
  most common admin use-case — revoke via multisig — which was not tested by the
  existing `test_multisig_admin_flow` (pause) or `test_multisig_create_schedule`.

### Changed — 2026-10-16

- contracts/token/src/lib.rs: added `///` doc comments to all 18 public
  functions (#95). Each comment states purpose, auth requirement, panic
  conditions, and return value. Covers `__constructor`, `mint`, `burn`,
  `transfer`, `transfer_from`, `approve`, `pause`, `unpause`,
  `transfer_admin`, `name`, `symbol`, `decimals`, `total_supply`, `balance`,
  `allowance`, `is_paused`, `admin`, and `extend_ttl`. No behaviour changes;
  all existing tests continue to pass.

### Added — 2026-10-16

- CONTRIBUTING.md: "Writing CHANGELOG entries" section (#293). Documents the Keep a Changelog format used by this project: which subsection to use (Added/Changed/Fixed/Removed), entry format with component and PR number, a concrete example matching existing entries, and guidance on when to skip an entry.

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

### Changed

- contracts/token/src/lib.rs, contracts/vesting/src/lib.rs: extract private `get_admin_address(env)` helper (#298). Both `require_admin()` and the public `admin()`/`get_admin()` functions now delegate the storage read to this single helper, eliminating duplicated `env.storage().instance().get(&ADMIN).expect("not initialized")` patterns. Closes #298.

### Added

- contracts/vesting: `total_claimable_for_beneficiary(env, beneficiary) -> i128` read-only view (#302). Returns the aggregate vested-but-unclaimed amount across all `Active` schedules for a given beneficiary, without the caller needing to iterate schedule IDs. `Revoked` and `Completed` schedules are excluded. No write footprint. Five new tests: multiple active schedules, mix of active/revoked/completed, no schedules (returns 0), before cliff (returns 0), after partial claim. `contracts/vesting/README.md` updated. Closes #302.

- docs/contributing-contracts.md: guide covering all five steps to add a new Soroban contract to the workspace — crate creation and workspace registration, `soroban-sdk` dependency with `testutils` feature flag, writing a first test, CI integration (automatic via workspace), and README architecture diagram update (#301). Linked from CONTRIBUTING.md. Closes #301.

### Added — 2026-11-19

- contracts/vesting/README.md: "Rounding" subsection documenting that
  `vested_amount()` uses integer division (truncates toward zero), with a
  maximum rounding error of `total_duration - 1` tokens per call, a worked
  numeric example (`total_amount = 100`, `total_duration = 3`, `elapsed = 1`
  → `100 / 3 = 33`, 1 token lost), and the implication that the last claim may
  receive slightly more than intermediate claims (#294).
- scripts/query-token.sh: CLI helper to query a SEP-0041 token balance and
  `total_supply` from testnet (or mainnet) via `stellar contract invoke`,
  modeled on `scripts/invoke.sh` (#296).
- scripts/examples/submit-multisig-pause.js: Node.js example that builds the
  `CallData` ScVal for `vesting.pause()` using `nativeToScVal`, wraps it as
  `ProposalAction::Call { target: vesting, function: "pause", args: [] }`,
  simulates `multisig.submit()` via `@stellar/stellar-sdk`, and prints the
  simulated transaction XDR. Linked from `docs/testnet-demo.md` (#300).

### Changed — 2026-11-19

- contracts/vesting/src/lib.rs, contracts/token/src/lib.rs,
  contracts/multisig/src/lib.rs: removed unjustified
  `#![allow(deprecated)]` and `#![allow(clippy::needless_borrows_for_generic_args)]`
  attributes that masked warnings from soroban-sdk internals. No deprecated or
  clippy-flagged constructs found in our own code; if SDK-internal warnings
  surface they can be re-added with a `// reason:` comment (#297).

### Added — 2026-10-16

- contracts/vesting/src/lib.rs: `is_admin(env, address) -> bool` (#221).
  Pure-read predicate that returns `true` when `address` equals the stored
  admin. No write footprint; reads instance storage and compares. Matches the
  `is_owner()` pattern in the multisig contract, eliminating the need for
  callers to call `get_admin()` and compare addresses manually. Two new unit
  tests: `test_is_admin_true_for_admin` and `test_is_admin_false_for_non_admin`.
  `contracts/vesting/README.md` updated with the new function reference entry.

### Added — 2026-10-17

- contracts/vesting: `is_admin(env, address: Address) -> bool` (#221). Pure
  read-only predicate that returns `true` when `address` equals the current
  admin. Matches the `is_owner()` pattern in the multisig contract. No write
  footprint. New tests: `test_is_admin` (true for admin, false for non-admin).

### Added — 2026-10-16

- contracts/vesting/README.md: "Rounding behavior" section (#96). Documents
  that `vested_amount` uses integer (floor) division
  (`elapsed * total_amount / total_duration`), explains dust accumulation until
  schedule completion, includes a worked example (99-token / 100-second
  schedule), and adds integrator notes. Linked from the `vested_amount` source
  comment.

- contracts/vesting/src/lib.rs: inline `// Note:` comment in `vested_amount()`
  (#96). Points to the README "Rounding behavior" section and explains that the
  fractional remainder (dust) is released only when `elapsed >= total_duration`.

### Added — 2026-10-16

- contracts/multisig/src/lib.rs: `execute()` and `cancel()` now include the
  proposer/caller address in their event payloads (#220). `execute()` emits
  `("executed",) → (proposal_id: u64, proposer: Address)` and `cancel()` emits
  `("cancelled",) → (proposal_id: u64, caller: Address)`, matching the
  `("submitted",) → (id, proposer)` pattern already used in `submit()`. Off-chain
  indexers can now identify the proposer and caller from events without a separate
  `get_proposal()` call. Tests `test_execute_proposal_stored_fields` and
  `test_cancel_by_proposer` updated to assert the new payload. README events table
  updated.

### Added — 2026-10-16

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

### Added

- .pre-commit-config.yaml: pre-commit hook configuration running `cargo fmt --all -- --check` on Rust files and `cd backend && npm run lint` on JavaScript files (#299). Install with `pip install pre-commit && pre-commit install`. Closes #299.

### Added — 2026-10-15

- contracts/vesting: `total_claimable_for_beneficiary(env, beneficiary) -> i128` read-only view (#302). Returns the aggregate vested-but-unclaimed amount across all `Active` schedules for a given beneficiary, without the caller needing to iterate schedule IDs. `Revoked` and `Completed` schedules are excluded. No write footprint. Five new tests: multiple active schedules, mix of active/revoked/completed, no schedules (returns 0), before cliff (returns 0), after partial claim. `contracts/vesting/README.md` updated. Closes #302.

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

### Added — 2026-11-18

- Makefile: added `make check-env` target that validates
  `VESTING_CONTRACT_ID`, `TOKEN_CONTRACT_ID` and `MULTISIG_CONTRACT_ID`
  are set before deploying, failing fast with a clear message instead of
  a cryptic stellar-cli error. `deploy-testnet` now depends on
  `check-env`; documented in CONTRIBUTING.md (#273).

### Changed — 2026-11-18

- CI: pinned the `stellar/stellar-cli` GitHub Actions step to the
  immutable commit SHA `300aaf69ab100536678bdb641428b06f06b318ea`
  (`stellar/stellar-cli@v28.0.0`) for supply-chain security (#274).

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
