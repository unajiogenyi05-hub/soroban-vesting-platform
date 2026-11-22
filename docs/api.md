# REST API Reference

Reference for the Soroban Vesting Platform backend REST API. All shapes below
are taken directly from the route handlers in
[`backend/src/routes/`](../backend/src/routes/) — no fields are invented.

**Base URL:** `http://localhost:3001` (configurable via `PORT`)

**Route groups:**

| Group | Mount path | Source |
|-------|-----------|--------|
| Health | `/api/health` | [`backend/src/routes/health.js`](../backend/src/routes/health.js) |
| Vesting | `/api/vesting` | [`backend/src/routes/vesting.js`](../backend/src/routes/vesting.js) |
| Token | `/api/token` | [`backend/src/routes/token.js`](../backend/src/routes/token.js) |
| Multisig | `/api/multisig` | [`backend/src/routes/multisig.js`](../backend/src/routes/multisig.js) |

---

## Conventions

### Contract ID

Every route in the vesting, token, and multisig groups resolves a contract ID
with the same precedence:

1. `contractId` in the JSON request body
2. `contractId` as a query parameter (`?contractId=...`)
3. Environment variable — `VESTING_CONTRACT_ID`, `TOKEN_CONTRACT_ID`, or
   `MULTISIG_CONTRACT_ID` respectively

If none is available the request fails with:

```json
{ "error": "contractId is required" }
```

(HTTP 400.) `GET /api/health` and `GET /api/health/ready` do not require one.

### Success responses

- `GET` returns HTTP `200` with `{ "success": true, "data": { ... } }`.
- `POST` returns HTTP `202` with `{ "success": true, "message": "...", "data": { ... } }`. The backend does not submit transactions; the 202 means the transaction is ready for the caller to sign and submit.

### Error responses

| Status | Shape | Source |
|--------|-------|--------|
| `400` | `{ "error": "<message>" }` | Validation in the route handler |
| `404` | `{ "error": "Not found" }` | Unmatched route |
| `429` | `{ "error": "Too many requests, please slow down." }` | Rate limiter (120 requests/minute) |
| `500` | `{ "error": "<message>", "code": "INTERNAL_ERROR" }` | Central error handler |

Unhandled handler errors always produce `{ "error", "code" }` (`code` is
`"INTERNAL_ERROR"` for 500s, `"REQUEST_ERROR"` otherwise).

### Address validation

Address fields are validated with `validateStellarAddress()` (56-character
`G` ed25519 public keys). An invalid value returns HTTP 400 with
`{ "error": "invalid Stellar address: <field>" }`.

### Stub data

