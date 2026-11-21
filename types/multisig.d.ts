/**
 * TypeScript type definitions for the Multisig API.
 *
 * These types match the JSON shapes returned by the backend multisig routes
 * (`backend/src/routes/multisig.js`) and reflect the on-chain data structures
 * defined in `contracts/multisig/src/lib.rs`.
 */

// ---------------------------------------------------------------------------
// Core domain types
// ---------------------------------------------------------------------------

/** Lifecycle status of a multisig proposal (matches the on-chain ProposalStatus enum). */
export type ProposalStatus = 'Pending' | 'Executed' | 'Cancelled';

/**
 * A cross-contract call specification inside a Call proposal action.
 * Mirrors the on-chain CallData struct.
 */
export interface CallData {
  /** Target contract address. */
  target: string;
  /** Function name to invoke. */
  function: string;
  /**
   * Encoded call arguments.
   * Off-chain consumers pass these as JSON-serialisable values; the backend
   * or stellar-cli encodes them to ScVal for the actual transaction.
   */
  args: unknown[];
}

/**
 * The action a proposal will perform when executed.
 * Mirrors the on-chain ProposalAction enum (tagged-union style).
 *
 * Exactly one of the action fields is present:
 *   - call           → cross-contract Call action
 *   - addOwner       → AddOwner action
 *   - removeOwner    → RemoveOwner action
 *   - updateThreshold → UpdateThreshold action
 */
export type ProposalAction =
  | { call: CallData }
  | { addOwner: string }
  | { removeOwner: string }
  | { updateThreshold: number };

/**
 * A fully typed proposal as returned by GET /api/multisig/proposal/:id.
 * Mirrors the on-chain ProposalData struct.
 */
export interface Proposal {
  /** Sequential proposal ID (u64 as number). */
  id: number;
  /** Address of the owner who submitted the proposal. */
  proposer: string;
  action: ProposalAction;
  /** Human-readable description supplied at submit time. */
  description: string;
  /** Number of owner confirmations received so far. */
  confirmationCount: number;
  status: ProposalStatus;
  /** Ledger timestamp (Unix seconds) at which the proposal was submitted. */
  createdAt: number;
}

/** Owner list returned by GET /api/multisig/owners. */
export interface OwnersResponse {
  owners: string[];
}

/** Confirmation threshold returned by GET /api/multisig/threshold. */
export interface ThresholdResponse {
  threshold: number;
}

/** Proposal count returned by GET /api/multisig/count. */
export interface ProposalCount {
  count: number;
}

// ---------------------------------------------------------------------------
// Mutating operation request bodies
// ---------------------------------------------------------------------------

/** Body for POST /api/multisig/proposal. */
export interface SubmitProposalParams {
  contractId?: string;
  /** Address of the owner submitting the proposal (must be in owner set). */
  proposer: string;
  action: ProposalAction;
  description: string;
}

/** Body for POST /api/multisig/confirm. */
export interface ConfirmParams {
  contractId?: string;
  /** Owner address adding the confirmation. */
  owner: string;
  proposalId: number;
}

/** Body for POST /api/multisig/revoke-confirm. */
export interface RevokeConfirmParams {
  contractId?: string;
  owner: string;
  proposalId: number;
}

/** Body for POST /api/multisig/execute. */
export interface ExecuteParams {
  contractId?: string;
  proposalId: number;
}

/** Body for POST /api/multisig/cancel. */
export interface CancelParams {
  contractId?: string;
  /** Address of the original proposer (only the proposer may cancel). */
  caller: string;
  proposalId: number;
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
