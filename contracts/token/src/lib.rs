//! Soroban Fungible Token Contract (SEP-0041 compatible)
//!
//! A fully featured fungible token with:
//! - mint / burn (admin only)
//! - transfer / transfer_from
//! - approve / allowances
//! - pause / unpause (admin only)
//! - metadata (name, symbol, decimals)
//!
//! # TTL management
//! Instance storage holds ADMIN, PAUSED, TOTAL, NAME, SYMBOL, DECIMALS.
//! If the instance entry is archived the whole contract becomes unusable until
//! it is restored.  Every public entry point calls `bump_instance()`.
//! Persistent Balance and Allowance entries are bumped on every write.

#![no_std]
#![allow(deprecated)]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, String, Symbol,
};

// ─── Storage keys ─────────────────────────────────────────────────────────────

const ADMIN: Symbol = symbol_short!("ADMIN");
const PAUSED: Symbol = symbol_short!("PAUSED");
const TOTAL: Symbol = symbol_short!("TOTAL");
const NAME: Symbol = symbol_short!("NAME");
const SYMBOL_KEY: Symbol = symbol_short!("SYMBOL");
const DECIMALS: Symbol = symbol_short!("DECIMALS");

// ─── TTL constants ────────────────────────────────────────────────────────────

/// Extend instance and persistent storage to ~180 days (ledgers of ~5 s each).
/// This equals the network's `max_entry_ttl` (3 110 400 ledgers on both
/// testnet and mainnet as of protocol 29, per stellar-core soroban-settings).
/// Values above `max_entry_ttl` are silently clamped by the host, so using
/// the exact maximum is both correct and maximally protective against archival.
pub const INSTANCE_BUMP_LEDGERS: u32 = 3_110_400;
/// Only extend when the remaining TTL drops below ~30 days.
pub const INSTANCE_BUMP_THRESHOLD: u32 = 518_400;
/// Persistent entries use the same window.
pub const PERSISTENT_BUMP_LEDGERS: u32 = 3_110_400;
pub const PERSISTENT_BUMP_THRESHOLD: u32 = 518_400;

#[contracttype]
pub enum DataKey {
    Balance(Address),
    Allowance(Address, Address), // (owner, spender)
}

// ─── Events ───────────────────────────────────────────────────────────────────

const EVT_MINT: Symbol = symbol_short!("mint");
const EVT_BURN: Symbol = symbol_short!("burn");
const EVT_TRANSFER: Symbol = symbol_short!("transfer");
const EVT_APPROVE: Symbol = symbol_short!("approve");
const EVT_PAUSE: Symbol = symbol_short!("pause");
const EVT_UNPAUSE: Symbol = symbol_short!("unpause");

// ─── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct TokenContract;

#[contractimpl]
impl TokenContract {
    // ── Constructor ─────────────────────────────────────────────────────────

    /// Constructor: set token metadata and mint initial supply at deploy time.
    pub fn __constructor(
        env: Env,
        admin: Address,
        name: String,
        symbol: String,
        decimals: u32,
        initial_supply: i128,
    ) {
        if decimals > 18 {
            panic!("decimals too large");
        }

        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&PAUSED, &false);
        env.storage().instance().set(&NAME, &name);
        env.storage().instance().set(&SYMBOL_KEY, &symbol);
        env.storage().instance().set(&DECIMALS, &decimals);
        env.storage().instance().set(&TOTAL, &0i128);
        Self::bump_instance(&env);

