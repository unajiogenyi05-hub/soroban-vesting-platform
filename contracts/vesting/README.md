# Vesting Contract

Linear token vesting with cliff support. Locks tokens for a beneficiary and
releases them linearly over a configurable schedule.

## Roles

| Role | Description |
|------|-------------|
| **admin** | Creates schedules, revokes unvested tokens, pauses/unpauses. Set at deploy time via constructor. |
| **beneficiary** | Claims vested tokens on their own schedule. |

## Functions

### Constructor

#### `__constructor(env, admin: Address)`

Called automatically at deploy time. Sets `admin`, clears the paused flag,
and initialises the schedule counter to 0. There is no separate `initialize`
step.

Authorization: none (constructor).

---

### `create_schedule(env, params: CreateScheduleParams) -> u64`

Creates a new vesting schedule. Transfers `params.total_amount` tokens from
`params.from` into the contract.

**`CreateScheduleParams` fields:**

| Field | Type | Description |
|-------|------|-------------|
| `from` | `Address` | Account that holds the tokens to be deposited. |
| `beneficiary` | `Address` | Account that may claim vested tokens. |
| `token_address` | `Address` | SEP-0041 token contract address. |
| `total_amount` | `i128` | Total tokens locked. Must be > 0. |
| `start_time` | `u64` | Vesting start timestamp (ledger seconds). |
| `cliff_duration` | `u64` | Seconds before any tokens vest. May be 0. |
| `total_duration` | `u64` | Total vesting period in seconds. Must be > 0; must be ≥ `cliff_duration`. |

Returns the new schedule ID (u64, increments from 1).

Authorization: admin.

**Panics (error codes):**
- `VestingError::AmountMustBePositive` (1) — `total_amount ≤ 0`
- `VestingError::DurationMustBePositive` (2) — `total_duration == 0`
- `VestingError::CliffExceedsDuration` (3) — `cliff_duration > total_duration`
- `VestingError::StartPlusCliffOverflow` (4) — `start_time + cliff_duration` overflows `u64`
- `VestingError::StartPlusDurationOverflow` (5) — `start_time + total_duration` overflows `u64`
- `VestingError::ContractIsPaused` (9) — contract is paused

**Events:** `("created",)` → `(id: u64, beneficiary: Address, total_amount: i128)`

---

### `claim(env, schedule_id: u64) -> i128`

Claims all vested-but-unclaimed tokens for a schedule. Transfers tokens from
the contract to the beneficiary. Marks the schedule `Completed` when
`claimed_amount >= total_amount`.

Returns the amount transferred.

Authorization: beneficiary of the schedule.

**Panics (error codes):**
- `VestingError::ScheduleNotFound` (6) — schedule ID does not exist
- `VestingError::ScheduleNotActive` (7) — schedule is Revoked or Completed
- `VestingError::NothingToClaim` (8) — vested amount equals already-claimed amount
- `VestingError::ContractIsPaused` (9) — contract is paused

**Events:** `("claimed",)` → `(schedule_id: u64, beneficiary: Address, amount: i128)`

---

### `revoke(env, schedule_id: u64, recipient: Address) -> i128`

Revokes an active schedule. State is updated before token transfers
(checks-effects-interactions pattern):

1. Computes vested and unvested amounts at the current timestamp.
2. Marks schedule `Revoked` and updates `claimed_amount`.
3. Transfers any vested-but-unclaimed tokens to the beneficiary.
4. Returns unvested tokens to `recipient` (typically the treasury).

Returns the unvested amount returned to `recipient`.

Authorization: admin.

**Panics (error codes):**
- `VestingError::ScheduleNotFound` (6) — schedule ID does not exist
- `VestingError::ScheduleNotActive` (7) — schedule is already Revoked or Completed

**Events:** `("revoked",)` → `(schedule_id: u64, unvested: i128)`

---

### `pause(env)`

Blocks `create_schedule` and `claim`. Does not affect `revoke` or
`transfer_admin`.

Authorization: admin.

**Events:** `("paused",)` → `()`

