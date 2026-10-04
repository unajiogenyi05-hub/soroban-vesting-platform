# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added (2026-10-04)

- contracts/multisig/src/lib.rs: 15 new tests (19 total) covering update_threshold,
  update_threshold_zero_panics, update_threshold_exceeds_owners_panics, add_owner,
  add_owner_duplicate_panics, remove_owner, remove_owner_below_threshold_panics,
  double_confirm_panics, non_owner_submit_panics, non_owner_confirm_panics,
  execute_by_non_owner_succeeds_when_threshold_met, execute_twice_panics,
  cancel_by_proposer, cancel_by_non_proposer_panics, and
  revoke_drops_below_threshold_then_reconfirm_executes. (PR #30)
- README.md: "Status and limitations" table listing audit status, backend stub
  behaviour, missing wallet integration, proptest status, multisig execute
  status, and mainnet recommendation.
- docs/architecture.md: signing model section clarifying that the backend does
  not hold private keys or submit transactions.
- contracts/vesting/src/lib.rs: manual parameterised tests for four vesting
  math properties (vested never decreases, never exceeds total, zero before
  cliff, equals total at or after end).
- contracts/vesting/src/lib.rs: multisig-as-admin end-to-end tests
  (test_multisig_admin_flow, test_multisig_below_threshold_panics): multisig
  contract is the vesting admin; a proposal confirmed to threshold executes via
  a real cross-contract call; execution below threshold panics with "not enough
  confirmations".
- contracts/vesting/src/lib.rs: events emitted for create_schedule, claim,
  revoke, pause/unpause, transfer_admin; tests assert events are published.
- contracts/token/src/lib.rs: events emitted for mint, burn, transfer, approve,
  pause/unpause.
- backend/src/routes/*.test.js: Jest tests for vesting, token, multisig, and
  health routes with Stellar service mocked (validation errors, success
  responses, health endpoint).
- SECURITY.md: threat model covering admin key compromise, multisig threshold,
  revocation, token pause, replay, TTL expiry; signing model documentation.
- .github/workflows/ci.yml: frontend job extended with html-validate and
  eslint steps.

### Fixed (2026-10-04) — Task A: multisig owner management authorization

- contracts/multisig/src/lib.rs: critical authorization bug fixed. `add_owner`,
  `remove_owner`, and `update_threshold` had no `require_auth` and no caller
  check — any account could add themselves as owner and lower the threshold to 1,
  gaining full control of any contract the multisig administers.
- contracts/multisig/src/lib.rs: introduced `ProposalAction` enum
  (`Call(CallData)`, `AddOwner(Address)`, `RemoveOwner(Address)`,
  `UpdateThreshold(u32)`) replacing the flat `(target, function, args)` fields
  on `ProposalData`. Owner management variants are handled directly inside
  `execute()` — no public entry points exist for `add_owner`, `remove_owner`,
  or `update_threshold`.
- contracts/multisig/src/lib.rs: `execute()` now dispatches `ProposalAction::Call`
  via `env.invoke_contract`, and internal variants (`AddOwner`, `RemoveOwner`,
  `UpdateThreshold`) directly — no cross-contract call is issued for internal
  actions (Soroban disallows self-invocation).
- contracts/multisig/src/lib.rs: when `RemoveOwner` executes, existing
  confirmations by the removed owner on pending proposals are cleared and
  `confirmation_count` is decremented, keeping threshold accounting accurate.
- contracts/multisig/src/lib.rs: 28 tests total (was 24); new tests:
  `test_outsider_cannot_add_owner`, `test_outsider_cannot_remove_owner`,
  `test_outsider_cannot_update_threshold` (prove the bug pre-fix; pass after fix),
  `test_owner_management_only_via_proposal`, `test_remove_owner_only_via_proposal`,
  `test_update_threshold_only_via_proposal`, `test_self_call_reentrance_fails`,
  `test_remove_owner_clears_confirmations`.
- contracts/vesting/src/lib.rs: updated `test_multisig_admin_flow`,
  `test_multisig_below_threshold_panics` to use new `ProposalAction` API.
- contracts/vesting/src/lib.rs: added `test_multisig_create_schedule` — end-to-end
  test where the multisig (as vesting admin) calls `create_schedule` with a real
  token; asserts token balances, schedule count, and beneficiary can claim.
- README.md, docs/architecture.md: updated to describe the actual implemented
  behaviour.

### Changed (2026-10-04)

- README.md: layout tree corrected to match the repo; wording updated to
  "reference implementation, unaudited, testnet-ready"; multisig-as-admin flow
  described; testnet-demo section updated to reflect what
  scripts/demo-testnet.sh actually does.
- backend/package.json: removed --passWithNoTests flag from jest invocation.
- SECURITY.md: rewritten to be specific to this repo with real commands,
  threat model, and signing architecture.
- CONTRIBUTING.md: rewritten with repo-specific commands, structure, and risks.

### Fixed (2026-10-04)

- .github/workflows/ci.yml: fixed YAML parse error (inline JSON in --rule flag
  broke the workflow before any jobs ran); removed || true from lint step;
  extracted ESLint config to frontend/.eslintrc.json.
- frontend/index.html: fixed all 37 html-validate errors (void element
  self-closing slashes, redundant ARIA landmark roles, missing type= on
  buttons, wcag/h32 missing submit button on confirmForm).
- contracts/vesting/src/lib.rs: applied cargo fmt (three formatting diffs).

### Removed (2026-10-04)

- Cargo.lock removed from .gitignore so the lockfile is tracked by git (CI
  already caches on it).

### Added (2026-09-20)

- contracts/vesting/src/lib.rs: 29 `#[test]` functions covering create and
  claim (partial and full), zero cliff, cliff blocks early claim, revoke returns
  unvested tokens, revoke pays vested to beneficiary, claim after revoke panics,
  claim before cliff panics, pause blocks create_schedule and claim, unpause
  restores claim, transfer_admin, schedule_count, multiple beneficiaries,
  beneficiary_schedules (multiple), double revoke panics, double initialize
  panics, is_paused state, and events (created, claimed, revoked,
  pause/unpause, transfer_admin).
- scripts/demo-testnet.sh: keygen, Friendbot funding, build, deploy token +
  vesting, mint, approve, create schedule, claim if the cliff has passed.
- docs/testnet-demo.md: manual walkthrough with the 1-year cliff / 4-year DAO
  example.

### Changed (2026-09-20)

- README.md: architecture diagram, "What this platform does" example, contract
  API tables, schedule lifecycle diagram, repository layout, prerequisites,
  link to testnet demo docs.

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
