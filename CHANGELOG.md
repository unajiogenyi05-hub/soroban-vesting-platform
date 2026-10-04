# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
  via `env.invoke_contract`; 4 new tests (PR #33, PR #34):
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
- CHANGELOG.md: removed meta-doc entry that documented an earlier documentation
  change (PR #36).

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
