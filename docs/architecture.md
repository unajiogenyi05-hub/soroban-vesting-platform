# Architecture

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
3. **Multisig → Any**: The multisig `execute` function records a proposal as executed once threshold confirmations are reached. Dispatching the actual admin call to the token or vesting contract requires encoding the target function call in the proposal description and wiring the execute logic accordingly — this is not implemented in the current multisig contract.

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
