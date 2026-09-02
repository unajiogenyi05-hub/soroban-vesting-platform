# Contributing to Soroban Vesting Platform

Thank you for your interest in contributing! This project spans three layers — Soroban smart contracts (Rust), a Node.js REST API backend, and a vanilla JS frontend. This guide covers everything you need to get from a fresh clone to a running development environment.

---

## Prerequisites

| Tool | Minimum Version | Install |
|------|----------------|---------|
| Rust | stable | [rustup.rs](https://rustup.rs) |
| wasm32v1-none target | — | `rustup target add wasm32v1-none` |
| stellar-cli | v28+ | `curl -sSfL https://install.stellar.org \| sh` |
| Node.js | ≥ 18 | [nodejs.org](https://nodejs.org) |
| npm | ≥ 9 | Included with Node.js |

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
```

### 3. Backend (Node.js / Express)

```bash
cd backend
npm ci
npm run dev        # starts on port 3001 with hot reload (nodemon)
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

See [`.env.example`](../.env.example) for all variables with inline comments. Key ones:

| Variable | Description |
|----------|-------------|
| `STELLAR_NETWORK` | `testnet` or `mainnet` |
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
- [ ] `cd backend && npm test` passes
- [ ] `cd backend && npm run lint` passes
- [ ] New functions have `///` doc comments (contracts) or JSDoc (backend/frontend)
- [ ] `CHANGELOG.md` updated under `[Unreleased]`
- [ ] PR description references the related issue with `Closes #<number>`

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
│       └── index.js    # App entry point
├── frontend/
│   ├── index.html
│   ├── js/app.js
│   └── css/styles.css
├── scripts/            # Deploy, invoke, fund helpers
├── docs/               # Architecture docs
└── .github/workflows/  # CI pipeline
```

---

## Reporting Issues

Use the GitHub issue tracker. Check existing issues before opening a new one. For security vulnerabilities, see [SECURITY.md](SECURITY.md).
