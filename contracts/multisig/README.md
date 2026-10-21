# Multisig Contract

N-of-M multisignature governance primitive. Owners submit proposals, confirm
them, and execute when the confirmation threshold is met.

## Roles

| Role | Description |
|------|-------------|
| **owner** | Submits proposals, confirms/revokes confirmations, cancels own proposals. |
| **anyone** | May call `execute()` once threshold confirmations are reached. |

## Proposal lifecycle

```
submit() → confirm() × threshold → execute()
                ↑
         revoke_confirmation() (before execute)
         cancel() (proposer only, before execute)
```

## Owner management

`add_owner`, `remove_owner`, and `update_threshold` are **not** public entry
points. They are triggered exclusively via `ProposalAction` variants
(`AddOwner`, `RemoveOwner`, `UpdateThreshold`) executed through the normal
proposal flow. Every owner-set change requires M-of-N agreement.

## Functions

### Constructor

#### `__constructor(env, owners: Vec<Address>, threshold: u32)`

Called automatically at deploy time. Sets the owner list and confirmation
threshold. There is no separate `initialize` step.

Authorization: none (constructor).

**Panics:**
- `"need at least one owner"` — `owners` is empty
- `"invalid threshold"` — `threshold == 0` or `threshold > owners.len()`

---

### `submit(env, proposer: Address, action: ProposalAction, description: String) -> u64`

Submits a new proposal. `proposer` must be an owner. Returns the new proposal
ID (u64, increments from 1).

**`ProposalAction` variants:**

| Variant | Payload | Effect when executed |
|---------|---------|----------------------|
| `Call(CallData)` | `target: Address`, `function: Symbol`, `args: Vec<Val>` | Cross-contract call via `env.invoke_contract` |
| `AddOwner(Address)` | new owner address | Adds address to owner set |
| `RemoveOwner(Address)` | owner address | Removes address; decrements pending confirmation counts |
| `UpdateThreshold(u32)` | new threshold | Updates confirmation requirement |

Authorization: `proposer` (must be an owner).

**Panics:**
- `"not an owner"` — `proposer` is not in the owner set

**Events:** `("submitted",)` → `(id: u64, proposer: Address)`

---

### `confirm(env, owner: Address, proposal_id: u64)`

Adds `owner`'s confirmation to a pending proposal. Each owner may confirm
at most once.

Authorization: `owner` (must be an owner).

**Panics:**
- `"proposal not found"` — proposal ID does not exist
- `"proposal not pending"` — proposal is Executed or Cancelled
- `"already confirmed"` — `owner` has already confirmed this proposal
- `"not an owner"` — `owner` is not in the owner set

**Events:** `("confirmed",)` → `(proposal_id: u64, owner: Address, confirmation_count: u32)`

---

### `revoke_confirmation(env, owner: Address, proposal_id: u64)`

Removes `owner`'s confirmation from a pending proposal. Decrements
`confirmation_count`.

Authorization: `owner` (must be an owner).

**Panics:**
- `"proposal not found"` — proposal ID does not exist
- `"proposal not pending"` — proposal is Executed or Cancelled
- `"not confirmed"` — `owner` has not confirmed this proposal
- `"not an owner"` — `owner` is not in the owner set

**Events:** `("revoked",)` → `(proposal_id: u64, owner: Address)`

---

### `execute(env, proposal_id: u64)`

Executes a proposal once `confirmation_count >= threshold`. Permissionless —
any caller may trigger execution once threshold is met.

Marks the proposal `Executed` before dispatching the action
(checks-effects-interactions pattern). Internal actions (`AddOwner`,
`RemoveOwner`, `UpdateThreshold`) are handled directly without a
cross-contract call. `Call` actions dispatch to the target contract via
`env.invoke_contract`. A `Call` targeting the multisig contract itself will
trap (Soroban disallows re-entrancy).

When `RemoveOwner` executes, the contract scans all pending proposals and
removes the departed owner's confirmations, decrementing `confirmation_count`
for each affected proposal.

Authorization: none (permissionless).

