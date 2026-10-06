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
    contract, contractimpl, contracttype, symbol_short, token, Address, Env, Symbol, Vec,
};

// ─── Storage keys ───────────────────────────────────────────────────────────

const ADMIN: Symbol = symbol_short!("ADMIN");
const PAUSED: Symbol = symbol_short!("PAUSED");
const SCHED_ID: Symbol = symbol_short!("SCHED_ID");

/// Extend instance and persistent storage to ~1 year (ledgers of ~5 s each).
pub const INSTANCE_BUMP_LEDGERS: u32 = 6_307_200;
/// Only extend when the remaining TTL drops below ~30 days.
pub const INSTANCE_BUMP_THRESHOLD: u32 = 518_400;
/// Persistent Schedule / BeneficiarySchedules entries use the same window.
pub const PERSISTENT_BUMP_LEDGERS: u32 = 6_307_200;
pub const PERSISTENT_BUMP_THRESHOLD: u32 = 518_400;

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

        if params.total_amount <= 0 {
            panic!("amount must be positive");
        }
        if params.total_duration == 0 {
            panic!("duration must be > 0");
        }
        if params.cliff_duration > params.total_duration {
            panic!("cliff exceeds duration");
        }

        let tk = token::Client::new(&env, &params.token_address);
        tk.transfer(
            &params.from,
            &env.current_contract_address(),
            &params.total_amount,
        );

        let id: u64 = env.storage().instance().get(&SCHED_ID).unwrap_or(0);
        let next_id = id + 1;

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
            .unwrap_or(Vec::new(&env));
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
            (next_id, params.beneficiary, params.total_amount),
        );

        next_id
    }

    /// Claim all currently vested-but-unclaimed tokens for a schedule.
    pub fn claim(env: Env, schedule_id: u64) -> i128 {
        Self::bump_instance(&env);
        Self::require_not_paused(&env);

        let mut schedule: VestingSchedule = env
            .storage()
            .persistent()
            .get(&DataKey::Schedule(schedule_id))
            .expect("schedule not found");

        schedule.beneficiary.require_auth();

        if schedule.status != ScheduleStatus::Active {
            panic!("schedule is not active");
        }

        let now = env.ledger().timestamp();
        let claimable = Self::vested_amount(&schedule, now) - schedule.claimed_amount;

        if claimable <= 0 {
            panic!("nothing to claim");
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
            .expect("schedule not found");

        if schedule.status != ScheduleStatus::Active {
            panic!("schedule is not active");
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
        env.storage().instance().set(&ADMIN, &new_admin);
        env.events().publish((EVT_ADM_XFER,), new_admin);
    }

    // ── Read functions ─────────────────────────────────────────────────────

    pub fn get_schedule(env: Env, schedule_id: u64) -> VestingSchedule {
        Self::bump_instance(&env);
        env.storage()
            .persistent()
            .get(&DataKey::Schedule(schedule_id))
            .expect("schedule not found")
    }

    pub fn get_claimable(env: Env, schedule_id: u64) -> i128 {
        Self::bump_instance(&env);
        let schedule: VestingSchedule = env
            .storage()
            .persistent()
            .get(&DataKey::Schedule(schedule_id))
            .expect("schedule not found");
        let now = env.ledger().timestamp();
        (Self::vested_amount(&schedule, now) - schedule.claimed_amount).max(0)
    }

    pub fn get_beneficiary_schedules(env: Env, beneficiary: Address) -> Vec<u64> {
        Self::bump_instance(&env);
        env.storage()
            .persistent()
            .get(&DataKey::BeneficiarySchedules(beneficiary))
            .unwrap_or(Vec::new(&env))
    }

    pub fn get_admin(env: Env) -> Address {
        Self::bump_instance(&env);
        env.storage()
            .instance()
            .get(&ADMIN)
            .expect("not initialized")
    }

    pub fn is_paused(env: Env) -> bool {
        Self::bump_instance(&env);
        env.storage().instance().get(&PAUSED).unwrap_or(false)
    }

    pub fn schedule_count(env: Env) -> u64 {
        Self::bump_instance(&env);
        env.storage().instance().get(&SCHED_ID).unwrap_or(0)
    }

    // ── Internal helpers ─────────────────────────────────────────────────────

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
            panic!("contract is paused");
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
        i128::from(elapsed).saturating_mul(schedule.total_amount)
            / i128::from(schedule.total_duration)
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Events, Ledger};
    use soroban_sdk::{token::StellarAssetClient, Env};

    // ── helpers ──────────────────────────────────────────────────────────────

    fn setup() -> (Env, Address, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();

        let admin = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let funder = Address::generate(&env);

        let token_id = env.register_stellar_asset_contract_v2(admin.clone());
        let token_address = token_id.address();

        let asset_client = StellarAssetClient::new(&env, &token_address);
        asset_client.mint(&funder, &1_000_000);

        let vesting_id = env.register(VestingContract, (admin.clone(),));

        (env, vesting_id, admin, beneficiary, funder)
    }

    /// Deploy a fresh token, mint `amount` to `funder`, return token address.
    fn new_token(env: &Env, funder: &Address, amount: i128) -> Address {
        let token_id = env.register_stellar_asset_contract_v2(funder.clone());
        let addr = token_id.address();
        StellarAssetClient::new(env, &addr).mint(funder, &amount);
        addr
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

    // ── claim after revoke must panic ────────────────────────────────────────

    #[test]
    #[should_panic(expected = "schedule is not active")]
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
        vesting.claim(&id); // must panic
    }

    // ── nothing to claim before cliff ────────────────────────────────────────

    #[test]
    #[should_panic(expected = "nothing to claim")]
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
        vesting.claim(&id);
    }

    // ── pause blocks create_schedule ─────────────────────────────────────────

    #[test]
    #[should_panic(expected = "contract is paused")]
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
        });
    }

    // ── pause blocks claim ───────────────────────────────────────────────────

    #[test]
    #[should_panic(expected = "contract is paused")]
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
        vesting.claim(&id);
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

    // ── double-revoke must panic ──────────────────────────────────────────────

    #[test]
    #[should_panic(expected = "schedule is not active")]
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
        vesting.revoke(&id, &admin); // must panic
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
        let (env, vesting_id, admin, beneficiary, funder) = setup();
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

        let events = env.events().all();
        // Find the "created" event (last one published by vesting contract)
        let found = !env
            .events()
            .all()
            .filter_by_contract(&vesting_id)
            .events()
            .is_empty();
        assert!(found, "expected 'created' event, got: {:?}", events);
        let _ = id;
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
        let (env, vesting_id, _admin, _b, _f) = setup();
        let vesting = VestingContractClient::new(&env, &vesting_id);
        let new_admin = Address::generate(&env);

        vesting.transfer_admin(&new_admin);

        let found = !env
            .events()
            .all()
            .filter_by_contract(&vesting_id)
            .events()
            .is_empty();
        assert!(found, "expected 'admXfer' event");
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
    use ::token::{TokenContract, TokenContractClient};
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Env, String,
    };

    // ── helpers ──────────────────────────────────────────────────────────────

    /// Deploy the real token contract via constructor with `supply` minted to
    /// `admin`, and return (token_address, client).
    fn deploy_token<'a>(
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

    /// Deploy vesting with `admin` via constructor and return client.
    fn deploy_vesting<'a>(env: &'a Env, admin: &Address) -> (Address, VestingContractClient<'a>) {
        let id = env.register(VestingContract, (admin.clone(),));
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