---

### `unpause(env)`

Re-enables `create_schedule` and `claim`.

Authorization: admin.

**Events:** `("unpaused",)` → `()`

---

### `transfer_admin(env, new_admin: Address)`

Transfers the admin role. Requires authorization from both the current admin
and the `new_admin`.  Reads the current admin address before overwriting it so
the event payload contains both the old and new admin addresses.

Authorization: current admin **and** `new_admin`.

**Events:** `("admXfer",)` → `(old_admin: Address, new_admin: Address)`

---

### `get_schedule(env, schedule_id: u64) -> VestingSchedule`

Returns the full schedule struct. Read-only; does not extend TTL.

**Panics:** `VestingError::ScheduleNotFound` (6)

---

### `get_claimable(env, schedule_id: u64) -> i128`

Returns the vested-but-unclaimed amount at the current ledger timestamp.
Returns 0 if nothing is claimable. Read-only; does not extend TTL.

**Panics:** `VestingError::ScheduleNotFound` (6)

---

### `get_beneficiary_schedules(env, beneficiary: Address) -> Vec<u64>`

Returns the list of schedule IDs for a beneficiary. Returns an empty vector
if no schedules exist. Read-only; does not extend TTL.

---

### `get_admin(env) -> Address`

Returns the current admin address. Read-only; does not extend TTL.

**Panics:** `"not initialized"` (storage key missing — contract not deployed correctly)

---

### `is_paused(env) -> bool`

Returns `true` if the contract is paused. Read-only; does not extend TTL.

---

### `schedule_count(env) -> u64`

Returns the total number of schedules created (the last assigned ID).
Read-only; does not extend TTL.

---

### `get_unvested_amount(env, schedule_id: u64) -> i128`

Returns `total_amount - vested_amount(now)`, clamped to `0`.  Represents the
amount that would be returned to the recipient if `revoke()` were called at
this instant.  Returns `0` once the schedule is fully vested.
Read-only; does not extend TTL.

**Panics:** `VestingError::ScheduleNotFound` (6) — schedule ID does not exist

---

### `extend_ttl(env)`

Permissionless. Extends the contract instance TTL to `INSTANCE_BUMP_LEDGERS`
(≈ 180 days). Call from an off-chain keep-alive bot to prevent instance
archival.

Authorization: none.

---

## Storage Keys and TTL Behaviour

### Instance storage (archived → contract unusable)

| Key | Symbol | Type | Bumped on |
|-----|--------|------|-----------|
| `ADMIN` | `"ADMIN"` | `Address` | Every state-changing call |
| `PAUSED` | `"PAUSED"` | `bool` | Every state-changing call |
| `SCHED_ID` | `"SCHED_ID"` | `u64` | Every state-changing call |

Every public entry point that writes state calls `bump_instance()`.  
Pure getters do **not** bump instance TTL.

### Persistent storage (archived → entry unavailable, restorable)

| Key | Type | Bumped on |
|-----|------|-----------|
| `Schedule(id: u64)` | `VestingSchedule` | `create_schedule`, `claim`, `revoke` |
| `BeneficiarySchedules(addr: Address)` | `Vec<u64>` | `create_schedule` |

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

## Events

| Topic | Data | Emitted by |
|-------|------|-----------|
| `("created",)` | `(id, beneficiary, total_amount)` | `create_schedule` |
| `("claimed",)` | `(schedule_id, beneficiary, amount)` | `claim` |
| `("revoked",)` | `(schedule_id, unvested)` | `revoke` |
| `("paused",)` | `()` | `pause` |
| `("unpaused",)` | `()` | `unpause` |
| `("admXfer",)` | `(old_admin, new_admin)` | `transfer_admin` |

---

## Error Codes

The contract uses a `#[contracterror]` enum (`VestingError`) so callers receive
a stable `u32` code instead of matching on string messages. All `panic_with_error!`
calls map to one of the codes below.

