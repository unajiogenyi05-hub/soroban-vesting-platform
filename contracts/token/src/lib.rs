//! Soroban Fungible Token Contract (SEP-0041 compatible)
//!
//! A fully featured fungible token with:
//! - mint / burn (admin only)
//! - transfer / transfer_from
//! - approve / allowances
//! - pause / unpause (admin only)
//! - metadata (name, symbol, decimals)

#![no_std]

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
        let allowance: i128 = env
            .storage()
            .persistent()
            .get(&allow_key)
            .unwrap_or(0);
        if allowance < amount {
            panic!("allowance exceeded");
        }
        env.storage()
            .persistent()
            .set(&allow_key, &(allowance - amount));

        Self::_transfer(&env, &from, &to, amount);
        env.events()
            .publish((EVT_TRANSFER,), (from, to, amount));
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
        env.events().publish((EVT_APPROVE,), (owner, spender, amount));
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
        env.storage().instance().get(&NAME).expect("not initialized")
    }

    pub fn symbol(env: Env) -> String {
        env.storage().instance().get(&SYMBOL_KEY).expect("not initialized")
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
        env.storage().instance().get(&ADMIN).expect("not initialized")
    }

    // ── Internal ────────────────────────────────────────────────────────────

    fn require_admin(env: &Env) {
        let admin: Address = env.storage().instance().get(&ADMIN).expect("not initialized");
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
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::Env;

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
}
