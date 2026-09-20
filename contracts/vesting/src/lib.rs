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

// ─── Contract ──────────────────────────────────────────────────────────────

#[contract]
pub struct VestingContract;

#[contractimpl]
impl VestingContract {
    // ── Admin init ──────────────────────────────────────────────────────────

    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&ADMIN) {
            panic!("already initialized");
        }
        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&PAUSED, &false);
        env.storage().instance().set(&SCHED_ID, &0u64);
    }

    // ── Schedule management ──────────────────────────────────────────────────

    /// Create a new vesting schedule. Caller must be admin.
    pub fn create_schedule(env: Env, params: CreateScheduleParams) -> u64 {
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

        env.storage().instance().set(&SCHED_ID, &next_id);

        env.events().publish(
            (EVT_CREATED,),
            (next_id, params.beneficiary, params.total_amount),
        );

        next_id
    }

    /// Claim all currently vested-but-unclaimed tokens for a schedule.
    pub fn claim(env: Env, schedule_id: u64) -> i128 {
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

    /// Revoke an active schedule. Unvested tokens return to `recipient`.
    pub fn revoke(env: Env, schedule_id: u64, recipient: Address) -> i128 {
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
        let unvested = schedule.total_amount - vested;

        schedule.status = ScheduleStatus::Revoked;
        env.storage()
            .persistent()
            .set(&DataKey::Schedule(schedule_id), &schedule);

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
        Self::require_admin(&env);
        env.storage().instance().set(&PAUSED, &true);
        env.events().publish((EVT_PAUSED,), ());
    }

    pub fn unpause(env: Env) {
        Self::require_admin(&env);
        env.storage().instance().set(&PAUSED, &false);
        env.events().publish((EVT_UNPAUSED,), ());
    }

    // ── Admin transfer ──────────────────────────────────────────────────────

    pub fn transfer_admin(env: Env, new_admin: Address) {
        Self::require_admin(&env);
        new_admin.require_auth();
        env.storage().instance().set(&ADMIN, &new_admin);
    }

    // ── Read functions ─────────────────────────────────────────────────────

    pub fn get_schedule(env: Env, schedule_id: u64) -> VestingSchedule {
        env.storage()
            .persistent()
            .get(&DataKey::Schedule(schedule_id))
            .expect("schedule not found")
    }

    pub fn get_claimable(env: Env, schedule_id: u64) -> i128 {
        let schedule: VestingSchedule = env
            .storage()
            .persistent()
            .get(&DataKey::Schedule(schedule_id))
            .expect("schedule not found");
        let now = env.ledger().timestamp();
        (Self::vested_amount(&schedule, now) - schedule.claimed_amount).max(0)
    }

    pub fn get_beneficiary_schedules(env: Env, beneficiary: Address) -> Vec<u64> {
        env.storage()
            .persistent()
            .get(&DataKey::BeneficiarySchedules(beneficiary))
            .unwrap_or(Vec::new(&env))
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

    // ── Internal helpers ─────────────────────────────────────────────────────

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

    fn vested_amount(schedule: &VestingSchedule, now: u64) -> i128 {
        if now < schedule.start_time + schedule.cliff_duration {
            return 0;
        }
        let elapsed = now.saturating_sub(schedule.start_time);
        if elapsed >= schedule.total_duration {
            return schedule.total_amount;
        }
        (schedule.total_amount * i128::from(elapsed)) / i128::from(schedule.total_duration)
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger};
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

        let vesting_id = env.register(VestingContract, ());
        let vesting = VestingContractClient::new(&env, &vesting_id);
        vesting.initialize(&admin);

        (env, vesting_id, admin, beneficiary, funder)
    }

    /// Deploy a fresh token, mint `amount` to `funder`, return token address.
    fn new_token(env: &Env, funder: &Address, amount: i128) -> Address {
        let token_id = env.register_stellar_asset_contract_v2(funder.clone());
        let addr = token_id.address();
        StellarAssetClient::new(env, &addr).mint(funder, &amount);
        addr
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

    // ── revoke returns unvested tokens ───────────────────────────────────────

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

    // ── double initialize must panic ─────────────────────────────────────────

    #[test]
    #[should_panic(expected = "already initialized")]
    fn test_double_initialize_panics() {
        let (env, vesting_id, admin, _b, _f) = setup();
        let vesting = VestingContractClient::new(&env, &vesting_id);
        vesting.initialize(&admin); // second call must panic
    }
}
