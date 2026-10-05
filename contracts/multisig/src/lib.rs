//! Soroban Multisig Governance Contract
//!
//! N-of-M multisignature wallet / governance primitive.
//!
//! # Flow
//! 1. An owner submits a proposal, specifying a `ProposalAction` and a
//!    human-readable description.
//! 2. Other owners confirm it.
//! 3. Once `threshold` confirmations are reached anyone can execute it.
//!    execute() marks the proposal Executed **before** dispatching the
//!    action (checks-effects-interactions pattern).
//! 4. Any owner can revoke their own confirmation before execution.
//!
//! # Permissionless execute
//! Once the threshold is met, any caller (owner or not) may trigger execution.
//! Owners expressed consent through their confirmations; no additional gate
//! is needed at execution time.
//!
//! # Owner management
//! `add_owner`, `remove_owner`, and `update_threshold` are NOT public entry
//! points.  They can only be triggered by submitting a proposal with the
//! corresponding `ProposalAction` variant, confirming it to threshold, and
//! calling `execute()`.  This means every owner-set change requires M-of-N
//! agreement — no single key can alter the owner set unilaterally.
//!
//! Soroban does not allow a contract to invoke itself via `env.invoke_contract`
//! (re-entrancy is disallowed at the host level).  Therefore using a
//! `ProposalAction::Call` that targets the multisig contract itself would
//! trap.  Internal actions (`AddOwner`, `RemoveOwner`, `UpdateThreshold`) are
//! handled directly inside `execute()` without a cross-contract call, so they
//! work correctly.
//!
//! # Confirmation invalidation on owner removal
//! When an owner is removed via `execute(ProposalAction::RemoveOwner)`, any
//! existing confirmations they have given on *other* pending proposals remain
//! in storage but are logically void: `require_owner` will reject them from
//! confirming again, and any pending proposal they confirmed will have its
//! `confirmation_count` decremented by 1 so the threshold accounting stays
//! accurate.  The implementation scans all proposals up to `prop_count` and
//! removes `Confirm(id, removed_owner)` keys for pending proposals,
//! decrementing `confirmation_count` accordingly.
//!
//! # Storage layout
//! - `owners`     — Vec<Address> of current owners
//! - `threshold`  — u32 required confirmations
//! - `prop_count` — running proposal counter
//! - `Proposal(id)` — ProposalData
//! - `Confirm(id, address)` — bool

#![no_std]
#![allow(deprecated)]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, String, Symbol, Val, Vec,
};

// ─── Storage symbols ─────────────────────────────────────────────────────────

const OWNERS: Symbol = symbol_short!("OWNERS");
const THRESHOLD: Symbol = symbol_short!("THRESH");
const PROP_COUNT: Symbol = symbol_short!("PROPCOUNT");

// ─── Data types ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum ProposalStatus {
    Pending,
    Executed,
    Cancelled,
}

/// Arguments for a cross-contract call proposal.
#[contracttype]
#[derive(Clone, Debug)]
pub struct CallData {
    pub target: Address,
    pub function: Symbol,
    pub args: Vec<Val>,
}

/// The action a proposal will perform when executed.
///
/// - `Call`            — cross-contract call to an external `target`
/// - `AddOwner`        — add a new address to the owner set
/// - `RemoveOwner`     — remove an address from the owner set (cannot drop
///                       the count below `threshold`)
/// - `UpdateThreshold` — change the required confirmation count (must be
///                       between 1 and the current owner count, inclusive)
///
/// Internal variants (`AddOwner`, `RemoveOwner`, `UpdateThreshold`) are
/// executed directly inside `execute()` without a cross-contract call,
/// which is the only safe approach: Soroban disallows a contract from
/// calling itself via `env.invoke_contract`.
#[contracttype]
#[derive(Clone, Debug)]
pub enum ProposalAction {
    Call(CallData),
    AddOwner(Address),
    RemoveOwner(Address),
    UpdateThreshold(u32),
}

/// A fully typed proposal: stores the action to perform when executed plus
/// governance metadata.  A human-readable `description` is kept for UI / event
/// logs.
#[contracttype]
#[derive(Clone, Debug)]
pub struct ProposalData {
    pub id: u64,
    pub proposer: Address,
    /// The action that will be performed when the proposal is executed.
    pub action: ProposalAction,
    /// Human-readable label — not interpreted by the contract.
    pub description: String,
    pub confirmation_count: u32,
    pub status: ProposalStatus,
    pub created_at: u64,
}

#[contracttype]
pub enum DataKey {
    Proposal(u64),
    Confirm(u64, Address),
}

// ─── Events ──────────────────────────────────────────────────────────────────

