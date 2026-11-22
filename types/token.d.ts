/**
 * TypeScript type definitions for the Token API.
 *
 * These types match the JSON shapes returned by the backend token routes
 * (`backend/src/routes/token.js`) and reflect the on-chain SEP-0041 token
 * contract defined in `contracts/token/src/lib.rs`.
 *
 * Token amounts are serialised as strings to avoid JavaScript's 53-bit integer
 * limit on `i128` values from the contract.
 */

// ---------------------------------------------------------------------------
// Core domain types
// ---------------------------------------------------------------------------

/** Token metadata and total supply returned by GET /api/token/info. */
export interface TokenInfo {
  contractId: string;
  name: string;
  symbol: string;
  /** Number of decimal places (matches the on-chain `decimals()` value). */
  decimals: number;
  /** Total minted supply (i128 as string). */
  totalSupply: string;
}

/** Token balance for one account returned by GET /api/token/balance/:addr. */
export interface TokenBalance {
  address: string;
  /** Account balance (i128 as string). */
  balance: string;
}

/**
 * Allowance for a (owner, spender) pair returned by GET /api/token/allowance.
 * Mirrors the on-chain `allowance(owner, spender) -> i128`.
 */
export interface TokenAllowance {
  owner: string;
  spender: string;
  /** Approved spending amount (i128 as string). */
  allowance: string;
}

// ---------------------------------------------------------------------------
// Mutating operation request bodies
// ---------------------------------------------------------------------------

/** Body for POST /api/token/mint. */
export interface MintParams {
  contractId?: string;
  /** Recipient address. */
  to: string;
  /** Amount to mint (i128 as string). */
  amount: string;
}

/** Body for POST /api/token/burn. */
export interface BurnParams {
  contractId?: string;
  /** Address whose tokens are burned. */
  from: string;
  /** Amount to burn (i128 as string). */
  amount: string;
}

/** Body for POST /api/token/transfer. */
export interface TransferParams {
  contractId?: string;
  /** Sender address. */
  from: string;
  /** Recipient address. */
  to: string;
  /** Amount to transfer (i128 as string). */
  amount: string;
}

/** Body for POST /api/token/approve. */
export interface ApproveParams {
  contractId?: string;
  /** Token owner granting the allowance. */
  owner: string;
  /** Spender being granted the allowance. */
  spender: string;
  /** Allowance amount (i128 as string). */
  amount: string;
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
