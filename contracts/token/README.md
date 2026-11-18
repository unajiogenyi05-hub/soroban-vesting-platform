# Token Contract

SEP-0041 compatible fungible token with mint/burn, transfer, allowances,
pause/unpause, and metadata.

## Error codes

All contract errors are returned as typed [`TokenError`] variants via the
`#[contracterror]` attribute. Clients receive a stable `u32` error code rather
than matching on string messages.

| Code | Variant | Condition |
|------|---------|-----------|
| 1 | `DecimalsTooLarge` | `decimals > 18` in constructor |
| 2 | `AmountMustBePositive` | `amount ≤ 0` in mint, burn, transfer |
| 3 | `AmountCannotBeNegative` | `amount < 0` in approve |
| 4 | `InsufficientBalance` | balance < requested transfer or burn amount |
| 5 | `AllowanceExceeded` | spender allowance < requested amount |
| 6 | `TokenIsPaused` | contract is paused |
| 7 | `BalanceOverflow` | per-account balance would overflow `i128` |
| 8 | `TotalSupplyOverflow` | total supply would overflow `i128` |
| 9 | `NotInitialized` | required storage key not found |

## Roles

| Role | Description |
|------|-------------|
| **admin** | Mints, burns, pauses/unpauses, transfers admin role. Set at deploy time via constructor. |
| **holder** | Transfers their own tokens, approves spenders, burns their own tokens. |
| **spender** | Transfers tokens on behalf of a holder up to the approved allowance. |

## Functions

### Constructor

#### `__constructor(env, admin: Address, name: String, symbol: String, decimals: u32, initial_supply: i128)`

Called automatically at deploy time. Sets metadata and admin. If
`initial_supply > 0`, mints that amount to `admin`.

Authorization: none (constructor).

**Errors:** `TokenError::DecimalsTooLarge` (code 1) — `decimals > 18`

---

### `mint(env, to: Address, amount: i128)`

Mints `amount` new tokens to `to`. Increases `to`'s balance and total supply.
Uses `checked_add` for both per-account balance and total supply.

Authorization: admin.

**Errors:**
- `TokenError::AmountMustBePositive` (code 2) — `amount ≤ 0`
- `TokenError::TokenIsPaused` (code 6) — contract is paused
- `TokenError::BalanceOverflow` (code 7) — per-account balance would overflow
- `TokenError::TotalSupplyOverflow` (code 8) — total supply would overflow

**Events:** `("mint",)` → `(to: Address, amount: i128)`

---

### `burn(env, from: Address, amount: i128)`

Burns `amount` tokens from `from`. Decreases `from`'s balance and total supply.

Authorization: `from`.

**Errors:**
- `TokenError::AmountMustBePositive` (code 2) — `amount ≤ 0`
- `TokenError::InsufficientBalance` (code 4) — `from`'s balance < `amount`
- `TokenError::TokenIsPaused` (code 6) — contract is paused

**Events:** `("burn",)` → `(from: Address, amount: i128)`

---

### `transfer(env, from: Address, to: Address, amount: i128)`

Transfers `amount` tokens from `from` to `to`.

Authorization: `from`.

**Errors:**
- `TokenError::AmountMustBePositive` (code 2) — `amount ≤ 0`
- `TokenError::InsufficientBalance` (code 4) — `from`'s balance < `amount`
- `TokenError::TokenIsPaused` (code 6) — contract is paused

**Events:** `("transfer",)` → `(from: Address, to: Address, amount: i128)`

---

### `transfer_from(env, spender: Address, from: Address, to: Address, amount: i128)`

Transfers `amount` tokens from `from` to `to` using `spender`'s allowance.
Deducts `amount` from the allowance before transferring.

Authorization: `spender`.

**Errors:**
- `TokenError::AllowanceExceeded` (code 5) — `spender`'s allowance for `from` < `amount`
- `TokenError::InsufficientBalance` (code 4) — `from`'s balance < `amount`
- `TokenError::TokenIsPaused` (code 6) — contract is paused

**Events:** `("transfer",)` → `(from: Address, to: Address, amount: i128)`

---

### `approve(env, owner: Address, spender: Address, amount: i128)`

Sets `spender`'s allowance for `owner` to exactly `amount`. Setting `amount`
to 0 clears the allowance.

Authorization: `owner`.

**Errors:**
- `TokenError::AmountCannotBeNegative` (code 3) — `amount < 0`
- `TokenError::TokenIsPaused` (code 6) — contract is paused

**Events:** `("approve",)` → `(owner: Address, spender: Address, amount: i128)`

---

### `pause(env)`

Blocks `mint`, `burn`, `transfer`, `transfer_from`, and `approve`.
Does not affect `transfer_admin`.

Authorization: admin.

**Events:** `("pause",)` → `()`

---

### `unpause(env)`

Re-enables state-changing operations.

Authorization: admin.

**Events:** `("unpause",)` → `()`

---

### `transfer_admin(env, new_admin: Address)`

Transfers the admin role. Requires authorization from both the current admin
and `new_admin`. Not pause-gated.

Authorization: current admin **and** `new_admin`.

**Events:** `("admXfer",)` → `(old_admin: Address, new_admin: Address)`

---

### `name(env) -> String`

Returns the token name. Read-only; does not extend TTL.

**Errors:** `TokenError::NotInitialized` (code 9) — contract not initialized

---

### `symbol(env) -> String`

Returns the token symbol. Read-only; does not extend TTL.