const EVT_SUBMITTED: Symbol = symbol_short!("submitted");
const EVT_CONFIRMED: Symbol = symbol_short!("confirmed");
const EVT_REVOKED: Symbol = symbol_short!("revoked");
const EVT_EXECUTED: Symbol = symbol_short!("executed");
const EVT_CANCELLED: Symbol = symbol_short!("cancelled");
const EVT_OWNER_ADD: Symbol = symbol_short!("ownerAdd");
const EVT_OWNER_RM: Symbol = symbol_short!("ownerRm");

// ─── Contract ────────────────────────────────────────────────────────────────

#[contract]
pub struct MultisigContract;

#[contractimpl]
impl MultisigContract {
    // ── Constructor ─────────────────────────────────────────────────────────

    /// Constructor: initialise with owner list and confirmation threshold.
    ///
    /// Called automatically at deploy time — there is no separate
    /// `initialize` step.  Because the constructor runs atomically with
    /// deployment the front-running window that existed with a two-step
    /// initialize() is eliminated.
    pub fn __constructor(env: Env, owners: Vec<Address>, threshold: u32) {
        if owners.is_empty() {
            panic!("need at least one owner");
        }
        if threshold == 0 || threshold > owners.len() {
            panic!("invalid threshold");
        }
        env.storage().instance().set(&OWNERS, &owners);
        env.storage().instance().set(&THRESHOLD, &threshold);
        env.storage().instance().set(&PROP_COUNT, &0u64);
    }

    // ── Proposals ───────────────────────────────────────────────────────────

    /// Submit a proposal.  The proposer must be an owner.
    ///
    /// `action`      — the typed action to perform when executed  
    /// `description` — human-readable label for UIs / logs  
    pub fn submit(env: Env, proposer: Address, action: ProposalAction, description: String) -> u64 {
        proposer.require_auth();
        Self::require_owner(&env, &proposer);

        let count: u64 = env.storage().instance().get(&PROP_COUNT).unwrap_or(0);
        let id = count + 1;

        let proposal = ProposalData {
            id,
            proposer: proposer.clone(),
            action,
            description,
            confirmation_count: 0,
            status: ProposalStatus::Pending,
            created_at: env.ledger().timestamp(),
        };

        env.storage()
            .persistent()
            .set(&DataKey::Proposal(id), &proposal);
        env.storage().instance().set(&PROP_COUNT, &id);

        env.events().publish((EVT_SUBMITTED,), (id, proposer));
        id
    }

    /// Confirm a pending proposal. Caller must be an owner.
    pub fn confirm(env: Env, owner: Address, proposal_id: u64) {
        owner.require_auth();
        Self::require_owner(&env, &owner);

        let mut proposal: ProposalData = env
            .storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .expect("proposal not found");

        if proposal.status != ProposalStatus::Pending {
            panic!("proposal not pending");
        }

        let key = DataKey::Confirm(proposal_id, owner.clone());
        if env
            .storage()
            .persistent()
            .get::<DataKey, bool>(&key)
            .unwrap_or(false)
        {
            panic!("already confirmed");
        }

        env.storage().persistent().set(&key, &true);
        proposal.confirmation_count += 1;
        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        env.events().publish((EVT_CONFIRMED,), (proposal_id, owner));
    }

    /// Revoke own confirmation from a pending proposal.
    pub fn revoke_confirmation(env: Env, owner: Address, proposal_id: u64) {
        owner.require_auth();
        Self::require_owner(&env, &owner);

        let mut proposal: ProposalData = env
            .storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .expect("proposal not found");

        if proposal.status != ProposalStatus::Pending {
            panic!("proposal not pending");
        }

        let key = DataKey::Confirm(proposal_id, owner.clone());
        if !env
            .storage()
            .persistent()
            .get::<DataKey, bool>(&key)
            .unwrap_or(false)
        {
            panic!("not confirmed");
        }

        env.storage().persistent().remove(&key);
        proposal.confirmation_count -= 1;
        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        env.events().publish((EVT_REVOKED,), (proposal_id, owner));
    }

    /// Execute a proposal once threshold is met.
    ///
    /// This function is **permissionless** — any caller may trigger execution
    /// once the required number of owner confirmations has been reached.
    /// Owners expressed consent through their on-chain confirmations; no
    /// additional gate is needed at execution time.
    ///
    /// The proposal is marked `Executed` before the action is dispatched
    /// (checks-effects-interactions pattern) so that re-entrant calls on this
    /// same proposal see a non-pending status and panic.
    ///
    /// Internal actions (`AddOwner`, `RemoveOwner`, `UpdateThreshold`) are
    /// handled directly — no cross-contract call is made for them.
    /// `Call` actions dispatch to an external contract via `env.invoke_contract`.
    pub fn execute(env: Env, proposal_id: u64) {
        let mut proposal: ProposalData = env
            .storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .expect("proposal not found");

        if proposal.status != ProposalStatus::Pending {
            panic!("proposal not pending");
        }

        let threshold: u32 = env.storage().instance().get(&THRESHOLD).unwrap();
        if proposal.confirmation_count < threshold {
            panic!("not enough confirmations");
        }

        // Mark Executed BEFORE the action (CEI pattern).
        proposal.status = ProposalStatus::Executed;
        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        env.events().publish((EVT_EXECUTED,), proposal_id);

        // Dispatch the action.
        match proposal.action {
            ProposalAction::Call(cd) => {
                env.invoke_contract::<Val>(&cd.target, &cd.function, cd.args);
            }
            ProposalAction::AddOwner(new_owner) => {
                Self::internal_add_owner(&env, new_owner);
            }
            ProposalAction::RemoveOwner(owner) => {
                Self::internal_remove_owner(&env, owner, proposal_id);
            }
            ProposalAction::UpdateThreshold(new_threshold) => {
                Self::internal_update_threshold(&env, new_threshold);
            }
        }
    }

