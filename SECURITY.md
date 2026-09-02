# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | ✅ Yes    |

This platform targets **`soroban-sdk = "27.0.6"`** and **Node.js ≥ 18**. Security fixes will be applied to the latest minor version only.

> **Important:** This codebase has not been formally audited. The smart contracts handle real token transfers. Conduct a thorough security review and professional audit before deploying to mainnet with real value.

---

## Reporting a Vulnerability

Please **do not** open a public GitHub issue for security vulnerabilities.

Use GitHub's built-in private vulnerability reporting:
1. Go to the [Security tab](https://github.com/unajiogenyi05-hub/soroban-vesting-platform/security)
2. Click **"Report a vulnerability"**
3. Provide as much detail as possible: affected component (contract / backend / frontend), reproduction steps, and potential impact

### Response SLA

| Stage | Target |
|-------|--------|
| Acknowledgement | Within 48 hours |
| Initial assessment | Within 5 business days |
| Patch / mitigation | Within 14 days for critical issues |

---

## Scope

### Smart Contracts (Critical)

- **Authorization bypass** — `create_schedule`, `revoke`, `pause`, `transfer_admin` not properly checking `require_auth()`
- **Integer overflow in vesting arithmetic** — `vested_amount()` calculation overflowing on large `total_amount` or `elapsed` values
- **Storage key collision** — `DataKey` variants colliding in persistent storage
- **TTL expiry** — `VestingSchedule` persistent storage entries expiring before the vesting period ends, causing unrecoverable state
- **Re-entrancy via token callbacks** — cross-contract calls to the token client during `claim` or `revoke` that could be exploited
- **Cliff/duration validation bypass** — creating a schedule where `cliff_duration > total_duration` reaches the contract

### Backend (High)

- **Private key / secret key logging** — `signerKey` or `adminKey` fields appearing in Winston logs or error responses
- **Injection via unsanitized inputs** — malicious Stellar addresses or amounts passed through to SDK calls without validation
- **Rate limit bypass** — circumventing `express-rate-limit` to flood mutating endpoints
- **Dependency vulnerabilities** — known CVEs in `express`, `@stellar/stellar-sdk`, or other dependencies

### Frontend (Medium)

- **XSS via API response data** — schedule or account data rendered via `innerHTML` instead of `textContent`
- **Sensitive data in localStorage** — private keys or seed phrases accidentally persisted

### Out of scope

- Issues on testnet only with no mainnet impact
- Publicly visible Stellar blockchain data (already public by design)
- Issues in `soroban-sdk` or `@stellar/stellar-sdk` themselves — report to [Stellar security](https://stellar.org/security)
- UI/UX bugs without security impact

---

## Security Architecture Notes

- The backend **never** stores private keys — it constructs unsigned XDR for client-side signing
- The frontend communicates directly with the public Stellar Horizon API for read operations
- All token transfers are authorized on-chain via `require_auth()` — the backend cannot move tokens unilaterally
- Admin functions require explicit admin address authorization on every call

---

## Acknowledgements

Responsible disclosure is appreciated. Valid vulnerability reporters will be credited in release notes unless they prefer anonymity.
