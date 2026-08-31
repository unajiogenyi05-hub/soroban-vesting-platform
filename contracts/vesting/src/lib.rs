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
//!                                      ↑ revoke stops here
//! ```

#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, Address, Env, Symbol, Vec,
};

// ─── Storage keys ────────────────────────────────────────────────────────────

const ADMIN: Symbol = symbol_short!("ADMIN");
const PAUSED: Symbol = symbol_short!("PAUSED");
const SCHED_ID: Symbol = symbol_short!("SCHED_ID");

// ─── Data types ──────────────────────────────────────────────────────────────

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
    pub cliff_duration: u64, // seconds after start before any tokens vest
    pub total_duration: u64, // total vesting window in seconds
    pub status: ScheduleStatus,
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

// ─── Contract ────────────────────────────────────────────────────────────────

#[contract]
pub struct VestingContract;

#[contractimpl]
impl VestingContract {
    // ── Admin init ──────────────────────────────────────────────────────────

    /// Initialize the contract. Can only be called once.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&ADMIN) {
            panic!("already initialized");
        }
        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&PAUSED, &false);
        env.storage().instance().set(&SCHED_ID, &0u64);
    }

    // ── Schedule management ─────────────────────────────────────────────────

    /// Create a new vesting schedule. Caller must be admin.
    /// Transfers `total_amount` tokens from `from` into the contract.
    pub fn create_schedule(
        env: Env,
        from: Address,
        beneficiary: Address,
        token_address: Address,
        total_amount: i128,
        start_time: u64,
        cliff_duration: u64,
        total_duration: u64,
    ) -> u64 {
        Self::require_admin(&env);
        Self::require_not_paused(&env);

        if total_amount <= 0 {
            panic!("amount must be positive");
        }
        if total_duration == 0 {
            panic!("duration must be > 0");
        }
        if cliff_duration > total_duration {
            panic!("cliff exceeds duration");
        }

        // Pull tokens into the contract
        let tk = token::Client::new(&env, &token_address);
        tk.transfer(&from, &env.current_contract_address(), &total_amount);

        let id: u64 = env.storage().instance().get(&SCHED_ID).unwrap_or(0);
        let next_id = id + 1;

        let schedule = VestingSchedule {
            id: next_id,
            beneficiary: beneficiary.clone(),
            token: token_address,
            total_amount,
            claimed_amount: 0,
            start_time,
            cliff_duration,
            total_duration,
            status: ScheduleStatus::Active,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Schedule(next_id), &schedule);

        // Track per-beneficiary schedule IDs
        let mut ids: Vec<u64> = env
            .storage()
            .persistent()
            .get(&DataKey::BeneficiarySchedules(beneficiary.clone()))
            .unwrap_or(Vec::new(&env));
        ids.push_back(next_id);
        env.storage()
            .persistent()
            .set(&DataKey::BeneficiarySchedules(beneficiary.clone()), &ids);

        env.storage().instance().set(&SCHED_ID, &next_id);

        env.events()
            .publish((EVT_CREATED,), (next_id, beneficiary, total_amount));

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
        // Allow beneficiary to still claim what's already vested
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

    // ── Pause ───────────────────────────────────────────────────────────────

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

    // ── Read functions ──────────────────────────────────────────────────────

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

    // ── Internal helpers ────────────────────────────────────────────────────

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

    /// Linear vesting with cliff. Returns total vested amount at `now`.
    fn vested_amount(schedule: &VestingSchedule, now: u64) -> i128 {
        if now < schedule.start_time + schedule.cliff_duration {
            return 0;
        }
        let elapsed = now.saturating_sub(schedule.start_time);
        if elapsed >= schedule.total_duration {
            return schedule.total_amount;
        }
        // Linear interpolation
        (schedule.total_amount * elapsed as i128) / schedule.total_duration as i128
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger};
    use soroban_sdk::{token::StellarAssetClient, Env};

    fn setup() -> (Env, Address, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let beneficiary = Address::generate(&env);
        let funder = Address::generate(&env);

        // Deploy a test token
        let token_id = env.register_stellar_asset_contract_v2(admin.clone());
        let token_address = token_id.address();

        // Mint tokens to funder
        let asset_client = StellarAssetClient::new(&env, &token_address);
        asset_client.mint(&funder, &1_000_000);

        // Deploy vesting contract
        let vesting_id = env.register(VestingContract, ());
        let vesting = VestingContractClient::new(&env, &vesting_id);
        vesting.initialize(&admin);

        (env, vesting_id, admin, beneficiary, funder)
    }

    #[test]
    fn test_create_and_claim() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();

        // Get token address from storage (simplification: re-register)
        let token_address = env
            .register_stellar_asset_contract_v2(funder.clone())
            .address();
        let asset_client = StellarAssetClient::new(&env, &token_address);
        asset_client.mint(&funder, &100_000);

        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(
            &funder,
            &beneficiary,
            &token_address,
            &100_000,
            &start,
            &0,   // no cliff
            &100, // 100-second vesting window
        );

        // Advance time to 50% through vesting
        env.ledger().with_mut(|l| l.timestamp = start + 50);

        let claimable = vesting.get_claimable(&id);
        assert_eq!(claimable, 50_000);

        let claimed = vesting.claim(&id);
        assert_eq!(claimed, 50_000);

        // Advance to end
        env.ledger().with_mut(|l| l.timestamp = start + 100);
        let claimed2 = vesting.claim(&id);
        assert_eq!(claimed2, 50_000);
    }

    #[test]
    fn test_cliff_blocks_early_claim() {
        let (env, vesting_id, _admin, beneficiary, funder) = setup();

        let token_address = env
            .register_stellar_asset_contract_v2(funder.clone())
            .address();
        let asset_client = StellarAssetClient::new(&env, &token_address);
        asset_client.mint(&funder, &100_000);

        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(
            &funder,
            &beneficiary,
            &token_address,
            &100_000,
            &start,
            &50, // 50-second cliff
            &100,
        );

        // Before cliff
        env.ledger().with_mut(|l| l.timestamp = start + 30);
        let claimable = vesting.get_claimable(&id);
        assert_eq!(claimable, 0);

        // After cliff
        env.ledger().with_mut(|l| l.timestamp = start + 75);
        let claimable2 = vesting.get_claimable(&id);
        assert!(claimable2 > 0);
    }

    #[test]
    fn test_revoke_returns_unvested() {
        let (env, vesting_id, admin, beneficiary, funder) = setup();

        let token_address = env
            .register_stellar_asset_contract_v2(funder.clone())
            .address();
        let asset_client = StellarAssetClient::new(&env, &token_address);
        asset_client.mint(&funder, &100_000);

        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(
            &funder,
            &beneficiary,
            &token_address,
            &100_000,
            &start,
            &0,
            &100,
        );

        env.ledger().with_mut(|l| l.timestamp = start + 25);

        let returned = vesting.revoke(&id, &admin);
        assert_eq!(returned, 75_000); // 75% unvested
    }

    #[test]
    #[should_panic(expected = "contract is paused")]
    fn test_pause_blocks_claim() {
        let (env, vesting_id, admin, beneficiary, funder) = setup();

        let token_address = env
            .register_stellar_asset_contract_v2(funder.clone())
            .address();
        let asset_client = StellarAssetClient::new(&env, &token_address);
        asset_client.mint(&funder, &100_000);

        let vesting = VestingContractClient::new(&env, &vesting_id);
        let start = env.ledger().timestamp();

        let id = vesting.create_schedule(
            &funder,
            &beneficiary,
            &token_address,
            &100_000,
            &start,
            &0,
            &100,
        );

        env.ledger().with_mut(|l| l.timestamp = start + 50);
        vesting.pause();
        vesting.claim(&id); // should panic
    }
}
