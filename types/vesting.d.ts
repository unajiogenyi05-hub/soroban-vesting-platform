/**
 * TypeScript type definitions for the Vesting API.
 *
 * These types match the JSON shapes returned by the backend vesting routes
 * (`backend/src/routes/vesting.js`) and reflect the on-chain `VestingSchedule`
 * struct defined in `contracts/vesting/src/lib.rs`.
 *
 * Token amounts are serialised as strings to avoid JavaScript's 53-bit integer
 * limit on `i128` values from the contract.
 */

// ---------------------------------------------------------------------------
// Core domain types
// ---------------------------------------------------------------------------

/** Lifecycle status of a vesting schedule (matches the on-chain ScheduleStatus enum). */
export type ScheduleStatus = 'Active' | 'Revoked' | 'Completed';

/**
 * A single vesting schedule as returned by GET /api/vesting/schedule/:id.
 *
 * Mirrors VestingSchedule in contracts/vesting/src/lib.rs:
 *   - id               u64   → number
 *   - beneficiary      Address → string (Stellar account or contract ID)
 *   - token            Address → string
 *   - total_amount     i128  → string (avoid 53-bit JS limit)
 *   - claimed_amount   i128  → string
 *   - start_time       u64   → number (Unix timestamp in seconds)
 *   - cliff_duration   u64   → number (seconds)
 *   - total_duration   u64   → number (seconds)
 *   - status           ScheduleStatus
 */
export interface VestingSchedule {
  id: number;
  beneficiary: string;
  token: string;
  totalAmount: string;
  claimedAmount: string;
  startTime: number;
  cliffDuration: number;
  totalDuration: number;
  status: ScheduleStatus;
}

/** Claimable-amount response returned by GET /api/vesting/claimable/:id. */
export interface ClaimableAmount {
  scheduleId: number;
  claimableAmount: string;
}

/** Parameters for creating a vesting schedule (POST /api/vesting/schedule body). */
export interface CreateScheduleParams {
  contractId?: string;
  /** Address of the account funding the schedule. */
  from: string;
  /** Address that will receive vested tokens. */
  beneficiary: string;
  /** Token contract address. */
  tokenAddress: string;
  /** Total amount to lock (i128 as string). */
  totalAmount: string;
  /** Schedule start time (Unix timestamp, seconds). Defaults to now when omitted. */
  startTime?: number;
  /** Seconds before anything vests. 0 for no cliff. */
  cliffDuration?: number;
  /** Total seconds over which tokens vest linearly. */
  totalDuration: number;
}

/** Beneficiary schedule list returned by GET /api/vesting/beneficiary/:addr. */
export interface BeneficiarySchedules {
  beneficiary: string;
  scheduleIds: number[];
}

/** Schedule count returned by GET /api/vesting/count. */
export interface ScheduleCount {
  count: number;
}

// ---------------------------------------------------------------------------
// Shared API envelope
// ---------------------------------------------------------------------------

/**
 * Standard API response wrapper used by every backend route.
 *
 * On success `success` is `true` and `data` holds the result.
 * On failure `success` is `false` and `error` describes what went wrong.
 */
export interface ApiResponse<T> {
  success: boolean;
  data: T;
  error?: string;
}