    /// Cancel a pending proposal. Only the original proposer may cancel.
    pub fn cancel(env: Env, caller: Address, proposal_id: u64) {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        let mut proposal: ProposalData = env
            .storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .expect("proposal not found");

        if proposal.status != ProposalStatus::Pending {
            panic!("proposal not pending");
        }

        if proposal.proposer != caller {
            panic!("only proposer can cancel");
        }

        proposal.status = ProposalStatus::Cancelled;
        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        env.events().publish((EVT_CANCELLED,), proposal_id);
    }

    // ── Read ────────────────────────────────────────────────────────────────

    pub fn get_proposal(env: Env, proposal_id: u64) -> ProposalData {
        env.storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .expect("not found")
    }

    pub fn get_owners(env: Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&OWNERS)
            .unwrap_or(Vec::new(&env))
    }

    pub fn get_threshold(env: Env) -> u32 {
        env.storage().instance().get(&THRESHOLD).unwrap_or(0)
    }

    pub fn proposal_count(env: Env) -> u64 {
        env.storage().instance().get(&PROP_COUNT).unwrap_or(0)
    }

    pub fn has_confirmed(env: Env, proposal_id: u64, owner: Address) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Confirm(proposal_id, owner))
            .unwrap_or(false)
    }

    pub fn is_owner(env: Env, address: Address) -> bool {
        let owners: Vec<Address> = env
            .storage()
            .instance()
            .get(&OWNERS)
            .unwrap_or(Vec::new(&env));
        owners.iter().any(|o| o == address)
    }

    // ── Internal ────────────────────────────────────────────────────────────

    fn require_owner(env: &Env, address: &Address) {
        let owners: Vec<Address> = env
            .storage()
            .instance()
            .get(&OWNERS)
            .unwrap_or(Vec::new(env));
        if !owners.iter().any(|o| o == *address) {
            panic!("not an owner");
        }
    }

    /// Add a new owner.  Only callable from `execute()`.
    fn internal_add_owner(env: &Env, new_owner: Address) {
        let mut owners: Vec<Address> = env.storage().instance().get(&OWNERS).unwrap();
        for o in owners.iter() {
            if o == new_owner {
                panic!("already an owner");
            }
        }
        owners.push_back(new_owner.clone());
        env.storage().instance().set(&OWNERS, &owners);
        env.events().publish((EVT_OWNER_ADD,), new_owner);
    }

    /// Remove an owner.  Threshold must remain satisfiable after removal.
    ///
    /// When an owner is removed, any pending proposals they have confirmed
    /// are updated: their `Confirm` key is removed and `confirmation_count`
    /// is decremented.  This keeps threshold accounting accurate — a pending
    /// proposal that relied on the removed owner's confirmation may no longer
    /// be executable until another owner confirms it.
    fn internal_remove_owner(env: &Env, owner: Address, executing_proposal_id: u64) {
        let mut owners: Vec<Address> = env.storage().instance().get(&OWNERS).unwrap();
        let threshold: u32 = env.storage().instance().get(&THRESHOLD).unwrap();

        if owners.len() <= threshold {
            panic!("cannot remove: would breach threshold");
        }

        let pos = owners.iter().position(|o| o == owner);
        match pos {
            Some(i) => {
                owners.remove(i as u32);
            }
            None => panic!("not an owner"),
        }

        env.storage().instance().set(&OWNERS, &owners);

        // Invalidate the removed owner's confirmations on pending proposals.
        let prop_count: u64 = env.storage().instance().get(&PROP_COUNT).unwrap_or(0);
        for pid in 1..=prop_count {
            if pid == executing_proposal_id {
                continue; // this proposal is already Executed
            }
            let maybe: Option<ProposalData> =
                env.storage().persistent().get(&DataKey::Proposal(pid));
            if let Some(mut prop) = maybe {
                if prop.status == ProposalStatus::Pending {
                    let key = DataKey::Confirm(pid, owner.clone());
                    if env
                        .storage()
                        .persistent()
                        .get::<DataKey, bool>(&key)
                        .unwrap_or(false)
                    {
                        env.storage().persistent().remove(&key);
                        prop.confirmation_count -= 1;
                        env.storage()
                            .persistent()
                            .set(&DataKey::Proposal(pid), &prop);
                    }
                }
            }
        }

        env.events().publish((EVT_OWNER_RM,), owner);
    }

    /// Update the confirmation threshold.  Only callable from `execute()`.
    fn internal_update_threshold(env: &Env, new_threshold: u32) {
        let owners: Vec<Address> = env.storage().instance().get(&OWNERS).unwrap();
        if new_threshold == 0 || new_threshold > owners.len() {
            panic!("invalid threshold");
        }
        env.storage().instance().set(&THRESHOLD, &new_threshold);
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{symbol_short, Env, String};

    // ── Helpers ─────────────────────────────────────────────────────────────

    /// Token contract stub used to test cross-contract calls through the
    /// multisig without depending on the real token crate.  Exposes a `ping`
    /// function that records it was called, and a `fail` function that panics.
    mod stub_token {
        use soroban_sdk::{contract, contractimpl, symbol_short, Env, Symbol};

        const CALLED: Symbol = symbol_short!("CALLED");

        #[contract]
        pub struct StubToken;

        #[contractimpl]
        impl StubToken {
            /// Records a call and returns 1.
            pub fn ping(env: Env) -> i32 {
                env.storage().instance().set(&CALLED, &true);
                1
            }

            /// Always panics — used to verify that a failing target reverts
            /// the whole execute() transaction.
            pub fn fail(_env: Env) {
                panic!("stub: intentional failure");
            }

            pub fn was_called(env: Env) -> bool {
                env.storage().instance().get(&CALLED).unwrap_or(false)
            }
        }
    }

    fn setup_2of3() -> (Env, Address, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();

        let o1 = Address::generate(&env);
        let o2 = Address::generate(&env);
        let o3 = Address::generate(&env);

        let mut owners = Vec::new(&env);
        owners.push_back(o1.clone());
        owners.push_back(o2.clone());
        owners.push_back(o3.clone());

        let contract_id = env.register(MultisigContract, (owners, 2u32));

        (env, contract_id, o1, o2, o3)
    }

    fn no_args(env: &Env) -> Vec<Val> {
        Vec::new(env)
    }

    fn call_action(env: &Env, target: &Address, func: Symbol) -> ProposalAction {
        ProposalAction::Call(CallData {
            target: target.clone(),
            function: func,
            args: no_args(env),
        })
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Constructor tests
    // ─────────────────────────────────────────────────────────────────────────

    /// Constructor with an empty owner list panics with "need at least one owner".
    #[test]
    #[should_panic(expected = "need at least one owner")]
    fn test_constructor_empty_owners_panics() {
        let env = Env::default();
        env.mock_all_auths();
        let owners: Vec<Address> = Vec::new(&env);
        env.register(MultisigContract, (owners, 1u32));
    }

    /// Constructor with threshold = 0 panics with "invalid threshold".
    #[test]
    #[should_panic(expected = "invalid threshold")]
    fn test_constructor_zero_threshold_panics() {
        let env = Env::default();
        env.mock_all_auths();
        let mut owners = Vec::new(&env);
        owners.push_back(Address::generate(&env));
        env.register(MultisigContract, (owners, 0u32));
    }

    /// Constructor with threshold > owner count panics with "invalid threshold".
    #[test]
    #[should_panic(expected = "invalid threshold")]
    fn test_constructor_threshold_exceeds_owners_panics() {
        let env = Env::default();
        env.mock_all_auths();
        let mut owners = Vec::new(&env);
        owners.push_back(Address::generate(&env));
        owners.push_back(Address::generate(&env));
        // threshold = 3 but only 2 owners
        env.register(MultisigContract, (owners, 3u32));
    }

    /// An owner added via AddOwner proposal and then removed via RemoveOwner
    /// proposal leaves the owner set in its original state.
    #[test]
    fn test_owner_list_after_add_remove_cycle() {
        let (env, contract_id, o1, o2, _o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);

        let new_owner = Address::generate(&env);

        // Add new_owner
        let add_id = ms.submit(
            &o1,
            &ProposalAction::AddOwner(new_owner.clone()),
            &String::from_str(&env, "add"),
        );
        ms.confirm(&o1, &add_id);
        ms.confirm(&o2, &add_id);
        ms.execute(&add_id);
        assert_eq!(ms.get_owners().len(), 4);
        assert!(ms.is_owner(&new_owner));

        // Remove new_owner — now 4 owners, threshold 2, so removal is safe
        let remove_id = ms.submit(
            &o1,
            &ProposalAction::RemoveOwner(new_owner.clone()),
            &String::from_str(&env, "remove"),
        );
        ms.confirm(&o1, &remove_id);
        ms.confirm(&o2, &remove_id);
        ms.execute(&remove_id);
        assert_eq!(ms.get_owners().len(), 3);
        assert!(!ms.is_owner(&new_owner));
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Authorization proof tests
    //
    // These tests prove that owner management is ONLY possible via a proposal
    // that has reached the confirmation threshold.  There are no public entry
    // points for add_owner / remove_owner / update_threshold; the only way to
    // trigger them is through execute() after threshold confirmations.
    //
    //   1. Owner management actions require threshold confirmations — an
    //      under-confirmed proposal panics with "not enough confirmations".
    //   2. A fully-confirmed proposal succeeds and mutates state.
    //   3. A self-call (re-entrancy) traps — Soroban disallows it.
    // ─────────────────────────────────────────────────────────────────────────

    /// AddOwner proposals require threshold confirmations.
    /// A proposal submitted but confirmed only once (below a threshold of 2)
    /// must panic with "not enough confirmations" when execute() is called.
    #[test]
    #[should_panic(expected = "not enough confirmations")]
    fn test_add_owner_requires_threshold() {
        let env = Env::default();
        env.mock_all_auths(); // satisfies require_auth on submit/confirm; execute is permissionless

        let o1 = Address::generate(&env);
        let o2 = Address::generate(&env);
        let mut owners = Vec::new(&env);
        owners.push_back(o1.clone());
        owners.push_back(o2.clone());
        let contract_id = env.register(MultisigContract, (owners, 2u32));
        let ms = MultisigContractClient::new(&env, &contract_id);

        let new_owner = Address::generate(&env);
        let id = ms.submit(
            &o1,
            &ProposalAction::AddOwner(new_owner.clone()),
            &String::from_str(&env, "add owner"),
        );
        ms.confirm(&o1, &id);
        // Only 1 of 2 confirmations — must panic with "not enough confirmations".
        ms.execute(&id);
    }

    /// Owner management can ONLY succeed via a fully-confirmed proposal.
    /// Here we confirm to threshold and verify the owner was added.
    #[test]
    fn test_owner_management_only_via_proposal() {
        let (env, contract_id, o1, o2, _o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);

        let new_owner = Address::generate(&env);
        assert!(!ms.is_owner(&new_owner));

        let id = ms.submit(
            &o1,
            &ProposalAction::AddOwner(new_owner.clone()),
            &String::from_str(&env, "add new owner"),
        );
        ms.confirm(&o1, &id);
        ms.confirm(&o2, &id);
        ms.execute(&id);

        assert!(ms.is_owner(&new_owner));
        assert_eq!(ms.get_owners().len(), 4);
    }

    /// A proposal to remove an owner requires threshold confirmations too.
    #[test]
    fn test_remove_owner_only_via_proposal() {
        let (env, contract_id, o1, o2, o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);

        assert!(ms.is_owner(&o3));

        let id = ms.submit(
            &o1,
            &ProposalAction::RemoveOwner(o3.clone()),
            &String::from_str(&env, "remove o3"),
        );
        ms.confirm(&o1, &id);
        ms.confirm(&o2, &id);
        ms.execute(&id);

        assert!(!ms.is_owner(&o3));
        assert_eq!(ms.get_owners().len(), 2);
    }

    /// A proposal to update the threshold requires threshold confirmations.
    #[test]
    fn test_update_threshold_only_via_proposal() {
        let (env, contract_id, o1, o2, _o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);

        assert_eq!(ms.get_threshold(), 2);

        let id = ms.submit(
            &o1,
            &ProposalAction::UpdateThreshold(3),
            &String::from_str(&env, "raise threshold"),
        );
        ms.confirm(&o1, &id);
        ms.confirm(&o2, &id);
        ms.execute(&id);

        assert_eq!(ms.get_threshold(), 3);
    }

    /// Attempting update_threshold below the confirmation count must panic.
    #[test]
    #[should_panic(expected = "not enough confirmations")]
    fn test_update_threshold_below_threshold_panics() {
        let (env, contract_id, o1, _o2, _o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);

        let id = ms.submit(
            &o1,
            &ProposalAction::UpdateThreshold(1),
            &String::from_str(&env, "lower threshold"),
        );
        ms.confirm(&o1, &id);
        ms.execute(&id); // only 1 of 2 required — must panic
    }

    /// Setting threshold to zero must panic with "invalid threshold".
    #[test]
    #[should_panic(expected = "invalid threshold")]
    fn test_update_threshold_zero_panics() {
        let (env, contract_id, o1, o2, _o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);

        let id = ms.submit(
            &o1,
            &ProposalAction::UpdateThreshold(0),
            &String::from_str(&env, "invalid threshold"),
        );
        ms.confirm(&o1, &id);
        ms.confirm(&o2, &id);
        ms.execute(&id);
    }

    /// Setting threshold above the owner count must panic with "invalid threshold".
    #[test]
    #[should_panic(expected = "invalid threshold")]
    fn test_update_threshold_exceeds_owners_panics() {
        let (env, contract_id, o1, o2, _o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);

        let id = ms.submit(
            &o1,
            &ProposalAction::UpdateThreshold(4), // only 3 owners
            &String::from_str(&env, "too high threshold"),
        );
        ms.confirm(&o1, &id);
        ms.confirm(&o2, &id);
        ms.execute(&id);
    }

    /// Adding a duplicate owner must panic with "already an owner".
    #[test]
    #[should_panic(expected = "already an owner")]
    fn test_add_owner_duplicate_panics() {
        let (env, contract_id, o1, o2, _o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);

        let id = ms.submit(
            &o1,
            &ProposalAction::AddOwner(o1.clone()),
            &String::from_str(&env, "duplicate"),
        );
        ms.confirm(&o1, &id);
        ms.confirm(&o2, &id);
        ms.execute(&id);
    }

    /// Removing an owner that would breach the threshold must panic.
    #[test]
    #[should_panic(expected = "cannot remove: would breach threshold")]
    fn test_remove_owner_below_threshold_panics() {
        // 2-of-3: removing would leave 2 owners with threshold 2 (ok).
        // Then removing another would leave 1 owner with threshold 2 (breach).
        let (env, contract_id, o1, o2, o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);

        // Remove o3 → 2 owners, threshold 2 (ok).
        let id1 = ms.submit(
            &o1,
            &ProposalAction::RemoveOwner(o3.clone()),
            &String::from_str(&env, "remove o3"),
        );
        ms.confirm(&o1, &id1);
        ms.confirm(&o2, &id1);
        ms.execute(&id1);

        // Now try to remove o2 → would leave 1 owner, threshold 2 → breach.
        let id2 = ms.submit(
            &o1,
            &ProposalAction::RemoveOwner(o2.clone()),
            &String::from_str(&env, "remove o2"),
        );
        ms.confirm(&o1, &id2);
        ms.confirm(&o2, &id2);
        ms.execute(&id2); // must panic
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Re-entrancy / self-call test
    //
    // Soroban disallows a contract from invoking itself via invoke_contract.
    // A Call proposal targeting the multisig contract itself must trap.
    // ─────────────────────────────────────────────────────────────────────────

    /// A ProposalAction::Call that targets the multisig contract itself traps
    /// because Soroban disallows re-entrancy (self-call via invoke_contract).
    /// This confirms that internal owner management actions (AddOwner etc.)
    /// cannot be triggered by a self-referencing Call proposal — they MUST use
    /// the dedicated enum variants, which are handled inside execute() directly.
    #[test]
    #[should_panic]
    fn test_self_call_reentrance_fails() {
        let (env, contract_id, o1, o2, _o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);

        // Submit a Call that targets the multisig contract itself.
        // Any function name will do — the host will reject the re-entrant call.
        let id = ms.submit(
            &o1,
            &ProposalAction::Call(CallData {
                target: contract_id.clone(),
                function: symbol_short!("getOwners"),
                args: no_args(&env),
            }),
            &String::from_str(&env, "self call — must trap"),
        );
        ms.confirm(&o1, &id);
        ms.confirm(&o2, &id);
        ms.execute(&id); // must panic — Soroban disallows self-invocation
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Confirmation invalidation on owner removal
    // ─────────────────────────────────────────────────────────────────────────

    /// When an owner is removed, their confirmations on pending proposals are
    /// cleared and confirmation_count is decremented.  This means a proposal
    /// that relied solely on the removed owner's confirmation to reach threshold
    /// is no longer executable until another owner re-confirms it.
    #[test]
    fn test_remove_owner_clears_confirmations() {
        let (env, contract_id, o1, o2, o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);
        let stub_id = env.register(stub_token::StubToken, ());

        // Submit a pending Call proposal and have o3 confirm it.
        // It needs 2 confirmations; o3 gives 1.
        let pending_id = ms.submit(
            &o1,
            &call_action(&env, &stub_id, symbol_short!("ping")),
            &String::from_str(&env, "pending proposal"),
        );
        ms.confirm(&o3, &pending_id);
        assert_eq!(ms.get_proposal(&pending_id).confirmation_count, 1);
        assert!(ms.has_confirmed(&pending_id, &o3));

        // Now submit + confirm + execute a RemoveOwner(o3) proposal.
        let remove_id = ms.submit(
            &o1,
            &ProposalAction::RemoveOwner(o3.clone()),
            &String::from_str(&env, "remove o3"),
        );
        ms.confirm(&o1, &remove_id);
        ms.confirm(&o2, &remove_id);
        ms.execute(&remove_id);

        // o3 is no longer an owner.
        assert!(!ms.is_owner(&o3));

        // The pending proposal's confirmation_count must have been decremented.
        assert_eq!(ms.get_proposal(&pending_id).confirmation_count, 0);
        // The confirm key for o3 must have been removed.
        assert!(!ms.has_confirmed(&pending_id, &o3));
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Core proposal lifecycle tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_submit_and_execute() {
        let (env, contract_id, o1, o2, _o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);
        let target = env.register(stub_token::StubToken, ());

        let id = ms.submit(
            &o1,
            &call_action(&env, &target, symbol_short!("ping")),
            &String::from_str(&env, "ping the stub"),
        );

        ms.confirm(&o1, &id);
        ms.confirm(&o2, &id);
        ms.execute(&id);

        let prop = ms.get_proposal(&id);
        assert_eq!(prop.status, ProposalStatus::Executed);
    }

    #[test]
    #[should_panic(expected = "not enough confirmations")]
    fn test_execute_insufficient_confirmations() {
        let (env, contract_id, o1, _o2, _o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);
        let target = env.register(stub_token::StubToken, ());

        let id = ms.submit(
            &o1,
            &call_action(&env, &target, symbol_short!("ping")),
            &String::from_str(&env, "test"),
        );
        ms.confirm(&o1, &id);
        ms.execute(&id); // needs 2, only has 1
    }

    #[test]
    fn test_revoke_and_reconfirm() {
        let (env, contract_id, o1, o2, _o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);
        let target = env.register(stub_token::StubToken, ());

        let id = ms.submit(
            &o1,
            &call_action(&env, &target, symbol_short!("ping")),
            &String::from_str(&env, "test"),
        );
        ms.confirm(&o1, &id);
        ms.confirm(&o2, &id);
        ms.revoke_confirmation(&o2, &id);

        let prop = ms.get_proposal(&id);
        assert_eq!(prop.confirmation_count, 1);

        ms.confirm(&o2, &id);
        ms.execute(&id);
    }

    #[test]
    fn test_is_owner() {
        let (env, contract_id, o1, _o2, _o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);

        assert!(ms.is_owner(&o1));
        let stranger = Address::generate(&env);
        assert!(!ms.is_owner(&stranger));
    }

    #[test]
    #[should_panic(expected = "already confirmed")]
    fn test_double_confirm_panics() {
        let (env, contract_id, o1, _o2, _o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);
        let target = env.register(stub_token::StubToken, ());

        let id = ms.submit(
            &o1,
            &call_action(&env, &target, symbol_short!("ping")),
            &String::from_str(&env, "test"),
        );
        ms.confirm(&o1, &id);
        ms.confirm(&o1, &id); // double-confirm
    }

    #[test]
    #[should_panic(expected = "not an owner")]
    fn test_non_owner_submit_panics() {
        let (env, contract_id, _o1, _o2, _o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);
        let target = env.register(stub_token::StubToken, ());
        let stranger = Address::generate(&env);

        ms.submit(
            &stranger,
            &call_action(&env, &target, symbol_short!("ping")),
            &String::from_str(&env, "test"),
        );
    }

    #[test]
    #[should_panic(expected = "not an owner")]
    fn test_non_owner_confirm_panics() {
        let (env, contract_id, o1, _o2, _o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);
        let target = env.register(stub_token::StubToken, ());
        let stranger = Address::generate(&env);

        let id = ms.submit(
            &o1,
            &call_action(&env, &target, symbol_short!("ping")),
            &String::from_str(&env, "test"),
        );
        ms.confirm(&stranger, &id);
    }

    /// execute() is permissionless — a non-owner can trigger it once the
    /// threshold of owner confirmations has been reached.
    #[test]
    fn test_execute_by_non_owner_succeeds_when_threshold_met() {
        let (env, contract_id, o1, o2, _o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);
        let target = env.register(stub_token::StubToken, ());

        let id = ms.submit(
            &o1,
            &call_action(&env, &target, symbol_short!("ping")),
            &String::from_str(&env, "test"),
        );
        ms.confirm(&o1, &id);
        ms.confirm(&o2, &id);

        // Execute as a stranger — must succeed because execute() is permissionless.
        // Owners expressed their consent through on-chain confirmations; the
        // final trigger does not require an additional gate.
        ms.execute(&id);
        assert_eq!(ms.get_proposal(&id).status, ProposalStatus::Executed);
    }

    #[test]
    #[should_panic(expected = "proposal not pending")]
    fn test_execute_twice_panics() {
        let (env, contract_id, o1, o2, _o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);
        let target = env.register(stub_token::StubToken, ());

        let id = ms.submit(
            &o1,
            &call_action(&env, &target, symbol_short!("ping")),
            &String::from_str(&env, "test"),
        );
        ms.confirm(&o1, &id);
        ms.confirm(&o2, &id);
        ms.execute(&id);
        ms.execute(&id); // second call must panic
    }

    #[test]
    fn test_cancel_by_proposer() {
        let (env, contract_id, o1, _o2, _o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);
        let target = env.register(stub_token::StubToken, ());

        let id = ms.submit(
            &o1,
            &call_action(&env, &target, symbol_short!("ping")),
            &String::from_str(&env, "test"),
        );
        ms.cancel(&o1, &id);
        assert_eq!(ms.get_proposal(&id).status, ProposalStatus::Cancelled);
    }

    #[test]
    #[should_panic(expected = "only proposer can cancel")]
    fn test_cancel_by_non_proposer_panics() {
        let (env, contract_id, o1, o2, _o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);
        let target = env.register(stub_token::StubToken, ());

        let id = ms.submit(
            &o1,
            &call_action(&env, &target, symbol_short!("ping")),
            &String::from_str(&env, "test"),
        );
        ms.cancel(&o2, &id); // o2 is not the proposer
    }

    #[test]
    fn test_revoke_drops_below_threshold_then_reconfirm_executes() {
        let (env, contract_id, o1, o2, _o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);
        let target = env.register(stub_token::StubToken, ());

        let id = ms.submit(
            &o1,
            &call_action(&env, &target, symbol_short!("ping")),
            &String::from_str(&env, "test"),
        );
        ms.confirm(&o1, &id);
        ms.confirm(&o2, &id);
        ms.revoke_confirmation(&o1, &id);
        assert_eq!(ms.get_proposal(&id).confirmation_count, 1);
        ms.confirm(&o1, &id);
        ms.execute(&id);
        assert_eq!(ms.get_proposal(&id).status, ProposalStatus::Executed);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Cross-contract call tests
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_execute_real_cross_contract_call() {
        let (env, contract_id, o1, o2, _o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);

        let stub_id = env.register(stub_token::StubToken, ());
        let stub = stub_token::StubTokenClient::new(&env, &stub_id);

        let id = ms.submit(
            &o1,
            &call_action(&env, &stub_id, symbol_short!("ping")),
            &String::from_str(&env, "call ping on stub"),
        );

        ms.confirm(&o1, &id);
        ms.confirm(&o2, &id);
        ms.execute(&id);

        assert_eq!(ms.get_proposal(&id).status, ProposalStatus::Executed);
        assert!(stub.was_called());
    }

    /// A proposal with a target call that panics: the whole execute() reverts.
    #[test]
    #[should_panic]
    fn test_execute_failing_target_reverts() {
        let (env, contract_id, o1, o2, _o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);

        let stub_id = env.register(stub_token::StubToken, ());

        let id = ms.submit(
            &o1,
            &call_action(&env, &stub_id, symbol_short!("fail")),
            &String::from_str(&env, "will fail"),
        );

        ms.confirm(&o1, &id);
        ms.confirm(&o2, &id);
        ms.execute(&id); // must panic because fail() panics
    }

    /// Execute below threshold panics with "not enough confirmations".
    #[test]
    #[should_panic(expected = "not enough confirmations")]
    fn test_execute_below_threshold_panics() {
        let (env, contract_id, o1, _o2, _o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);
        let stub_id = env.register(stub_token::StubToken, ());

        let id = ms.submit(
            &o1,
            &call_action(&env, &stub_id, symbol_short!("ping")),
            &String::from_str(&env, "below threshold"),
        );
        ms.confirm(&o1, &id);
        ms.execute(&id);
    }

    /// Execute twice panics with "proposal not pending".
    #[test]
    #[should_panic(expected = "proposal not pending")]
    fn test_execute_twice_panics_t3() {
        let (env, contract_id, o1, o2, _o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);
        let stub_id = env.register(stub_token::StubToken, ());

        let id = ms.submit(
            &o1,
            &call_action(&env, &stub_id, symbol_short!("ping")),
            &String::from_str(&env, "double execute"),
        );
        ms.confirm(&o1, &id);
        ms.confirm(&o2, &id);
        ms.execute(&id);
        ms.execute(&id);
    }

    /// Verify that ProposalData stores the action and description correctly.
    #[test]
    fn test_execute_proposal_stored_fields() {
        let (env, contract_id, o1, o2, _o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);
        let stub_id = env.register(stub_token::StubToken, ());

        let id = ms.submit(
            &o1,
            &call_action(&env, &stub_id, symbol_short!("ping")),
            &String::from_str(&env, "governance call"),
        );

        let prop = ms.get_proposal(&id);
        assert_eq!(prop.description, String::from_str(&env, "governance call"));

        ms.confirm(&o1, &id);
        ms.confirm(&o2, &id);
        ms.execute(&id);
        assert_eq!(ms.get_proposal(&id).status, ProposalStatus::Executed);
    }
}
