# Soroban Vesting Platform

A reference implementation, unaudited, testnet-ready token vesting platform
built on the [Stellar](https://stellar.org) network using
[Soroban](https://soroban.stellar.org) smart contracts.

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
├── CHANGELOG.md
├── SECURITY.md
└── CONTRIBUTING.md
```

---

## Prerequisites

| Tool | Install |
|------|---------|
| Rust stable + wasm32v1-none | `rustup target add wasm32v1-none` |
| stellar-cli | `curl -fsSL https://github.com/stellar/stellar-cli/raw/main/install.sh \| sh` (or `cargo binstall -y stellar-cli`) |
| Node.js ≥ 18 | [nodejs.org](https://nodejs.org) |
| jq (for demo scripts) | `apt install jq` / `brew install jq` |

---

## Quick start

```bash
# 1. Clone
git clone https://github.com/unajiogenyi05-hub/soroban-vesting-platform
cd soroban-vesting-platform

# 2. Build contracts (outputs to target/wasm32v1-none/release/)
stellar contract build

# 3. Run tests
cargo test --all

# 4. Deploy to testnet — follow "Deploy to Testnet" below,
#    or run the end-to-end demo:
chmod +x scripts/demo-testnet.sh
./scripts/demo-testnet.sh

# 5. Start backend API
cd backend && npm install && npm start
```

> **Note:** `make deploy-testnet` and `scripts/deploy.sh` are out of date
> (old build target, no constructor arguments). Use the `stellar contract`
> commands below or `docs/testnet-demo.md` instead.

---

## Deploy to Testnet

Works in a fresh GitHub Codespace. Constructor arguments are passed at deploy
time (there is no separate `initialize` call).

```bash
# Setup (once)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustup target add wasm32v1-none
curl -fsSL https://github.com/stellar/stellar-cli/raw/main/install.sh | sh
stellar --version

# Build
stellar contract build

# Create and fund a Testnet account (secret key stays in your Stellar CLI config — never commit it)
stellar keys generate my-admin --network testnet --fund
export ADMIN=$(stellar keys address my-admin)

# Deploy token
export TOKEN_ID=$(stellar contract deploy \
  --wasm target/wasm32v1-none/release/token.wasm \
  --source my-admin --network testnet \
  -- --admin "$ADMIN" --name '"My Token"' --symbol '"MTK"' --decimals 7 --initial_supply 0)

# Deploy vesting
export VESTING_ID=$(stellar contract deploy \
  --wasm target/wasm32v1-none/release/vesting.wasm \
  --source my-admin --network testnet \
  -- --admin "$ADMIN")
```

Optional multisig (single owner, threshold 1, for testing):

```bash
stellar contract deploy \
  --wasm target/wasm32v1-none/release/multisig.wasm \
  --source my-admin --network testnet \
  -- --owners "[\"$ADMIN\"]" --threshold 1
```

### Current Testnet deployment

| Contract | ID | Explorer |
|----------|----|----------|
| Token | `CBFVQMSOIMRWLZGKY4MMBASEJLKBLGBIYWT76MH47M5B2ODRDT264KKH` | [Stellar Expert](https://stellar.expert/explorer/testnet/contract/CBFVQMSOIMRWLZGKY4MMBASEJLKBLGBIYWT76MH47M5B2ODRDT264KKH) |
| Vesting | `CDBSJEAWCMVWQQDVPY6I7Y4TAHWXFLGE2MEYG2UIOC2A6SQMJ5V3NMJF` | [Stellar Expert](https://stellar.expert/explorer/testnet/contract/CDBSJEAWCMVWQQDVPY6I7Y4TAHWXFLGE2MEYG2UIOC2A6SQMJ5V3NMJF) |

The same IDs are recorded in [`deployments.json`](deployments.json). These are
Testnet-only contracts with no real value.

---

## Testnet demo

See [docs/testnet-demo.md](docs/testnet-demo.md) for a step-by-step guide
that walks through:

1. Generating and funding accounts via Friendbot
2. Deploying both contracts
3. Minting tokens and creating a vesting schedule
4. Claiming vested tokens as the beneficiary

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
| `__constructor(admin, name, symbol, decimals, initial_supply)` | — | Called once at deploy time; sets metadata and mints initial supply |
| `mint(to, amount)` | admin | Mint new tokens |
| `burn(from, amount)` | from | Burn own tokens |
| `transfer(from, to, amount)` | from | Transfer |
| `transfer_from(spender, from, to, amount)` | spender | Transfer via allowance |
| `approve(owner, spender, amount)` | owner | Set allowance |
| `pause()` / `unpause()` | admin | Emergency freeze |
| `transfer_admin(new_admin)` | admin + new_admin | Transfer admin role |
| `extend_ttl()` | anyone | Permissionless instance keep-alive |
| `name()` / `symbol()` / `decimals()` | — | Read-only metadata (no write footprint) |
| `total_supply()` / `balance(account)` / `allowance(owner, spender)` | — | Read-only queries (no write footprint) |
| `is_paused()` / `admin()` | — | Read-only state queries (no write footprint) |

### Vesting

| Function | Auth | Description |
|----------|------|-------------|
| `__constructor(admin)` | — | Called once at deploy time; sets admin |
| `create_schedule(params)` | admin | Lock tokens, create schedule |
| `claim(schedule_id)` | beneficiary | Claim vested tokens |
| `revoke(schedule_id, recipient)` | admin | Return unvested tokens |
| `pause()` / `unpause()` | admin | Emergency freeze |
| `transfer_admin(new_admin)` | admin + new_admin | Transfer admin role |
| `extend_ttl()` | anyone | Permissionless instance keep-alive |
| `get_schedule(id)` | — | Read-only: schedule data |
| `get_claimable(id)` | — | Read-only: claimable amount now |
| `get_beneficiary_schedules(beneficiary)` | — | Read-only: schedule IDs for an address |
| `schedule_count()` / `get_admin()` / `is_paused()` | — | Read-only state queries |

### Multisig

| Function | Auth | Description |
|----------|------|-------------|
| `__constructor(owners, threshold)` | — | Called once at deploy time; sets owner list and threshold |
| `submit(proposer, action, description)` | proposer (owner) | Create proposal with a `ProposalAction` |
| `confirm(owner, proposal_id)` | owner | Add confirmation |
| `revoke_confirmation(owner, proposal_id)` | owner | Remove confirmation |
| `execute(proposal_id)` | anyone | Execute if threshold met (permissionless) |
| `cancel(caller, proposal_id)` | proposer | Cancel proposal |
| `extend_ttl()` | anyone | Permissionless instance keep-alive |
| `get_proposal(id)` / `get_owners()` / `get_threshold()` | — | Read-only queries |
| `proposal_count()` / `has_confirmed(id, owner)` / `is_owner(address)` | — | Read-only queries |

`ProposalAction` variants:

| Variant | Effect |
|---------|--------|
| `Call(CallData)` | Dispatch `target.function(args)` via cross-contract call |
| `AddOwner(address)` | Add a new owner (handled internally, no external call) |
| `RemoveOwner(address)` | Remove an owner; clears their confirmations on pending proposals |
| `UpdateThreshold(u32)` | Change the required confirmation count |

There are no public `add_owner`, `remove_owner`, or `update_threshold` entry points.
Every change to the owner set or threshold must go through a fully-confirmed proposal.

---

## Multisig as vesting admin

The contracts support a flow where the multisig contract acts as the vesting
admin. An owner submits a proposal with a `ProposalAction::Call` targeting the
vesting contract (e.g. `create_schedule`, `pause`, `revoke`), other owners
confirm it to the threshold, and anyone calls `execute()`. The multisig contract
address is set as the vesting admin, so `admin.require_auth()` inside vesting is
satisfied when the call originates from the multisig.

Owner-set changes (add/remove owner, update threshold) also go through proposals
(`ProposalAction::AddOwner`, `RemoveOwner`, `UpdateThreshold`) — there are no
public entry points for these operations. This means no single key can alter the
owner set unilaterally.

`execute()` is permissionless: once the threshold is met, any caller can trigger
it. The owners expressed consent through their on-chain confirmations; the final
trigger needs no additional gate.

Unit tests in `contracts/vesting/src/lib.rs` cover:
- `test_multisig_admin_flow` — submit → confirm-to-threshold → execute pause()
- `test_multisig_below_threshold_panics` — execution below threshold panics
- `test_multisig_create_schedule` — end-to-end: multisig calls create_schedule
  with a real token, asserts token balances and beneficiary can claim

---

## Pause semantics

Each contract has a boolean pause flag set by the admin via `pause()` /
`unpause()`.  The flag is intentionally not a blanket lock — some functions
must remain available even during an emergency freeze.

### Vesting contract

| Function | Pause-gated? | Rationale |
|----------|:------------:|-----------|
| `create_schedule` | ✓ yes | No new funds should be locked while paused |
| `claim` | ✓ yes | Outbound transfers halted during an incident |
| `revoke` | **✗ no** | The admin must be able to recover unvested funds and pay out already-vested tokens even while the contract is paused, so that a paused contract is never a permanent fund lock |
| `transfer_admin` | **✗ no** | Admin hand-off must always be possible; blocking it could leave the contract stuck with no way to unpause |

### Token contract

| Function | Pause-gated? | Rationale |
|----------|:------------:|-----------|
| `mint` | ✓ yes | No new supply should be created during an incident |
| `burn` | ✓ yes | Consistent with mint pause |
| `transfer` / `transfer_from` | ✓ yes | All outbound token movement halted |
| `approve` | ✓ yes | No new spending allowances while paused |
| `transfer_admin` | **✗ no** | Same reason as vesting — admin transfer must always work |

The multisig contract has no pause mechanism; it acts as the vesting admin and
can call `pause()` / `unpause()` on the vesting contract through normal proposal
flow.

---

## Storage TTL constants

All three contracts share the same TTL strategy.  Instance storage (which holds
admin, owners, pause flag, and counters) is extended on every public call.
Persistent entries (schedules, balances, proposals) are extended on every write.

| Constant | Value | Approx. |
|----------|-------|---------|
| `INSTANCE_BUMP_LEDGERS` / `PERSISTENT_BUMP_LEDGERS` | 3 110 400 | ~180 days |
| `INSTANCE_BUMP_THRESHOLD` / `PERSISTENT_BUMP_THRESHOLD` | 518 400 | ~30 days |

See [docs/ttl.md](docs/ttl.md) for full details on what expires, the restore
procedure, and how to tune the constants for a production deployment.

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
after deploying the contracts (Testnet IDs are listed in
[Deploy to Testnet](#deploy-to-testnet) above):

```
VESTING_CONTRACT_ID=<deployed vesting contract ID>
TOKEN_CONTRACT_ID=<deployed token contract ID>
MULTISIG_CONTRACT_ID=<deployed multisig contract ID>
SOURCE_SECRET_KEY=<Stellar secret key for transaction signing>
```

Without these values the API returns documented stub responses so the frontend
can be developed independently of a live deployment.

---

## Status and limitations

| Item | Status |
|------|--------|
| Smart contracts | Unaudited. Do not deploy with real value without a professional audit. |
| Backend API | Returns documented stubs for all mutating operations until contract IDs and a signing setup are configured. Does not hold private keys. |
| Frontend | Calls the backend API. No wallet (Freighter) integration. |
| proptest | Not used. Property-based tests are manual parameterised tables. |
| Testnet deployment | Token and vesting contracts are deployed on Testnet (see [Deploy to Testnet](#deploy-to-testnet)). Multisig is not deployed. Run the steps above or `scripts/demo-testnet.sh` to deploy your own. |
| Multisig execute | `execute()` dispatches the proposal action. `ProposalAction::Call` makes a real cross-contract call via `env.invoke_contract`. Owner management actions (`AddOwner`, `RemoveOwner`, `UpdateThreshold`) are handled internally — no public entry points exist for them. |
| Mainnet | Not recommended. No audit, no mainnet deployment. |

---

## Security

See [SECURITY.md](SECURITY.md) for the vulnerability disclosure policy and
security architecture notes.

---

## License

MIT
