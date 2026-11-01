# Contributing to Soroban Vesting Platform

Thank you for your interest in contributing! This project spans three layers — Soroban smart contracts (Rust), a Node.js REST API backend, and a vanilla JS frontend. This guide covers everything you need to get from a fresh clone to a running development environment.

---

## Prerequisites

| Tool | Minimum Version | Install |
|------|-----------------|--------|
| Rust | stable | [rustup.rs](https://rustup.rs) |
| wasm32v1-none target | — | `rustup target add wasm32v1-none` |
| stellar-cli | v28+ | `curl -sSfL https://install.stellar.org \ sh` |
| Node.js | ≥ 18 | [nodejs.org](https://nodejs.org) |
| npm | ≥ 9 | Included with Node.js |
| cargo-audit | latest | `cargo install --locked cargo-audit` |

---

## Setup

### 1. Clone and configure

```bash
git clone https://github.com/unajiogenyi05-hub/soroban-vesting-platform
cd soroban-vesting-platform
cp .env.example .env
# Edit .env — see comments in the file for each variable
```

### 2. Contracts (Rust / Soroban)

```bash
# Build all contracts to WASM
stellar contract build

# Run all unit tests
cargo test --all

# Lint
cargo clippy --all --target wasm32v1-none -- -D warnings

# Format
cargo fmt --all

# Scan dependencies for known CVEs (RustSec advisory database)
cargo audit
```

### 3. Backend (Node.js / Express)

```bash
cd backend
npm ci
npmr run dev        # starts on port 3001 with hot reload (nodemon)
npm test           # run Jest test suite
npm run lint       # run ESLint
```

### 4. Frontend

```bash
# Open directly in browser (no build step needed)
open frontend/index.html

# Or serve locally
npx serve frontend/
```

---

## Environment Variables

See [`.env.example`(../.env.example) for all variables with inline comments. Key ones:

| Variable | Description |
|----------|-------------|
| `STELLAR_NETWORKP | `testnet` or `mainnet` |
| `STELLAR_RPC_URL` | Soroban RPC endpoint |
| `ADMIN_SECRET_KEY` | Secret key for contract admin operations |
| `VESTING_CONTRACT_ID` | Deployed vesting contract ID (set after deploy) |
| `TOKEN_CONTRACT_ID` | Deployed token contract ID (set after deploy) |
| `MULTISIG_CONTRACT_ID` | Deployed multisig contract ID (set after deploy) |

---

## Running All Tests

```bash
# Contracts
cargo test --all

# Backend
cd backend && npm test

# Both together (from repo root)
cargo test --all & (cd backend && npm test) && wait
```

---

## Running All CI Checks Locally

Use `make ci-check` to run every check that CI runs, in the same order, with
a single command. It fails fast on the first error so you can fix issues
before pushing.

```bash
make ci-check
```

This runs the following six steps in order:

1. `cargo fmt --all -- --check` — formatting check
2. `cargo clippy --all --target wasm32v1-none -- -D warnings` — linting
3. `cargo test --all --locked` — contract unit tests
4. `cargo audit` — dependency vulnerability scan (RustSec)
5. `cd backend && npm run lint` — backend ESLint
6. `cd backend && npm test` — backend Jest tests

---

## Security Audits

CI runs `cargo audit` on every push and pull request to `main` via the
[`rustsec/audit-check`](https://github.com/rustsec/audit-check) GitHub Action.
It checks `Cargo.lock` against the [RustSec advisory database](https://github.com/rustsec/rustsec/tree/main/crates/advisory-db)
and fails the build if a known vulnerability is detected.

Run the same check locally before pushing:

```bash
cargo install --locked cargo-audit  # one-time setup
cargo audit
```

If `cargo audit` reports an advisory, update the affected dependency (`Cargo.lock`)
or, for advisories that do not apply to our usage, add a justified entry to
`audit.toml` and reference it in your PR.

---

## PR Process

### Branch naming
```
feat/short-description
fix/short-description
docs/short-description
ci/short-description
```

### Commit messages
Follow [Conventional Commits](https://www.conventionalcommits.org/):
```
feat: add batch_create_schedule function
fix: extend TTL on persistent storage entries
docs: add api.md REST reference
ci: add cargo audit to pipeline
```

### PR checklist
Before opening a PR, confirm:

- [ ] `cargo test --all` passes with zero failures
- [ ] `cargo clippy --all --target wasm32v1-none -- -D warnings` passes
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo audit` passes with no known vulnerabilities
- [ ] `cd backend && npm test` passes
- [ ] `cd backend && npm run lint` passes
- [ ] New functions have `///` doc comments (contracts) or JSDoc (backend/frontend)
- [ ] `CHANGELOG.md` updated under `[Unreleased]`
- [ ] PR description references the related issue with `Closes #<number>

---

## Project Structure

```
soroban-vesting-platform/
├── contracts/
│   ├── token/          # SEP-0041 fungible token
│   ├── vesting/        # Linear vesting with cliff
│   └── multisig/       # N-of-M governance wallet
├── backend/
│   └── src/
│       ├── routes/     # Express route handlers
│       ├── services/   # Stellar SDK integration
│       └── index.js     # App entry point
├── frontend/
│   ├── index.html
│   ├── js/app.js
│   └── css/styles.css
├── scripts/            # Deploy, invoke, fund helpers
├── docs/               # Architecture docs
└── .github/workflows/  # CI pipeline
```

---

## Code of Conduct

This project follows the [Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md).
By participating you agree to abide by its terms. Report unacceptable behavior
via the GitHub private security reporting URL:
https://github.com/unajiogenyi05-hub/soroban-vesting-platform/security

## Reporting Issues

Use the GitHub Issue tracker. Check existing issues before opening a new one. For security vulnerabilities, see [SECURITY.md](SECURITY.md).
