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

**Panics:**
- `"amount must be positive"` — `total_amount ≤ 0`
- `"duration must be > 0"` — `total_duration == 0`
- `"cliff exceeds duration"` — `cliff_duration > total_duration`
- `"start_time + cliff_duration overflows u64"` — arithmetic overflow check
- `"start_time + total_duration overflows u64"` — arithmetic overflow check
- `"contract is paused"` — contract is paused

**Events:** `("created",)` → `(id: u64, beneficiary: Address, total_amount: i128, token_address: Address)`

---

### `claim(env, schedule_id: u64) -> i128`

Claims all vested-but-unclaimed tokens for a schedule. Transfers tokens from
the contract to the beneficiary. Marks the schedule `Completed` when
`claimed_amount >= total_amount`.

Returns the amount transferred.

Authorization: beneficiary of the schedule.

**Panics:**
- `"schedule not found"` — schedule ID does not exist
- `"schedule is not active"` — schedule is Revoked or Completed
- `"nothing to claim"` — vested amount equals already-claimed amount
- `"contract is paused"` — contract is paused

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

**Panics:**
- `"schedule not found"` — schedule ID does not exist
- `"schedule is not active"` — schedule is already Revoked or Completed

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

### `transfer_beneficiary(env, schedule_id: u64, new_beneficiary: Address)`

Transfers a vesting schedule from its current beneficiary to `new_beneficiary`.
All vesting parameters and `claimed_amount` are preserved — the new beneficiary
inherits exactly where the schedule left off. After the call, the new
beneficiary appears in `get_beneficiary_schedules` and can call `claim`; the
old beneficiary no longer has the schedule in their index.

The schedule must be `Active`. The contract must not be paused.

Authorization: current beneficiary of the schedule.

**Panics:**
- `"schedule not found"` — schedule ID does not exist
- `"schedule is not active"` — schedule is Revoked or Completed
- `"contract is paused"` — contract is paused

**Events:** `("benXfer",)` → `(schedule_id: u64, old_beneficiary: Address, new_beneficiary: Address)`

---

### `get_schedule(env, schedule_id: u64) -> VestingSchedule`

Returns the full schedule struct. Read-only; does not extend TTL.

**Panics:** `"schedule not found"`

---

### `get_claimable(env, schedule_id: u64) -> i128`

Returns the vested-but-unclaimed amount at the current ledger timestamp.
Returns 0 if nothing is claimable. Read-only; does not extend TTL.

**Panics:** `"schedule not found"`

---

### `get_claimable_at(env, schedule_id: u64, timestamp: u64) -> i128`

Returns the projected claimable amount for a schedule at an arbitrary
`timestamp`. Useful for dashboards that want to show future vesting
projections without waiting for that ledger.

Computes `vested_amount(timestamp) − claimed_amount`, clamped to 0.
Read-only — does **not** bump the instance or persistent TTL.

**Panics:** `"schedule not found"` — schedule ID does not exist

---

### `get_beneficiary_schedules(env, beneficiary: Address) -> Vec<u64>`

Returns the list of schedule IDs for a beneficiary. Returns an empty vector
if no schedules exist. Read-only; does not extend TTL.

---

### `get_claimed_by_beneficiary(env, beneficiary: Address) -> i128`

Returns the sum of `claimed_amount` across all schedules owned by
`beneficiary`. Useful for dashboards that need a single aggregate without
iterating individual schedules off-chain. Returns 0 when the beneficiary has
no schedules. Read-only; does not extend TTL.

---

### `get_admin(env) -> Address`

Returns the current admin address. Read-only; does not extend TTL.

**Panics:** `"not initialized"`

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

**Panics:** `"schedule not found"` — schedule ID does not exist

---

### `schedule_exists(env, schedule_id: u64) -> bool`

