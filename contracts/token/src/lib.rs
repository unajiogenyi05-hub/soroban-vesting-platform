//! Soroban Fungible Token Contract (SEP-0041 compatible)
//!
//! A fully featured fungible token with:
//! - mint / burn (admin only)
//! - transfer / transfer_from
//! - approve / allowances
//! - pause / unpause (admin only)
//! - metadata (name, symbol, decimals)
//!
//! # Error codes
//! All contract errors are represented as [`TokenError`] variants. Clients
//! receive a stable `u32` error code via the `#[contracterror]` attribute
//! instead of parsing string messages.
//!
//! # TTL management
//! Instance storage holds ADMIN, PAUSED, TOTAL, NAME, SYMBOL, DECIMALS.
//! If the instance entry is archived the whole contract becomes unusable until
//! it is restored.  State-changing entry points call `bump_instance()` to keep
//! the instance alive on every write.  Pure read-only getters (`name`,
//! `symbol`, `decimals`, `total_supply`, `balance`, `allowance`, `is_paused`,
//! `admin`) do NOT extend the TTL so they remain read-only calls (no write
//! footprint, no fee beyond the base transaction fee).  Use `extend_ttl()` to
//! keep the instance alive from off-chain keep-alive bots.
//! Persistent Balance and Allowance entries are bumped on every write.

#![no_std]
#![allow(deprecated)]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, symbol_short, Address,
    Env, String, Symbol,
};

// ─── Storage keys ─────────────────────────────────────────────────────────────

const ADMIN: Symbol = symbol_short!("ADMIN");
const PAUSED: Symbol = symbol_short!("PAUSED");
const TOTAL: Symbol = symbol_short!("TOTAL");
const NAME: Symbol = symbol_short!("NAME");
const SYMBOL_KEY: Symbol = symbol_short!("SYMBOL");
const DECIMALS: Symbol = symbol_short!("DECIMALS");

// ─── TTL constants ────────────────────────────────────────────────────────────

/// Re-exported from `common` — see that crate for the rationale.
pub use common::{
    INSTANCE_BUMP_LEDGERS, INSTANCE_BUMP_THRESHOLD, PERSISTENT_BUMP_LEDGERS,
    PERSISTENT_BUMP_THRESHOLD,
};

// ─── Error type ──────────────────────────────────────────────────────────────

/// Typed error codes for the token contract.
///
/// Using `#[contracterror]` maps each variant to a stable `u32` code that
/// Soroban clients can match on without string parsing.
#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u32)]
pub enum TokenError {
    /// `decimals > 18` supplied to the constructor.
    DecimalsTooLarge = 1,
    /// `amount` is zero or negative where a strictly positive value is required
    /// (mint, burn, transfer).
    AmountMustBePositive = 2,
    /// `amount < 0` supplied to `approve`.
    AmountCannotBeNegative = 3,
    /// The caller's balance is less than the requested transfer or burn amount.
    InsufficientBalance = 4,
    /// The spender's allowance for `from` is less than the requested amount.
    AllowanceExceeded = 5,
    /// The contract is paused; state-changing operations are blocked.
    TokenIsPaused = 6,
    /// Per-account balance would overflow `i128` on mint.
    BalanceOverflow = 7,
    /// Total supply would overflow `i128` on mint.
    TotalSupplyOverflow = 8,
    /// A required storage key was not found (contract not initialized).
    NotInitialized = 9,
}

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
const EVT_ADM_XFER: Symbol = symbol_short!("admXfer");

// ─── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct TokenContract;

#[contractimpl]
impl TokenContract {
    // ── Constructor ─────────────────────────────────────────────────────────

