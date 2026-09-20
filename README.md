# Soroban Vesting Platform

A production-ready token vesting platform built on the [Stellar](https://stellar.org)
network using [Soroban](https://soroban.stellar.org) smart contracts.

[![CI](https://github.com/unajiogenyi05-hub/soroban-vesting-platform/actions/workflows/ci.yml/badge.svg)](https://github.com/unajiogenyi05-hub/soroban-vesting-platform/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

---

## What this platform does

The Soroban Vesting Platform lets a DAO, startup, or protocol treasury lock
tokens for contributors, investors, or team members and release them on a
customisable linear schedule with an optional cliff period. An N-of-M multisig
contract governs admin operations so no single key can create or revoke
schedules unilaterally.

**Example — DAO team vesting:**

> A DAO mints 4,000,000 governance tokens for five core contributors.
> The treasury (controlled by a 3-of-5 multisig) calls `create_schedule` for
> each contributor with a 1-year cliff and a 4-year total duration. No tokens
> are claimable for the first year. After the cliff, each contributor can call
> `claim` at any time to receive their pro-rata share of vested tokens. If a
> contributor leaves before the end of their schedule the multisig calls
> `revoke`, which returns unvested tokens to the treasury.

---

## Architecture

```
┌───────────────────────────────────────────────────────┐
│                   Browser / CLI                        │
│   (frontend SPA or stellar-cli invoke commands)        │
└────────────────────┬──────────────────────────────────┘
                     │ HTTP
┌────────────────────▼──────────────────────────────────┐
│              Node.js REST API (backend/)               │
│  /vesting  /token  /multisig  /health                  │
│  stellar.js — wraps @stellar/stellar-sdk Soroban RPC   │
└──┬─────────────────┬──────────────────┬───────────────┘
   │ Soroban RPC     │ Soroban RPC      │ Soroban RPC
┌──▼──────────┐  ┌───▼──────────┐  ┌───▼──────────────┐
│   Token     │  │   Vesting    │  │    Multisig      │
│  Contract   │  │  Contract    │  │    Contract      │
│(SEP-0041)   │  │(linear+cliff)│  │   (N-of-M)       │
└─────────────┘  └──────────────┘  └──────────────────┘
        │               │
        └───────────────┘
           transfer_from
     (vesting pulls tokens on create_schedule)
```

### Contract responsibilities

| Contract | Role |
|----------|------|
| **Token** | SEP-0041 fungible token. Admin mints; beneficiaries receive via vesting. |
| **Vesting** | Holds locked tokens. Tracks schedules, computes linearly vested amount, transfers claimable balance to beneficiary on demand. |
| **Multisig** | N-of-M governance. Admin operations on Token + Vesting should be submitted as multisig proposals to prevent unilateral action. |

---

## Repository layout

```
soroban-vesting-platform/
├── contracts/
│   ├── token/         # SEP-0041 token (mint, burn, transfer, approve, pause)
│   ├── vesting/       # Linear vesting with cliff, claim, revoke, pause
│   └── multisig/      # N-of-M governance (submit, confirm, execute)
├── backend/
│   ├── src/
│   │   ├── index.js          # Express app entry point
│   │   ├── routes/           # /vesting /token /multisig /health
│   │   └── services/stellar.js  # Soroban RPC helper
│   └── package.json
├── frontend/
│   ├── index.html
│   ├── js/app.js
│   └── css/styles.css
├── scripts/
│   ├── deploy.sh       # Deploy a single contract
│   ├── fund.sh         # Fund via Friendbot
│   ├── invoke.sh       # Invoke a contract function
│   └── demo-testnet.sh # End-to-end testnet lifecycle demo
├── docs/
│   ├── architecture.md
│   └── testnet-demo.md   # Manual walkthrough
├── .github/workflows/ci.yml
├── .env.example
├── Cargo.toml            # Workspace root
├── Makefile
└── EMMY_CHANGELOG.md
```

---

## Prerequisites

| Tool | Install |
|------|---------|
| Rust stable + wasm32v1-none | `rustup target add wasm32v1-none` |
| stellar-cli | `curl -sSfL https://install.stellar.org \| sh` |
| Node.js ≥ 18 | [nodejs.org](https://nodejs.org) |
| jq (for demo scripts) | `apt install jq` / `brew install jq` |

---

## Quick start

```bash
# 1. Clone
git clone https://github.com/unajiogenyi05-hub/soroban-vesting-platform
cd soroban-vesting-platform

# 2. Configure
cp .env.example .env
# Edit .env — set SOURCE_SECRET_KEY and contract IDs after deployment

# 3. Build contracts
make build

# 4. Run tests
make test

# 5. Deploy to testnet
make deploy-testnet

# 6. Or run the end-to-end testnet demo
chmod +x scripts/demo-testnet.sh
./scripts/demo-testnet.sh

# 7. Start backend API
cd backend && npm install && npm start
```

---

## Testnet demo

See [docs/testnet-demo.md](docs/testnet-demo.md) for a step-by-step guide
that walks through:

1. Generating and funding accounts via Friendbot
2. Deploying both contracts
3. Minting tokens and creating a vesting schedule
4. Claiming vested tokens as the beneficiary
5. Verifying on Stellar Expert and Stellar Lab

---

## Vesting schedule lifecycle

```
create_schedule(params)
       │
       ▼
   [cliff period]
   nothing claimable
       │
       ▼  cliff expires
   [linear vesting]
   get_claimable() > 0   ──── claim() ──►  tokens transferred
       │
       ▼  total_duration reached
   [completed]
       │
       └── (or) revoke() called by admin
               │
               ▼
           unvested tokens returned to recipient
```

---

## Contract API reference

### Token

| Function | Auth | Description |
|----------|------|-------------|
| `initialize(admin, name, symbol, decimals, initial_supply)` | — | One-time init |
| `mint(to, amount)` | admin | Mint new tokens |
| `burn(from, amount)` | from | Burn own tokens |
| `transfer(from, to, amount)` | from | Transfer |
| `transfer_from(spender, from, to, amount)` | spender | Transfer via allowance |
| `approve(owner, spender, amount)` | owner | Set allowance |
| `pause()` / `unpause()` | admin | Emergency freeze |
| `transfer_admin(new_admin)` | admin + new_admin | Transfer admin role |

### Vesting

| Function | Auth | Description |
|----------|------|-------------|
| `initialize(admin)` | — | One-time init |
| `create_schedule(params)` | admin | Lock tokens, create schedule |
| `claim(schedule_id)` | beneficiary | Claim vested tokens |
| `revoke(schedule_id, recipient)` | admin | Return unvested tokens |
| `pause()` / `unpause()` | admin | Emergency freeze |
| `transfer_admin(new_admin)` | admin + new_admin | Transfer admin role |
| `get_schedule(id)` | — | Read schedule data |
| `get_claimable(id)` | — | How many tokens are claimable now |
| `get_beneficiary_schedules(beneficiary)` | — | List schedule IDs for an address |
| `schedule_count()` | — | Total number of schedules created |

### Multisig

| Function | Auth | Description |
|----------|------|-------------|
| `initialize(owners, threshold)` | — | One-time init |
| `submit(proposer, description)` | proposer (owner) | Create proposal |
| `confirm(owner, proposal_id)` | owner | Add confirmation |
| `revoke_confirmation(owner, proposal_id)` | owner | Remove confirmation |
| `execute(proposal_id)` | anyone | Execute if threshold met |
| `cancel(caller, proposal_id)` | proposer | Cancel proposal |

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

The backend routes are pre-wired. For live on-chain data, set these in `.env`
after deploying the contracts:

```
VESTING_CONTRACT_ID=<deployed vesting contract ID>
TOKEN_CONTRACT_ID=<deployed token contract ID>
MULTISIG_CONTRACT_ID=<deployed multisig contract ID>
SOURCE_SECRET_KEY=<Stellar secret key for transaction signing>
```

Without these values the API returns documented stub responses so the frontend
can be developed independently of a live deployment.

---

## Security

See [SECURITY.md](SECURITY.md) for the vulnerability disclosure policy and
security architecture notes.

---

## License

MIT