**Panics:**
- `"proposal not found"` — proposal ID does not exist
- `"proposal not pending"` — proposal is already Executed or Cancelled
- `"not enough confirmations"` — `confirmation_count < threshold`
- `"cannot remove: would breach threshold"` — `RemoveOwner` would leave fewer owners than required
- `"already an owner"` — `AddOwner` with an existing owner address
- `"not an owner"` — `RemoveOwner` with an address not in the owner set
- `"invalid threshold"` — `UpdateThreshold` with 0 or > owner count

**Events:**
- `("executed",)` → `proposal_id: u64`
- `("ownerAdd",)` → `new_owner: Address` (if `AddOwner`)
- `("ownerRm",)` → `owner: Address` (if `RemoveOwner`)

---

### `cancel(env, caller: Address, proposal_id: u64)`

Cancels a pending proposal. Only the original proposer may cancel.

Authorization: `caller` (must be an owner and the proposer).

**Panics:**
- `"proposal not found"` — proposal ID does not exist
- `"proposal not pending"` — proposal is already Executed or Cancelled
- `"only proposer can cancel"` — `caller` is not the proposer
- `"not an owner"` — `caller` is not in the owner set

**Events:** `("cancelled",)` → `proposal_id: u64`

---

### `get_proposal(env, proposal_id: u64) -> ProposalData`

Returns the full proposal struct. Read-only; does not extend TTL.

**Panics:** `"not found"`

---

### `get_owners(env) -> Vec<Address>`

Returns the current owner list. Read-only; does not extend TTL.

---

### `get_threshold(env) -> u32`

Returns the current confirmation threshold. Read-only; does not extend TTL.

---

### `proposal_count(env) -> u64`

Returns the total number of proposals submitted (the last assigned ID).
Read-only; does not extend TTL.

---

### `has_confirmed(env, proposal_id: u64, owner: Address) -> bool`

Returns `true` if `owner` has confirmed `proposal_id`. Read-only; does not
extend TTL.

---

### `get_confirmations(env, proposal_id: u64) -> Vec<Address>`

Returns the owners who have confirmed `proposal_id`, in owner-list order.
Empty when there are no confirmations. Read-only; does not extend TTL.

---

### `is_owner(env, address: Address) -> bool`

Returns `true` if `address` is in the current owner set. Read-only; does not
extend TTL.

---

### `is_executable(env, proposal_id: u64) -> bool`

Returns `true` if the proposal identified by `proposal_id` is ready to
execute: its status is `Pending` **and** `confirmation_count >= threshold`.

Encapsulates the two-step check (`get_proposal` + `get_threshold`) that
callers would otherwise need to perform manually. Useful for UIs and off-chain
bots that poll proposal readiness without triggering execution.

Read-only — does not extend TTL or write any storage.

**Panics:** `"proposal not found"` — `proposal_id` does not exist in storage.

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
| `OWNERS` | `"OWNERS"` | `Vec<Address>` | Every state-changing call |
| `THRESHOLD` | `"THRESH"` | `u32` | Every state-changing call |
| `PROP_COUNT` | `"PROPCOUNT"` | `u64` | Every state-changing call |

Pure getters do **not** bump instance TTL.

### Persistent storage (archived → entry unavailable, restorable)

| Key | Type | Bumped on |
|-----|------|-----------|
| `Proposal(id: u64)` | `ProposalData` | `submit`, `confirm`, `revoke_confirmation`, `execute`, `cancel` |
| `Confirm(id: u64, addr: Address)` | `bool` | `confirm` (set); `revoke_confirmation`, `RemoveOwner` (removed) |

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
| `("submitted",)` | `(id, proposer)` | `submit` |
| `("confirmed",)` | `(proposal_id, owner, confirmation_count)` | `confirm` |
| `("revoked",)` | `(proposal_id, owner)` | `revoke_confirmation` |
| `("executed",)` | `proposal_id` | `execute` |
| `("cancelled",)` | `proposal_id` | `cancel` |
| `("ownerAdd",)` | `new_owner` | `execute` (AddOwner action) |
| `("ownerRm",)` | `owner` | `execute` (RemoveOwner action) |

---

## Panic conditions reference

