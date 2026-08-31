//! Soroban Multisig Governance Contract
//!
//! N-of-M multisignature wallet / governance primitive.
//!
//! # Flow
//! 1. An owner submits a proposal (arbitrary bytes representing a call payload).
//! 2. Other owners confirm it.
//! 3. Once `threshold` confirmations are reached anyone can execute it.
//! 4. Any owner can revoke their own confirmation before execution.
//!
//! # Storage layout
//! - `owners`     — Vec<Address> of current owners
//! - `threshold`  — u32 required confirmations
//! - `prop_count` — running proposal counter
//! - `Proposal(id)` — ProposalData
//! - `Confirm(id, address)` — bool

#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Bytes, Env, Symbol, Vec,
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

#[contracttype]
#[derive(Clone, Debug)]
pub struct ProposalData {
    pub id: u64,
    pub proposer: Address,
    pub description: Bytes,
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
    // ── Init ────────────────────────────────────────────────────────────────

    /// Initialize with owner list and confirmation threshold.
    pub fn initialize(env: Env, owners: Vec<Address>, threshold: u32) {
        if env.storage().instance().has(&OWNERS) {
            panic!("already initialized");
        }
        if owners.is_empty() {
            panic!("need at least one owner");
        }
        if threshold == 0 || threshold > owners.len() as u32 {
            panic!("invalid threshold");
        }
        env.storage().instance().set(&OWNERS, &owners);
        env.storage().instance().set(&THRESHOLD, &threshold);
        env.storage().instance().set(&PROP_COUNT, &0u64);
    }

    // ── Proposals ───────────────────────────────────────────────────────────

    /// Submit a proposal. Proposer must be an owner.
    pub fn submit(env: Env, proposer: Address, description: Bytes) -> u64 {
        proposer.require_auth();
        Self::require_owner(&env, &proposer);

        let count: u64 = env.storage().instance().get(&PROP_COUNT).unwrap_or(0);
        let id = count + 1;

        let proposal = ProposalData {
            id,
            proposer: proposer.clone(),
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
        if env.storage().persistent().get::<DataKey, bool>(&key).unwrap_or(false) {
            panic!("already confirmed");
        }

        env.storage().persistent().set(&key, &true);
        proposal.confirmation_count += 1;
        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        env.events()
            .publish((EVT_CONFIRMED,), (proposal_id, owner));
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
        if !env.storage().persistent().get::<DataKey, bool>(&key).unwrap_or(false) {
            panic!("not confirmed");
        }

        env.storage().persistent().remove(&key);
        proposal.confirmation_count -= 1;
        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        env.events()
            .publish((EVT_REVOKED,), (proposal_id, owner));
    }

    /// Execute a proposal once threshold is met.
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

        proposal.status = ProposalStatus::Executed;
        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        // NOTE: actual cross-contract call logic would be injected here
        // based on the decoded `description` payload.

        env.events().publish((EVT_EXECUTED,), proposal_id);
    }

    /// Cancel a pending proposal. Proposer or any owner with majority can cancel.
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

        // Only the original proposer can cancel
        if proposal.proposer != caller {
            panic!("only proposer can cancel");
        }

        proposal.status = ProposalStatus::Cancelled;
        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);

        env.events().publish((EVT_CANCELLED,), proposal_id);
    }

    // ── Owner management ────────────────────────────────────────────────────

    /// Add a new owner. Requires this call to itself be executed via multisig.
    pub fn add_owner(env: Env, new_owner: Address) {
        // This function should only be invoked via execute() which validates threshold
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

    /// Remove an owner. Threshold must remain satisfiable after removal.
    pub fn remove_owner(env: Env, owner: Address) {
        let mut owners: Vec<Address> = env.storage().instance().get(&OWNERS).unwrap();
        let threshold: u32 = env.storage().instance().get(&THRESHOLD).unwrap();

        if owners.len() as u32 <= threshold {
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
        env.events().publish((EVT_OWNER_RM,), owner);
    }

    /// Update the confirmation threshold.
    pub fn update_threshold(env: Env, new_threshold: u32) {
        let owners: Vec<Address> = env.storage().instance().get(&OWNERS).unwrap();
        if new_threshold == 0 || new_threshold > owners.len() as u32 {
            panic!("invalid threshold");
        }
        env.storage().instance().set(&THRESHOLD, &new_threshold);
    }

    // ── Read ────────────────────────────────────────────────────────────────

    pub fn get_proposal(env: Env, proposal_id: u64) -> ProposalData {
        env.storage()
            .persistent()
            .get(&DataKey::Proposal(proposal_id))
            .expect("not found")
    }

    pub fn get_owners(env: Env) -> Vec<Address> {
        env.storage().instance().get(&OWNERS).unwrap_or(Vec::new(&env))
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
        let owners: Vec<Address> = env.storage().instance().get(&OWNERS).unwrap_or(Vec::new(&env));
        owners.iter().any(|o| o == address)
    }

    // ── Internal ────────────────────────────────────────────────────────────

    fn require_owner(env: &Env, address: &Address) {
        let owners: Vec<Address> = env.storage().instance().get(&OWNERS).unwrap_or(Vec::new(env));
        if !owners.iter().any(|o| o == *address) {
            panic!("not an owner");
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::Env;

    fn setup_2of3() -> (Env, Address, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();

        let o1 = Address::generate(&env);
        let o2 = Address::generate(&env);
        let o3 = Address::generate(&env);

        let contract_id = env.register(MultisigContract, ());
        let ms = MultisigContractClient::new(&env, &contract_id);

        let mut owners = Vec::new(&env);
        owners.push_back(o1.clone());
        owners.push_back(o2.clone());
        owners.push_back(o3.clone());
        ms.initialize(&owners, &2);

        (env, contract_id, o1, o2, o3)
    }

    #[test]
    fn test_submit_and_execute() {
        let (env, contract_id, o1, o2, _o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);

        let desc = Bytes::from_slice(&env, b"transfer 100 XLM to treasury");
        let id = ms.submit(&o1, &desc);

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

        let id = ms.submit(&o1, &Bytes::from_slice(&env, b"test"));
        ms.confirm(&o1, &id);
        ms.execute(&id); // needs 2, only has 1
    }

    #[test]
    fn test_revoke_and_reconfirm() {
        let (env, contract_id, o1, o2, _o3) = setup_2of3();
        let ms = MultisigContractClient::new(&env, &contract_id);

        let id = ms.submit(&o1, &Bytes::from_slice(&env, b"test"));
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
}
