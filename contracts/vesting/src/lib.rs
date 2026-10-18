//! Soroban Token Vesting Contract
//!
//! Locks tokens for a beneficiary and releases them linearly over a schedule.
//!
//! # Roles
//! - **admin**      — can create schedules, revoke unvested tokens, pause/unpause
//! - **beneficiary** — claims vested tokens on their schedule
//!
//! # Schedule lifecycle
//! ```text
//!  created → [cliff] → linear release → fully_vested
//!                                   ↑ revoke stops here
//! ```

#![no_std]
#![allow(deprecated)]
#![allow(clippy::needless_borrows_for_generic_args)]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, symbol_short, token,
    Address, Env, Symbol, Vec,
};

// ─── Storage keys ───────────────────────────────────────────────────────────

const ADMIN: Symbol = symbol_short!("ADMIN");
const PAUSED: Symbol = symbol_short!("PAUSED");
const SCHED_ID: Symbol = symbol_short!("SCHED_ID");

/// Extend instance and persistent storage to ~180 days (ledgers of ~5 s each).
/// This equals the network's `max_entry_ttl` (3 110 400 ledgers on both
/// testnet and mainnet as of protocol 29, per stellar-core soroban-settings).
/// Values above `max_entry_ttl` are silently clamped by the host, so using
/// the exact maximum is both correct and maximally protective against archival.
pub const INSTANCE_BUMP_LEDGERS: u32 = 3_110_400;
/// Only extend when the remaining TTL drops below ~30 days.
pub const INSTANCE_BUMP_THRESHOLD: u32 = 518_400;
/// Persistent Schedule / BeneficiarySchedules entries use the same window.
pub const PERSISTENT_BUMP_LEDGERS: u32 = 3_110_400;
pub const PERSISTENT_BUMP_THRESHOLD: u32 = 518_400;

// ─── Error type ─────────────────────────────────────────────────────────────

/// Typed error codes for the vesting contract.
///
/// Using `#[contracterror]` maps each variant to a stable `u32` code that
/// Soroban clients can match on without string parsing.
#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u32)]
pub enum VestingError {
    /// `total_amount` is zero or negative.
    AmountMustBePositive = 1,
    /// `total_duration` is zero.
    DurationMustBePositive = 2,
    /// `cliff_duration` is greater than `total_duration`.
    CliffExceedsDuration = 3,
    /// `start_time + cliff_duration` would overflow `u64`.
    StartPlusCliffOverflow = 4,
    /// `start_time + total_duration` would overflow `u64`.
    StartPlusDurationOverflow = 5,
    /// A schedule with the given ID does not exist in storage.
    ScheduleNotFound = 6,
    /// The schedule's status is not `Active` (already Revoked or Completed).
    ScheduleNotActive = 7,
    /// The vested amount equals the already-claimed amount; nothing to transfer.
    NothingToClaim = 8,
    /// The contract is paused; `create_schedule` and `claim` are blocked.
    ContractIsPaused = 9,
    /// `batch_create_schedule` was called with an empty params vector.
    BatchEmpty = 10,
}

// ─── Data types ────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum ScheduleStatus {
    Active,
    Revoked,
    Completed,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct VestingSchedule {
    pub id: u64,
    pub beneficiary: Address,
    pub token: Address,
    pub total_amount: i128,
    pub claimed_amount: i128,
    pub start_time: u64,
    pub cliff_duration: u64,
    pub total_duration: u64,
    pub status: ScheduleStatus,
}

/// Parameters for creating a vesting schedule (avoids >7 arg clippy lint).
#[contracttype]
#[derive(Clone, Debug)]
pub struct CreateScheduleParams {
    pub from: Address,
    pub beneficiary: Address,
    pub token_address: Address,
    pub total_amount: i128,
    pub start_time: u64,
    pub cliff_duration: u64,
    pub total_duration: u64,
}

#[contracttype]
pub enum DataKey {
    Schedule(u64),
    BeneficiarySchedules(Address),
}

// ─── Events ──────────────────────────────────────────────────────────────────

const EVT_CREATED: Symbol = symbol_short!("created");
const EVT_CLAIMED: Symbol = symbol_short!("claimed");
const EVT_REVOKED: Symbol = symbol_short!("revoked");
const EVT_PAUSED: Symbol = symbol_short!("paused");
const EVT_UNPAUSED: Symbol = symbol_short!("unpaused");
const EVT_ADM_XFER: Symbol = symbol_short!("admXfer");
const EVT_BEN_XFER: Symbol = symbol_short!("benXfer");

// ─── Contract ──────────────────────────────────────────────────────────────

#[contract]
pub struct VestingContract;

#[contractimpl]
impl VestingContract {
    // ── Constructor ─────────────────────────────────────────────────────────