        if initial_supply > 0 {
            Self::_mint(&env, &admin, initial_supply);
        }
    }

    // ── Mint / Burn ─────────────────────────────────────────────────────────

    pub fn mint(env: Env, to: Address, amount: i128) {
        Self::bump_instance(&env);
        Self::require_admin(&env);
        Self::require_not_paused(&env);
        if amount <= 0 {
            panic!("amount must be positive");
        }
        Self::_mint(&env, &to, amount);
        env.events().publish((EVT_MINT,), (to, amount));
    }

    pub fn burn(env: Env, from: Address, amount: i128) {
        Self::bump_instance(&env);
        from.require_auth();
        Self::require_not_paused(&env);
        if amount <= 0 {
            panic!("amount must be positive");
        }
        let bal = Self::balance_of(&env, &from);
        if bal < amount {
            panic!("insufficient balance");
        }
        let new_bal = bal - amount;
        env.storage()
            .persistent()
            .set(&DataKey::Balance(from.clone()), &new_bal);
        env.storage().persistent().extend_ttl(
            &DataKey::Balance(from.clone()),
            PERSISTENT_BUMP_THRESHOLD,
            PERSISTENT_BUMP_LEDGERS,
        );
        let total: i128 = env.storage().instance().get(&TOTAL).unwrap_or(0);
        env.storage().instance().set(&TOTAL, &(total - amount));
        env.events().publish((EVT_BURN,), (from, amount));
    }

    // ── Transfer ────────────────────────────────────────────────────────────

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        Self::bump_instance(&env);
        from.require_auth();
        Self::require_not_paused(&env);
        Self::_transfer(&env, &from, &to, amount);
        env.events().publish((EVT_TRANSFER,), (from, to, amount));
    }

    pub fn transfer_from(env: Env, spender: Address, from: Address, to: Address, amount: i128) {
        Self::bump_instance(&env);
        spender.require_auth();
        Self::require_not_paused(&env);

        let allow_key = DataKey::Allowance(from.clone(), spender.clone());
        let allowance: i128 = env.storage().persistent().get(&allow_key).unwrap_or(0);
        if allowance < amount {
            panic!("allowance exceeded");
        }
        let new_allowance = allowance - amount;
        env.storage().persistent().set(&allow_key, &new_allowance);
        env.storage().persistent().extend_ttl(
            &allow_key,
            PERSISTENT_BUMP_THRESHOLD,
            PERSISTENT_BUMP_LEDGERS,
        );

        Self::_transfer(&env, &from, &to, amount);
        env.events().publish((EVT_TRANSFER,), (from, to, amount));
    }

    // ── Allowances ──────────────────────────────────────────────────────────

    pub fn approve(env: Env, owner: Address, spender: Address, amount: i128) {
        Self::bump_instance(&env);
        owner.require_auth();
        Self::require_not_paused(&env);
        if amount < 0 {
            panic!("amount cannot be negative");
        }
        let allow_key = DataKey::Allowance(owner.clone(), spender.clone());
        env.storage().persistent().set(&allow_key, &amount);
        env.storage().persistent().extend_ttl(
            &allow_key,
            PERSISTENT_BUMP_THRESHOLD,
            PERSISTENT_BUMP_LEDGERS,
        );
        env.events()
            .publish((EVT_APPROVE,), (owner, spender, amount));
    }

    // ── Pause ───────────────────────────────────────────────────────────────

    pub fn pause(env: Env) {
        Self::bump_instance(&env);
        Self::require_admin(&env);
        env.storage().instance().set(&PAUSED, &true);
        env.events().publish((EVT_PAUSE,), ());
    }

    pub fn unpause(env: Env) {
        Self::bump_instance(&env);
        Self::require_admin(&env);
        env.storage().instance().set(&PAUSED, &false);
        env.events().publish((EVT_UNPAUSE,), ());
    }

    // ── Admin ───────────────────────────────────────────────────────────────

    pub fn transfer_admin(env: Env, new_admin: Address) {
        Self::bump_instance(&env);
        Self::require_admin(&env);
        new_admin.require_auth();
        env.storage().instance().set(&ADMIN, &new_admin);
    }

    // ── Metadata reads ──────────────────────────────────────────────────────

    pub fn name(env: Env) -> String {
        Self::bump_instance(&env);
        env.storage()
            .instance()
            .get(&NAME)
            .expect("not initialized")
    }

    pub fn symbol(env: Env) -> String {
        Self::bump_instance(&env);
        env.storage()
            .instance()
            .get(&SYMBOL_KEY)
            .expect("not initialized")
    }

    pub fn decimals(env: Env) -> u32 {
        Self::bump_instance(&env);
        env.storage().instance().get(&DECIMALS).unwrap_or(7)
    }

    pub fn total_supply(env: Env) -> i128 {
        Self::bump_instance(&env);
        env.storage().instance().get(&TOTAL).unwrap_or(0)
    }

    pub fn balance(env: Env, account: Address) -> i128 {
        Self::bump_instance(&env);
        Self::balance_of(&env, &account)
    }

    pub fn allowance(env: Env, owner: Address, spender: Address) -> i128 {
        Self::bump_instance(&env);
        env.storage()
            .persistent()
            .get(&DataKey::Allowance(owner, spender))
            .unwrap_or(0)
    }

    pub fn is_paused(env: Env) -> bool {
        Self::bump_instance(&env);
        env.storage().instance().get(&PAUSED).unwrap_or(false)
    }

    pub fn admin(env: Env) -> Address {
        Self::bump_instance(&env);
        env.storage()
            .instance()
            .get(&ADMIN)
            .expect("not initialized")
    }

    // ── Internal ────────────────────────────────────────────────────────────

    /// Extend instance storage TTL so ADMIN, PAUSED, TOTAL, NAME, SYMBOL and
    /// DECIMALS do not expire while the contract is in active use.
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
            panic!("token is paused");
        }
    }

    fn balance_of(env: &Env, account: &Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(account.clone()))
            .unwrap_or(0)
    }

    fn _mint(env: &Env, to: &Address, amount: i128) {
        let bal = Self::balance_of(env, to);
        let new_bal = bal + amount;
        env.storage()
            .persistent()
            .set(&DataKey::Balance(to.clone()), &new_bal);
        env.storage().persistent().extend_ttl(
            &DataKey::Balance(to.clone()),
            PERSISTENT_BUMP_THRESHOLD,
            PERSISTENT_BUMP_LEDGERS,
        );
        let total: i128 = env.storage().instance().get(&TOTAL).unwrap_or(0);
        env.storage().instance().set(&TOTAL, &(total + amount));
    }

    fn _transfer(env: &Env, from: &Address, to: &Address, amount: i128) {
        if amount <= 0 {
            panic!("amount must be positive");
        }
        let from_bal = Self::balance_of(env, from);
        if from_bal < amount {
            panic!("insufficient balance");
        }
        let new_from = from_bal - amount;
        env.storage()
            .persistent()
            .set(&DataKey::Balance(from.clone()), &new_from);
        env.storage().persistent().extend_ttl(
            &DataKey::Balance(from.clone()),
            PERSISTENT_BUMP_THRESHOLD,
            PERSISTENT_BUMP_LEDGERS,
        );
        let to_bal = Self::balance_of(env, to);
        let new_to = to_bal + amount;
        env.storage()
            .persistent()
            .set(&DataKey::Balance(to.clone()), &new_to);
        env.storage().persistent().extend_ttl(
            &DataKey::Balance(to.clone()),
            PERSISTENT_BUMP_THRESHOLD,
            PERSISTENT_BUMP_LEDGERS,
        );
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Events};
    use soroban_sdk::{vec, Env, IntoVal};

    fn deploy() -> (Env, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let contract_id = env.register(
            TokenContract,
            (
                admin.clone(),
                String::from_str(&env, "Vesting Token"),
                String::from_str(&env, "VEST"),
                7u32,
                1_000_000i128,
            ),
        );
        (env, contract_id, admin)
    }

    // ── TTL tests ─────────────────────────────────────────────────────────────

    /// mint() bumps instance TTL above INSTANCE_BUMP_THRESHOLD.
    #[test]
    fn test_ttl_instance_bumped_on_mint() {
        use soroban_sdk::testutils::storage::Instance as _;
        let (env, contract_id, _admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let recipient = Address::generate(&env);

        token.mint(&recipient, &1_000);

        let ttl = env.as_contract(&contract_id, || env.storage().instance().get_ttl());
        assert!(
            ttl > INSTANCE_BUMP_THRESHOLD,
            "instance TTL {ttl} should exceed {INSTANCE_BUMP_THRESHOLD}"
        );
    }

    /// _mint (via mint()) bumps persistent Balance TTL.
    #[test]
    fn test_ttl_balance_bumped_on_mint() {
        use soroban_sdk::testutils::storage::Persistent as _;
        let (env, contract_id, _admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let recipient = Address::generate(&env);

        token.mint(&recipient, &1_000);

        let ttl = env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .get_ttl(&DataKey::Balance(recipient.clone()))
        });
        assert!(
            ttl > PERSISTENT_BUMP_THRESHOLD,
            "balance TTL {ttl} should exceed {PERSISTENT_BUMP_THRESHOLD}"
        );
    }

    /// approve() bumps persistent Allowance TTL.
    #[test]
    fn test_ttl_allowance_bumped_on_approve() {
        use soroban_sdk::testutils::storage::Persistent as _;
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let spender = Address::generate(&env);

        token.approve(&admin, &spender, &500);

        let ttl = env.as_contract(&contract_id, || {
            env.storage()
                .persistent()
                .get_ttl(&DataKey::Allowance(admin.clone(), spender.clone()))
        });
        assert!(
            ttl > PERSISTENT_BUMP_THRESHOLD,
            "allowance TTL {ttl} should exceed {PERSISTENT_BUMP_THRESHOLD}"
        );
    }

    // ── Original baseline tests ────────────────────────────────────────────────

    #[test]
    fn test_initial_supply() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        assert_eq!(token.total_supply(), 1_000_000);
        assert_eq!(token.balance(&admin), 1_000_000);
    }

    #[test]
    fn test_transfer() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let recipient = Address::generate(&env);
        token.transfer(&admin, &recipient, &250_000);
        assert_eq!(token.balance(&recipient), 250_000);
        assert_eq!(token.balance(&admin), 750_000);
    }

    #[test]
    fn test_approve_and_transfer_from() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let spender = Address::generate(&env);
        let recipient = Address::generate(&env);

        token.approve(&admin, &spender, &100_000);
        assert_eq!(token.allowance(&admin, &spender), 100_000);

        token.transfer_from(&spender, &admin, &recipient, &60_000);
        assert_eq!(token.allowance(&admin, &spender), 40_000);
        assert_eq!(token.balance(&recipient), 60_000);
    }

    #[test]
    #[should_panic(expected = "token is paused")]
    fn test_pause_blocks_transfer() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let recipient = Address::generate(&env);
        token.pause();
        token.transfer(&admin, &recipient, &100);
    }

    #[test]
    fn test_burn() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        token.burn(&admin, &200_000);
        assert_eq!(token.total_supply(), 800_000);
        assert_eq!(token.balance(&admin), 800_000);
    }

    // ── Event assertions ──────────────────────────────────────────────────────

    #[test]
    fn test_event_mint() {
        let (env, contract_id, _admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let recipient = Address::generate(&env);

        token.mint(&recipient, &5_000);

        assert_eq!(
            env.events().all(),
            vec![
                &env,
                (
                    contract_id.clone(),
                    (symbol_short!("mint"),).into_val(&env),
                    (recipient.clone(), 5_000i128).into_val(&env),
                )
            ]
        );
    }

    #[test]
    fn test_event_burn() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);

        token.burn(&admin, &1_000);

        assert_eq!(
            env.events().all(),
            vec![
                &env,
                (
                    contract_id.clone(),
                    (symbol_short!("burn"),).into_val(&env),
                    (admin.clone(), 1_000i128).into_val(&env),
                )
            ]
        );
    }

    #[test]
    fn test_event_transfer() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let recipient = Address::generate(&env);

        token.transfer(&admin, &recipient, &1_000);

        assert_eq!(
            env.events().all(),
            vec![
                &env,
                (
                    contract_id.clone(),
                    (symbol_short!("transfer"),).into_val(&env),
                    (admin.clone(), recipient.clone(), 1_000i128).into_val(&env),
                )
            ]
        );
    }

    #[test]
    fn test_event_transfer_from() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let spender = Address::generate(&env);
        let recipient = Address::generate(&env);

        token.approve(&admin, &spender, &500);
        token.transfer_from(&spender, &admin, &recipient, &200);

        let contract_events = env.events().all().filter_by_contract(&contract_id);
        let events_slice = contract_events.events();
        assert!(!events_slice.is_empty(), "expected transfer event");
        assert_eq!(token.balance(&recipient), 200);
        assert_eq!(token.allowance(&admin, &spender), 300);
    }

    #[test]
    fn test_event_approve() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let spender = Address::generate(&env);

        token.approve(&admin, &spender, &50_000);

        assert_eq!(
            env.events().all(),
            vec![
                &env,
                (
                    contract_id.clone(),
                    (symbol_short!("approve"),).into_val(&env),
                    (admin.clone(), spender.clone(), 50_000i128).into_val(&env),
                )
            ]
        );
    }

    #[test]
    fn test_event_pause() {
        let (env, contract_id, _admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);

        token.pause();

        assert_eq!(
            env.events().all(),
            vec![
                &env,
                (
                    contract_id.clone(),
                    (symbol_short!("pause"),).into_val(&env),
                    ().into_val(&env),
                )
            ]
        );
    }

    #[test]
    fn test_event_unpause() {
        let (env, contract_id, _admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);

        token.pause();
        token.unpause();

        assert_eq!(
            env.events().all(),
            vec![
                &env,
                (
                    contract_id.clone(),
                    (symbol_short!("unpause"),).into_val(&env),
                    ().into_val(&env),
                )
            ]
        );
    }

    // ── Allowance edge cases ──────────────────────────────────────────────────

    #[test]
    #[should_panic(expected = "allowance exceeded")]
    fn test_allowance_exhaustion() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let spender = Address::generate(&env);
        let recipient = Address::generate(&env);

        token.approve(&admin, &spender, &100);
        token.transfer_from(&spender, &admin, &recipient, &60);
        assert_eq!(token.allowance(&admin, &spender), 40);
        token.transfer_from(&spender, &admin, &recipient, &40);
        assert_eq!(token.allowance(&admin, &spender), 0);
        token.transfer_from(&spender, &admin, &recipient, &1);
    }

    #[test]
    #[should_panic(expected = "allowance exceeded")]
    fn test_transfer_from_over_allowance() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let spender = Address::generate(&env);
        let recipient = Address::generate(&env);

        token.approve(&admin, &spender, &50);
        token.transfer_from(&spender, &admin, &recipient, &100);
    }

    #[test]
    fn test_approve_zero_resets_allowance() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let spender = Address::generate(&env);

        token.approve(&admin, &spender, &500);
        assert_eq!(token.allowance(&admin, &spender), 500);

        token.approve(&admin, &spender, &0);
        assert_eq!(token.allowance(&admin, &spender), 0);
    }

    // ── Paused-state edge cases ───────────────────────────────────────────────

    #[test]
    fn test_unpause_restores_transfer() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let recipient = Address::generate(&env);

        token.pause();
        assert!(token.is_paused());
        token.unpause();
        assert!(!token.is_paused());

        token.transfer(&admin, &recipient, &100);
        assert_eq!(token.balance(&recipient), 100);
    }

    #[test]
    #[should_panic(expected = "token is paused")]
    fn test_approve_while_paused() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let spender = Address::generate(&env);

        token.pause();
        token.approve(&admin, &spender, &100);
    }

    #[test]
    #[should_panic(expected = "token is paused")]
    fn test_transfer_from_while_paused() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let spender = Address::generate(&env);
        let recipient = Address::generate(&env);

        token.approve(&admin, &spender, &100);
        token.pause();
        token.transfer_from(&spender, &admin, &recipient, &50);
    }

    #[test]
    #[should_panic(expected = "token is paused")]
    fn test_burn_while_paused() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        token.pause();
        token.burn(&admin, &100);
    }

    #[test]
    #[should_panic(expected = "token is paused")]
    fn test_mint_while_paused() {
        let (env, contract_id, _admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let recipient = Address::generate(&env);
        token.pause();
        token.mint(&recipient, &100);
    }

    // ── Invalid amount tests ──────────────────────────────────────────────────

    #[test]
    #[should_panic(expected = "insufficient balance")]
    fn test_burn_more_than_balance() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        token.burn(&admin, &1_000_001);
    }

    #[test]
    #[should_panic(expected = "amount must be positive")]
    fn test_mint_zero_amount() {
        let (env, contract_id, _admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let recipient = Address::generate(&env);
        token.mint(&recipient, &0);
    }

    #[test]
    #[should_panic(expected = "amount must be positive")]
    fn test_mint_negative_amount() {
        let (env, contract_id, _admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let recipient = Address::generate(&env);
        token.mint(&recipient, &-1);
    }

    #[test]
    #[should_panic(expected = "amount must be positive")]
    fn test_transfer_zero_amount() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let recipient = Address::generate(&env);
        token.transfer(&admin, &recipient, &0);
    }

    #[test]
    #[should_panic(expected = "amount cannot be negative")]
    fn test_approve_negative_amount() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let spender = Address::generate(&env);
        token.approve(&admin, &spender, &-1);
    }

    #[test]
    #[should_panic(expected = "amount must be positive")]
    fn test_burn_zero_amount() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        token.burn(&admin, &0);
    }

    // ── Unauthorized mint ─────────────────────────────────────────────────────

    #[test]
    #[should_panic]
    fn test_unauthorized_mint() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let stranger = Address::generate(&env);
        let contract_id = env.register(
            TokenContract,
            (
                admin.clone(),
                String::from_str(&env, "T"),
                String::from_str(&env, "T"),
                7u32,
                0i128,
            ),
        );
        let token = TokenContractClient::new(&env, &contract_id);

        let recipient = Address::generate(&env);
        token.mint(&recipient, &1_000);

        let _ = stranger;
    }
}