A consolidated table of every panic message the contract can produce, the
function(s) that may produce it, and the condition that triggers it.

| Panic message | Function(s) | Condition |
|---------------|-------------|-----------|
| `"need at least one owner"` | `__constructor` | `owners` vector is empty |
| `"invalid threshold"` | `__constructor`, `execute` (`UpdateThreshold`) | `threshold == 0` or `threshold > owners.len()` |
| `"not an owner"` | `submit`, `confirm`, `revoke_confirmation`, `cancel`, `execute` (`RemoveOwner`) | Caller / specified address is not in the current owner set |
| `"proposal not found"` | `confirm`, `revoke_confirmation`, `execute`, `cancel` | `proposal_id` key does not exist in persistent storage |
| `"not found"` | `get_proposal` | `proposal_id` key does not exist in persistent storage |
| `"proposal not pending"` | `confirm`, `revoke_confirmation`, `execute`, `cancel` | Proposal status is `Executed` or `Cancelled` |
| `"already confirmed"` | `confirm` | `owner` has already recorded a confirmation for this proposal |
| `"not confirmed"` | `revoke_confirmation` | `owner` has not yet confirmed this proposal |
| `"not enough confirmations"` | `execute` | `confirmation_count < threshold` |
| `"only proposer can cancel"` | `cancel` | `caller` is not the original `proposer` of the proposal |
| `"already an owner"` | `execute` (`AddOwner` action) | The address to add is already in the owner set |
| `"cannot remove: would breach threshold"` | `execute` (`RemoveOwner` action) | `owners.len() <= threshold`; removing one more owner would make the threshold unsatisfiable |

### `__constructor` panics

| Condition | Message |
|-----------|---------|
| `owners` is empty | `"need at least one owner"` |
| `threshold == 0` or `threshold > owners.len()` | `"invalid threshold"` |

### `submit` panics

| Condition | Message |
|-----------|---------|
| `proposer` is not in the owner set | `"not an owner"` |

### `confirm` panics

| Condition | Message |
|-----------|---------|
| `proposal_id` not in storage | `"proposal not found"` |
| Proposal is not `Pending` | `"proposal not pending"` |
| `owner` already confirmed | `"already confirmed"` |
| `owner` not in owner set | `"not an owner"` |

### `revoke_confirmation` panics

| Condition | Message |
|-----------|---------|
| `proposal_id` not in storage | `"proposal not found"` |
| Proposal is not `Pending` | `"proposal not pending"` |
| `owner` has not confirmed | `"not confirmed"` |
| `owner` not in owner set | `"not an owner"` |

### `execute` panics

| Condition | Message |
|-----------|---------|
| `proposal_id` not in storage | `"proposal not found"` |
| Proposal is not `Pending` | `"proposal not pending"` |
| `confirmation_count < threshold` | `"not enough confirmations"` |
| `AddOwner`: address already an owner | `"already an owner"` |
| `RemoveOwner`: address not in owner set | `"not an owner"` |
| `RemoveOwner`: would leave fewer owners than threshold | `"cannot remove: would breach threshold"` |
| `UpdateThreshold`: new value is 0 or > owner count | `"invalid threshold"` |

### `cancel` panics

| Condition | Message |
|-----------|---------|
| `proposal_id` not in storage | `"proposal not found"` |
| Proposal is not `Pending` | `"proposal not pending"` |
| `caller` is not the original proposer | `"only proposer can cancel"` |
| `caller` not in owner set | `"not an owner"` |

### `get_proposal` panics

| Condition | Message |
|-----------|---------|
| `proposal_id` not in storage | `"not found"` |

---

## Data Types

### `ProposalData`

| Field | Type |
|-------|------|
| `id` | `u64` |
| `proposer` | `Address` |
| `action` | `ProposalAction` |
| `description` | `String` |
| `confirmation_count` | `u32` |
| `status` | `ProposalStatus` |
| `created_at` | `u64` (ledger timestamp) |

### `ProposalStatus`

`Pending` | `Executed` | `Cancelled`

### `CallData`

| Field | Type |
|-------|------|
| `target` | `Address` |
| `function` | `Symbol` |
| `args` | `Vec<Val>` |
