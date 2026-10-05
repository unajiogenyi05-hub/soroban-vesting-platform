# Architecture

## Deployment model

All three contracts use Soroban constructors (`__constructor`) introduced in
soroban-sdk 27. Constructor arguments are supplied once at deploy time after
the `--` separator in `stellar contract deploy`. There is no separate
`initialize` call.

Because the constructor runs atomically with deployment, it is impossible for a
third party to front-run the initialization window that existed with the old
two-step `deploy → initialize` pattern (see also SECURITY.md).

```bash
# Token — admin, name, symbol, decimals, initial_supply
stellar contract deploy --wasm token.wasm --source admin --network testnet \
  -- --admin "$ADMIN" --name '"VEST"' --symbol '"VST"' --decimals 7 --initial_supply 0

# Vesting — admin
stellar contract deploy --wasm vesting.wasm --source admin --network testnet \
  -- --admin "$ADMIN"

# Multisig — owners (Vec<Address>), threshold (u32)
stellar contract deploy --wasm multisig.wasm --source admin --network testnet \
  -- --owners '["G...", "G...", "G..."]' --threshold 2
```

## Overview

```
┌─────────────────────────────────────────────────────┐
│                     Frontend (HTML/JS)               │
│  - View vesting schedules and claimable amounts      │
│  - Admin: create schedules, pause, revoke            │
│  - Calls the backend REST API for all operations     │
│  Note: no wallet integration; the frontend sends     │
│  parameters to the backend which returns stubs until │
│  contract IDs are configured in .env                 │
└────────────────────┬────────────────────────────────┘
                     │ HTTP (REST)
┌────────────────────▼────────────────────────────────┐
│                  Backend (Node.js)                   │
│  - Express REST API (input validation, rate limit)   │
│  - Uses @stellar/stellar-sdk for Soroban RPC calls   │
│  - Returns documented stubs when contract IDs are    │
│    not configured; real simulated reads when they are│
│  - Does NOT hold private keys or submit transactions │
└────────────────────┬────────────────────────────────┘
                     │ Soroban RPC
┌────────────────────▼────────────────────────────────┐
│              Stellar Network (Testnet/Mainnet)       │
│  ┌──────────────┐ ┌─────────────┐ ┌──────────────┐ │
│  │ Token        │ │  Vesting    │ │  Multisig    │ │
│  │ Contract     │ │  Contract   │ │  Contract    │ │
│  └──────────────┘ └─────────────┘ └──────────────┘ │
└─────────────────────────────────────────────────────┘
```

## Contract interactions

1. **Token → Vesting**: `create_schedule` calls `token.transfer` to pull funds into the vesting contract.
2. **Vesting → Token**: `claim` and `revoke` call `token.transfer` to release funds.
3. **Multisig → Any**: `execute()` dispatches the proposal action. `ProposalAction::Call` makes a real cross-contract call via `env.invoke_contract` to any target contract. `ProposalAction::AddOwner`, `RemoveOwner`, and `UpdateThreshold` are handled internally inside `execute()` — no cross-contract call is made for these variants. There are no public `add_owner`, `remove_owner`, or `update_threshold` entry points; every change to the owner set must go through a fully-confirmed proposal.

## Signing model

The backend does **not** hold private keys and does not submit transactions on-chain. It performs Soroban RPC simulations and returns unsigned XDR or stub responses. On-chain operations (create_schedule, claim, revoke, pause, etc.) must be signed and submitted by the caller using stellar-cli or a client-side wallet.

See [SECURITY.md](../SECURITY.md) for the full signing model.

## Vesting schedule lifecycle

```
create_schedule()
      │
      ▼
  [Active] ──── time passes ────► claim() repeatable
      │                                  │
      │                           [Completed] when fully claimed
      │
      └─── revoke() ──► [Revoked] (unvested returned, vested still claimable)
```

## Status

This is an unaudited, testnet-ready reference implementation. No real value
should be deployed without a professional security audit. See the
"Status and limitations" section in the README.