    /// Constructor: set the admin address at deploy time.
    ///
    /// Called automatically at deploy time — there is no separate
    /// `initialize` step.  Because the constructor runs atomically with
    /// deployment the front-running window that existed with a two-step
    /// initialize() is eliminated.
    pub fn __constructor(env: Env, admin: Address) {
        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&PAUSED, &false);
        env.storage().instance().set(&SCHED_ID, &0u64);
        Self::bump_instance(&env);
    }

    // ── Schedule management ──────────────────────────────────────────────────

    /// Create a new vesting schedule. Caller must be admin.
    pub fn create_schedule(env: Env, params: CreateScheduleParams) -> u64 {
        Self::bump_instance(&env);
        Self::require_admin(&env);
        Self::require_not_paused(&env);
        Self::create_schedule_internal(&env, params)
    }

    /// Create multiple vesting schedules atomically in a single transaction.
    ///
    /// All schedules are validated and created together. Because this executes
    /// within a single Soroban transaction, failure at any step rolls back all
    /// state changes — no partial batches are possible.
    ///
    /// Requires admin auth. Contract must not be paused.
    /// `params` must be non-empty.
    ///
    /// Returns a `Vec<u64>` of the newly created schedule IDs in the same
    /// order as the input `params`.
    pub fn batch_create_schedule(env: Env, params: Vec<CreateScheduleParams>) -> Vec<u64> {
        Self::bump_instance(&env);
        Self::require_admin(&env);
        Self::require_not_paused(&env);

        if params.is_empty() {
            panic_with_error!(&env, VestingError::BatchEmpty);
        }

        let mut ids: Vec<u64> = Vec::new(&env);
        for p in params.iter() {
            let id = Self::create_schedule_internal(&env, p);
            ids.push_back(id);
        }
        ids
    }

    /// Claim all currently vested-but-unclaimed tokens for a schedule.
    pub fn claim(env: Env, schedule_id: u64) -> i128 {
        Self::bump_instance(&env);
        Self::require_not_paused(&env);

        let mut schedule: VestingSchedule = env
            .storage()
            .persistent()
            .get(&DataKey::Schedule(schedule_id))
            .unwrap_or_else(|| panic_with_error!(&env, VestingError::ScheduleNotFound));

        schedule.beneficiary.require_auth();

        if schedule.status != ScheduleStatus::Active {
            panic_with_error!(&env, VestingError::ScheduleNotActive);
        }

        let now = env.ledger().timestamp();
        let claimable = Self::vested_amount(&schedule, now) - schedule.claimed_amount;

        if claimable <= 0 {
            panic_with_error!(&env, VestingError::NothingToClaim);
        }

        schedule.claimed_amount += claimable;

        if schedule.claimed_amount >= schedule.total_amount {
            schedule.status = ScheduleStatus::Completed;
        }

        env.storage()
            .persistent()
            .set(&DataKey::Schedule(schedule_id), &schedule);
        env.storage().persistent().extend_ttl(
            &DataKey::Schedule(schedule_id),
            PERSISTENT_BUMP_THRESHOLD,
            PERSISTENT_BUMP_LEDGERS,
        );

        let tk = token::Client::new(&env, &schedule.token);
        tk.transfer(
            &env.current_contract_address(),
            &schedule.beneficiary,
            &claimable,
        );

        env.events().publish(
            (EVT_CLAIMED,),
            (schedule_id, schedule.beneficiary, claimable),
        );

        claimable
    }

    /// Revoke an active schedule.
    ///
    /// Vested-but-unclaimed tokens are paid to the beneficiary first.
    /// Only the truly unvested portion is returned to `recipient`.
    pub fn revoke(env: Env, schedule_id: u64, recipient: Address) -> i128 {
        Self::bump_instance(&env);
        Self::require_admin(&env);

        let mut schedule: VestingSchedule = env
            .storage()
            .persistent()
            .get(&DataKey::Schedule(schedule_id))
            .unwrap_or_else(|| panic_with_error!(&env, VestingError::ScheduleNotFound));

        if schedule.status != ScheduleStatus::Active {
            panic_with_error!(&env, VestingError::ScheduleNotActive);
        }

        let now = env.ledger().timestamp();
        let vested = Self::vested_amount(&schedule, now);

        // Compute amounts before any state mutation.
        let unclaimed_vested = vested - schedule.claimed_amount;
        let unvested = schedule.total_amount - vested;

        // ── Effects: update state before any external calls (CEI pattern) ──
        if unclaimed_vested > 0 {
            schedule.claimed_amount += unclaimed_vested;
        }
        schedule.status = ScheduleStatus::Revoked;
        env.storage()
            .persistent()
            .set(&DataKey::Schedule(schedule_id), &schedule);
        env.storage().persistent().extend_ttl(
            &DataKey::Schedule(schedule_id),
            PERSISTENT_BUMP_THRESHOLD,
            PERSISTENT_BUMP_LEDGERS,
        );

        // ── Interactions: token transfers happen after state is finalised ──
        // Pay any vested-but-unclaimed tokens to the beneficiary.
        if unclaimed_vested > 0 {
            let tk = token::Client::new(&env, &schedule.token);
            tk.transfer(
                &env.current_contract_address(),
                &schedule.beneficiary,
                &unclaimed_vested,
            );
        }

        // Return the unvested portion to the recipient (typically treasury).
        if unvested > 0 {
            let tk = token::Client::new(&env, &schedule.token);
            tk.transfer(&env.current_contract_address(), &recipient, &unvested);
        }

        env.events()
            .publish((EVT_REVOKED,), (schedule_id, unvested));

        unvested
    }

    // ── Pause ────────────────────────────────────────────────────────────

    pub fn pause(env: Env) {
        Self::bump_instance(&env);
        Self::require_admin(&env);
        env.storage().instance().set(&PAUSED, &true);
        env.events().publish((EVT_PAUSED,), ());
    }

    pub fn unpause(env: Env) {
        Self::bump_instance(&env);
        Self::require_admin(&env);
        env.storage().instance().set(&PAUSED, &false);
        env.events().publish((EVT_UNPAUSED,), ());
    }

    // ── Admin transfer ──────────────────────────────────────────────────────

    pub fn transfer_admin(env: Env, new_admin: Address) {
        Self::bump_instance(&env);
        Self::require_admin(&env);
        new_admin.require_auth();
        let old_admin: Address = env
            .storage()
            .instance()
            .get(&ADMIN)
            .expect("not initialized");
        env.storage().instance().set(&ADMIN, &new_admin);
        env.events()
            .publish((EVT_ADM_XFER,), (old_admin, new_admin));
    }

    // ── Beneficiary transfer ──────────────────────────────────────────────

    /// Transfer a vesting schedule from the current beneficiary to a new one.
    ///
    /// The current beneficiary must authorise the call.  The schedule must be
    /// `Active` (not Revoked or Completed) and the contract must not be paused.
    /// All vesting parameters and `claimed_amount` are preserved — the new
    /// beneficiary inherits exactly where the schedule left off.
    ///
    /// Updates both `BeneficiarySchedules` index entries atomically:
    /// - Removes `schedule_id` from `old_beneficiary`'s list.
    /// - Appends `schedule_id` to `new_beneficiary`'s list.
    ///
    /// Emits `("benXfer",)` → `(schedule_id, old_beneficiary, new_beneficiary)`.
    pub fn transfer_beneficiary(env: Env, schedule_id: u64, new_beneficiary: Address) {
        Self::bump_instance(&env);
        Self::require_not_paused(&env);

        let mut schedule: VestingSchedule = env
            .storage()
            .persistent()
            .get(&DataKey::Schedule(schedule_id))
            .unwrap_or_else(|| panic_with_error!(&env, VestingError::ScheduleNotFound));

        if schedule.status != ScheduleStatus::Active {
            panic_with_error!(&env, VestingError::ScheduleNotActive);
        }

        // Current beneficiary must authorise the transfer.
        schedule.beneficiary.require_auth();
        let old_beneficiary = schedule.beneficiary.clone();

        // ── Effects ──────────────────────────────────────────────────────────
        // 1. Update schedule record.
        schedule.beneficiary = new_beneficiary.clone();
        env.storage()
            .persistent()
            .set(&DataKey::Schedule(schedule_id), &schedule);
        env.storage().persistent().extend_ttl(
            &DataKey::Schedule(schedule_id),
            PERSISTENT_BUMP_THRESHOLD,
            PERSISTENT_BUMP_LEDGERS,
        );

        // 2. Remove schedule_id from old beneficiary's index.
        let mut old_ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::BeneficiarySchedules(old_beneficiary.clone()))
            .unwrap_or(Vec::new(&env));
        if let Some(pos) = old_ids.iter().position(|id| id == schedule_id) {
            old_ids.remove(pos as u32);
        }
        env.storage().persistent().set(
            &DataKey::BeneficiarySchedules(old_beneficiary.clone()),
            &old_ids,
        );
        env.storage().persistent().extend_ttl(
            &DataKey::BeneficiarySchedules(old_beneficiary.clone()),
            PERSISTENT_BUMP_THRESHOLD,
            PERSISTENT_BUMP_LEDGERS,
        );

        // 3. Append schedule_id to new beneficiary's index.
        let mut new_ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::BeneficiarySchedules(new_beneficiary.clone()))
            .unwrap_or(Vec::new(&env));
        new_ids.push_back(schedule_id);
        env.storage().persistent().set(
            &DataKey::BeneficiarySchedules(new_beneficiary.clone()),
            &new_ids,
        );
        env.storage().persistent().extend_ttl(
            &DataKey::BeneficiarySchedules(new_beneficiary.clone()),
            PERSISTENT_BUMP_THRESHOLD,
            PERSISTENT_BUMP_LEDGERS,
        );

        env.events().publish(
            (EVT_BEN_XFER,),
            (schedule_id, old_beneficiary, new_beneficiary),
        );
    }

    // ── Read functions ─────────────────────────────────────────────────────

    pub fn get_schedule(env: Env, schedule_id: u64) -> VestingSchedule {
        env.storage()
            .persistent()
            .get(&DataKey::Schedule(schedule_id))
            .unwrap_or_else(|| panic_with_error!(&env, VestingError::ScheduleNotFound))
    }

    pub fn get_claimable(env: Env, schedule_id: u64) -> i128 {
        let schedule: VestingSchedule = env
            .storage()
            .persistent()
            .get(&DataKey::Schedule(schedule_id))
            .unwrap_or_else(|| panic_with_error!(&env, VestingError::ScheduleNotFound));
        let now = env.ledger().timestamp();
        (Self::vested_amount(&schedule, now) - schedule.claimed_amount).max(0)
    }

    /// Returns the projected claimable amount for a schedule at an arbitrary
    /// `timestamp`, without waiting for that ledger.
    ///
    /// Useful for dashboards that want to show future vesting projections.
    /// Computes `vested_amount(timestamp) − claimed_amount`, clamped to 0.
    ///
    /// Read-only — does **not** bump the instance or persistent TTL.
    ///
    /// # Panics
    /// - [`VestingError::ScheduleNotFound`] — schedule ID does not exist.
    pub fn get_claimable_at(env: Env, schedule_id: u64, timestamp: u64) -> i128 {
        let schedule: VestingSchedule = env
            .storage()
            .persistent()
            .get(&DataKey::Schedule(schedule_id))
            .unwrap_or_else(|| panic_with_error!(&env, VestingError::ScheduleNotFound));
        (Self::vested_amount(&schedule, timestamp) - schedule.claimed_amount).max(0)
    }

    pub fn get_beneficiary_schedules(env: Env, beneficiary: Address) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::BeneficiarySchedules(beneficiary))
            .unwrap_or(Vec::new(&env))
    }

    /// Return the sum of all `claimed_amount` values across every schedule
    /// for `beneficiary`.  Useful for dashboards that need a single total
    /// without iterating individual schedules off-chain.
    ///
    /// Returns 0 when the beneficiary has no schedules.
    ///
    /// Read-only: does **not** extend any TTL.
    pub fn get_claimed_by_beneficiary(env: Env, beneficiary: Address) -> i128 {
        let ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::BeneficiarySchedules(beneficiary))
            .unwrap_or(Vec::new(&env));

        let mut total: i128 = 0;
        for id in ids.iter() {
            let schedule: VestingSchedule = env
                .storage()
                .persistent()
                .get(&DataKey::Schedule(id))
                .unwrap_or_else(|| panic_with_error!(&env, VestingError::ScheduleNotFound));
            total = total.saturating_add(schedule.claimed_amount);
        }
        total
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&ADMIN)
            .expect("not initialized")
    }

    pub fn is_paused(env: Env) -> bool {
        env.storage().instance().get(&PAUSED).unwrap_or(false)
    }

    pub fn schedule_count(env: Env) -> u64 {
        env.storage().instance().get(&SCHED_ID).unwrap_or(0)
    }

    /// Return the amount that is still unvested (what would be returned to
    /// the recipient if `revoke()` were called now).
    ///
    /// Computed as `total_amount - vested_amount(schedule, now)`, clamped to
    /// `0`.  Returns `0` once the schedule is fully vested.
    /// Read-only — does not extend the TTL or write any state.
    ///
    /// # Auth
    /// None.
    ///
    /// # Panics
    /// - `schedule_id` does not exist (`VestingError::ScheduleNotFound`)
    pub fn get_unvested_amount(env: Env, schedule_id: u64) -> i128 {
        let schedule: VestingSchedule = env
            .storage()
            .persistent()
            .get(&DataKey::Schedule(schedule_id))
            .unwrap_or_else(|| panic_with_error!(&env, VestingError::ScheduleNotFound));
        let now = env.ledger().timestamp();
        let vested = VestingContract::vested_amount(&schedule, now);
        (schedule.total_amount - vested).max(0)
    }

    /// Return the number of seconds remaining until a schedule is fully vested.
    ///
    /// Computes `(start_time + total_duration).saturating_sub(now)` where `now`
    /// is the current ledger timestamp.  Returns `0` once the schedule has
    /// reached or passed its end time.
    ///
    /// Useful for client-side countdown timers and dashboards that display
    /// "time remaining" without requiring off-chain arithmetic.
    ///
    /// Read-only — does **not** bump the instance or persistent TTL.
    ///
    /// # Auth
    /// None.
    ///
    /// # Panics
    /// - [`VestingError::ScheduleNotFound`] — schedule ID does not exist.
    pub fn time_until_fully_vested(env: Env, schedule_id: u64) -> u64 {
        let schedule: VestingSchedule = env
            .storage()
            .persistent()
            .get(&DataKey::Schedule(schedule_id))
            .unwrap_or_else(|| panic_with_error!(&env, VestingError::ScheduleNotFound));
        let end = schedule.start_time.saturating_add(schedule.total_duration);
        let now = env.ledger().timestamp();
        end.saturating_sub(now)
    }

    /// Return `true` if a schedule with the given ID exists in storage,
    /// `false` otherwise.
    ///
    /// Clients can safely probe for schedule existence without catching a
    /// panic, which is particularly useful when iterating over a range of IDs.
    /// Read-only — does not extend the TTL or write any state.
    ///
    /// # Auth
    /// None.
    pub fn schedule_exists(env: Env, schedule_id: u64) -> bool {
        env.storage()
            .persistent()
            .get::<DataKey, VestingSchedule>(&DataKey::Schedule(schedule_id))
            .is_some()
    }

    /// Extend the contract instance TTL to `INSTANCE_BUMP_LEDGERS`.
    ///
    /// This is a permissionless keep-alive for contracts that are only read
    /// (pure getters do not extend the TTL themselves).  Anyone may call this
    /// to prevent the instance from being archived.
    pub fn extend_ttl(env: Env) {
        env.storage()
            .instance()
            .extend_ttl(INSTANCE_BUMP_THRESHOLD, INSTANCE_BUMP_LEDGERS);
    }

    // ── Internal helpers ─────────────────────────────────────────────────────

    /// Shared per-schedule creation logic used by both `create_schedule` and
    /// `batch_create_schedule`. Assumes admin auth and pause check have already
    /// been performed by the caller.
    fn create_schedule_internal(env: &Env, params: CreateScheduleParams) -> u64 {
        if params.total_amount <= 0 {
            panic_with_error!(env, VestingError::AmountMustBePositive);
        }
        if params.total_duration == 0 {
            panic_with_error!(env, VestingError::DurationMustBePositive);
        }
        if params.cliff_duration > params.total_duration {
            panic_with_error!(env, VestingError::CliffExceedsDuration);
        }

        // Reject schedules where start_time + cliff_duration or
        // start_time + total_duration would overflow u64.  If either sum
        // overflows, vested_amount() would trap on every call to claim() or
        // revoke(), permanently locking the deposited funds.
        params
            .start_time
            .checked_add(params.cliff_duration)
            .unwrap_or_else(|| panic_with_error!(env, VestingError::StartPlusCliffOverflow));
        params
            .start_time
            .checked_add(params.total_duration)
            .unwrap_or_else(|| panic_with_error!(env, VestingError::StartPlusDurationOverflow));

        let tk = token::Client::new(env, &params.token_address);
        tk.transfer(
            &params.from,
            &env.current_contract_address(),
            &params.total_amount,
        );

        let id: u64 = env.storage().instance().get(&SCHED_ID).unwrap_or(0);
        let next_id = id + 1;

        let token_address = params.token_address.clone();
        let schedule = VestingSchedule {
            id: next_id,
            beneficiary: params.beneficiary.clone(),
            token: params.token_address,
            total_amount: params.total_amount,
            claimed_amount: 0,
            start_time: params.start_time,
            cliff_duration: params.cliff_duration,
            total_duration: params.total_duration,
            status: ScheduleStatus::Active,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Schedule(next_id), &schedule);
        env.storage().persistent().extend_ttl(
            &DataKey::Schedule(next_id),
            PERSISTENT_BUMP_THRESHOLD,
            PERSISTENT_BUMP_LEDGERS,
        );

        let mut ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::BeneficiarySchedules(params.beneficiary.clone()))
            .unwrap_or(Vec::new(env));
        ids.push_back(next_id);
        env.storage().persistent().set(
            &DataKey::BeneficiarySchedules(params.beneficiary.clone()),
            &ids,
        );
        env.storage().persistent().extend_ttl(
            &DataKey::BeneficiarySchedules(params.beneficiary.clone()),
            PERSISTENT_BUMP_THRESHOLD,
            PERSISTENT_BUMP_LEDGERS,
        );

        env.storage().instance().set(&SCHED_ID, &next_id);

        env.events().publish(
            (EVT_CREATED,),
            (
                next_id,
                params.beneficiary,
                params.total_amount,
                token_address,
            ),
        );

        next_id
    }

    /// Extend instance storage TTL so ADMIN, PAUSED and SCHED_ID do not expire
    /// while the contract is in active use.
    fn bump_instance(env: &Env) {
        env.storage()
            .instance()
            .extend_ttl(INSTANCE_BUMP_THRESHOLD, INSTANCE_BUMP_LEDGERS);
    }

    fn require_admin(env: &Env) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&ADMIN)
            .expect("not initialized");
        admin.require_auth();
    }

    fn require_not_paused(env: &Env) {
        let paused: bool = env.storage().instance().get(&PAUSED).unwrap_or(false);
        if paused {
            panic_with_error!(env, VestingError::ContractIsPaused);
        }
    }

    /// Compute how much of `total_amount` has vested by timestamp `now`.
    ///
    /// Uses `saturating_mul` to avoid overflow on large amounts × elapsed.
    /// Division by zero is impossible because `total_duration > 0` is enforced
    /// at schedule creation.
    fn vested_amount(schedule: &VestingSchedule, now: u64) -> i128 {
        if now < schedule.start_time + schedule.cliff_duration {
            return 0;
        }
        let elapsed = now.saturating_sub(schedule.start_time);
        if elapsed >= schedule.total_duration {
            return schedule.total_amount;
        }
        // saturating_mul prevents i128 overflow for very large amounts × elapsed.
        // Note: integer division truncates toward zero (floor division).
        // Dust — the fractional remainder — accumulates in the contract and is
        // only released when elapsed >= total_duration (the branch above).
        // See README "Rounding behavior" for a worked example.
        i128::from(elapsed).saturating_mul(schedule.total_amount)
            / i128::from(schedule.total_duration)
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{deploy_stellar_asset_token, deploy_vesting_with_admin};
    use soroban_sdk::testutils::{Address as _, Events, Ledger};
    use soroban_sdk::Env;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn setup() -> (Env, Address, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let admin = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let funder = Address::generate(&env);

        let token_address = deploy_stellar_asset_token(&env, &funder, 1_000_000);
        let vesting_id = deploy_vesting_with_admin(&env, &admin);

        (env, vesting_id, admin, beneficiary, funder)
    }

    /// Deploy a fresh token, mint `amount` to `funder`, return token address.
    fn new_token(env: &Env, funder: &Address, amount: i128) -> Address {
        deploy_stellar_asset_token(env, funder, amount)
    }

    // ── constructor ──────────────────────────────────────────────────────────

    /// Constructor sets admin, paused=false, and schedule_count=0.
    #[test]
    fn test_constructor_sets_state() {
        let (env, vesting_id, admin, _b, _f) = setup();
        let vesting = VestingContractClient::new(&env, &vesting_id);
        assert_eq!(vesting.get_admin(), admin);
        assert!(!vesting.is_paused());
        assert_eq!(vesting.schedule_count(), 0);
    }

    // ── TTL tests ─────────────────────────────────────────────────────────────

    /// create_schedule() bumps instance TTL above INSTANCE_BUMP_THRESHOLD.
    #[test]
    fn test_ttl_instance_bumped_on_create_schedule() {
        use soroban_sdk::testutils::storage::Instance as _;
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        let ttl = env.as_contract(&vesting_id, || env.storage().instance().get_ttl());
        assert!(
            ttl > INSTANCE_BUMP_THRESHOLD,
            "instance TTL {ttl} should exceed INSTANCE_BUMP_THRESHOLD {INSTANCE_BUMP_THRESHOLD}"
        );
    }

    /// create_schedule() bumps persistent Schedule TTL.
    #[test]
    fn test_ttl_schedule_bumped_on_create() {
        use soroban_sdk::testutils::storage::Persistent as _;
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        let ttl = env.as_contract(&vesting_id, || {
            env.storage().persistent().get_ttl(&DataKey::Schedule(id))
        });
        assert!(
            ttl > PERSISTENT_BUMP_THRESHOLD,
            "schedule TTL {ttl} should exceed PERSISTENT_BUMP_THRESHOLD {PERSISTENT_BUMP_THRESHOLD}"
        );
    }

    /// claim() bumps instance TTL.
    #[test]
    fn test_ttl_instance_bumped_on_claim() {
        use soroban_sdk::testutils::storage::Instance as _;
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        env.ledger().with_mut(|l| l.timestamp = start + 50);
        vesting.claim(&id);

        let ttl = env.as_contract(&vesting_id, || env.storage().instance().get_ttl());
        assert!(
            ttl > INSTANCE_BUMP_THRESHOLD,
            "instance TTL {ttl} after claim should exceed {INSTANCE_BUMP_THRESHOLD}"
        );
    }

    // ── basic create + claim ─────────────────────────────────────────────────

    /// extend_ttl() bumps the instance TTL above INSTANCE_BUMP_THRESHOLD
    /// without requiring any auth (permissionless keep-alive).
    #[test]
    fn test_extend_ttl() {
        use soroban_sdk::testutils::storage::Instance as _;
        let (env, vesting_id, _admin, _beneficiary, _funder) = setup();
        let vesting = VestingContractClient::new(&env, &vesting_id);
        vesting.extend_ttl();
        let ttl = env.as_contract(&vesting_id, || env.storage().instance().get_ttl());
        assert!(
            ttl > INSTANCE_BUMP_THRESHOLD,
            "instance TTL {ttl} should exceed INSTANCE_BUMP_THRESHOLD after extend_ttl()"
        );
    }

    #[test]
    fn test_create_and_claim() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token.clone(),
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        env.ledger().with_mut(|l| l.timestamp = start + 50);
        assert_eq!(vesting.get_claimable(&id), 50_000);

        let claimed = vesting.claim(&id);
        assert_eq!(claimed, 50_000);

        env.ledger().with_mut(|l| l.timestamp = start + 100);
        let claimed2 = vesting.claim(&id);
        assert_eq!(claimed2, 50_000);
    }

    // ── cliff blocks early claim, unlocks after cliff ────────────────────────

    #[test]
    fn test_cliff_blocks_early_claim() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 50,
            total_duration: 100,
        });

        // Before cliff: nothing claimable
        env.ledger().with_mut(|l| l.timestamp = start + 30);
        assert_eq!(vesting.get_claimable(&id), 0);

        // Exactly at cliff: linear vesting has been running since start, not since cliff
        env.ledger().with_mut(|l| l.timestamp = start + 50);
        // elapsed=50, total_duration=100 -> 50% = 50_000
        assert_eq!(vesting.get_claimable(&id), 50_000);

        // After cliff
        env.ledger().with_mut(|l| l.timestamp = start + 75);
        assert!(vesting.get_claimable(&id) > 50_000);
    }

    // ── zero cliff: immediate linear vesting ─────────────────────────────────

    #[test]
    fn test_zero_cliff_immediate_vesting() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 1_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 1_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 1_000,
        });

        env.ledger().with_mut(|l| l.timestamp = start + 1);
        assert_eq!(vesting.get_claimable(&id), 1);
    }

    // ── full vest after duration ends ────────────────────────────────────────

    #[test]
    fn test_full_vest_after_duration() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 500_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 500_000,
            start_time: start,
            cliff_duration: 100,
            total_duration: 200,
        });

        env.ledger().with_mut(|l| l.timestamp = start + 300); // well past end
        assert_eq!(vesting.get_claimable(&id), 500_000);

        let claimed = vesting.claim(&id);
        assert_eq!(claimed, 500_000);

        let sched = vesting.get_schedule(&id);
        assert_eq!(sched.status, ScheduleStatus::Completed);
    }

    // ── revoke returns unvested tokens; vested-but-unclaimed go to beneficiary

    #[test]
    fn test_revoke_pays_vested_to_beneficiary() {
        let (env, vesting_id, admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token.clone(),
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        // At t=25: 25_000 vested, unclaimed. Revoke should send 25_000 to
        // beneficiary and 75_000 to admin (treasury).
        env.ledger().with_mut(|l| l.timestamp = start + 25);
        let returned = vesting.revoke(&id, &admin);
        assert_eq!(returned, 75_000); // unvested

        let sched = vesting.get_schedule(&id);
        assert_eq!(sched.status, ScheduleStatus::Revoked);

        // Beneficiary should have received the vested 25_000 automatically.
        let token_client = soroban_sdk::token::Client::new(&env, &token);
        assert_eq!(token_client.balance(&beneficiary), 25_000);
        assert_eq!(token_client.balance(&admin), 75_000);
    }

    // ── revoke after full claim: no beneficiary transfer when unclaimed==0 ──

    /// When the beneficiary has already claimed all vested tokens before
    /// revoke() is called, `unclaimed_vested == 0` and the guarded beneficiary
    /// transfer must be skipped.  The beneficiary balance must stay at the
    /// prior claim amount (no double payment), the treasury must receive
    /// exactly the unvested amount, and the vesting contract must hold 0.
    #[test]
    fn test_revoke_after_full_claim_no_beneficiary_transfer() {
        let (env, vesting_id, admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let treasury = Address::generate(&env);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token.clone(),
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        // Advance to 50% vested and claim all 50_000.
        env.ledger().with_mut(|l| l.timestamp = start + 50);
        assert_eq!(vesting.get_claimable(&id), 50_000);
        let claimed = vesting.claim(&id);
        assert_eq!(claimed, 50_000);

        let token_client = soroban_sdk::token::Client::new(&env, &token);
        assert_eq!(token_client.balance(&beneficiary), 50_000);

        // Revoke immediately after the claim: unclaimed_vested == 0.
        let returned = vesting.revoke(&id, &treasury);
        assert_eq!(returned, 50_000, "unvested amount must be returned");

        // Beneficiary balance must remain at the prior claim amount — no
        // additional (zero-amount or otherwise) transfer occurred.
        assert_eq!(
            token_client.balance(&beneficiary),
            50_000,
            "beneficiary must not receive a second payment on revoke"
        );

        // Treasury receives exactly the unvested amount.
        assert_eq!(
            token_client.balance(&treasury),
            50_000,
            "treasury must receive exactly the unvested amount"
        );

        // Vesting contract holds nothing after revoke.
        assert_eq!(
            token_client.balance(&vesting_id),
            0,
            "vesting contract must hold 0 after revoke"
        );

        // Schedule is Revoked and claimed_amount reflects the full vested portion.
        let sched = vesting.get_schedule(&id);
        assert_eq!(sched.status, ScheduleStatus::Revoked);
        assert_eq!(sched.claimed_amount, 50_000);
    }

    // ── revoke returns unvested tokens (original test variant) ───────────────

    #[test]
    fn test_revoke_returns_unvested() {
        let (env, vesting_id, admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        // At t=25: 25k vested (unclaimed), 75k unvested → revoke returns 75k
        env.ledger().with_mut(|l| l.timestamp = start + 25);
        let returned = vesting.revoke(&id, &admin);
        assert_eq!(returned, 75_000);

        let sched = vesting.get_schedule(&id);
        assert_eq!(sched.status, ScheduleStatus::Revoked);
    }

    // ── revoke before cliff: 0 vested, 100% returned to recipient ────────────

    /// When revoke() is called before the cliff period has elapsed, zero tokens
    /// have vested.  The entire total_amount must be returned to the recipient
    /// and the beneficiary receives nothing.
    ///
    /// Schedule: cliff_duration=100, total_duration=200, total_amount=100_000.
    /// Revoke at t = start + 50 (halfway through the cliff, nothing vested).
    ///
    /// Expected:
    ///   - return value == total_amount (100_000)
    ///   - beneficiary balance == 0
    ///   - recipient balance == 100_000
    ///   - schedule.status == ScheduleStatus::Revoked
    #[test]
    fn test_revoke_before_cliff() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let recipient = Address::generate(&env); // treasury / unvested-token recipient
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token.clone(),
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 100, // cliff of 100 seconds
            total_duration: 200,
        });

        // Advance to t = start + 50 — halfway through the cliff, nothing vested yet.
        env.ledger().with_mut(|l| l.timestamp = start + 50);

        // Verify claimable is 0 before calling revoke.
        assert_eq!(vesting.get_claimable(&id), 0);

        let returned = vesting.revoke(&id, &recipient);

        // All tokens are unvested → entire amount returned to recipient.
        assert_eq!(
            returned, 100_000,
            "expected all 100_000 returned as unvested"
        );

        // Beneficiary received nothing (zero vested, zero paid out).
        let token_client = soroban_sdk::token::Client::new(&env, &token);
        assert_eq!(
            token_client.balance(&beneficiary),
            0,
            "beneficiary should receive 0 tokens when revoked before cliff"
        );

        // Recipient received the full unvested amount.
        assert_eq!(
            token_client.balance(&recipient),
            100_000,
            "recipient should receive full total_amount"
        );

        // Schedule is marked Revoked.
        let sched = vesting.get_schedule(&id);
        assert_eq!(
            sched.status,
            ScheduleStatus::Revoked,
            "schedule status must be Revoked"
        );
    }

    // ── claim after revoke must error with ScheduleNotActive ─────────────────

    #[test]
    #[should_panic]
    fn test_claim_after_revoke_panics() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let admin = Address::generate(&env);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        env.ledger().with_mut(|l| l.timestamp = start + 50);
        vesting.revoke(&id, &admin);
        vesting.claim(&id); // must panic with VestingError::ScheduleNotActive
    }

    // ── nothing to claim before cliff ────────────────────────────────────────

    #[test]
    #[should_panic]
    fn test_claim_before_cliff_panics() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 100,
            total_duration: 200,
        });

        env.ledger().with_mut(|l| l.timestamp = start + 50); // before cliff
        vesting.claim(&id); // must panic with VestingError::NothingToClaim
    }

    // ── pause blocks create_schedule ─────────────────────────────────────────

    #[test]
    #[should_panic]
    fn test_pause_blocks_create_schedule() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        vesting.pause();
        vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        }); // must panic with VestingError::ContractIsPaused
    }

    // ── pause blocks claim ───────────────────────────────────────────────────

    #[test]
    #[should_panic]
    fn test_pause_blocks_claim() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        env.ledger().with_mut(|l| l.timestamp = start + 50);
        vesting.pause();
        vesting.claim(&id); // must panic with VestingError::ContractIsPaused
    }

    // ── unpause restores functionality ───────────────────────────────────────

    #[test]
    fn test_unpause_restores_claim() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        env.ledger().with_mut(|l| l.timestamp = start + 50);
        vesting.pause();
        vesting.unpause();
        let claimed = vesting.claim(&id);
        assert_eq!(claimed, 50_000);
    }

    // ── transfer_admin ───────────────────────────────────────────────────────

    #[test]
    fn test_transfer_admin() {
        let (env, vesting_id, _admin, _beneficiary, _funder) = setup();
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let new_admin = Address::generate(&env);

        vesting.transfer_admin(&new_admin);
        assert_eq!(vesting.get_admin(), new_admin);
    }

    /// After transfer_admin, only the old admin's auth is not enough:
    /// create_schedule must panic.
    #[test]
    #[should_panic]
    fn test_vesting_transfer_admin_revokes_old_admin() {
        use soroban_sdk::testutils::{MockAuth, MockAuthInvoke};
        use soroban_sdk::IntoVal;

        let (env, vesting_id, old_admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let new_admin = Address::generate(&env);
        vesting.transfer_admin(&new_admin);

        let params = CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 100_000,
            start_time: env.ledger().timestamp(),
            cliff_duration: 0,
            total_duration: 100,
        };

        // Only the old admin authorizes the call; require_admin() now checks new_admin.
        env.mock_auths(&[MockAuth {
            address: &old_admin,
            invoke: &MockAuthInvoke {
                contract: &vesting_id,
                fn_name: "create_schedule",
                args: (params.clone(),).into_val(&env),
                sub_invokes: &[],
            },
        }]);
        vesting.create_schedule(&params); // must panic: new_admin auth missing
    }

    /// After transfer_admin, the new admin can create schedules.
    #[test]
    fn test_vesting_transfer_admin_new_admin_can_create_schedule() {
        let (env, vesting_id, _old_admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let new_admin = Address::generate(&env);
        vesting.transfer_admin(&new_admin);

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 100_000,
            start_time: env.ledger().timestamp(),
            cliff_duration: 0,
            total_duration: 100,
        });

        assert_eq!(vesting.get_admin(), new_admin);
        assert_eq!(vesting.schedule_count(), 1);
        assert_eq!(id, 1);
    }

    // ── schedule_count increments ────────────────────────────────────────────

    #[test]
    fn test_schedule_count() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let vesting = VestingContractClient::new(&env, &vesting_id);
        assert_eq!(vesting.schedule_count(), 0);

        let token = new_token(&env, &funder, 200_000);
        let start = env.ledger().timestamp();

        vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token.clone(),
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });
        assert_eq!(vesting.schedule_count(), 1);

        let b2 = Address::generate(&env);
        vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: b2.clone(),
            token_address: token.clone(),
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });
        assert_eq!(vesting.schedule_count(), 2);
    }

    // ── multiple beneficiaries have independent schedules ────────────────────

    #[test]
    fn test_multiple_beneficiaries() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let b2 = Address::generate(&env);
        let token = new_token(&env, &funder, 300_000);
        let start = env.ledger().timestamp();

        let id1 = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token.clone(),
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });
        let id2 = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: b2.clone(),
            token_address: token.clone(),
            total_amount: 200_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 200,
        });

        env.ledger().with_mut(|l| l.timestamp = start + 100);
        // beneficiary1: fully vested (100_000)
        assert_eq!(vesting.get_claimable(&id1), 100_000);
        // beneficiary2: only 50% (100_000)
        assert_eq!(vesting.get_claimable(&id2), 100_000);

        // beneficiary_schedules returns the right IDs
        let ids1 = vesting.get_beneficiary_schedules(&beneficiary);
        assert_eq!(ids1.len(), 1);
        assert_eq!(ids1.get(0).unwrap(), id1);

        let ids2 = vesting.get_beneficiary_schedules(&b2);
        assert_eq!(ids2.len(), 1);
        assert_eq!(ids2.get(0).unwrap(), id2);
    }

    // ── beneficiary_schedules accumulates multiple schedules ─────────────────

    #[test]
    fn test_beneficiary_schedules_multiple() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let token = new_token(&env, &funder, 300_000);
        let start = env.ledger().timestamp();

        let id1 = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token.clone(),
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });
        let id2 = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token.clone(),
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 200,
        });
        let id3 = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token.clone(),
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 50,
            total_duration: 300,
        });

        let ids = vesting.get_beneficiary_schedules(&beneficiary);
        assert_eq!(ids.len(), 3);
        assert_eq!(ids.get(0).unwrap(), id1);
        assert_eq!(ids.get(1).unwrap(), id2);
        assert_eq!(ids.get(2).unwrap(), id3);
    }

    // ── get_total_claimed_for_beneficiary (#128) ──────────────────────────────

    /// Returns 0 for an address with no schedules.
    #[test]
    fn test_total_claimed_no_schedules() {
        let (env, vesting_id, _admin, _beneficiary, _funder) = setup();
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let stranger = Address::generate(&env);
        assert_eq!(vesting.get_claimed_by_beneficiary(&stranger), 0);
    }

    /// Returns the claimed amount for a single schedule after a partial claim.
    #[test]
    fn test_total_claimed_single_schedule() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        // Before any claim: total claimed = 0
        assert_eq!(vesting.get_claimed_by_beneficiary(&beneficiary), 0);

        // Claim at t=50: 50_000 claimed
        env.ledger().with_mut(|l| l.timestamp = start + 50);
        vesting.claim(&id);
        assert_eq!(vesting.get_claimed_by_beneficiary(&beneficiary), 50_000);

        // Claim remainder at t=100: 100_000 total claimed
        env.ledger().with_mut(|l| l.timestamp = start + 100);
        vesting.claim(&id);
        assert_eq!(vesting.get_claimed_by_beneficiary(&beneficiary), 100_000);
    }

    /// Returns the sum across multiple schedules.
    #[test]
    fn test_total_claimed_multiple_schedules() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 300_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id1 = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token.clone(),
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });
        let id2 = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token.clone(),
            total_amount: 200_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        // Claim from schedule 1 at t=50: 50_000 claimed
        env.ledger().with_mut(|l| l.timestamp = start + 50);
        vesting.claim(&id1);
        assert_eq!(vesting.get_claimed_by_beneficiary(&beneficiary), 50_000);

        // Claim from schedule 2 at t=50: 100_000 + 50_000 = 150_000 total
        vesting.claim(&id2);
        assert_eq!(vesting.get_claimed_by_beneficiary(&beneficiary), 150_000);

        // Fully claim schedule 1 at t=100
        env.ledger().with_mut(|l| l.timestamp = start + 100);
        vesting.claim(&id1);
        // schedule 1 claimed_amount is now 100_000; schedule 2 still 100_000
        assert_eq!(vesting.get_claimed_by_beneficiary(&beneficiary), 200_000);

        // Fully claim schedule 2 at t=100
        vesting.claim(&id2);
        assert_eq!(vesting.get_claimed_by_beneficiary(&beneficiary), 300_000);
    }

    // ── double-revoke must error with ScheduleNotActive ───────────────────────

    #[test]
    #[should_panic]
    fn test_double_revoke_panics() {
        let (env, vesting_id, admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        vesting.revoke(&id, &admin);
        vesting.revoke(&id, &admin); // must panic with VestingError::ScheduleNotActive
    }

    // ── E3: revoke() state is finalised before token transfers (CEI) ─────────
    //
    // Verifies that after revoke():
    //  - schedule.status == Revoked
    //  - schedule.claimed_amount reflects the vested portion paid out
    //  - beneficiary receives exactly the vested-but-unclaimed amount
    //  - treasury/recipient receives exactly the unvested amount
    //  - balances sum to total_amount (conservation)
    #[test]
    fn test_revoke_cei_state_and_balances() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 120_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let treasury = Address::generate(&env);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token.clone(),
            total_amount: 120_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 120,
        });

        // Advance time to 40 s → 40_000 vested, 80_000 unvested.
        env.ledger().with_mut(|l| l.timestamp = start + 40);

        let unvested_returned = vesting.revoke(&id, &treasury);
        assert_eq!(unvested_returned, 80_000);

        // State must be finalised before transfers reach the token contract.
        let sched = vesting.get_schedule(&id);
        assert_eq!(sched.status, ScheduleStatus::Revoked);
        assert_eq!(sched.claimed_amount, 40_000); // vested portion accounted

        // Token balances must match exactly.
        let tk = soroban_sdk::token::Client::new(&env, &token);
        assert_eq!(tk.balance(&beneficiary), 40_000); // vested-but-unclaimed paid
        assert_eq!(tk.balance(&treasury), 80_000); // unvested returned
                                                   // Conservation: beneficiary + treasury == total_amount
        assert_eq!(tk.balance(&beneficiary) + tk.balance(&treasury), 120_000);
    }

    // ── is_paused reflects state correctly ───────────────────────────────────

    #[test]
    fn test_is_paused_state() {
        let (env, vesting_id, _admin, _b, _f) = setup();
        let vesting = VestingContractClient::new(&env, &vesting_id);
        assert!(!vesting.is_paused());
        vesting.pause();
        assert!(vesting.is_paused());
        vesting.unpause();
        assert!(!vesting.is_paused());
    }

    // ── events: create_schedule emits "created" ───────────────────────────────

    #[test]
    fn test_event_created() {
        use soroban_sdk::IntoVal;
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token.clone(),
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        // The "created" event must include token_address as the fourth field so
        // off-chain indexers tracking multiple token types can identify which
        // token a schedule locks without an extra RPC call.
        assert_eq!(
            env.events().all().filter_by_contract(&vesting_id),
            soroban_sdk::vec![
                &env,
                (
                    vesting_id.clone(),
                    (symbol_short!("created"),).into_val(&env),
                    (id, beneficiary.clone(), 100_000i128, token.clone()).into_val(&env),
                )
            ],
            "expected 'created' event with (id, beneficiary, total_amount, token_address)"
        );
    }

    // ── events: claim emits "claimed" ─────────────────────────────────────────

    #[test]
    fn test_event_claimed() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });
        env.ledger().with_mut(|l| l.timestamp = start + 50);
        vesting.claim(&id);

        let found = !env
            .events()
            .all()
            .filter_by_contract(&vesting_id)
            .events()
            .is_empty();
        assert!(found, "expected 'claimed' event");
    }

    // ── events: revoke emits "revoked" ────────────────────────────────────────

    #[test]
    fn test_event_revoked() {
        let (env, vesting_id, admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });
        vesting.revoke(&id, &admin);

        let found = !env
            .events()
            .all()
            .filter_by_contract(&vesting_id)
            .events()
            .is_empty();
        assert!(found, "expected 'revoked' event");
    }

    // ── events: pause/unpause ─────────────────────────────────────────────────

    #[test]
    fn test_event_pause_unpause() {
        let (env, vesting_id, _admin, _b, _f) = setup();
        let vesting = VestingContractClient::new(&env, &vesting_id);

        vesting.pause();
        let paused_found = !env
            .events()
            .all()
            .filter_by_contract(&vesting_id)
            .events()
            .is_empty();
        assert!(paused_found, "expected 'paused' event");

        vesting.unpause();
        let unpaused_found = !env
            .events()
            .all()
            .filter_by_contract(&vesting_id)
            .events()
            .is_empty();
        assert!(unpaused_found, "expected 'unpaused' event");
    }

    // ── events: transfer_admin emits "admXfer" ────────────────────────────────

    #[test]
    fn test_event_transfer_admin() {
        use soroban_sdk::IntoVal;
        let (env, vesting_id, old_admin, _b, _f) = setup();
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let new_admin = Address::generate(&env);

        vesting.transfer_admin(&new_admin);

        // The event payload must include both old_admin and new_admin so that
        // off-chain indexers can reconstruct the full admin hand-off chain.
        //
        // transfer_admin is the only call here, so the vesting contract emits
        // exactly one event.  Use the same tuple comparison pattern as the
        // token contract event tests.
        assert_eq!(
            env.events().all().filter_by_contract(&vesting_id),
            soroban_sdk::vec![
                &env,
                (
                    vesting_id.clone(),
                    (symbol_short!("admXfer"),).into_val(&env),
                    (old_admin.clone(), new_admin.clone()).into_val(&env),
                )
            ]
        );
    }

    // ── get_unvested_amount ──────────────────────────────────────────────────

    /// get_unvested_amount returns the correct unvested amount mid-schedule
    /// and 0 at/after the end.
    #[test]
    fn test_get_unvested_amount_mid_schedule_and_at_end() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        // At t=0: nothing vested yet, full amount unvested
        assert_eq!(vesting.get_unvested_amount(&id), 100_000);

        // At t=25: 25_000 vested, 75_000 unvested
        env.ledger().with_mut(|l| l.timestamp = start + 25);
        assert_eq!(vesting.get_unvested_amount(&id), 75_000);

        // At t=50: 50_000 vested, 50_000 unvested
        env.ledger().with_mut(|l| l.timestamp = start + 50);
        assert_eq!(vesting.get_unvested_amount(&id), 50_000);

        // At end (t=100): fully vested, 0 unvested
        env.ledger().with_mut(|l| l.timestamp = start + 100);
        assert_eq!(vesting.get_unvested_amount(&id), 0);

        // Past end: still 0
        env.ledger().with_mut(|l| l.timestamp = start + 200);
        assert_eq!(vesting.get_unvested_amount(&id), 0);
    }

    /// get_unvested_amount returns total_amount before the cliff.
    #[test]
    fn test_get_unvested_amount_before_cliff() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 50,
            total_duration: 100,
        });

        // Before cliff: 0 vested, full amount unvested
        env.ledger().with_mut(|l| l.timestamp = start + 25);
        assert_eq!(vesting.get_unvested_amount(&id), 100_000);

        // At cliff: 50_000 vested, 50_000 unvested
        env.ledger().with_mut(|l| l.timestamp = start + 50);
        assert_eq!(vesting.get_unvested_amount(&id), 50_000);
    }

    /// get_unvested_amount + get_claimable + claimed_amount == total_amount
    /// (conservation of tokens at any point during the schedule).
    #[test]
    fn test_unvested_plus_claimable_plus_claimed_equals_total() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        // Claim at t=40
        env.ledger().with_mut(|l| l.timestamp = start + 40);
        vesting.claim(&id);

        // Conservation invariant: unvested + claimable + claimed == total
        let unvested = vesting.get_unvested_amount(&id);
        let claimable = vesting.get_claimable(&id);
        let schedule = vesting.get_schedule(&id);
        assert_eq!(
            unvested + claimable + schedule.claimed_amount,
            100_000,
            "token conservation violated: unvested={unvested} + claimable={claimable} + claimed={} != 100_000",
            schedule.claimed_amount
        );
    }

    // ── schedule_exists ───────────────────────────────────────────────────────

    /// schedule_exists returns true for a created schedule and false for an ID
    /// that has never been created.
    #[test]
    fn test_schedule_exists_true_and_false() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        // Before any schedule: ID 1 does not exist.
        assert!(
            !vesting.schedule_exists(&1u64),
            "schedule_exists should be false before any schedule is created"
        );

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        // Existing schedule must return true.
        assert!(
            vesting.schedule_exists(&id),
            "schedule_exists should be true for created schedule"
        );

        // A never-created ID must return false.
        assert!(
            !vesting.schedule_exists(&(id + 1)),
            "schedule_exists should be false for a non-existent schedule ID"
        );
    }

    /// schedule_exists remains true after claim (schedule is Completed, not deleted).
    #[test]
    fn test_schedule_exists_after_claim() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        // Fully claim the schedule.
        env.ledger().with_mut(|l| l.timestamp = start + 100);
        vesting.claim(&id);

        // Schedule entry still exists in storage even after completion.
        assert!(
            vesting.schedule_exists(&id),
            "schedule_exists should be true even after the schedule is completed"
        );
    }

    // ── Upgrade 1: vesting arithmetic uses saturating_mul ────────────────────
    //
    // For very large amounts (close to i128::MAX / max realistic elapsed),
    // the formula should not overflow. We test with a large but realistic
    // token supply (10^24 stroop equivalent).

    #[test]
    fn test_no_arithmetic_overflow_large_amount() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        // 10^18 is within i128 range for realistic vesting durations
        let large_amount: i128 = 1_000_000_000_000_000_000;
        let token = new_token(&env, &funder, large_amount);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: large_amount,
            start_time: start,
            cliff_duration: 0,
            total_duration: 1_000_000_000, // ~31 years in seconds
        });

        env.ledger().with_mut(|l| l.timestamp = start + 500_000_000);
        // Should return exactly half, not overflow
        let claimable = vesting.get_claimable(&id);
        assert_eq!(claimable, large_amount / 2);
    }

    // ── Property-based tests (manual parameterised) ───────────────────────────
    //
    // Full proptest would require a build-std-compatible runner. These manual
    // parameterised tests cover the four properties without needing proptest:
    //   P1. vested amount never decreases over time
    //   P2. never exceeds total_amount
    //   P3. is 0 before the cliff
    //   P4. equals total_amount at or after end

    #[test]
    fn test_prop_vested_never_decreases() {
        // Check at t=0,25,50,75,100 for a 0-cliff 100-second schedule
        let total: i128 = 10_000;
        let duration: u64 = 100;
        let cliff: u64 = 0;
        let start: u64 = 0;

        let dummy_sched = VestingSchedule {
            id: 1,
            beneficiary: {
                // Use a stand-in address value — only testing pure math
                let env2 = Env::default();
                Address::generate(&env2)
            },
            token: {
                let env2 = Env::default();
                Address::generate(&env2)
            },
            total_amount: total,
            claimed_amount: 0,
            start_time: start,
            cliff_duration: cliff,
            total_duration: duration,
            status: ScheduleStatus::Active,
        };

        let times: [u64; 5] = [0, 25, 50, 75, 100];
        let mut prev = 0i128;
        for t in times {
            let v = VestingContract::vested_amount(&dummy_sched, t);
            assert!(v >= prev, "vested decreased at t={t}: {v} < {prev}");
            assert!(v <= total, "vested exceeded total at t={t}");
            prev = v;
        }
    }

    #[test]
    fn test_prop_zero_before_cliff() {
        let total: i128 = 10_000;
        let cliff: u64 = 50;
        let dummy_sched = VestingSchedule {
            id: 1,
            beneficiary: Address::generate(&Env::default()),
            token: Address::generate(&Env::default()),
            total_amount: total,
            claimed_amount: 0,
            start_time: 0,
            cliff_duration: cliff,
            total_duration: 100,
            status: ScheduleStatus::Active,
        };
        // Any time strictly before cliff → 0
        for t in [0u64, 1, 25, 49] {
            let v = VestingContract::vested_amount(&dummy_sched, t);
            assert_eq!(v, 0, "expected 0 before cliff at t={t}, got {v}");
        }
    }

    #[test]
    fn test_prop_equals_total_at_end() {
        let total: i128 = 10_000;
        let dummy_sched = VestingSchedule {
            id: 1,
            beneficiary: Address::generate(&Env::default()),
            token: Address::generate(&Env::default()),
            total_amount: total,
            claimed_amount: 0,
            start_time: 0,
            cliff_duration: 0,
            total_duration: 100,
            status: ScheduleStatus::Active,
        };
        // At or after end → total
        for t in [100u64, 101, 200, 1_000_000] {
            let v = VestingContract::vested_amount(&dummy_sched, t);
            assert_eq!(v, total, "expected total at t={t}, got {v}");
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // batch_create_schedule tests (#122)
    // ─────────────────────────────────────────────────────────────────────────

    /// Batch of 3 schedules for different beneficiaries: all IDs returned,
    /// schedule_count increments by 3, token balances are correct.
    #[test]
    fn test_batch_create_schedule_three() {
        let (env, vesting_id, _admin, _b, funder) = setup();
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let b1 = Address::generate(&env);
        let b2 = Address::generate(&env);
        let b3 = Address::generate(&env);

        // Mint enough for all three schedules from funder
        let token = new_token(&env, &funder, 300_000);
        let start = env.ledger().timestamp();

        let mut params: Vec<CreateScheduleParams> = Vec::new(&env);
        params.push_back(CreateScheduleParams {
            from: funder.clone(),
            beneficiary: b1.clone(),
            token_address: token.clone(),
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });
        params.push_back(CreateScheduleParams {
            from: funder.clone(),
            beneficiary: b2.clone(),
            token_address: token.clone(),
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 200,
        });
        params.push_back(CreateScheduleParams {
            from: funder.clone(),
            beneficiary: b3.clone(),
            token_address: token.clone(),
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 50,
            total_duration: 300,
        });

        let ids = vesting.batch_create_schedule(&params);

        // Returns correct number of IDs
        assert_eq!(ids.len(), 3);
        // IDs are sequential starting from 1
        assert_eq!(ids.get(0).unwrap(), 1u64);
        assert_eq!(ids.get(1).unwrap(), 2u64);
        assert_eq!(ids.get(2).unwrap(), 3u64);

        // schedule_count reflects all three
        assert_eq!(vesting.schedule_count(), 3);

        // Token conservation: all 300_000 moved to vesting contract
        let tk = soroban_sdk::token::Client::new(&env, &token);
        assert_eq!(tk.balance(&funder), 0);
        assert_eq!(tk.balance(&vesting_id), 300_000);

        // Each beneficiary has exactly one schedule
        assert_eq!(vesting.get_beneficiary_schedules(&b1).len(), 1);
        assert_eq!(vesting.get_beneficiary_schedules(&b2).len(), 1);
        assert_eq!(vesting.get_beneficiary_schedules(&b3).len(), 1);
    }

    /// Token conservation: total deposited == sum of schedule total_amounts.
    #[test]
    fn test_batch_token_conservation() {
        let (env, vesting_id, _admin, _b, funder) = setup();
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let b1 = Address::generate(&env);
        let b2 = Address::generate(&env);

        let token = new_token(&env, &funder, 500_000);
        let start = env.ledger().timestamp();

        let mut params: Vec<CreateScheduleParams> = Vec::new(&env);
        params.push_back(CreateScheduleParams {
            from: funder.clone(),
            beneficiary: b1.clone(),
            token_address: token.clone(),
            total_amount: 200_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });
        params.push_back(CreateScheduleParams {
            from: funder.clone(),
            beneficiary: b2.clone(),
            token_address: token.clone(),
            total_amount: 300_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        let ids = vesting.batch_create_schedule(&params);
        assert_eq!(ids.len(), 2);

        let tk = soroban_sdk::token::Client::new(&env, &token);
        // All tokens transferred to vesting
        assert_eq!(tk.balance(&funder), 0);
        assert_eq!(tk.balance(&vesting_id), 500_000);

        // Advance time to full vest and claim both
        env.ledger().with_mut(|l| l.timestamp = start + 100);
        let c1 = vesting.claim(&ids.get(0).unwrap());
        let c2 = vesting.claim(&ids.get(1).unwrap());
        assert_eq!(c1, 200_000);
        assert_eq!(c2, 300_000);
        // Conservation: beneficiaries received exactly what was deposited
        assert_eq!(tk.balance(&b1) + tk.balance(&b2), 500_000);
        assert_eq!(tk.balance(&vesting_id), 0);
    }

    /// An empty params vector must error with BatchEmpty.
    #[test]
    #[should_panic]
    fn test_batch_empty_panics() {
        let (env, vesting_id, _admin, _b, _f) = setup();
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let empty: Vec<CreateScheduleParams> = Vec::new(&env);
        vesting.batch_create_schedule(&empty); // must panic with VestingError::BatchEmpty
    }

    /// batch_create_schedule with an invalid entry (amount=0) must reject the
    /// entire batch atomically — no schedules created, no tokens transferred.
    #[test]
    #[should_panic]
    fn test_batch_all_or_nothing_invalid_amount() {
        let (env, vesting_id, _admin, _b, funder) = setup();
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let b1 = Address::generate(&env);
        let b2 = Address::generate(&env);

        let token = new_token(&env, &funder, 200_000);
        let start = env.ledger().timestamp();

        let mut params: Vec<CreateScheduleParams> = Vec::new(&env);
        // Valid first entry
        params.push_back(CreateScheduleParams {
            from: funder.clone(),
            beneficiary: b1.clone(),
            token_address: token.clone(),
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });
        // Invalid second entry: amount == 0 → VestingError::AmountMustBePositive
        params.push_back(CreateScheduleParams {
            from: funder.clone(),
            beneficiary: b2.clone(),
            token_address: token.clone(),
            total_amount: 0,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        // This must panic, rolling back the entire transaction
        vesting.batch_create_schedule(&params);
    }

    /// batch_create_schedule is blocked when the contract is paused.
    #[test]
    #[should_panic]
    fn test_batch_blocked_when_paused() {
        let (env, vesting_id, _admin, _b, funder) = setup();
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let b1 = Address::generate(&env);
        let token = new_token(&env, &funder, 100_000);
        let start = env.ledger().timestamp();

        vesting.pause();

        let mut params: Vec<CreateScheduleParams> = Vec::new(&env);
        params.push_back(CreateScheduleParams {
            from: funder.clone(),
            beneficiary: b1.clone(),
            token_address: token.clone(),
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        vesting.batch_create_schedule(&params); // must panic with VestingError::ContractIsPaused
    }

    // ── Multisig as vesting admin — end-to-end ───────────────────────────────
    //
    // The multisig contract is the vesting admin.  All tests use the real
    // ProposalAction enum introduced in Task A.  Owner management actions
    // (add_owner, remove_owner, update_threshold) are no longer public entry
    // points; they can only be triggered through an executed proposal.
    //
    // execute() dispatches a real cross-contract call for ProposalAction::Call
    // variants.  The multisig is the vesting admin, so its invocation satisfies
    // admin.require_auth() inside pause() / create_schedule() / etc.

    #[test]
    fn test_multisig_admin_flow() {
        use multisig::{CallData, MultisigContract, MultisigContractClient, ProposalAction};

        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let o1 = Address::generate(&env);
        let o2 = Address::generate(&env);
        let o3 = Address::generate(&env);

        // Deploy multisig (2-of-3) using constructor
        let mut owners = soroban_sdk::Vec::new(&env);
        owners.push_back(o1.clone());
        owners.push_back(o2.clone());
        owners.push_back(o3.clone());
        let ms_id = env.register(MultisigContract, (owners, 2u32));
        let ms = MultisigContractClient::new(&env, &ms_id);

        // Deploy vesting with multisig contract address as admin
        let vesting_id = env.register(VestingContract, (ms_id.clone(),));
        let vesting = VestingContractClient::new(&env, &vesting_id);

        // Submit a proposal that calls vesting.pause() through the multisig.
        // The multisig contract is the vesting admin so its invocation
        // satisfies admin.require_auth() inside pause().
        let id = ms.submit(
            &o1,
            &ProposalAction::Call(CallData {
                target: vesting_id.clone(),
                function: soroban_sdk::symbol_short!("pause"),
                args: soroban_sdk::Vec::new(&env),
            }),
            &soroban_sdk::String::from_str(&env, "pause vesting via multisig"),
        );

        // Confirm by two owners (reaches threshold)
        ms.confirm(&o1, &id);
        ms.confirm(&o2, &id);
        ms.execute(&id);

        let prop = ms.get_proposal(&id);
        assert_eq!(prop.status, multisig::ProposalStatus::Executed);
        // The vesting contract should now be paused, confirming the
        // cross-contract call was actually dispatched.
        assert!(vesting.is_paused());
    }

    #[test]
    #[should_panic(expected = "not enough confirmations")]
    fn test_multisig_below_threshold_panics() {
        use multisig::{CallData, MultisigContract, MultisigContractClient, ProposalAction};

        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let o1 = Address::generate(&env);
        let o2 = Address::generate(&env);
        let o3 = Address::generate(&env);

        let mut owners = soroban_sdk::Vec::new(&env);
        owners.push_back(o1.clone());
        owners.push_back(o2.clone());
        owners.push_back(o3.clone());
        let ms_id = env.register(MultisigContract, (owners, 2u32));
        let ms = MultisigContractClient::new(&env, &ms_id);

        let vesting_id = env.register(VestingContract, (ms_id.clone(),));
        let vesting = VestingContractClient::new(&env, &vesting_id);

        let id = ms.submit(
            &o1,
            &ProposalAction::Call(CallData {
                target: vesting_id.clone(),
                function: soroban_sdk::symbol_short!("pause"),
                args: soroban_sdk::Vec::new(&env),
            }),
            &soroban_sdk::String::from_str(&env, "pause vesting"),
        );
        ms.confirm(&o1, &id); // only 1 of 2 needed -> should panic on execute
        ms.execute(&id);

        let _ = vesting;
    }

    /// End-to-end: multisig calls create_schedule with a real token.
    ///
    /// The multisig is the vesting admin.  A 2-of-3 proposal submits
    /// create_schedule, both required owners confirm, and execute() is called.
    /// We assert that the schedule was created and the token balance was
    /// transferred into the vesting contract.
    #[test]
    fn test_multisig_create_schedule() {
        use multisig::{CallData, MultisigContract, MultisigContractClient, ProposalAction};
        use soroban_sdk::{token::StellarAssetClient, IntoVal};

        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let o1 = Address::generate(&env);
        let o2 = Address::generate(&env);
        let o3 = Address::generate(&env);
        let funder = Address::generate(&env);
        let beneficiary = Address::generate(&env);

        // Deploy multisig (2-of-3) using constructor
        let mut owners = soroban_sdk::Vec::new(&env);
        owners.push_back(o1.clone());
        owners.push_back(o2.clone());
        owners.push_back(o3.clone());
        let ms_id = env.register(MultisigContract, (owners, 2u32));
        let ms = MultisigContractClient::new(&env, &ms_id);

        // Deploy vesting with multisig as admin
        let vesting_id = env.register(VestingContract, (ms_id.clone(),));
        let vesting = VestingContractClient::new(&env, &vesting_id);

        // Mint tokens to funder
        let token_id = env.register_stellar_asset_contract_v2(funder.clone());
        let token_addr = token_id.address();
        StellarAssetClient::new(&env, &token_addr).mint(&funder, &100_000);

        let token_client = soroban_sdk::token::Client::new(&env, &token_addr);
        assert_eq!(token_client.balance(&funder), 100_000);

        let start = env.ledger().timestamp();

        // Build the create_schedule params and encode them as contract args.
        // create_schedule takes a single CreateScheduleParams argument.
        let params = CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token_addr.clone(),
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        };

        // Encode the params as Vec<Val> — create_schedule takes one argument.
        let mut args: soroban_sdk::Vec<soroban_sdk::Val> = soroban_sdk::Vec::new(&env);
        args.push_back(params.into_val(&env));

        let id = ms.submit(
            &o1,
            &ProposalAction::Call(CallData {
                target: vesting_id.clone(),
                function: soroban_sdk::Symbol::new(&env, "create_schedule"),
                args,
            }),
            &soroban_sdk::String::from_str(&env, "create schedule for beneficiary"),
        );

        ms.confirm(&o1, &id);
        ms.confirm(&o2, &id);
        ms.execute(&id);

        assert_eq!(
            ms.get_proposal(&id).status,
            multisig::ProposalStatus::Executed
        );

        // Vesting contract should have 1 schedule
        assert_eq!(vesting.schedule_count(), 1);

        // Tokens should have been transferred from funder to vesting contract
        assert_eq!(token_client.balance(&funder), 0);
        assert_eq!(token_client.balance(&vesting_id), 100_000);

        // Advance time past the full duration and let beneficiary claim
        env.ledger().with_mut(|l| l.timestamp = start + 100);
        assert_eq!(vesting.get_claimable(&1u64), 100_000);

        let claimed = vesting.claim(&1u64);
        assert_eq!(claimed, 100_000);
        assert_eq!(token_client.balance(&beneficiary), 100_000);
    }

    // ── Issue #63: create_schedule with a start_time in the future ───────────
    //
    // vested_amount() uses now.saturating_sub(start_time) for elapsed.
    // If now < start_time, saturating_sub returns 0 so nothing is claimable.
    // Once now >= start_time the linear formula applies as normal.
    //
    // Schedule: start_time = now + 500, cliff_duration = 0,
    //           total_duration = 100, total_amount = 1_000
    //
    // Expected:
    //   - At t = start_time - 1: get_claimable() == 0
    //   - At t = start_time + 50 (50% through): get_claimable() == 500
    //   - claim() returns 500; beneficiary balance == 500

    #[test]
    fn test_create_schedule_future_start_time() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 1_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);

        // current ledger timestamp
        let now = env.ledger().timestamp();
        // schedule starts 500 seconds in the future
        let future_start = now + 500;

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token.clone(),
            total_amount: 1_000,
            start_time: future_start,
            cliff_duration: 0,
            total_duration: 100,
        });

        // Before start_time: nothing vested regardless of cliff.
        // now.saturating_sub(future_start) == 0 and now < future_start + 0,
        // so vested_amount returns 0.
        env.ledger().with_mut(|l| l.timestamp = future_start - 1);
        assert_eq!(
            vesting.get_claimable(&id),
            0,
            "expected 0 claimable one second before start_time"
        );

        // At start_time + 50 (50% of total_duration = 100):
        // elapsed = 50, vested = 50 * 1_000 / 100 = 500
        env.ledger().with_mut(|l| l.timestamp = future_start + 50);
        assert_eq!(
            vesting.get_claimable(&id),
            500,
            "expected 500 claimable at 50% of schedule after future start_time"
        );

        // claim returns the correct amount
        let claimed = vesting.claim(&id);
        assert_eq!(claimed, 500, "claim should transfer 500 tokens");

        // beneficiary balance matches
        let token_client = soroban_sdk::token::Client::new(&env, &token);
        assert_eq!(
            token_client.balance(&beneficiary),
            500,
            "beneficiary balance should be 500 after claim"
        );
    }

    // ── Overflow guards ───────────────────────────────────────────────────────

    /// create_schedule with start_time + total_duration overflowing u64 must
    /// panic before any funds are transferred.
    #[test]
    #[should_panic]
    fn test_create_schedule_start_plus_duration_overflow() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);

        // start_time near u64::MAX + total_duration of 1 overflows.
        vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 100_000,
            start_time: u64::MAX,
            cliff_duration: 0,
            total_duration: 1,
        });
    }

    // ── Issue #61: get_claimable() returns 0 for Completed schedules ─────────
    //
    // After fully claiming all tokens the schedule transitions to Completed.
    // get_claimable() should resolve to (total_amount - total_amount).max(0) == 0.

    #[test]
    fn test_get_claimable_on_completed_schedule() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        // Advance past full duration and claim everything.
        env.ledger().with_mut(|l| l.timestamp = start + 100);
        let claimed = vesting.claim(&id);
        assert_eq!(claimed, 100_000);

        // Schedule is now Completed.
        let sched = vesting.get_schedule(&id);
        assert_eq!(sched.status, ScheduleStatus::Completed);

        // get_claimable() must return 0 for a Completed schedule:
        // vested_amount == total_amount, claimed_amount == total_amount,
        // so (total_amount - total_amount).max(0) == 0.
        assert_eq!(vesting.get_claimable(&id), 0);
    }

    // ── Issue #61: get_claimable() returns 0 for Revoked schedules ───────────
    //
    // After revoke() the vested-but-unclaimed portion is paid to the
    // beneficiary and claimed_amount is set to vested_amount.  The unvested
    // portion goes to the recipient.  vested_amount - claimed_amount == 0,
    // so get_claimable() must return 0.

    #[test]
    fn test_get_claimable_on_revoked_schedule() {
        let (env, vesting_id, admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        // Revoke mid-vest at t=40: 40_000 vested, 60_000 unvested.
        // revoke() pays 40_000 to beneficiary and returns 60_000 to admin.
        env.ledger().with_mut(|l| l.timestamp = start + 40);
        let returned = vesting.revoke(&id, &admin);
        assert_eq!(returned, 60_000);

        // Schedule is now Revoked.
        let sched = vesting.get_schedule(&id);
        assert_eq!(sched.status, ScheduleStatus::Revoked);

        // get_claimable() must return 0 after revoke:
        // claimed_amount was set to vested_amount (40_000) inside revoke(),
        // so (40_000 - 40_000).max(0) == 0.
        assert_eq!(vesting.get_claimable(&id), 0);
    }

    // ── Issue #62: create_schedule when cliff_duration equals total_duration ─
    //
    // When cliff_duration == total_duration the entire period is a cliff —
    // there is no linear release phase.  The transition from 0 to total_amount
    // happens in a single ledger step at start_time + total_duration.
    //
    // Schedule: cliff_duration = total_duration = 100, total_amount = 1_000
    //
    // Expected:
    //   - At t = start + 99: get_claimable() == 0 (still in cliff)
    //   - At t = start + 100: get_claimable() == 1_000 (full amount)
    //   - claim() returns 1_000; schedule transitions to Completed

    #[test]
    fn test_create_schedule_cliff_equals_duration() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 1_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        // cliff_duration == total_duration: the whole period is the cliff
        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token.clone(),
            total_amount: 1_000,
            start_time: start,
            cliff_duration: 100,
            total_duration: 100,
        });

        // One second before the end: now < start + cliff_duration (100),
        // so vested_amount returns 0.
        env.ledger().with_mut(|l| l.timestamp = start + 99);
        assert_eq!(
            vesting.get_claimable(&id),
            0,
            "expected 0 claimable one second before cliff/total_duration boundary"
        );

        // Exactly at start + total_duration: elapsed >= total_duration branch
        // returns total_amount directly.
        env.ledger().with_mut(|l| l.timestamp = start + 100);
        assert_eq!(
            vesting.get_claimable(&id),
            1_000,
            "expected full total_amount claimable at start + total_duration"
        );

        // Claim the full amount
        let claimed = vesting.claim(&id);
        assert_eq!(claimed, 1_000, "claim should return the full total_amount");

        // Schedule transitions to Completed
        let sched = vesting.get_schedule(&id);
        assert_eq!(
            sched.status,
            ScheduleStatus::Completed,
            "schedule should be Completed after full claim"
        );

        // Beneficiary received all tokens
        let token_client = soroban_sdk::token::Client::new(&env, &token);
        assert_eq!(
            token_client.balance(&beneficiary),
            1_000,
            "beneficiary should hold the full total_amount"
        );
    }

    /// create_schedule with start_time + cliff_duration overflowing u64 must
    /// panic before any funds are transferred.
    #[test]
    #[should_panic]
    fn test_create_schedule_start_plus_cliff_overflow() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);

        // cliff_duration > total_duration check passes (both near MAX),
        // but start_time + cliff_duration overflows.
        vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 100_000,
            start_time: u64::MAX - 1,
            cliff_duration: u64::MAX - 1,
            total_duration: u64::MAX - 1,
        });
    }

    // ── time_until_fully_vested (#127) ────────────────────────────────────────

    /// Returns a positive value mid-schedule.
    #[test]
    fn test_time_until_fully_vested_mid_schedule() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 200,
        });

        // At t = start + 50: 150 seconds remain.
        env.ledger().with_mut(|l| l.timestamp = start + 50);
        assert_eq!(
            vesting.time_until_fully_vested(&id),
            150,
            "expected 150 seconds remaining mid-schedule"
        );
    }

    /// Returns 0 exactly at total_duration.
    #[test]
    fn test_time_until_fully_vested_at_end() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        env.ledger().with_mut(|l| l.timestamp = start + 100);
        assert_eq!(
            vesting.time_until_fully_vested(&id),
            0,
            "expected 0 at exactly total_duration"
        );
    }

    /// Returns 0 after total_duration (schedule already completed).
    #[test]
    fn test_time_until_fully_vested_after_end() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();
        let token = new_token(&env, &funder, 100_000);
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: funder.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token,
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        // Well past the end.
        env.ledger().with_mut(|l| l.timestamp = start + 500);
        assert_eq!(
            vesting.time_until_fully_vested(&id),
            0,
            "expected 0 after schedule end"
        );
    }

    /// Panics for a non-existent schedule ID.
    #[test]
    #[should_panic]
    fn test_time_until_fully_vested_not_found_panics() {
        let (env, vesting_id, _admin, _beneficiary, _funder) = setup();
        let vesting = VestingContractClient::new(&env, &vesting_id);
        vesting.time_until_fully_vested(&999u64);
    }
}

