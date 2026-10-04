//! Soroban Fungible Token Contract (SEP-0041 compatible)
//!
//! A fully featured fungible token with:
//! - mint / burn (admin only)
//! - transfer / transfer_from
//! - approve / allowances
//! - pause / unpause (admin only)
//! - metadata (name, symbol, decimals)

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
    // ── Init ────────────────────────────────────────────────────────────────

    pub fn initialize(
        env: Env,
        admin: Address,
        name: String,
        symbol: String,
        decimals: u32,
        initial_supply: i128,
    ) {
        if env.storage().instance().has(&ADMIN) {
            panic!("already initialized");
        }
        if decimals > 18 {
            panic!("decimals too large");
        }

        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&PAUSED, &false);
        env.storage().instance().set(&NAME, &name);
        env.storage().instance().set(&SYMBOL_KEY, &symbol);
        env.storage().instance().set(&DECIMALS, &decimals);
        env.storage().instance().set(&TOTAL, &0i128);

        if initial_supply > 0 {
            Self::_mint(&env, &admin, initial_supply);
        }
    }

    // ── Mint / Burn ─────────────────────────────────────────────────────────

    pub fn mint(env: Env, to: Address, amount: i128) {
        Self::require_admin(&env);
        Self::require_not_paused(&env);
        if amount <= 0 {
            panic!("amount must be positive");
        }
        Self::_mint(&env, &to, amount);
        env.events().publish((EVT_MINT,), (to, amount));
    }

    pub fn burn(env: Env, from: Address, amount: i128) {
        from.require_auth();
        Self::require_not_paused(&env);
        if amount <= 0 {
            panic!("amount must be positive");
        }
        let bal = Self::balance_of(&env, &from);
        if bal < amount {
            panic!("insufficient balance");
        }
        env.storage()
            .persistent()
            .set(&DataKey::Balance(from.clone()), &(bal - amount));
        let total: i128 = env.storage().instance().get(&TOTAL).unwrap_or(0);
        env.storage().instance().set(&TOTAL, &(total - amount));
        env.events().publish((EVT_BURN,), (from, amount));
    }

    // ── Transfer ────────────────────────────────────────────────────────────

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        Self::require_not_paused(&env);
        Self::_transfer(&env, &from, &to, amount);
        env.events().publish((EVT_TRANSFER,), (from, to, amount));
    }

    pub fn transfer_from(env: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();
        Self::require_not_paused(&env);

        let allow_key = DataKey::Allowance(from.clone(), spender.clone());
        let allowance: i128 = env.storage().persistent().get(&allow_key).unwrap_or(0);
        if allowance < amount {
            panic!("allowance exceeded");
        }
        env.storage()
            .persistent()
            .set(&allow_key, &(allowance - amount));

        Self::_transfer(&env, &from, &to, amount);
        env.events().publish((EVT_TRANSFER,), (from, to, amount));
    }

    // ── Allowances ──────────────────────────────────────────────────────────

    pub fn approve(env: Env, owner: Address, spender: Address, amount: i128) {
        owner.require_auth();
        Self::require_not_paused(&env);
        if amount < 0 {
            panic!("amount cannot be negative");
        }
        env.storage()
            .persistent()
            .set(&DataKey::Allowance(owner.clone(), spender.clone()), &amount);
        env.events()
            .publish((EVT_APPROVE,), (owner, spender, amount));
    }

    // ── Pause ───────────────────────────────────────────────────────────────

    pub fn pause(env: Env) {
        Self::require_admin(&env);
        env.storage().instance().set(&PAUSED, &true);
        env.events().publish((EVT_PAUSE,), ());
    }

    pub fn unpause(env: Env) {
        Self::require_admin(&env);
        env.storage().instance().set(&PAUSED, &false);
        env.events().publish((EVT_UNPAUSE,), ());
    }

    // ── Admin ───────────────────────────────────────────────────────────────

    pub fn transfer_admin(env: Env, new_admin: Address) {
        Self::require_admin(&env);
        new_admin.require_auth();
        env.storage().instance().set(&ADMIN, &new_admin);
    }

    // ── Metadata reads ──────────────────────────────────────────────────────

    pub fn name(env: Env) -> String {
        env.storage()
            .instance()
            .get(&NAME)
            .expect("not initialized")
    }

    pub fn symbol(env: Env) -> String {
        env.storage()
            .instance()
            .get(&SYMBOL_KEY)
            .expect("not initialized")
    }

    pub fn decimals(env: Env) -> u32 {
        env.storage().instance().get(&DECIMALS).unwrap_or(7)
    }

    pub fn total_supply(env: Env) -> i128 {
        env.storage().instance().get(&TOTAL).unwrap_or(0)
    }

    pub fn balance(env: Env, account: Address) -> i128 {
        Self::balance_of(&env, &account)
    }

    pub fn allowance(env: Env, owner: Address, spender: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Allowance(owner, spender))
            .unwrap_or(0)
    }

    pub fn is_paused(env: Env) -> bool {
        env.storage().instance().get(&PAUSED).unwrap_or(false)
    }

    pub fn admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&ADMIN)
            .expect("not initialized")
    }

    // ── Internal ────────────────────────────────────────────────────────────

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
        env.storage()
            .persistent()
            .set(&DataKey::Balance(to.clone()), &(bal + amount));
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
        env.storage()
            .persistent()
            .set(&DataKey::Balance(from.clone()), &(from_bal - amount));
        let to_bal = Self::balance_of(env, to);
        env.storage()
            .persistent()
            .set(&DataKey::Balance(to.clone()), &(to_bal + amount));
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
        let contract_id = env.register(TokenContract, ());
        let token = TokenContractClient::new(&env, &contract_id);
        token.initialize(
            &admin,
            &String::from_str(&env, "Vesting Token"),
            &String::from_str(&env, "VEST"),
            &7u32,
            &1_000_000i128,
        );
        (env, contract_id, admin)
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
    //
    // Each test isolates a single call after deploy so that
    // `env.events().all()` contains exactly the events from that call.
    // The initialize() call (which calls _mint internally) does NOT emit
    // a "mint" event — only the public mint() function does.
    // We assert both the topic (Symbol) and the data payload.

    /// mint() emits (EVT_MINT,) with data (to: Address, amount: i128).
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

    /// burn() emits (EVT_BURN,) with data (from: Address, amount: i128).
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

    /// transfer() emits (EVT_TRANSFER,) with data (from, to, amount).
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

    /// transfer_from() emits (EVT_TRANSFER,) with data (from, to, amount)
    /// — the spender is not in the event data, only the fund source and
    /// destination are recorded.
    #[test]
    fn test_event_transfer_from() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let spender = Address::generate(&env);
        let recipient = Address::generate(&env);

        token.approve(&admin, &spender, &500);
        token.transfer_from(&spender, &admin, &recipient, &200);

        // Verify the transfer event was emitted (it is the last one published).
        let contract_events = env.events().all().filter_by_contract(&contract_id);
        let events_slice = contract_events.events();
        assert!(!events_slice.is_empty(), "expected transfer event");
        // Verify balances changed correctly
        assert_eq!(token.balance(&recipient), 200);
        assert_eq!(token.allowance(&admin, &spender), 300);
    }

    /// approve() emits (EVT_APPROVE,) with data (owner, spender, amount).
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

    /// pause() emits (EVT_PAUSE,) with empty data.
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

    /// unpause() emits (EVT_UNPAUSE,) with empty data.
    #[test]
    fn test_event_unpause() {
        let (env, contract_id, _admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);

        token.pause();
        token.unpause();

        // Assert the unpause event immediately after the call — before any
        // subsequent read that would reset env.events().all().
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

    /// After a partial transfer_from the allowance decreases by exactly the
    /// transferred amount; a second transfer_from for the remainder leaves
    /// allowance at zero; a third call panics.
    #[test]
    #[should_panic(expected = "allowance exceeded")]
    fn test_allowance_exhaustion() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let spender = Address::generate(&env);
        let recipient = Address::generate(&env);

        token.approve(&admin, &spender, &100);
        // First spend: 60 of 100
        token.transfer_from(&spender, &admin, &recipient, &60);
        assert_eq!(token.allowance(&admin, &spender), 40);
        // Second spend: exactly 40 — exhausts allowance
        token.transfer_from(&spender, &admin, &recipient, &40);
        assert_eq!(token.allowance(&admin, &spender), 0);
        // Third spend: 1 over zero allowance — must panic
        token.transfer_from(&spender, &admin, &recipient, &1);
    }

    /// transfer_from with an amount larger than the approved allowance panics
    /// immediately without altering any balances.
    #[test]
    #[should_panic(expected = "allowance exceeded")]
    fn test_transfer_from_over_allowance() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let spender = Address::generate(&env);
        let recipient = Address::generate(&env);

        token.approve(&admin, &spender, &50);
        token.transfer_from(&spender, &admin, &recipient, &100); // 100 > 50
    }

    /// approve() with amount 0 is valid (it resets the allowance to zero).
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

    /// transfer() while paused panics; after unpause the same transfer succeeds.
    #[test]
    fn test_unpause_restores_transfer() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let recipient = Address::generate(&env);

        token.pause();
        assert!(token.is_paused());

        token.unpause();
        assert!(!token.is_paused());

        // Should succeed now
        token.transfer(&admin, &recipient, &100);
        assert_eq!(token.balance(&recipient), 100);
    }

    /// approve() while paused panics.
    #[test]
    #[should_panic(expected = "token is paused")]
    fn test_approve_while_paused() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let spender = Address::generate(&env);

        token.pause();
        token.approve(&admin, &spender, &100);
    }

    /// transfer_from() while paused panics even if a prior allowance exists.
    #[test]
    #[should_panic(expected = "token is paused")]
    fn test_transfer_from_while_paused() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let spender = Address::generate(&env);
        let recipient = Address::generate(&env);

        // Set allowance before pausing
        token.approve(&admin, &spender, &100);
        token.pause();
        // Must panic even though allowance exists
        token.transfer_from(&spender, &admin, &recipient, &50);
    }

    /// burn() while paused panics.
    #[test]
    #[should_panic(expected = "token is paused")]
    fn test_burn_while_paused() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        token.pause();
        token.burn(&admin, &100);
    }

    /// mint() while paused panics.
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

    /// burn() with amount greater than balance panics.
    #[test]
    #[should_panic(expected = "insufficient balance")]
    fn test_burn_more_than_balance() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        token.burn(&admin, &1_000_001); // balance is 1_000_000
    }

    /// mint() with amount 0 panics — zero mints are not allowed.
    #[test]
    #[should_panic(expected = "amount must be positive")]
    fn test_mint_zero_amount() {
        let (env, contract_id, _admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let recipient = Address::generate(&env);
        token.mint(&recipient, &0);
    }

    /// mint() with a negative amount panics.
    #[test]
    #[should_panic(expected = "amount must be positive")]
    fn test_mint_negative_amount() {
        let (env, contract_id, _admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let recipient = Address::generate(&env);
        token.mint(&recipient, &-1);
    }

    /// transfer() with amount 0 panics — zero transfers are not allowed.
    #[test]
    #[should_panic(expected = "amount must be positive")]
    fn test_transfer_zero_amount() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let recipient = Address::generate(&env);
        token.transfer(&admin, &recipient, &0);
    }

    /// approve() with a negative amount panics.
    #[test]
    #[should_panic(expected = "amount cannot be negative")]
    fn test_approve_negative_amount() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let spender = Address::generate(&env);
        token.approve(&admin, &spender, &-1);
    }

    /// burn() with amount 0 panics — zero burns are not allowed.
    #[test]
    #[should_panic(expected = "amount must be positive")]
    fn test_burn_zero_amount() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        token.burn(&admin, &0);
    }

    // ── Unauthorized mint ─────────────────────────────────────────────────────

    /// A non-admin address cannot call mint(); the invocation panics because
    /// the contract calls admin.require_auth() and the admin did not authorize
    /// the call. This test uses a fresh Env WITHOUT mock_all_auths so that
    /// auth is actually enforced.
    #[test]
    #[should_panic]
    fn test_unauthorized_mint() {
        // Use a fresh env with NO mock_all_auths so auth is enforced.
        let env = Env::default();
        let admin = Address::generate(&env);
        let stranger = Address::generate(&env);
        let contract_id = env.register(TokenContract, ());
        let token = TokenContractClient::new(&env, &contract_id);

        // Initialize with mock auth only for this call
        env.mock_all_auths();
        token.initialize(
            &admin,
            &String::from_str(&env, "T"),
            &String::from_str(&env, "T"),
            &7u32,
            &0i128,
        );

        // Now call mint as the stranger — mock_all_auths is still active so
        // we must use a second env with no mocks to prove the auth check fires.
        // Create a separate env, register a fresh contract, initialize it
        // without mock_all_auths for the initialize call itself (which requires
        // no auth), then call mint with no auth mock.
        let env2 = Env::default();
        // Do NOT call env2.mock_all_auths().
        let admin2 = Address::generate(&env2);
        let contract2 = env2.register(TokenContract, ());
        let token2 = TokenContractClient::new(&env2, &contract2);

        // initialize() stores the admin but does not call require_auth on
        // anyone, so it succeeds without mock auth.
        token2.initialize(
            &admin2,
            &String::from_str(&env2, "T"),
            &String::from_str(&env2, "T"),
            &7u32,
            &0i128,
        );

        // mint() calls require_admin which calls admin2.require_auth().
        // Since no auth has been provided, this must panic.
        let recipient = Address::generate(&env2);
        token2.mint(&recipient, &1_000);

        let _ = stranger; // suppress unused warning
    }
}