Returns `true` if a schedule with the given ID exists in storage, `false`
otherwise.  Lets callers safely probe for schedule existence without catching a
panic, which is particularly useful when iterating over a range of IDs.
Read-only; does not extend TTL.

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
| `BeneficiarySchedules(addr: Address)` | `Vec<u64>` | `create_schedule`, `transfer_beneficiary` |

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
| `("created",)` | `(id, beneficiary, total_amount, token_address)` | `create_schedule` |
| `("claimed",)` | `(schedule_id, beneficiary, amount)` | `claim` |
| `("revoked",)` | `(schedule_id, unvested)` | `revoke` |
| `("paused",)` | `()` | `pause` |
| `("unpaused",)` | `()` | `unpause` |
| `("admXfer",)` | `(old_admin, new_admin)` | `transfer_admin` |

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

---

## Panic conditions

The tables below list every `panic_with_error!` call in the contract source.
Each row shows the function that raises it, the stable error code (from the
`VestingError` enum), and the condition that triggers it.

### `create_schedule` / `batch_create_schedule`

| Error code | `VestingError` variant | Triggering condition |
|:----------:|------------------------|----------------------|
| 9 | `ContractIsPaused` | Contract is paused (`pause()` was called and `unpause()` was not) |
| 1 | `AmountMustBePositive` | `params.total_amount <= 0` |
| 2 | `DurationMustBePositive` | `params.total_duration == 0` |
| 3 | `CliffExceedsDuration` | `params.cliff_duration > params.total_duration` |
| 4 | `StartPlusCliffOverflow` | `params.start_time + params.cliff_duration` overflows `u64` |
| 5 | `StartPlusDurationOverflow` | `params.start_time + params.total_duration` overflows `u64` |

`batch_create_schedule` additionally raises:

| Error code | `VestingError` variant | Triggering condition |
|:----------:|------------------------|----------------------|
| 10 | `BatchEmpty` | The `params` vector is empty |

### `claim`

| Error code | `VestingError` variant | Triggering condition |
|:----------:|------------------------|----------------------|
| 9 | `ContractIsPaused` | Contract is paused |
| 6 | `ScheduleNotFound` | No schedule exists with the given `schedule_id` |
| 7 | `ScheduleNotActive` | Schedule status is `Revoked` or `Completed` |
| 8 | `NothingToClaim` | `vested_amount(now) - claimed_amount <= 0` |

### `revoke`

| Error code | `VestingError` variant | Triggering condition |
|:----------:|------------------------|----------------------|
| 6 | `ScheduleNotFound` | No schedule exists with the given `schedule_id` |
| 7 | `ScheduleNotActive` | Schedule status is already `Revoked` or `Completed` |

### `transfer_beneficiary`

| Error code | `VestingError` variant | Triggering condition |
|:----------:|------------------------|----------------------|
| 9 | `ContractIsPaused` | Contract is paused |
| 6 | `ScheduleNotFound` | No schedule exists with the given `schedule_id` |
| 7 | `ScheduleNotActive` | Schedule status is `Revoked` or `Completed` |

### `pause` / `unpause` / `transfer_admin`

These functions panic with a Rust `expect` (not `VestingError`) when the
contract has not been initialised (i.e., the `ADMIN` storage key is missing):

| Panic message | Function | Triggering condition |
|---------------|----------|----------------------|
| `"not initialized"` | `transfer_admin`, `get_admin` | `ADMIN` key absent from instance storage |

Admin-only functions (`pause`, `unpause`, `transfer_admin`, `create_schedule`,
`revoke`) will also panic if the required authorization is not satisfied
(`require_auth` failure), which the Soroban host surfaces as an auth error
rather than a contract-level panic string.

### Read-only getters

| Error code | `VestingError` variant | Function | Triggering condition |
|:----------:|------------------------|----------|----------------------|
| 6 | `ScheduleNotFound` | `get_schedule`, `get_claimable`, `get_unvested_amount`, `get_claimed_by_beneficiary` | `schedule_id` does not exist |
