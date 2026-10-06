# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed — 2026-10-06

- CHANGELOG.md: added missing PR #37 reference to the five e2e vesting tests
  entry; removed one meta-doc bullet that described an earlier documentation
  correction (PR #36 entry in Fixed — 2026-10-04).

### Added — 2026-10-06

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
