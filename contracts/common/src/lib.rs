//! Shared constants for all Soroban Vesting Platform contracts.
//!
//! Import these instead of defining them locally so the values stay in
//! sync across the token, vesting, and multisig crates.

#![no_std]

// ─── TTL constants ────────────────────────────────────────────────────────────

/// Extend instance and persistent storage to ~180 days (ledgers of ~5 s each).
///
/// This equals the network's `max_entry_ttl` (3 110 400 ledgers on both
/// Testnet and Mainnet as of protocol 29, per stellar-core soroban-settings).
/// Values above `max_entry_ttl` are silently clamped by the host, so using
/// the exact maximum is both correct and maximally protective against archival.
pub const INSTANCE_BUMP_LEDGERS: u32 = 3_110_400;

/// Only extend instance storage when the remaining TTL drops below ~30 days.
pub const INSTANCE_BUMP_THRESHOLD: u32 = 518_400;

/// Extend persistent entries (schedules, balances, proposals) to ~180 days.
pub const PERSISTENT_BUMP_LEDGERS: u32 = 3_110_400;

/// Only extend persistent entries when the remaining TTL drops below ~30 days.
pub const PERSISTENT_BUMP_THRESHOLD: u32 = 518_400;