| Code | Variant | Triggered by |
|------|---------|--------------|
| 1 | `AmountMustBePositive` | `create_schedule` / `batch_create_schedule` — `total_amount ≤ 0` |
| 2 | `DurationMustBePositive` | `create_schedule` / `batch_create_schedule` — `total_duration == 0` |
| 3 | `CliffExceedsDuration` | `create_schedule` / `batch_create_schedule` — `cliff_duration > total_duration` |
| 4 | `StartPlusCliffOverflow` | `create_schedule` / `batch_create_schedule` — `start_time + cliff_duration` overflows `u64` |
| 5 | `StartPlusDurationOverflow` | `create_schedule` / `batch_create_schedule` — `start_time + total_duration` overflows `u64` |
| 6 | `ScheduleNotFound` | Any function that loads a schedule by ID — ID does not exist |
| 7 | `ScheduleNotActive` | `claim`, `revoke` — schedule status is not `Active` |
| 8 | `NothingToClaim` | `claim` — vested amount equals already-claimed amount |
| 9 | `ContractIsPaused` | `create_schedule`, `batch_create_schedule`, `claim` — contract is paused |
| 10 | `BatchEmpty` | `batch_create_schedule` — empty `params` vector |

---

## Vesting Formula

```
vested(now):
  if now < start_time + cliff_duration  →  0
  elif elapsed >= total_duration         →  total_amount
  else                                   →  elapsed * total_amount / total_duration
```

where `elapsed = now - start_time`.  
`saturating_mul` is used to prevent i128 overflow for large amounts × elapsed.

---

## Rounding behavior

`vested_amount` uses integer (floor) division:

```
elapsed * total_amount / total_duration
```

Integer division truncates toward zero, so the result is always rounded
**down** to the nearest whole token unit (stroop / base unit).

### Consequences

1. **Dust accumulates until schedule completion.** At any intermediate
   timestamp the calculated vested amount may be slightly less than the
   exact pro-rata share. The "missing" fraction of a token — the *dust* —
   is not lost; it accumulates in the contract and becomes claimable only
   when `elapsed >= total_duration`, at which point `vested_amount` returns
   `total_amount` exactly.

2. **Small totals amplify the effect.** The maximum dust at any point is
   `total_duration - 1` base units. For a schedule with `total_amount = 1`
   and `total_duration = 100`, the vested amount is `0` for the first 99
   seconds and jumps to `1` only at `elapsed == 100`.

3. **This is intentional.** The contract never over-pays; every token
   locked at creation is eventually paid out in full.

### Worked example

Schedule: `total_amount = 99`, `total_duration = 100 s`, no cliff.

| elapsed (s) | exact pro-rata | `vested_amount` (integer) | dust |
|-------------|---------------|--------------------------|------|
| 0 | 0.00 | 0 | 0 |
| 1 | 0.99 | 0 | 0 |
| 50 | 49.50 | 49 | 0.5 |
| 99 | 98.01 | 98 | 0.99 |
| 100 | 99.00 | 99 *(returns `total_amount`)* | 0 |

At `elapsed = 50` the beneficiary can claim `49` tokens, not `49.5`.
The remaining `0.5` (truncated to `0` in integer arithmetic) is held in
the contract until the schedule completes.

### Integrator notes

- Poll `get_claimable` to obtain the exact claimable amount at the current
  ledger timestamp before calling `claim`.
- Do not assume `get_claimable` grows by exactly `total_amount / total_duration`
  per second; some seconds it may grow by 0.
- The final claim (at or after `start_time + total_duration`) always returns
  whatever remains in the schedule, guaranteeing no token is permanently stuck.

---

## Data Types

### `VestingSchedule`

| Field | Type |
|-------|------|
| `id` | `u64` |
| `beneficiary` | `Address` |
| `token` | `Address` |
| `total_amount` | `i128` |
| `claimed_amount` | `i128` |
| `start_time` | `u64` |
| `cliff_duration` | `u64` |
| `total_duration` | `u64` |
| `status` | `ScheduleStatus` |

### `ScheduleStatus`

`Active` | `Revoked` | `Completed`
