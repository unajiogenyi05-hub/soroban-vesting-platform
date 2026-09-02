# Soroban Vesting Platform

A production-ready token vesting platform built on the [Stellar](https://stellar.org) network using [Soroban](https://soroban.stellar.org) smart contracts.

[![CI](https://github.com/unajiogenyi05-hub/soroban-vesting-platform/actions/workflows/ci.yml/badge.svg)](https://github.com/unajiogenyi05-hub/soroban-vesting-platform/actions/workflows/ci.yml)

---

## What's included

| Path | Description |
|------|-------------|
| `contracts/token/` | SEP-0041 compatible fungible token with mint, burn, pause |
| `contracts/vesting/` | Linear vesting with cliff, claim, and revoke |
| `contracts/multisig/` | N-of-M multisig governance for admin operations |
| `backend/` | Node.js REST API for interacting with deployed contracts |
| `frontend/` | Single-page UI for managing vesting schedules |
| `scripts/` | Deploy, invoke, and fund helper scripts |
| `.github/workflows/ci.yml` | CI: fmt, clippy, test, build on every push/PR |

---

## Contracts

### Token (`contracts/token/`)
SEP-0041 compatible fungible token featuring:
- `initialize` — set admin, name, symbol, decimals, initial supply
- `mint` / `burn` — admin-only supply management
- `transfer` / `transfer_from` — standard transfers with allowance support
- `approve` — set spending allowances
- `pause` / `unpause` — emergency freeze (admin only)

### Vesting (`contracts/vesting/`)
Linear token vesting with:
- `create_schedule` — lock tokens for a beneficiary with cliff + linear release
- `claim` — beneficiary claims currently vested tokens
- `revoke` — admin returns unvested tokens
- `get_claimable` — read how many tokens are available to claim

### Multisig (`contracts/multisig/`)
N-of-M governance wallet:
- `initialize` — set owners list and confirmation threshold
- `submit` / `confirm` / `revoke_confirmation` / `execute` — proposal lifecycle
- `add_owner` / `remove_owner` / `update_threshold` — owner management

---

## Prerequisites

| Tool | Install |
|------|---------|
| Rust (stable) | [rustup.rs](https://rustup.rs) |
| stellar-cli | `curl -fsSL https://github.com/stellar/stellar-cli/raw/main/install.sh \| sh` |
| Node.js ≥ 18 | [nodejs.org](https://nodejs.org) |

---

## Quick start

```bash
# 1. Clone
git clone https://github.com/unajiogenyi05-hub/soroban-vesting-platform
cd soroban-vesting-platform

# 2. Configure
cp .env.example .env
# Edit .env with your keypair and network settings

# 3. Build contracts
make build

# 4. Run tests
make test

# 5. Deploy to testnet
make deploy-testnet

# 6. Start backend API
cd backend && npm install && npm start
```

---

## Development

```bash
# Format
cargo fmt --all

# Lint
cargo clippy --all --target wasm32v1-none -- -D warnings

# Test contracts
cargo test --all

# Build WASM
stellar contract build
```

---

## Backend API — live data note

The backend routes are pre-wired and fully functional. To receive live on-chain data instead of scaffold responses, set the following variables in your `.env` file after deploying the contracts:

```
VESTING_CONTRACT_ID=<deployed vesting contract ID>
TOKEN_CONTRACT_ID=<deployed token contract ID>
MULTISIG_CONTRACT_ID=<deployed multisig contract ID>
SOURCE_SECRET_KEY=<Stellar secret key for transaction signing>
```

Without these values the API returns documented stub responses so the frontend can be developed and tested independently of a live deployment.

---

## Environment variables

See [`.env.example`](.env.example) for all required configuration.

---

## License

MIT
