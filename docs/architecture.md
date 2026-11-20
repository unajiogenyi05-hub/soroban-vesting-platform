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

## SDK upgrade procedure

The workspace pins a single version of `soroban-sdk` in the root
[`Cargo.toml`](../Cargo.toml):

```toml
[workspace.dependencies]
soroban-sdk = "27.0.6"
```

When a new SDK version is released, follow these five steps.

### Step 1 — Check the soroban-sdk CHANGELOG for breaking changes

Before bumping the version, review the release notes at
<https://github.com/stellar/rs-soroban-sdk/releases> for any breaking
changes. Pay close attention to:

- Constructor signature changes (`__constructor` argument order or types).
- Type changes that affect `#[contracttype]`, `#[contracterror]`, or
  `#[contractimpl]` attributes.
- Renamed or removed SDK macros (`symbol_short!`, `panic_with_error!`, etc.).
- Changes to the test harness (`testutils`, `mock_all_auths`, Env API).

### Step 2 — Update the version pin in Cargo.toml

Edit the workspace root `Cargo.toml` and change the version pin:

```toml
[workspace.dependencies]
soroban-sdk = "28.0.0"   # ← new version
```

All three contracts (`token`, `vesting`, `multisig`) share this single pin via
`soroban-sdk = { workspace = true }`, so one edit covers the entire workspace.

### Step 3 — Rebuild and retest

```bash
# Rebuild all contracts to catch compile-time breakage
stellar contract build

# Run the full test suite (contracts, and check for proptest failures)
cargo test --all

# Lint — treat all warnings as errors to catch newly deprecated items
cargo clippy --all --target wasm32v1-none -- -D warnings

# Formatting check
cargo fmt --all --check
```

Fix any compilation errors and update contract code to match the new API before
proceeding.

### Step 4 — Update the stellar-cli version in CI

The GitHub Actions workflow at [`.github/workflows/ci.yml`](../.github/workflows/ci.yml)
pins `stellar/stellar-cli` to a specific release:

```yaml
- uses: stellar/stellar-cli@v28.0.0
```

The Stellar CLI release that is compatible with a given SDK version is listed
on the [stellar-cli releases page](https://github.com/stellar/stellar-cli/releases)
and in the SDK release notes. Update the `@vX.Y.Z` tag to the compatible CLI
version and push the change alongside the SDK bump.

### Step 5 — Review contract READMEs after a breaking change

After a breaking SDK upgrade, verify that the contract READMEs remain
accurate:

- [`contracts/token/README.md`](../contracts/token/README.md) — error codes,
  function signatures, events.
- [`contracts/vesting/README.md`](../contracts/vesting/README.md) — schedule
  lifecycle, function signatures, error codes.
- [`contracts/multisig/README.md`](../contracts/multisig/README.md) —
  `ProposalAction` variants, function signatures.

Update any panic/error messages, function signatures, or type names that
changed in the new SDK. Then add an entry to [`CHANGELOG.md`](../CHANGELOG.md)
documenting the SDK bump and any contract changes made to accommodate it.

---

## Status

This is an unaudited, testnet-ready reference implementation. No real value
should be deployed without a professional security audit. See the
"Status and limitations" section in the README.
