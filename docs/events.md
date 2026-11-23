# Contract events

Every event the Token, Vesting and Multisig contracts publish through
`env.events().publish(topics, data)`. Each table lists the topic tuple, the
data payload with its Soroban types, and the contract function that emits it.

All events use a single-element topic tuple holding a `Symbol`, so you filter
by the first topic and decode the data payload by event name.

## Token (`contracts/token/src/lib.rs`)

| Event | Topics | Data | Emitted by |
|-------|--------|------|------------|
| `mint` | `(Symbol("mint"),)` | `(to: Address, amount: i128)` | `mint` |
| `burn` | `(Symbol("burn"),)` | `(from: Address, amount: i128)` | `burn` |
| `transfer` | `(Symbol("transfer"),)` | `(from: Address, to: Address, amount: i128)` | `transfer`, `transfer_from` |
| `approve` | `(Symbol("approve"),)` | `(owner: Address, spender: Address, amount: i128)` | `approve` |
| `pause` | `(Symbol("pause"),)` | `()` | `pause` |
| `unpause` | `(Symbol("unpause"),)` | `()` | `unpause` |
| `admXfer` | `(Symbol("admXfer"),)` | `(old_admin: Address, new_admin: Address)` | `transfer_admin` |

`transfer_from` publishes the same `transfer` event as `transfer`. The
spender is not in the payload, so look at the invocation itself if you need it.

`token::transfer_admin` does emit an event. Older notes, including the issue
that asked for this page, say it emits nothing, but `admXfer` was added in
#158 and is covered by a unit test in `contracts/token/src/lib.rs`.

## Vesting (`contracts/vesting/src/lib.rs`)

| Event | Topics | Data | Emitted by |
|-------|--------|------|------------|
| `created` | `(Symbol("created"),)` | `(id: u64, beneficiary: Address, total_amount: i128, token_address: Address)` | `create_schedule`, `batch_create_schedule` (once per schedule) |
| `claimed` | `(Symbol("claimed"),)` | `(schedule_id: u64, beneficiary: Address, amount: i128)` | `claim` |
| `revoked` | `(Symbol("revoked"),)` | `(schedule_id: u64, unvested: i128)` | `revoke` |
| `cancelled` | `(Symbol("cancelled"),)` | `(schedule_id: u64, total_amount: i128)` | `cancel_schedule` |
| `paused` | `(Symbol("paused"),)` | `()` | `pause` |
| `unpaused` | `(Symbol("unpaused"),)` | `()` | `unpause` |
| `admXfer` | `(Symbol("admXfer"),)` | `(old_admin: Address, new_admin: Address)` | `transfer_admin` |
| `benXfer` | `(Symbol("benXfer"),)` | `(schedule_id: u64, old_beneficiary: Address, new_beneficiary: Address)` | `transfer_beneficiary` |

`created` is published from the private helper `create_schedule_internal`,
which both public create functions call.

In `revoked`, `unvested` is the amount returned to the `recipient`. Any vested
but unclaimed balance paid to the beneficiary during the revoke is not in the
payload. Note that the vesting `revoked` event and the multisig `revoked`
event share a name but have different payloads, so always filter by contract
ID as well as topic.

`cancelled` is emitted by `cancel_schedule`, which only succeeds when the
schedule has not yet started (`now < start_time`). The full `total_amount` is
returned to `recipient` in that call; the event records it for indexers.

## Multisig (`contracts/multisig/src/lib.rs`)

| Event | Topics | Data | Emitted by |
|-------|--------|------|------------|
| `submitted` | `(Symbol("submitted"),)` | `(id: u64, proposer: Address)` | `submit` |
| `confirmed` | `(Symbol("confirmed"),)` | `(proposal_id: u64, owner: Address, confirmation_count: u32)` | `confirm` |
| `revoked` | `(Symbol("revoked"),)` | `(proposal_id: u64, owner: Address)` | `revoke_confirmation` |
| `executed` | `(Symbol("executed"),)` | `(proposal_id: u64, proposer: Address)` | `execute` |
| `cancelled` | `(Symbol("cancelled"),)` | `(proposal_id: u64, caller: Address)` | `cancel` |
| `ownerAdd` | `(Symbol("ownerAdd"),)` | `new_owner: Address` | `execute` with an `AddOwner` action (via `internal_add_owner`) |
| `ownerRm` | `(Symbol("ownerRm"),)` | `owner: Address` | `execute` with a `RemoveOwner` action (via `internal_remove_owner`) |

`executed`, `cancelled`, `ownerAdd` and `ownerRm` data encoding notes:

- `executed` carries `(proposal_id: u64, proposer: Address)` as a tuple.
- `cancelled` carries `(proposal_id: u64, caller: Address)` as a tuple, where
  `caller` is the owner who cancelled (always the original proposer).
- `ownerAdd` and `ownerRm` carry a bare `Address` value, not a tuple, so
  decode the data as a single `Address`.

`execute` publishes `executed` before it dispatches the action, so an
`AddOwner` or `RemoveOwner` proposal produces two events in this order:
`executed`, then `ownerAdd` or `ownerRm`. `UpdateThreshold` and `Call` actions
publish only `executed`. A `Call` action may still cause the target contract
to emit its own events.

## The 9-character `symbol_short!` limit

Event names are built with `symbol_short!`, which only accepts up to 9
characters from `[a-zA-Z0-9_]`. That is why names are abbreviated, for example
`admXfer` (admin transfer), `benXfer` (beneficiary transfer), `ownerAdd` and
`ownerRm`. A new event name longer than 9 characters will not compile with
`symbol_short!`; it would need `Symbol::new(&env, "...")`, which costs more at
runtime. Indexers should match the exact short names above.

## Subscribing with Soroban RPC `getEvents`

Topics are matched as base64 XDR `ScVal` values. Encode a symbol with
stellar-cli:

```bash
echo '{"symbol":"claimed"}' | stellar xdr encode --type ScVal
# AAAADwAAAAdjbGFpbWVkAA==
```

Fetch `claimed` events from the vesting contract:

```bash
curl -s "$RPC_URL" -H 'Content-Type: application/json' -d '{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "getEvents",
  "params": {
    "startLedger": 1000000,
    "filters": [
      {
        "type": "contract",
        "contractIds": ["'"$VESTING_CONTRACT_ID"'"],
        "topics": [["AAAADwAAAAdjbGFpbWVkAA=="]]
      }
    ]
  }
}'
```

Use `"*"` as a topic segment to match any value, for example
`"topics": [["*"]]` for every event from the contract. Without the `"**"`
wildcard, only events with exactly that many topics are returned; all events
here have one topic. Keep polling with the `cursor` returned in each response
instead of `startLedger`.

The same filter with stellar-cli:

```bash
stellar events --network testnet --start-ledger 1000000 \
  --id "$VESTING_CONTRACT_ID" --topic "AAAADwAAAAdjbGFpbWVkAA==" --output json
```

RPC nodes keep only a limited window of recent ledgers, so long-running
indexers should store their cursor and poll regularly.
