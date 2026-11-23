# Disaster Recovery: Restoring Archived Contract Entries

Soroban ledger entries have a TTL (time-to-live). When a TTL reaches zero the
entry transitions to **archived** state. An archived persistent entry is
inaccessible but recoverable. An archived instance entry makes the entire
contract unresponsive — every invocation returns an error until the instance is
restored.

This guide covers how to detect archival, restore each entry type, verify the
restoration, and prevent recurrence.

---

## Background

See [docs/ttl.md](ttl.md) for the full TTL constants and what each contract
stores. The key facts:

| Entry type | Effect of archival | Recoverable? |
|------------|--------------------|:------------:|
| Instance storage | Contract becomes uncallable | Yes |
| Persistent entry (`Schedule`, `Proposal`, `Balance`, …) | That specific record is unreadable; other entries still work | Yes |

---

## 1. Detecting archived entries

### Instance storage archived

If the contract instance is archived, every `stellar contract invoke` call
returns a host error such as:

```
error: transaction simulation failed: MissingValue
```

or the RPC returns `contract not found` / empty result for a known contract ID.

You can check the live TTL with:

```bash
stellar contract read \
  --id <CONTRACT_ID> \
  --network testnet \
  --key ADMIN
```

An empty result — no value returned — means the instance storage key is
either archived or the contract was never deployed.

### Persistent entries archived

A specific entry (a vesting schedule, a multisig proposal, a token balance) is
archived when `stellar contract read` returns nothing for that key even though
it was written previously:

```bash
# Check a vesting schedule by its numeric ID (here, ID = 0)
stellar contract read \
  --id <VESTING_CONTRACT_ID> \
  --network testnet \
  --key '{"Schedule":0}'

# Check a token balance
stellar contract read \
  --id <TOKEN_CONTRACT_ID> \
  --network testnet \
  --key '{"Balance":"<ADDRESS>"}'
```

An empty or missing value when you expect data signals that the entry has
been archived.

---

## 2. Restoring instance storage

Run `stellar contract restore` targeting the contract instance. This submits a
Stellar transaction that includes a `RestoreFootprint` operation for the
contract's instance storage entry.

```bash
stellar contract restore \
  --id <CONTRACT_ID> \
  --source <SIGNING_ACCOUNT> \
  --network testnet
```

Replace `<CONTRACT_ID>` with the relevant contract address (from
[`deployments.json`](../deployments.json) or your own deployment notes) and
`<SIGNING_ACCOUNT>` with the name of the funded account in your stellar-cli
key store.

After restoration the contract is callable again, but the instance TTL starts
at the minimum restored value. Immediately extend it to the maximum to avoid
re-archival soon after:

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source <SIGNING_ACCOUNT> \
  --network testnet \
  -- extend_ttl
```

`extend_ttl()` is permissionless — any funded account can call it.

### Testnet contract IDs

| Contract | ID |
|----------|----|
| Token | `CBFVQMSOIMRWLZGKY4MMBASEJLKBLGBIYWT76MH47M5B2ODRDT264KKH` |
| Vesting | `CDBSJEAWCMVWQQDVPY6I7Y4TAHWXFLGE2MEYG2UIOC2A6SQMJ5V3NMJF` |

---

## 3. Restoring persistent entries

### Vesting contract — schedules and beneficiary lists

Each vesting schedule is stored under a `Schedule(id)` key; the index of
schedule IDs per beneficiary is stored under `BeneficiarySchedules(address)`.
Restore them individually by supplying the XDR-encoded key.

```bash
# Restore a schedule (replace 0 with the numeric schedule ID)
stellar contract restore \
  --id <VESTING_CONTRACT_ID> \
  --source <SIGNING_ACCOUNT> \
  --network testnet \
  --key '{"Schedule":0}'

# Restore a beneficiary schedule list
stellar contract restore \
  --id <VESTING_CONTRACT_ID> \
  --source <SIGNING_ACCOUNT> \
  --network testnet \
  --key '{"BeneficiarySchedules":"<BENEFICIARY_ADDRESS>"}'
```

After restoring a schedule entry, bump its TTL:

```bash
# Calling claim() or revoke() on the restored schedule will bump its TTL
# automatically. If you just want to extend without claiming, call extend_ttl():
stellar contract invoke \
  --id <VESTING_CONTRACT_ID> \
  --source <SIGNING_ACCOUNT> \
  --network testnet \
  -- extend_ttl
```

### Multisig contract — proposals and confirmations

```bash
# Restore a proposal (replace 0 with the numeric proposal ID)
stellar contract restore \
  --id <MULTISIG_CONTRACT_ID> \
  --source <SIGNING_ACCOUNT> \
  --network testnet \
  --key '{"Proposal":0}'

# Restore a confirmation entry (proposal_id=0, owner=<ADDRESS>)
stellar contract restore \
  --id <MULTISIG_CONTRACT_ID> \
  --source <SIGNING_ACCOUNT> \
  --network testnet \
  --key '{"Confirm":{"proposal_id":0,"owner":"<OWNER_ADDRESS>"}}'
```

### Token contract — balances and allowances

```bash
# Restore a balance entry
stellar contract restore \
  --id <TOKEN_CONTRACT_ID> \
  --source <SIGNING_ACCOUNT> \
  --network testnet \
  --key '{"Balance":"<ADDRESS>"}'

# Restore an allowance entry
stellar contract restore \
  --id <TOKEN_CONTRACT_ID> \
  --source <SIGNING_ACCOUNT> \
  --network testnet \
  --key '{"Allowance":{"owner":"<OWNER>","spender":"<SPENDER>"}}'
```

---

## 4. Verifying restoration

After running `stellar contract restore`, confirm the data is readable:

```bash
# Instance storage — should return the admin address
stellar contract read \
  --id <CONTRACT_ID> \
  --network testnet \
  --key ADMIN

# Vesting schedule — should return the schedule struct
stellar contract read \
  --id <VESTING_CONTRACT_ID> \
  --network testnet \
  --key '{"Schedule":0}'

# Token balance — should return a numeric balance
stellar contract read \
  --id <TOKEN_CONTRACT_ID> \
  --network testnet \
  --key '{"Balance":"<ADDRESS>"}'
```

A non-empty return value confirms the entry is live. You can also call a
read-only getter as an end-to-end check:

```bash
stellar contract invoke \
  --id <VESTING_CONTRACT_ID> \
  --source <SIGNING_ACCOUNT> \
  --network testnet \
  -- get_schedule --id 0
```

---

## 5. Preventing recurrence

The root cause of archival is that `extend_ttl()` was not called often enough.
The TTL constants are set to ~180 days, with a threshold of ~30 days (see
[docs/ttl.md](ttl.md)).

**Short-term fix:** Call `extend_ttl()` on all three contracts immediately after
restoration and again every ~150 days.

**Long-term fix:** Run an automated keep-alive job. See
[docs/offchain-keepalive.md](offchain-keepalive.md) for a cron script and
GitHub Actions workflow that calls `extend_ttl()` on a schedule.

If the contracts see regular on-chain activity (create_schedule, claim,
confirm, etc.) the instance TTL is bumped automatically on every call, so
archival only threatens idle deployments.

---

## Reference

- [docs/ttl.md](ttl.md) — TTL constants and storage key inventory
- [docs/offchain-keepalive.md](offchain-keepalive.md) — automated keep-alive setup
- [stellar contract restore (Stellar docs)](https://developers.stellar.org/docs/smart-contracts/guides/archival)
- [`deployments.json`](../deployments.json) — testnet contract IDs
