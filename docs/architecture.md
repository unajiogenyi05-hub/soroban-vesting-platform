# Architecture

## Overview

```
┌─────────────────────────────────────────────────────┐
│                     Frontend (HTML/JS)               │
│  - Connect wallet (Freighter)                        │
│  - View / claim vesting schedules                    │
│  - Admin: create schedules, pause, revoke            │
└────────────────────┬────────────────────────────────┘
                     │ HTTP (REST)
┌────────────────────▼────────────────────────────────┐
│                  Backend (Node.js)                   │
│  - Express REST API                                  │
│  - Constructs + submits Stellar transactions         │
│  - Uses @stellar/stellar-sdk                         │
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
3. **Multisig → Any**: The multisig `execute` function is intended to gate admin operations on token and vesting contracts.

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