**Errors:** `TokenError::NotInitialized` (code 9) — contract not initialized

---

### `decimals(env) -> u32`

Returns the token decimals. Read-only; does not extend TTL.

---

### `total_supply(env) -> i128`

Returns total tokens in circulation. Read-only; does not extend TTL.

---

### `balance(env, account: Address) -> i128`

Returns the balance of `account`. Returns 0 if no entry exists.
Read-only; does not extend TTL.

---

### `allowance(env, owner: Address, spender: Address) -> i128`

Returns `spender`'s allowance for `owner`. Returns 0 if no entry exists.
Read-only; does not extend TTL.

---

### `is_paused(env) -> bool`

Returns `true` if the contract is paused. Read-only; does not extend TTL.

---

### `admin(env) -> Address`

Returns the current admin address. Read-only; does not extend TTL.

**Errors:** `TokenError::NotInitialized` (code 9) — contract not initialized

---

### `extend_ttl(env)`

Permissionless. Extends the contract instance TTL to `INSTANCE_BUMP_LEDGERS`
(≈ 180 days). Call from an off-chain keep-alive bot.

Authorization: none.

---

## Storage Keys and TTL Behaviour

### Instance storage (archived → contract unusable)

| Key | Symbol | Type | Bumped on |
|-----|--------|------|-----------|
| `ADMIN` | `"ADMIN"` | `Address` | Every state-changing call |
| `PAUSED` | `"PAUSED"` | `bool` | Every state-changing call |
| `TOTAL` | `"TOTAL"` | `i128` | Every state-changing call |
| `NAME` | `"NAME"` | `String` | Every state-changing call |
| `SYMBOL_KEY` | `"SYMBOL"` | `String` | Every state-changing call |
| `DECIMALS` | `"DECIMALS"` | `u32` | Every state-changing call |

Pure getters do **not** bump instance TTL.

### Persistent storage (archived → entry unavailable, restorable)

| Key | Type | Bumped on |
|-----|------|-----------|
| `Balance(addr: Address)` | `i128` | `mint`, `burn`, `transfer`, `transfer_from` |
| `Allowance(owner: Address, spender: Address)` | `i128` | `approve`, `transfer_from` |

### TTL constants

| Constant | Value | Approx. |
|----------|-------|---------|
| `INSTANCE_BUMP_LEDGERS` | 3 110 400 | ~180 days |
| `INSTANCE_BUMP_THRESHOLD` | 518 400 | ~30 days |
| `PERSISTENT_BUMP_LEDGERS` | 3 110 400 | ~180 days |
| `PERSISTENT_BUMP_THRESHOLD` | 518 400 | ~30 days |

Values equal `max_entry_ttl` (protocol 29, stellar-core soroban-settings).
See [docs/ttl.md](../../docs/ttl.md).

---

## Panic conditions

The table below consolidates every `panic!` message in `src/lib.rs`. Use it
to map on-chain error strings to the function and condition that triggered
them.

| Function | Panic message | Condition |
|----------|---------------|-----------|
| `__constructor` | `"decimals too large"` | `decimals > 18` |
| `mint` | `"amount must be positive"` | `amount ≤ 0` |
| `mint` | `"token is paused"` | Contract is paused |
| `mint` | `"balance overflow"` | `to`'s balance + `amount` overflows `i128` |
| `mint` | `"total supply overflow"` | `total_supply` + `amount` overflows `i128` |
| `burn` | `"amount must be positive"` | `amount ≤ 0` |
| `burn` | `"token is paused"` | Contract is paused |
| `burn` | `"insufficient balance"` | `from`'s balance < `amount` |
| `transfer` | `"amount must be positive"` | `amount ≤ 0` |
| `transfer` | `"token is paused"` | Contract is paused |
| `transfer` | `"insufficient balance"` | `from`'s balance < `amount` |
| `transfer_from` | `"token is paused"` | Contract is paused |
| `transfer_from` | `"allowance exceeded"` | Spender's allowance for `from` < `amount` |
| `transfer_from` | `"amount must be positive"` | `amount ≤ 0` (enforced by inner `_transfer`) |
| `transfer_from` | `"insufficient balance"` | `from`'s balance < `amount` |
| `approve` | `"amount cannot be negative"` | `amount < 0` |
| `approve` | `"token is paused"` | Contract is paused |
| `transfer_admin` | `"not initialized"` | Admin key missing from instance storage |
| `admin` | `"not initialized"` | Admin key missing from instance storage |

> **Notes**
> - `"balance overflow"` and `"total supply overflow"` originate in the
>   internal `_mint` helper and surface through both `mint` and the
>   `initial_supply > 0` branch of `__constructor`.
> - `"amount must be positive"` in `transfer` / `transfer_from` is enforced
>   inside the internal `_transfer` helper.
> - `pause()` and `unpause()` have no panics beyond the implicit auth failure
>   if the caller is not the admin (handled by `require_auth()`).

---

## Events

| Topic | Data | Emitted by |
|-------|------|-----------|
| `("mint",)` | `(to, amount)` | `mint` |
| `("burn",)` | `(from, amount)` | `burn` |
| `("transfer",)` | `(from, to, amount)` | `transfer`, `transfer_from` |
| `("approve",)` | `(owner, spender, amount)` | `approve` |
| `("pause",)` | `()` | `pause` |
| `("unpause",)` | `()` | `unpause` |
| `("admXfer",)` | `(old_admin, new_admin)` | `transfer_admin` |