Unless the contract ID and signing environment variables are configured (see
[README — Backend API — Live data note](../README.md#backend-api--live-data-note)), responses
contain documented stub values (e.g. `count: 0`, `claimableAmount: 0`) and a
`.note` field telling you which environment variable to set.

---

## Health

### `GET /api/health`

Liveness check. Requires no contract ID.

**Response `200`**

| Field | Type | Description |
|-------|------|-------------|
| `status` | string | Always `"ok"` |
| `timestamp` | string | ISO-8601 timestamp |
| `network` | string | `process.env.STELLAR_NETWORKP or `"testnet"` |
| `rpc` | string | Soroban RPC URL in use |

**Example**

```bash
curl http://localhost:3001/api/health
```

```json
{
  "status": "ok",
  "timestamp": "2026-10-19T12:00:00.000Z",
  "network": "testnet",
  "rpc": "https://soroban-testnet.stellar.org"
}
```

### `GET /api/health/ready`

Readiness check. Reports which contract IDs are configured.

**Response `200`** when at least one contract ID is set, **`503`** otherwise.

| Field | Type | Description |
|-------|------|-------------|
| `ready` | boolean | `true` if any contract ID is configured |
| `contracts` | object | `{ vesting, token, multisig }` — each a boolean |

**Example (not ready, HTTP 503)**

```json
{ "ready": false, "contracts": { "vesting": false, "token": false, "multisig": false } }
```

---

## Vesting — `/api/vesting`

### `POST /api/vesting/schedule`

Create a vesting schedule.

**Request body**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `contractId` | string | see [Contract ID](#contract-id) | Vesting contract ID |
| `from` | string | yes | Valid Stellar address (funder/owner) |
| `beneficiary` | string | yes | Valid Stellar address |
| `tokenAddress` | string | yes | Valid Stellar address of the token contract |
| `totalAmount` | number | yes | Must be `> 0` |
| `startTime` | number | no | Unix seconds; defaults to now |
| `cliffDuration` | number | no | Seconds; defaults to `0`; must be `<= totalDuration` |
| `totalDuration` | number | yes\* | Total vesting duration in seconds |

\* `totalDuration` is not checked by the required-field guard but is echoed in
the response and used by the `cliffDuration <= totalDuration` check; always
send it.

**Success `202`**

```json
{
  "success": true,
  "message": "Schedule creation queued. Sign and submit the transaction.",
  "data": {
    "contractId": "C...",
    "from": "G...",
    "beneficiary": "G...",
    "tokenAddress": "C...",
    "totalAmount": 1000000,
    "startTime": 1760000000,
    "cliffDuration": 0,
    "totalDuration": 86400
  }
}
```

**Errors**

| Status | `error` |
|--------|---------|
| 400 | `Missing required fields` |
| 400 | `invalid Stellar address: from` / `beneficiary` / `tokenAddress` |
| 400 | `totalAmount must be positive` |
| 400 | `cliffDuration cannot exceed totalDuration` |
| 400 | `contractId is required` |

### `GET /api/vesting/schedule/:id`

Read a schedule by ID.

**Path params**

| Param | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | integer string | yes | Must be a positive integer (`/^\d+$/` and `> 0`) |

**Success `200`**

```json
{
  "success": true,
  "data": { "id": 1, "contractId": "C...", "note": "Connect VESTING_CONTRACT_ID and SOURCE_SECRET_KEY to read live data." }
}
```

**Errors:** `400` `scheduleId must be a positive integer`; `400` `contractId is required`.

### `GET /api/vesting/claimable/:id`

Claimable amount for a schedule.

**Path params:** `id` (string, required) — a positive integer in the `u64`
range.

When `VESTING_CONTRACT_ID` and `SOURCE_SECRET_KEY` are configured, the route
simulates `get_claimable(schedule_id)` against the contract and returns the
on-chain `i128` amount as a decimal string. If either variable is absent, it
returns the documented stub amount (`0`).

**Success `200` — live response**

```json
{
  "success": true,
  "data": { "scheduleId": 1, "claimableAmount": "42000", "contractId": "C..." }
}
```

Stub response:

```json
{
  "success": true,
  "data": { "scheduleId": 1, "claimableAmount": 0, "note": "Set VESTING_CONTRACT_ID to query live contract." }
}
```

**Errors:** `400` `scheduleId must be a positive u64 integer`.

### `POST /api/vesting/claim`

Claim vested tokens.

**Request body**

| Field | Type | Required |
|-------|------|----------|
| `contractId` | string | see [Contract ID](#contract-id) |
| `scheduleId` | number/string | yes |
| `beneficiary` | string (valid Stellar address) | yes |

**Success `202`**

```json
{
  "success": true,
  "message": "Claim transaction ready for signing.",
  "data": { "scheduleId": 1, "beneficiary": "G...", "contractId": "C..." }
}
```

**Errors:** `400` `scheduleId and beneficiary are required`;
`400` `invalid Stellar address: beneficiary`.

### `POST /api/vesting/revoke`

Revoke a schedule (admin).

**Request body**

| Field | Type | Required |
|-------|------|----------|
| `contractId` | string | see [Contract ID](#contract-id) |
| `scheduleId` | number/string | yes |
| `recipient` | string (valid Stellar address) | yes |

**Success `202`**

```json
{
  "success": true,
  "message": "Revoke transaction ready for signing.",
  "data": { "scheduleId": 1, "recipient": "G...", "contractId": "C..." }
}
```

**Errors:** `400` `scheduleId and recipient are required`;
`400` `invalid Stellar address: recipient`.

### `POST /api/vesting/pause`

Pause the vesting contract (admin). No body fields beyond `contractId`.

**Success `202`**

```json
{ "success": true, "message": "Pause transaction ready." }
```

### `POST /api/vesting/unpause`

Unpause the vesting contract (admin). No body fields beyond `contractId`.

**Success `200`**

```json
{ "success": true, "message": "Unpause transaction ready." }
```

### `GET /api/vesting/beneficiary/:addr`

List schedule IDs for a beneficiary.

**Path params:** `addr` (string, required) — echoed back, no format validation
in this handler.

**Success `200`**

```json
{
  "success": true,
  "data": { "beneficiary": "G...", "scheduleIds": [], "contractId": "C..." }
}
```

### `GET /api/vesting/count`

Total schedule count.

**Success `200`**

```json
{ "success": true, "data": { "count": 0, "contractId": "C..." } }
```

**Example**

```bash
curl "http://localhost:3001/api/vesting/count?contractId=C..."
```

```json
{ "success": true, "data": { "count": 0, "contractId": "C..." } }
```

---

## Token — `/api/token`

### `GET /api/token/info`

Token metadata.

**Success `200`**

| Field | Type |
|-------|------|
| `contractId` | string |
| `name` | string (`"Vesting Token"` stub) |
| `symbol` | string (`"VEST"` stub) |
| `decimals` | number (`7`) |
| `totalSupply` | number (`0` stub) |
| `note` | string |

```json
{
  "success": true,
  "data": {
    "contractId": "C...",
    "name": "Vesting Token",
    "symbol": "VEST",
    "decimals": 7,
    "totalSupply": 0,
    "note": "Set TOKEN_CONTRACT_ID to read live data."
  }
}
```

### `GET /api/token/balance/:addr`

Token balance for an address.

**Path params:** `addr` (string, required) — valid Stellar address. When
`TOKEN_CONTRACT_ID` **and** `SOURCE_SECRET_KEY` are set, the handler validates
the address (400 on failure) and simulates `balance(account)` on-chain;
otherwise it returns a stub without address validation.

**Success `200` (live)**

```json
{
  "success": true,
  "data": { "address": "G...", "balance": "1000000", "contractId": "C..." }
}
```

**Success `200` (stub)**

```json
{
  "success": true,
  "data": {
    "address": "G...",
    "balance": 0,
    "contractId": "C...",
    "note": "Set TOKEN_CONTRACT_ID and SOURCE_SECRET_KEY to read live balance."
  }
}
```

Note: `balance` is a **string** in live mode (bigint serialization) and a
**number** in stub mode.

**Errors:** `400` `invalid Stellar address: addr` (live mode only);
`400` `contractId is required`; `500` eif the RPC simulation fails.

### `GET /api/token/allowance`

Allowance between an owner and a spender.

**Query params**

| Param | Type | Required |
|-------|------|----------|
| `owner` | string | yes |
| `spender` | string | yes |
| `contractId` | string | see [Contract ID](#contract-id) |

**Success `200`**

```json
{
  "success": true,
  "data": { "owner": "G...", "spender": "G...", "allowance": 0, "contractId": "C..." }
}
```

**Errors:** `400` `owner and spender query params required`.

**Example**

```bash
curl "http://localhost:3001/api/token/allowance?owner=GAAA...&spender=GBBB..."
```

### `POST /api/token/mint`

Mint tokens (admin).

**Request body**

| Field | Type | Required |
|-------|------|----------|
| `contractId` | string | see [Contract ID](#contract-id) |
| `to` | string (valid Stellar address) | yes |
| `amount` | number, `> 0` | yes |

**Success `202`**

```json
{
  "success": true,
  "message": "Mint transaction ready.",
  "data": { "to": "G...", "amount": 1000, "contractId": "C..." }
}
```

**Errors:** `400` `to and amount required`; `400` `invalid Stellar address: to`;
`400` `amount must be positive`.

### `POST /api/token/burn`

Burn tokens.

**Request body**

| Field | Type | Required |
|-------|------|----------|
| `contractId` | string | see [Contract ID](#contract-id) |
| `from` | string (valid Stellar address) | yes |
| `amount` | number | yes |

**Success `202`**

```json
{
  "success": true,
  "message": "Burn transaction ready.",
  "data": { "from": "G...", "amount": 100, "contractId": "C..." }
}
```

**Errors:** `400` `from and amount required`; `400` `invalid Stellar address: from`.

### `POST /api/token/transfer`

Transfer tokens between accounts.

**Request body**

| Field | Type | Required |
|-------|------|----------|
| `contractId` | string | see [Contract ID](#contract-id) |
| `from` | string (valid Stellar address) | yes |
| `to` | string (valid Stellar address) | yes |
| `amount` | number, `> 0` | yes |

**Success `202`**

```json
{
  "success": true,
  "message": "Transfer transaction ready.",
  "data": { "from": "G...", "to": "G...", "amount": 500, "contractId": "C..." }
}
```

**Errors:** `400` `from, to and amount required`;
``400` `invalid Stellar address: from` / `to`; `400` `amount must be positive`.

---

## Multisig — `/api/multisig`

### `GET /api/multisig/proposal/:id`

Read a multisig proposal by ID.

**Path params:** `id` (string, required) — a positive integer.

**Success `200`**

```json
{
  "success": true,
  "data": { "id": 1, "contractId": "C...", "note": "Connect MULTISIG_CONTRACT_ID and SOURCE_SECRET_KEY to read live data." }
}
```

**Errors:** `400` `proposalId must be a positive integer`; `400` `contractId is required`.

### `GET /api/multisig/proposals/pending`

List the IDs (numeric strings) of all proposals whose status is `Pending on the
multisig contract. This maps to the contract view `get_pending_proposals() -> Vec<u64>`.
Proposals that have been executed or cancelled are excluded.

The contract implementation scans proposal IDs `from 1 to proposal_count()` in
persistent storage, so the cost is O.(n) in the number of proposals and no TTL
bumps are performed by the view.

**Success `200`**

```json
{
  "success": true,
  "data": { "proposalIds": [1, 3, 7], "contractId": "C..." }
}
```

Empty list when there are no pending proposals:

```json
{
  "success": true,
  "data": { "proposalIds": [], "contractId": "C..." }
}
```

**Errors:** `400` `contractId is required`; `500` if the RPC simulation fails.

### `POST /api/multisig/proposal`

Create a multisig proposal.

**Request body**

| Field | Type | Required |
|-------|------|----------|
| `contractId` | string | see [Contract ID](#contract-id) |
| `proposer` | string (valid Stellar address) | yes |
| `to` | string (valid Stellar address) | yes |
| `amount` | number, `> 0` | yes |
| `description` | string | no |

**Success `202`**

```json
{
  "success": true,
  "message": "Proposal creation queued. Sign and submit the transaction.",
  "data": { "proposer": "G...", "to": "G...", "amount": 1000, "contractId": "C..." }
}
```

**Errors:** `400` `Missing required fields`;
`400` `invalid Stellar address: proposer` / `to`; `400` `amount must be positive`.

### `POST /api/multisig/approve`

Approve a proposal.

**Request body**

| Field | Type | Required |
|-------|------|----------|
| `contractId` | string | see [Contract ID](#contract-id) |
| `proposalId` | number/string | yes |
| `signer` | string (valid Stellar address) | yes |

**Success `202`**

```json
{
  "success": true,
  "message": "Approval transaction ready for signing.",
  "data": { "proposalId": 1, "signer": "G...", "contractId": "C..." }
}
```

**Errors:** `400` `proposalId and signer are required`;
``400` `invalid Stellar address: signer`.

### `POST /api/multisig/execute`

Execute a proposal.

**Request body**

| Field | Type | Required |
|-------|------|----------|
| `contractId` | string | see [Contract ID](#contract-id) |
| `proposalId` | number/string | yes |

**Success `202`**

```json
{
  "success": true,
  "message": "Execution transaction ready for signing.",
  "data": { "proposalId": 1, "contractId": "C..." }
}
```

**Errors:** `400` `proposalId is required`.

### `GET /api/multisig/count`

Total proposal count.

**Success `200`**

```json
{ "success": true, "data": { "count": 0, "contractId": "C..." } }
```
