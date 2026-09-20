# EMMY_CHANGELOG.md

This file is the single source of truth for all changes made during the
Stellar Wave Program appeal audit. Entries are append-only and dated.

---

## 2026-09-20

### PR: feat/vesting-tests — Comprehensive vesting contract tests

**File changed:** `contracts/vesting/src/lib.rs` (test module only)

**What changed:**
Replaced the 4 thin unit tests with 15 comprehensive tests covering:
- Basic create + claim with partial and full vesting
- Zero-cliff immediate linear vesting
- Full vest after duration ends (schedule becomes Completed)
- Cliff blocks early claim, unlocks after cliff
- Revoke returns unvested tokens and marks schedule Revoked
- Claiming after revoke panics with "schedule is not active"
- Claiming before cliff panics with "nothing to claim"
- Pause blocks create_schedule
- Pause blocks claim
- Unpause restores claim functionality
- transfer_admin changes the admin address
- schedule_count increments correctly
- Multiple beneficiaries have independent schedules
- beneficiary_schedules accumulates multiple schedules per address
- Double revoke panics
- Double initialize panics
- is_paused reflects state correctly

**Why:** The original test suite exercised only the happy path. Edge cases
around cliff, revocation, pause, admin transfer, and multi-beneficiary
scenarios were untested — direct weaknesses in an evaluation for the
Stellar Wave Program.

---

### PR: feat/testnet-demo — Testnet demo script and walkthrough doc

**Files added:**
- `scripts/demo-testnet.sh` — End-to-end bash script: generates keypairs,
  funds via Friendbot, builds, deploys token + vesting contracts, mints,
  approves, creates a vesting schedule, and claims if cliff has passed.
- `docs/testnet-demo.md` — Step-by-step manual walkthrough including a DAO
  vesting parameter table (1-year cliff, 4-year total duration).

**Why:** No testnet demo existed. Evaluators for the Stellar Wave Program
cannot verify the platform works on a live network without one. The script
provides a zero-setup path to see the full lifecycle.

---

### PR: feat/vesting-readme — Improved README

**File changed:** `README.md`

**What changed:**
- Added ASCII architecture diagram showing frontend → backend → contracts flow
- Added "What this platform does" section with concrete DAO team vesting example
- Added complete contract API reference table for all three contracts
- Added vesting schedule lifecycle diagram
- Added repository layout section
- Added links to testnet demo docs
- Expanded Prerequisites table

**Why:** The original README described what was included but did not explain
the architecture or provide a concrete use case, making it harder for
evaluators to understand the platform's purpose and completeness.

---