// ─── Task C: end-to-end tests using the real token contract ──────────────────
//
// These tests import the real `token::TokenContract` (added as a dev-dependency)
// instead of the Stellar asset contract helper.  The token is deployed, minted
// to a funder, and the funder approves the vesting contract to pull tokens via
// `create_schedule`.  Every test asserts exact balances and checks that the
// invariant `funder_out == beneficiary_in + treasury_in` holds (conservation of
// total supply).
//
// The vesting contract calls `token::Client::new(&env, &token_addr).transfer(from, vesting, amount)`
// inside `create_schedule`, which requires `from.require_auth()`.
// `env.mock_all_auths_allowing_non_root_auth()` satisfies all auth requirements.

#[cfg(test)]
mod e2e_token_tests {
    use super::*;
    use crate::test_helpers::{deploy_real_token, deploy_vesting_with_admin};
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Env,
    };

    // ── helpers ──────────────────────────────────────────────────────────────

    /// Deploy the real token contract via constructor with `supply` minted to
    /// `admin`, and return (token_address, client).
    fn deploy_token<'a>(
        env: &'a Env,
        admin: &Address,
        supply: i128,
    ) -> (Address, ::token::TokenContractClient<'a>) {
        deploy_real_token(env, admin, supply)
    }

    /// Deploy vesting with `admin` via constructor and return client.
    fn deploy_vesting<'a>(env: &'a Env, admin: &Address) -> (Address, VestingContractClient<'a>) {
        let id = deploy_vesting_with_admin(env, admin);
        let v = VestingContractClient::new(env, &id);
        (id, v)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // C-1: create_schedule pulls tokens; claim at several timestamps
    // ─────────────────────────────────────────────────────────────────────────

    /// create_schedule transfers `total_amount` from funder to the vesting
    /// contract; claims at t=25, t=50, t=100 return the correct incremental
    /// amounts with exact balance assertions.
    #[test]
    fn test_e2e_create_and_claim_partial_then_full() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let admin = Address::generate(&env);
        let beneficiary = Address::generate(&env);

        let (token_addr, token) = deploy_token(&env, &admin, 100_000);
        let (vesting_id, vesting) = deploy_vesting(&env, &admin);

        let start = env.ledger().timestamp();

        // admin is funder; 100 000 tokens minted to admin
        let id = vesting.create_schedule(&CreateScheduleParams {
            from: admin.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token_addr.clone(),
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        // After create: all tokens are in vesting contract.
        assert_eq!(token.balance(&admin), 0);
        assert_eq!(token.balance(&vesting_id), 100_000);

        // t=25 → 25 000 claimable
        env.ledger().with_mut(|l| l.timestamp = start + 25);
        assert_eq!(vesting.get_claimable(&id), 25_000);
        let c1 = vesting.claim(&id);
        assert_eq!(c1, 25_000);
        assert_eq!(token.balance(&beneficiary), 25_000);
        assert_eq!(token.balance(&vesting_id), 75_000);

        // t=50 → 25 000 more claimable (50 000 total vested − 25 000 claimed)
        env.ledger().with_mut(|l| l.timestamp = start + 50);
        assert_eq!(vesting.get_claimable(&id), 25_000);
        let c2 = vesting.claim(&id);
        assert_eq!(c2, 25_000);
        assert_eq!(token.balance(&beneficiary), 50_000);
        assert_eq!(token.balance(&vesting_id), 50_000);

        // t=100 → fully vested
        env.ledger().with_mut(|l| l.timestamp = start + 100);
        assert_eq!(vesting.get_claimable(&id), 50_000);
        let c3 = vesting.claim(&id);
        assert_eq!(c3, 50_000);
        assert_eq!(token.balance(&beneficiary), 100_000);
        assert_eq!(token.balance(&vesting_id), 0);

        // Schedule is Completed; nothing left to claim
        assert_eq!(vesting.get_schedule(&id).status, ScheduleStatus::Completed);

        // Conservation: total minted == beneficiary received
        assert_eq!(token.total_supply(), 100_000);
        assert_eq!(token.balance(&beneficiary), 100_000);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // C-2: claim after cliff
    // ─────────────────────────────────────────────────────────────────────────

    /// With a cliff, nothing is claimable before the cliff; the full linear
    /// amount is available at and after it.
    #[test]
    fn test_e2e_cliff_gates_claim() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let admin = Address::generate(&env);
        let beneficiary = Address::generate(&env);

        let (token_addr, token) = deploy_token(&env, &admin, 200_000);
        let (_vesting_id, vesting) = deploy_vesting(&env, &admin);

        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: admin.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token_addr,
            total_amount: 200_000,
            start_time: start,
            cliff_duration: 50,
            total_duration: 200,
        });

        // Before cliff: nothing claimable
        env.ledger().with_mut(|l| l.timestamp = start + 49);
        assert_eq!(vesting.get_claimable(&id), 0);

        // At cliff (t=50): elapsed=50, total=200 → 50/200 * 200 000 = 50 000
        env.ledger().with_mut(|l| l.timestamp = start + 50);
        assert_eq!(vesting.get_claimable(&id), 50_000);
        let claimed = vesting.claim(&id);
        assert_eq!(claimed, 50_000);
        assert_eq!(token.balance(&beneficiary), 50_000);

        // At end (t=200): 150 000 more available
        env.ledger().with_mut(|l| l.timestamp = start + 200);
        let claimed2 = vesting.claim(&id);
        assert_eq!(claimed2, 150_000);
        assert_eq!(token.balance(&beneficiary), 200_000);

        // Conservation
        assert_eq!(token.total_supply(), 200_000);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // C-3: revoke splits vested (→ beneficiary) and unvested (→ treasury)
    // ─────────────────────────────────────────────────────────────────────────

    /// At t=30 (30% vested, 0 claimed): revoke pays 30 000 to beneficiary
    /// and returns 70 000 to treasury.  Total supply is conserved.
    #[test]
    fn test_e2e_revoke_splits_correctly() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let admin = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let treasury = Address::generate(&env);

        let (token_addr, token) = deploy_token(&env, &admin, 100_000);
        let (vesting_id, vesting) = deploy_vesting(&env, &admin);

        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: admin.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token_addr.clone(),
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        // Admin has 0 after funding vesting contract
        assert_eq!(token.balance(&admin), 0);
        assert_eq!(token.balance(&vesting_id), 100_000);

        // t=30: 30 000 vested, 0 claimed
        env.ledger().with_mut(|l| l.timestamp = start + 30);

        let returned = vesting.revoke(&id, &treasury);
        assert_eq!(returned, 70_000); // unvested → treasury

        // Beneficiary received the vested 30 000 automatically on revoke
        assert_eq!(token.balance(&beneficiary), 30_000);
        assert_eq!(token.balance(&treasury), 70_000);
        assert_eq!(token.balance(&vesting_id), 0);

        // Conservation
        assert_eq!(token.total_supply(), 100_000);
        assert_eq!(
            token.balance(&beneficiary) + token.balance(&treasury),
            100_000
        );

        // Schedule is Revoked
        assert_eq!(vesting.get_schedule(&id).status, ScheduleStatus::Revoked);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // C-4: revoke after partial claim — only unclaimed vested goes to beneficiary
    // ─────────────────────────────────────────────────────────────────────────

    /// Beneficiary claims 20 000 at t=20, then admin revokes at t=40.
    /// Revoke pays the additional 20 000 vested-but-unclaimed to beneficiary
    /// and 60 000 unvested to treasury.
    #[test]
    fn test_e2e_revoke_after_partial_claim() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let admin = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let treasury = Address::generate(&env);

        let (token_addr, token) = deploy_token(&env, &admin, 100_000);
        let (vesting_id, vesting) = deploy_vesting(&env, &admin);

        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(&CreateScheduleParams {
            from: admin.clone(),
            beneficiary: beneficiary.clone(),
            token_address: token_addr.clone(),
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        // Claim 20 000 at t=20
        env.ledger().with_mut(|l| l.timestamp = start + 20);
        let claimed = vesting.claim(&id);
        assert_eq!(claimed, 20_000);
        assert_eq!(token.balance(&beneficiary), 20_000);
        assert_eq!(token.balance(&vesting_id), 80_000);

        // Revoke at t=40: vested=40 000, claimed=20 000 → pays 20 000 to bene, 60 000 to treasury
        env.ledger().with_mut(|l| l.timestamp = start + 40);
        let returned = vesting.revoke(&id, &treasury);
        assert_eq!(returned, 60_000); // unvested

        assert_eq!(token.balance(&beneficiary), 40_000); // 20k claimed + 20k from revoke
        assert_eq!(token.balance(&treasury), 60_000);
        assert_eq!(token.balance(&vesting_id), 0);

        // Conservation: 20k + 20k + 60k = 100k
        assert_eq!(
            token.balance(&beneficiary) + token.balance(&treasury),
            100_000
        );
        assert_eq!(token.total_supply(), 100_000);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // C-5: total supply conservation across multiple schedules
    // ─────────────────────────────────────────────────────────────────────────

    /// Two schedules for different beneficiaries.  After both fully vest and
    /// claim, the sum of beneficiary balances equals the total minted supply.
    /// No tokens are created or destroyed.
    #[test]
    fn test_e2e_total_supply_conservation() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let admin = Address::generate(&env);
        let b1 = Address::generate(&env);
        let b2 = Address::generate(&env);

        // Mint 300 000 total; 100 000 for b1, 200 000 for b2
        let (token_addr, token) = deploy_token(&env, &admin, 300_000);
        let (vesting_id, vesting) = deploy_vesting(&env, &admin);

        let start = env.ledger().timestamp();

        let id1 = vesting.create_schedule(&CreateScheduleParams {
            from: admin.clone(),
            beneficiary: b1.clone(),
            token_address: token_addr.clone(),
            total_amount: 100_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });
        let id2 = vesting.create_schedule(&CreateScheduleParams {
            from: admin.clone(),
            beneficiary: b2.clone(),
            token_address: token_addr.clone(),
            total_amount: 200_000,
            start_time: start,
            cliff_duration: 0,
            total_duration: 100,
        });

        // All tokens moved into vesting
        assert_eq!(token.balance(&admin), 0);
        assert_eq!(token.balance(&vesting_id), 300_000);

        // Advance past full duration and claim both
        env.ledger().with_mut(|l| l.timestamp = start + 100);

        let c1 = vesting.claim(&id1);
        let c2 = vesting.claim(&id2);
        assert_eq!(c1, 100_000);
        assert_eq!(c2, 200_000);

        // Vesting contract holds nothing
        assert_eq!(token.balance(&vesting_id), 0);

        // Exact balances
        assert_eq!(token.balance(&b1), 100_000);
        assert_eq!(token.balance(&b2), 200_000);

        // Conservation: no tokens created or destroyed
        assert_eq!(token.total_supply(), 300_000);
        assert_eq!(token.balance(&b1) + token.balance(&b2), 300_000);
    }
}

// ─── Property tests (proptest) ───────────────────────────────────────────────
//
// Tests the pure vesting math (VestingContract::vested_amount) over arbitrary
// inputs.  No Env is needed — the function is a pure calculation.
//
// Properties verified:
//   P1. vested never decreases as time advances
//   P2. vested never exceeds total_amount
//   P3. vested is 0 before the cliff
//   P4. vested equals total_amount at or after the end
//
// Run with:  cargo test --package vesting prop_

#[cfg(test)]
mod prop_tests {
    use super::*;
    use crate::test_helpers::dummy;
    use proptest::prelude::*;

    // dummy() is imported from test_helpers for pure-math tests.

    proptest! {
        /// P1 & P2: vested(t) is non-decreasing and never exceeds total_amount.
        #[test]
        fn prop_vested_never_decreases_and_never_exceeds_total(
            total_amount in 1i128..=i64::MAX as i128,
            start_time   in 0u64..=u32::MAX as u64,
            // keep total_duration > 0; cliff <= total_duration
            total_duration in 1u64..=100_000u64,
            cliff_ratio    in 0u64..=100u64,   // cliff = cliff_ratio% of total_duration
            t1             in 0u64..=200_000u64,
            t2             in 0u64..=200_000u64,
        ) {
            let cliff_duration = (cliff_ratio * total_duration) / 100;
            // Guard against overflow in start_time + total_duration
            prop_assume!(start_time.checked_add(total_duration).is_some());
            prop_assume!(start_time.checked_add(cliff_duration).is_some());

            let sched = dummy(total_amount, start_time, cliff_duration, total_duration);
            let (lo, hi) = if t1 <= t2 { (t1, t2) } else { (t2, t1) };

            let v_lo = VestingContract::vested_amount(&sched, lo);
            let v_hi = VestingContract::vested_amount(&sched, hi);

            // P1: non-decreasing
            prop_assert!(v_hi >= v_lo, "vested decreased: v({})={} > v({})={}", lo, v_lo, hi, v_hi);
            // P2: never exceeds total
            prop_assert!(v_lo <= total_amount, "vested({})={} > total={}", lo, v_lo, total_amount);
            prop_assert!(v_hi <= total_amount, "vested({})={} > total={}", hi, v_hi, total_amount);
        }

        /// P3: vested is 0 strictly before the cliff.
        #[test]
        fn prop_zero_before_cliff(
            total_amount   in 1i128..=i64::MAX as i128,
            start_time     in 0u64..=100_000u64,
            cliff_duration in 1u64..=100_000u64,
            // t is strictly before start_time + cliff_duration
            t_offset       in 0u64..=999_999u64,
        ) {
            let total_duration = cliff_duration + 1; // always > cliff
            prop_assume!(start_time.checked_add(total_duration).is_some());
            prop_assume!(start_time.checked_add(cliff_duration).is_some());

            let cliff_end = start_time + cliff_duration;
            // t must be strictly before cliff_end
            let t = t_offset % cliff_end.max(1);

            let sched = dummy(total_amount, start_time, cliff_duration, total_duration);
            let v = VestingContract::vested_amount(&sched, t);
            prop_assert_eq!(v, 0, "expected 0 before cliff at t={}, cliff_end={}", t, cliff_end);
        }

        /// P4: vested equals total_amount at or after end.
        #[test]
        fn prop_equals_total_at_end(
            total_amount   in 1i128..=i64::MAX as i128,
            start_time     in 0u64..=100_000u64,
            total_duration in 1u64..=100_000u64,
            extra          in 0u64..=100_000u64,
        ) {
            prop_assume!(start_time.checked_add(total_duration).is_some());
            let end = start_time + total_duration;
            let t = end.saturating_add(extra);

            let sched = dummy(total_amount, start_time, 0, total_duration);
            let v = VestingContract::vested_amount(&sched, t);
            prop_assert_eq!(v, total_amount, "expected total={} at t={} (end={})", total_amount, t, end);
        }
    }
}

// ─── Shared test helpers ──────────────────────────────────────────────────────
//
// Consolidated test setup utilities used by all three test modules:
//   `tests`, `e2e_token_tests`, and `prop_tests`.
//
// Having a single source of truth here means that adding a new test module
// only requires importing the helpers it needs rather than re-implementing
// them.

#[cfg(test)]
pub(crate) mod test_helpers {
    use super::*;
    use ::token::{TokenContract, TokenContractClient};
    use soroban_sdk::{
        testutils::Address as _,
        token::StellarAssetClient,
        Env, String,
    };

    /// Deploy a `VestingContract` with `admin` and return its contract address.
    pub fn deploy_vesting_with_admin(env: &Env, admin: &Address) -> Address {
        env.register(VestingContract, (admin.clone(),))
    }

    /// Register a `StellarAssetContract` with `funder` as the admin, mint
    /// `amount` to `funder`, and return the token contract address.
    pub fn deploy_stellar_asset_token(env: &Env, funder: &Address, amount: i128) -> Address {
        let token_id = env.register_stellar_asset_contract_v2(funder.clone());
        let addr = token_id.address();
        StellarAssetClient::new(env, &addr).mint(funder, &amount);
        addr
    }

    /// Deploy the real `TokenContract` (dev-dependency) with `supply` minted
    /// to `admin` and return `(token_address, TokenContractClient)`.
    pub fn deploy_real_token<'a>(
        env: &'a Env,
        admin: &Address,
        supply: i128,
    ) -> (Address, TokenContractClient<'a>) {
        let token_id = env.register(
            TokenContract,
            (
                admin.clone(),
                String::from_str(env, "Test Token"),
                String::from_str(env, "TT"),
                7u32,
                supply,
            ),
        );
        let token = TokenContractClient::new(env, &token_id);
        (token_id, token)
    }

    /// Build a minimal `VestingSchedule` for pure-math tests.
    ///
    /// Uses throw-away `Env` instances only to satisfy the `Address` type
    /// requirement; no ledger interaction occurs.
    pub fn dummy(
        total_amount: i128,
        start_time: u64,
        cliff_duration: u64,
        total_duration: u64,
    ) -> VestingSchedule {
        let env = Env::default();
        VestingSchedule {
            id: 1,
            beneficiary: Address::generate(&env),
            token: Address::generate(&env),
            total_amount,
            claimed_amount: 0,
            start_time,
            cliff_duration,
            total_duration,
            status: ScheduleStatus::Active,
        }
    }
}
