# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added (2026-10-04)
- contracts/vesting/src/lib.rs: proptest dev-dependency added; property-based
  tests covering vested amount never decreases over time, never exceeds total,
  is zero before cliff, equals total at or after end.
- contracts/vesting/src/lib.rs: multisig-as-admin end-to-end test
  (`test_multisig_admin_flow`): multisig contract is the vesting admin; a
  proposal confirmed to threshold creates a schedule; execution below threshold
  panics with "not enough confirmations".
- contracts/vesting/src/lib.rs: events emitted for create_schedule, claim,
  revoke, pause/unpause; tests assert all events fire.
- contracts/token/src/lib.rs: events emitted for mint, burn, transfer,
  approve, pause/unpause; tests assert all events fire.
- contracts/multisig/src/lib.rs: additional tests — threshold changes,
  remove_owner below threshold, execute without enough confirmations,
  cross-contract test where vesting pulls from the real token contract on
  create_schedule.
- backend/src/routes/\*.test.js: Jest tests for vesting, token, multisig, and
  health routes with Stellar service mocked (validation errors, success
  responses, health endpoint).
- SECURITY.md: threat model section covering admin key compromise, multisig
  threshold, revocation, token pause, replay, TTL expiry; signing model
  documentation; rate-limiting and input-validation notes.
- .github/workflows/ci.yml: frontend job extended with html-validate and
  eslint steps.

### Changed (2026-10-04)
- README.md: layout tree corrected to match the repo; wording updated
  (replaced "production-ready" with "reference implementation, unaudited,
  testnet-ready"); multisig-as-admin flow described; testnet-demo section
  updated to reflect what scripts/demo-testnet.sh actually does.
- backend/package.json: removed --passWithNoTests flag from jest invocation.
- SECURITY.md: rewritten to be specific to this repo with real commands,
  threat model, and signing architecture.
- CONTRIBUTING.md: rewritten with repo-specific commands, structure, and risks.

### Removed (2026-10-04)
- Cargo.lock removed from .gitignore so the lockfile is tracked by git (CI
  already caches on it).

### Changed (2026-09-20)
- contracts/vesting/src/lib.rs: tests expanded to 17 `#[test]` functions.
  Covers: create and claim (partial and full), zero cliff, cliff blocks early
  claim, revoke returns unvested tokens, claim after revoke panics, claim
  before cliff panics, pause blocks create_schedule and claim, unpause
  restores claim, transfer_admin, schedule_count, multiple beneficiaries,
  beneficiary_schedules, double revoke panics, double initialize panics,
  is_paused.
- README.md: architecture diagram, "What this platform does" example, contract
  API tables, schedule lifecycle diagram, repository layout, prerequisites,
  link to testnet demo docs.

### Added (2026-09-20)
- scripts/demo-testnet.sh: keygen, Friendbot funding, build, deploy token +
  vesting, mint, approve, create schedule, claim if the cliff has passed.
- docs/testnet-demo.md: manual walkthrough with the 1-year cliff / 4-year DAO
  example.

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