    /// Initialise the token at deploy time.
    ///
    /// Sets admin, name, symbol, decimals, and paused=false in instance
    /// storage, then mints `initial_supply` to `admin` if it is positive.
    /// Called exactly once by the Soroban host when the contract is deployed
    /// — there is no separate `initialize` call.
    ///
    /// # Auth
    /// None — the constructor runs atomically with deployment.
    ///
    /// # Panics
    /// - `decimals > 18`
    pub fn __constructor(
        env: Env,
        admin: Address,
        name: String,
        symbol: String,
        decimals: u32,
        initial_supply: i128,
    ) {
        if decimals > 18 {
            panic_with_error!(&env, TokenError::DecimalsTooLarge);
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

    /// Mint `amount` new tokens and credit them to `to`.
    ///
    /// Increases `total_supply` and the balance of `to` by `amount`.
    /// Bumps the instance and the `to` balance persistent TTL.
    ///
    /// # Auth
    /// Requires the current admin to have signed the transaction.
    ///
    /// # Panics
    /// - `amount <= 0`
    /// - Contract is paused (`require_not_paused`)
    /// - `to`'s balance would overflow `i128`
    /// - `total_supply` would overflow `i128`
    ///
    /// # Events
    /// Emits `(mint,)` → `(to, amount)`
    pub fn mint(env: Env, to: Address, amount: i128) {
        Self::bump_instance(&env);
        Self::require_admin(&env);
        Self::require_not_paused(&env);
        if amount <= 0 {
            panic_with_error!(&env, TokenError::AmountMustBePositive);
        }
        Self::_mint(&env, &to, amount);
        env.events().publish((EVT_MINT,), (to, amount));
    }

    /// Burn `amount` tokens from `from` and decrease `total_supply`.
    ///
    /// Subtracts `amount` from the balance of `from` and the total supply.
    /// Bumps the instance and the `from` balance persistent TTL.
    ///
    /// # Auth
    /// Requires `from` to have signed the transaction.
    ///
    /// # Panics
    /// - `amount <= 0`
    /// - Contract is paused (`require_not_paused`)
    /// - `from`'s balance < `amount`
    ///
    /// # Events
    /// Emits `(burn,)` → `(from, amount)`
    pub fn burn(env: Env, from: Address, amount: i128) {
        Self::bump_instance(&env);
        from.require_auth();
        Self::require_not_paused(&env);
        if amount <= 0 {
            panic_with_error!(&env, TokenError::AmountMustBePositive);
        }
        let bal = Self::balance_of(&env, &from);
        if bal < amount {
            panic_with_error!(&env, TokenError::InsufficientBalance);
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

    /// Transfer `amount` tokens from `from` to `to`.
    ///
    /// Decreases `from`'s balance and increases `to`'s balance by `amount`.
    /// Bumps the instance and both balances' persistent TTL.
    ///
    /// # Auth
    /// Requires `from` to have signed the transaction.
    ///
    /// # Panics
    /// - `amount <= 0`
    /// - Contract is paused (`require_not_paused`)
    /// - `from`'s balance < `amount`
    ///
    /// # Events
    /// Emits `(transfer,)` → `(from, to, amount)`
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        Self::bump_instance(&env);
        from.require_auth();
        Self::require_not_paused(&env);
        Self::_transfer(&env, &from, &to, amount);
        env.events().publish((EVT_TRANSFER,), (from, to, amount));
    }

    /// Transfer `amount` tokens from `from` to `to` using `spender`'s allowance.
    ///
    /// Deducts `amount` from the `(from, spender)` allowance, then moves the
    /// tokens from `from` to `to`.  Bumps the instance and the allowance and
    /// both balance persistent TTL entries.
    ///
    /// # Auth
    /// Requires `spender` to have signed the transaction.
    ///
    /// # Panics
    /// - `amount <= 0` (enforced by inner `_transfer`)
    /// - Contract is paused (`require_not_paused`)
    /// - Allowance of `(from, spender)` < `amount`
    /// - `from`'s balance < `amount`
    ///
    /// # Events
    /// Emits `(transfer,)` → `(from, to, amount)`
    pub fn transfer_from(env: Env, spender: Address, from: Address, to: Address, amount: i128) {
        Self::bump_instance(&env);
        spender.require_auth();
        Self::require_not_paused(&env);

        let allow_key = DataKey::Allowance(from.clone(), spender.clone());
        let allowance: i128 = env.storage().persistent().get(&allow_key).unwrap_or(0);
        if allowance < amount {
            panic_with_error!(&env, TokenError::AllowanceExceeded);
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

    /// Set the spending allowance that `spender` may use on behalf of `owner`.
    ///
    /// Stores `amount` as the `(owner, spender)` allowance.  Setting `amount`
    /// to `0` effectively revokes the allowance.  Bumps the instance and the
    /// allowance persistent TTL.
    ///
    /// # Auth
    /// Requires `owner` to have signed the transaction.
    ///
    /// # Panics
    /// - `amount < 0`
    /// - Contract is paused (`require_not_paused`)
    ///
    /// # Events
    /// Emits `(approve,)` → `(owner, spender, amount)`
    pub fn approve(env: Env, owner: Address, spender: Address, amount: i128) {
        Self::bump_instance(&env);
        owner.require_auth();
        Self::require_not_paused(&env);
        if amount < 0 {
            panic_with_error!(&env, TokenError::AmountCannotBeNegative);
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

    /// Pause the contract, blocking `mint`, `burn`, `transfer`,
    /// `transfer_from`, and `approve`.
    ///
    /// Sets the `PAUSED` flag to `true` in instance storage.
    /// `transfer_admin` and the read-only getters remain callable while
    /// paused.  Bumps the instance TTL.
    ///
    /// # Auth
    /// Requires the current admin to have signed the transaction.
    ///
    /// # Events
    /// Emits `(pause,)` → `()`
    pub fn pause(env: Env) {
        Self::bump_instance(&env);
        Self::require_admin(&env);
        env.storage().instance().set(&PAUSED, &true);
        env.events().publish((EVT_PAUSE,), ());
    }

    /// Unpause the contract, restoring normal operation.
    ///
    /// Sets the `PAUSED` flag to `false` in instance storage.  All
    /// previously blocked functions (`mint`, `burn`, `transfer`,
    /// `transfer_from`, `approve`) become callable again.  Bumps the
    /// instance TTL.
    ///
    /// # Auth
    /// Requires the current admin to have signed the transaction.
    ///
    /// # Events
    /// Emits `(unpause,)` → `()`
    pub fn unpause(env: Env) {
        Self::bump_instance(&env);
        Self::require_admin(&env);
        env.storage().instance().set(&PAUSED, &false);
        env.events().publish((EVT_UNPAUSE,), ());
    }

    // ── Admin ───────────────────────────────────────────────────────────────

    /// Transfer the admin role to `new_admin`.
    ///
    /// Overwrites the `ADMIN` key in instance storage with `new_admin`.
    /// Requires both the current admin and `new_admin` to sign, which
    /// prevents handing the role to an address that cannot accept it.
    /// Not gated by the pause flag — admin transfer must always be
    /// possible to allow recovery from a stuck state.  Bumps the instance
    /// TTL.
    ///
    /// # Auth
    /// Requires both the current admin **and** `new_admin` to have signed.
    ///
    /// # Panics
    /// - Current admin not initialized in storage
    ///
    /// # Events
    /// Emits `(admXfer,)` → `(old_admin: Address, new_admin: Address)`
    pub fn transfer_admin(env: Env, new_admin: Address) {
        Self::bump_instance(&env);
        let old_admin: Address = env
            .storage()
            .instance()
            .get(&ADMIN)
            .expect("not initialized");
        old_admin.require_auth();
        new_admin.require_auth();
        env.storage().instance().set(&ADMIN, &new_admin);
        env.events()
            .publish((EVT_ADM_XFER,), (old_admin, new_admin));
    }

    // ── Metadata reads ──────────────────────────────────────────────────────

    /// Return the human-readable token name (e.g. `"Vesting Token"`).
    ///
    /// Read-only — does not extend the TTL or write any state.
    ///
    /// # Auth
    /// None.
    ///
    /// # Panics
    /// - Contract was not initialised (storage key missing)
    pub fn name(env: Env) -> String {
        env.storage()
            .instance()
            .get(&NAME)
            .unwrap_or_else(|| panic_with_error!(&env, TokenError::NotInitialized))
    }

    /// Return the token ticker symbol (e.g. `"VEST"`).
    ///
    /// Read-only — does not extend the TTL or write any state.
    ///
    /// # Auth
    /// None.
    ///
    /// # Panics
    /// - Contract was not initialised (storage key missing)
    pub fn symbol(env: Env) -> String {
        env.storage()
            .instance()
            .get(&SYMBOL_KEY)
            .unwrap_or_else(|| panic_with_error!(&env, TokenError::NotInitialized))
    }

    /// Return the number of decimal places used to represent token amounts.
    ///
    /// Defaults to `7` if not set in storage (Stellar convention).
    /// Read-only — does not extend the TTL or write any state.
    ///
    /// # Auth
    /// None.
    pub fn decimals(env: Env) -> u32 {
        env.storage().instance().get(&DECIMALS).unwrap_or(7)
    }

    /// Return the current total token supply (sum of all account balances).
    ///
    /// Returns `0` if no tokens have been minted yet.
    /// Read-only — does not extend the TTL or write any state.
    ///
    /// # Auth
    /// None.
    pub fn total_supply(env: Env) -> i128 {
        env.storage().instance().get(&TOTAL).unwrap_or(0)
    }

    /// Return the token balance of `account`.
    ///
    /// Returns `0` if the account has never received tokens.
    /// Read-only — does not extend the TTL or write any state.
    ///
    /// # Auth
    /// None.
    pub fn balance(env: Env, account: Address) -> i128 {
        Self::balance_of(&env, &account)
    }

    /// Return the remaining spending allowance that `spender` has on behalf of `owner`.
    ///
    /// Returns `0` if no allowance has been set.
    /// Read-only — does not extend the TTL or write any state.
    ///
    /// # Auth
    /// None.
    pub fn allowance(env: Env, owner: Address, spender: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Allowance(owner, spender))
            .unwrap_or(0)
    }

    /// Return `true` if the contract is currently paused, `false` otherwise.
    ///
    /// Read-only — does not extend the TTL or write any state.
    ///
    /// # Auth
    /// None.
    pub fn is_paused(env: Env) -> bool {
        env.storage().instance().get(&PAUSED).unwrap_or(false)
    }

    /// Return the current admin address.
    ///
    /// Read-only — does not extend the TTL or write any state.
    ///
    /// # Auth
    /// None.
    ///
    /// # Panics
    /// - Contract was not initialised (storage key missing)
    pub fn admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&ADMIN)
            .unwrap_or_else(|| panic_with_error!(&env, TokenError::NotInitialized))
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
            .unwrap_or_else(|| panic_with_error!(env, TokenError::NotInitialized));
        admin.require_auth();
    }

    fn require_not_paused(env: &Env) {
        let paused: bool = env.storage().instance().get(&PAUSED).unwrap_or(false);
        if paused {
            panic_with_error!(env, TokenError::TokenIsPaused);
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
        let new_bal = bal
            .checked_add(amount)
            .unwrap_or_else(|| panic_with_error!(env, TokenError::BalanceOverflow));
        env.storage()
            .persistent()
            .set(&DataKey::Balance(to.clone()), &new_bal);
        env.storage().persistent().extend_ttl(
            &DataKey::Balance(to.clone()),
            PERSISTENT_BUMP_THRESHOLD,
            PERSISTENT_BUMP_LEDGERS,
        );
        let total: i128 = env.storage().instance().get(&TOTAL).unwrap_or(0);
        let new_total = total
            .checked_add(amount)
            .unwrap_or_else(|| panic_with_error!(env, TokenError::TotalSupplyOverflow));
        env.storage().instance().set(&TOTAL, &new_total);
    }

    fn _transfer(env: &Env, from: &Address, to: &Address, amount: i128) {
        if amount <= 0 {
            panic_with_error!(env, TokenError::AmountMustBePositive);
        }
        let from_bal = Self::balance_of(env, from);
        if from_bal < amount {
            panic_with_error!(env, TokenError::InsufficientBalance);
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
    use soroban_sdk::testutils::{Address as _, Events, MockAuth, MockAuthInvoke};
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

    /// extend_ttl() bumps the instance TTL above INSTANCE_BUMP_THRESHOLD
    /// without requiring any auth (permissionless keep-alive).
    #[test]
    fn test_extend_ttl() {
        use soroban_sdk::testutils::storage::Instance as _;
        let (env, contract_id, _admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        token.extend_ttl();
        let ttl = env.as_contract(&contract_id, || env.storage().instance().get_ttl());
        assert!(
            ttl > INSTANCE_BUMP_THRESHOLD,
            "instance TTL {ttl} should exceed INSTANCE_BUMP_THRESHOLD after extend_ttl()"
        );
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
    #[should_panic(expected = "Error(Contract, #6)")]
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

    #[test]
    fn test_event_transfer_admin() {
        let (env, contract_id, old_admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);

        token.transfer_admin(&new_admin);

        // transfer_admin() is the only call that emits an event here, so
        // the event list contains exactly one entry.  Both old and new admin
        // must appear in the payload so off-chain indexers can reconstruct the
        // full admin hand-off chain without extra RPC calls.
        assert_eq!(
            env.events().all(),
            vec![
                &env,
                (
                    contract_id.clone(),
                    (symbol_short!("admXfer"),).into_val(&env),
                    (old_admin.clone(), new_admin.clone()).into_val(&env),
                )
            ]
        );

        // The new admin is now in storage.
        assert_eq!(token.admin(), new_admin);
    }

    #[test]
    fn test_token_transfer_admin_revokes_old_admin() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let new_admin = Address::generate(&env);
        let recipient = Address::generate(&env);

        env.set_auths(&[]);
        env.mock_auths(&[
            MockAuth {
                address: &admin,
                invoke: &MockAuthInvoke {
                    contract: &contract_id,
                    fn_name: "transfer_admin",
                    args: (&new_admin,).into_val(&env),
                    sub_invokes: &[],
                },
            },
            MockAuth {
                address: &new_admin,
                invoke: &MockAuthInvoke {
                    contract: &contract_id,
                    fn_name: "transfer_admin",
                    args: (&new_admin,).into_val(&env),
                    sub_invokes: &[],
                },
            },
        ]);
        token.transfer_admin(&new_admin);
        assert_eq!(token.admin(), new_admin);

        env.mock_auths(&[MockAuth {
            address: &admin,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "mint",
                args: (&recipient, 100i128).into_val(&env),
                sub_invokes: &[],
            },
        }]);
        assert_eq!(
            token.try_mint(&recipient, &100),
            Err(Ok(soroban_sdk::Error::from_type_and_code(
                soroban_sdk::xdr::ScErrorType::Context,
                soroban_sdk::xdr::ScErrorCode::InvalidAction,
            )))
        );
        assert_eq!(token.balance(&recipient), 0);

        env.mock_auths(&[MockAuth {
            address: &new_admin,
            invoke: &MockAuthInvoke {
                contract: &contract_id,
                fn_name: "mint",
                args: (&recipient, 100i128).into_val(&env),
                sub_invokes: &[],
            },
        }]);
        token.mint(&recipient, &100);
        assert_eq!(token.balance(&recipient), 100);
    }

    // ── Allowance edge cases ──────────────────────────────────────────────────

    #[test]
    #[should_panic(expected = "Error(Contract, #5)")]
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
    #[should_panic(expected = "Error(Contract, #5)")]
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
    #[should_panic(expected = "Error(Contract, #6)")]
    fn test_approve_while_paused() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let spender = Address::generate(&env);

        token.pause();
        token.approve(&admin, &spender, &100);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #6)")]
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
    #[should_panic(expected = "Error(Contract, #6)")]
    fn test_burn_while_paused() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        token.pause();
        token.burn(&admin, &100);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #6)")]
    fn test_mint_while_paused() {
        let (env, contract_id, _admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let recipient = Address::generate(&env);
        token.pause();
        token.mint(&recipient, &100);
    }

    // ── Invalid amount tests ──────────────────────────────────────────────────

    #[test]
    #[should_panic(expected = "Error(Contract, #4)")]
    fn test_burn_more_than_balance() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        token.burn(&admin, &1_000_001);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #2)")]
    fn test_mint_zero_amount() {
        let (env, contract_id, _admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let recipient = Address::generate(&env);
        token.mint(&recipient, &0);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #2)")]
    fn test_mint_negative_amount() {
        let (env, contract_id, _admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let recipient = Address::generate(&env);
        token.mint(&recipient, &-1);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #2)")]
    fn test_transfer_zero_amount() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let recipient = Address::generate(&env);
        token.transfer(&admin, &recipient, &0);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #3)")]
    fn test_approve_negative_amount() {
        let (env, contract_id, admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let spender = Address::generate(&env);
        token.approve(&admin, &spender, &-1);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #2)")]
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

    // ── Constructor guards ────────────────────────────────────────────────────

    /// The constructor must reject `decimals` above 18 with
    /// `TokenError::DecimalsTooLarge` (contract error code 1). Registering the
    /// contract with `decimals = 19` exercises the guard on line ~84 of lib.rs.
    #[test]
    #[should_panic(expected = "Error(Contract, #1)")]
    fn test_constructor_rejects_decimals_too_large() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        // 19 > 18 — this must panic before any storage is written.
        env.register(
            TokenContract,
            (
                admin.clone(),
                String::from_str(&env, "Bad Token"),
                String::from_str(&env, "BAD"),
                19u32,
                0i128,
            ),
        );
    }

    /// The constructor must accept `decimals = 18` (the boundary, not too large).
    #[test]
    fn test_constructor_accepts_decimals_at_boundary() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let contract_id = env.register(
            TokenContract,
            (
                admin.clone(),
                String::from_str(&env, "Boundary Token"),
                String::from_str(&env, "BND"),
                18u32,
                0i128,
            ),
        );
        let token = TokenContractClient::new(&env, &contract_id);
        assert_eq!(token.decimals(), 18);
    }

    // ── Issue #66: transfer to self leaves balance and total_supply unchanged ─
    //
    // _transfer() reads from_bal, subtracts amount, writes it back, then reads
    // to_bal, adds amount, writes it back.  When from == to both reads hit the
    // same storage key — the net result must be no change.
    //
    // Setup: mint 1_000 to admin; transfer 100 from admin to admin.
    // Expected:
    //   - token.balance(&admin) == 1_000 (unchanged)
    //   - token.total_supply() == 1_000 (unchanged)

    #[test]
    fn test_transfer_to_self() {
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
                1_000i128,
            ),
        );
        let token = TokenContractClient::new(&env, &contract_id);

        // Precondition: admin holds all 1_000 minted tokens
        assert_eq!(token.balance(&admin), 1_000);
        assert_eq!(token.total_supply(), 1_000);

        // Transfer 100 from admin to admin (self-transfer)
        token.transfer(&admin, &admin, &100);

        // Balance must be unchanged
        assert_eq!(
            token.balance(&admin),
            1_000,
            "balance should be unchanged after self-transfer"
        );

        // Total supply must be unchanged
        assert_eq!(
            token.total_supply(),
            1_000,
            "total_supply should be unchanged after self-transfer"
        );
    }

    // ── Overflow guards ───────────────────────────────────────────────────────

    /// Minting i128::MAX into a non-zero balance overflows the per-account
    /// balance — _mint must panic with TokenError::BalanceOverflow.
    #[test]
    #[should_panic(expected = "Error(Contract, #7)")]
    fn test_mint_balance_overflow() {
        let (env, contract_id, _admin) = deploy();
        let token = TokenContractClient::new(&env, &contract_id);
        let recipient = Address::generate(&env);
        // First mint puts some tokens in the account.
        token.mint(&recipient, &1_000);
        // Second mint overflows i128.
        token.mint(&recipient, &i128::MAX);
    }

    /// Minting into two separate accounts where the sum exceeds i128::MAX
    /// overflows the total supply — _mint must panic with
    /// TokenError::TotalSupplyOverflow.
    #[test]
    #[should_panic(expected = "Error(Contract, #8)")]
    fn test_mint_total_supply_overflow() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
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
        let r1 = Address::generate(&env);
        let r2 = Address::generate(&env);
        // Fill total supply to i128::MAX.
        token.mint(&r1, &i128::MAX);
        // Minting any more overflows total supply.
        token.mint(&r2, &1);
    }
}

// ─── Property-based tests (proptest) ─────────────────────────────────────────
//
// Tests the pure token supply/balance arithmetic over arbitrary inputs without
// deploying a full Env.  Both `_mint` and `burn` modify the same TOTAL key;
// we model them here with plain i128 arithmetic to verify:
//
//   P1. total supply never goes negative after any sequence of valid mints and burns
//   P2. sum of all balances equals total supply (conservation)
//
// Run with:  cargo test --package token prop_

#[cfg(test)]
mod prop_tests {
    // The crate is #![no_std]; bring in std::vec::Vec explicitly for tests.
    extern crate std;
    use std::vec::Vec;

    use proptest::prelude::*;

    /// Simulate a sequence of (mint_amount, burn_pct) pairs.
    /// `burn_pct` ∈ [0, 100] determines what fraction of the last account's
    /// balance is burned, ensuring burn ≤ balance (always a valid operation).
    ///
    /// Returns (final_total_supply, sum_of_all_balances).
    fn simulate(ops: &[(i128, u8)]) -> (i128, i128) {
        let mut total: i128 = 0;
        let mut balances: Vec<i128> = Vec::new();

        for &(mint_amount, burn_pct) in ops {
            // Mint: stop if it would overflow i128 (mirrors checked_add in _mint)
            total = match total.checked_add(mint_amount) {
                Some(t) => t,
                None => break,
            };
            balances.push(mint_amount);

            // Burn: remove a fraction of the last account's balance
            if let Some(last) = balances.last_mut() {
                let burn_amount = (*last * burn_pct as i128) / 100;
                if burn_amount > 0 && *last >= burn_amount {
                    *last -= burn_amount;
                    total -= burn_amount;
                }
            }
        }

        let sum: i128 = balances.iter().sum();
        (total, sum)
    }

    proptest! {
        /// P1: total supply is never negative after a valid sequence of mints and burns.
        #[test]
        fn prop_total_supply_never_negative(
            ops in proptest::collection::vec(
                (1i128..=1_000_000i128, 0u8..=100u8),
                0..=50,
            )
        ) {
            let (total, _sum) = simulate(&ops);
            prop_assert!(total >= 0, "total supply went negative: {}", total);
        }

        /// P2: sum of all balances equals total supply (conservation).
        #[test]
        fn prop_balances_sum_equals_total_supply(
            ops in proptest::collection::vec(
                (1i128..=1_000_000i128, 0u8..=100u8),
                0..=50,
            )
        ) {
            let (total, sum) = simulate(&ops);
            prop_assert_eq!(
                total, sum,
                "conservation broken: total={}, sum_of_balances={}",
                total,
                sum
            );
        }
    }
}
