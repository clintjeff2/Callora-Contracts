//! # Callora Vault Contract
//!
//! ## Access Control
//!
//! The vault implements role-based access control for deposits:
//!
//! - **Owner**: Set at initialization, immutable. Always permitted to deposit.
//! - **Allowed Depositor**: Optional address (e.g., backend service) that can be
//!   explicitly approved by the owner. Can be set, changed, or cleared at any time.
//! - **Other addresses**: Rejected with an authorization error.
//!
//! ### Production Usage
//!
//! In production, the owner typically represents the end user's account, while the
//! allowed depositor is a backend service that handles automated deposits on behalf
//! of the user.
//!
//! ### Managing the Allowed Depositor
//!
//! - Set or update: `set_allowed_depositor(Some(address))`
//! - Clear (revoke access): `set_allowed_depositor(None)`
//! - Only the owner can call `set_allowed_depositor`
//!
//! ### Security Model
//!
//! - The owner has full control over who can deposit
//! - The allowed depositor is a trusted address (typically a backend service)
//! - Access can be revoked at any time by the owner
//! - All deposit attempts are authenticated against the caller's address

#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String, Symbol};

#[contracttype]
#[derive(Clone)]
pub struct VaultMeta {
    pub owner: Address,
    pub balance: i128,
}

/// Maximum allowed length for metadata strings (IPFS CID or URI).
/// IPFS CIDv1 (base32) is typically ~59 chars, CIDv0 is 46 chars.
/// HTTPS URIs can vary, but we cap at 256 chars to prevent storage abuse.
/// This limit balances flexibility with storage cost constraints.
pub const MAX_METADATA_LENGTH: u32 = 256;

#[contracttype]
pub enum StorageKey {
    Meta,
    AllowedDepositor,
    /// Offering metadata: maps offering_id (String) -> metadata (String)
    /// The metadata string typically contains an IPFS CID (e.g., "QmXxx..." or "bafyxxx...")
    /// or an HTTPS URI (e.g., "https://example.com/metadata/offering123.json")
    OfferingMetadata(String),
}

#[contract]
pub struct CalloraVault;

#[contractimpl]
impl CalloraVault {
    /// Initialize vault for an owner with optional initial balance.
    /// Emits an "init" event with the owner address and initial balance.
    pub fn init(env: Env, owner: Address, initial_balance: Option<i128>) -> VaultMeta {
        let balance = initial_balance.unwrap_or(0);
        let meta = VaultMeta {
            owner: owner.clone(),
            balance,
        };
        env.storage().instance().set(&StorageKey::Meta, &meta);

        // Emit event: topics = (init, owner), data = balance
        env.events()
            .publish((Symbol::new(&env, "init"), owner), balance);

        meta
    }

    /// Check if the caller is authorized to deposit (owner or allowed depositor).
    fn is_authorized_depositor(env: &Env, caller: &Address) -> bool {
        let meta = Self::get_meta(env.clone());

        // Owner is always authorized
        if caller == &meta.owner {
            return true;
        }

        // Check if caller is the allowed depositor
        if let Some(allowed) = env
            .storage()
            .instance()
            .get::<StorageKey, Address>(&StorageKey::AllowedDepositor)
        {
            if caller == &allowed {
                return true;
            }
        }

        false
    }

    /// Require that the caller is the owner, panic otherwise.
    fn require_owner(env: &Env, caller: &Address) {
        let meta = Self::get_meta(env.clone());
        assert!(caller == &meta.owner, "unauthorized: owner only");
    }

    /// Get vault metadata (owner and balance).
    pub fn get_meta(env: Env) -> VaultMeta {
        env.storage()
            .instance()
            .get(&StorageKey::Meta)
            .unwrap_or_else(|| panic!("vault not initialized"))
    }

    /// Set or clear the allowed depositor address. Owner-only.
    /// Pass `None` to revoke depositor access, `Some(address)` to grant or update.
    pub fn set_allowed_depositor(env: Env, caller: Address, depositor: Option<Address>) {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        match depositor {
            Some(addr) => {
                env.storage()
                    .instance()
                    .set(&StorageKey::AllowedDepositor, &addr);
            }
            None => {
                env.storage()
                    .instance()
                    .remove(&StorageKey::AllowedDepositor);
            }
        }
    }

    /// Deposit increases balance. Callable by owner or designated depositor.
    pub fn deposit(env: Env, caller: Address, amount: i128) -> i128 {
        caller.require_auth();

        assert!(
            Self::is_authorized_depositor(&env, &caller),
            "unauthorized: only owner or allowed depositor can deposit"
        );

        let mut meta = Self::get_meta(env.clone());
        meta.balance += amount;
        env.storage().instance().set(&StorageKey::Meta, &meta);
        meta.balance
    }

    /// Deduct balance for an API call. Only backend/authorized caller in production.
    pub fn deduct(env: Env, amount: i128) -> i128 {
        let mut meta = Self::get_meta(env.clone());
        assert!(meta.balance >= amount, "insufficient balance");
        meta.balance -= amount;
        env.storage().instance().set(&StorageKey::Meta, &meta);
        meta.balance
    }

    /// Return current balance.
    pub fn balance(env: Env) -> i128 {
        Self::get_meta(env).balance
    }

    // ========================================================================
    // Offering Metadata Management
    // ========================================================================

    /// Set metadata for an offering. Only the owner (issuer) can set metadata.
    ///
    /// # Parameters
    /// - `caller`: Must be the vault owner (authenticated via require_auth)
    /// - `offering_id`: Unique identifier for the offering (e.g., "offering-001")
    /// - `metadata`: Off-chain metadata reference (IPFS CID or HTTPS URI)
    ///
    /// # Metadata Format
    /// The metadata string should contain:
    /// - IPFS CID (v0): e.g., "QmXoypizjW3WknFiJnKLwHCnL72vedxjQkDDP1mXWo6uco"
    /// - IPFS CID (v1): e.g., "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi"
    /// - HTTPS URI: e.g., "https://example.com/metadata/offering123.json"
    ///
    /// # Off-chain Usage Pattern
    /// Clients should:
    /// 1. Call `get_metadata(offering_id)` to retrieve the reference
    /// 2. If IPFS CID: Fetch from IPFS gateway (e.g., https://ipfs.io/ipfs/{CID})
    /// 3. If HTTPS URI: Fetch directly via HTTP GET
    /// 4. Parse the JSON metadata (expected fields: name, description, image, etc.)
    ///
    /// # Storage Limits
    /// - Maximum metadata length: 256 characters
    /// - Exceeding this limit will cause a panic
    ///
    /// # Events
    /// Emits a "metadata_set" event with topics: (metadata_set, offering_id, caller)
    /// and data: metadata string
    ///
    /// # Errors
    /// - Panics if caller is not the owner
    /// - Panics if metadata exceeds MAX_METADATA_LENGTH
    /// - Panics if offering_id already has metadata (use update_metadata instead)
    pub fn set_metadata(
        env: Env,
        caller: Address,
        offering_id: String,
        metadata: String,
    ) -> String {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        // Validate metadata length
        let metadata_len = metadata.len();
        assert!(
            metadata_len <= MAX_METADATA_LENGTH,
            "metadata exceeds maximum length of {} characters",
            MAX_METADATA_LENGTH
        );

        // Check if metadata already exists
        let key = StorageKey::OfferingMetadata(offering_id.clone());
        assert!(
            !env.storage().instance().has(&key),
            "metadata already exists for this offering; use update_metadata to modify"
        );

        // Store metadata
        env.storage().instance().set(&key, &metadata);

        // Emit event: topics = (metadata_set, offering_id, caller), data = metadata
        env.events().publish(
            (
                Symbol::new(&env, "metadata_set"),
                offering_id,
                caller,
            ),
            metadata.clone(),
        );

        metadata
    }

    /// Update existing metadata for an offering. Only the owner (issuer) can update.
    ///
    /// # Parameters
    /// - `caller`: Must be the vault owner (authenticated via require_auth)
    /// - `offering_id`: Unique identifier for the offering
    /// - `metadata`: New off-chain metadata reference (IPFS CID or HTTPS URI)
    ///
    /// # Events
    /// Emits a "metadata_updated" event with topics: (metadata_updated, offering_id, caller)
    /// and data: (old_metadata, new_metadata) tuple
    ///
    /// # Errors
    /// - Panics if caller is not the owner
    /// - Panics if metadata exceeds MAX_METADATA_LENGTH
    /// - Panics if offering_id has no existing metadata (use set_metadata first)
    pub fn update_metadata(
        env: Env,
        caller: Address,
        offering_id: String,
        metadata: String,
    ) -> String {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        // Validate metadata length
        let metadata_len = metadata.len();
        assert!(
            metadata_len <= MAX_METADATA_LENGTH,
            "metadata exceeds maximum length of {} characters",
            MAX_METADATA_LENGTH
        );

        // Check if metadata exists
        let key = StorageKey::OfferingMetadata(offering_id.clone());
        let old_metadata: String = env
            .storage()
            .instance()
            .get(&key)
            .unwrap_or_else(|| panic!("no metadata exists for this offering; use set_metadata first"));

        // Update metadata
        env.storage().instance().set(&key, &metadata);

        // Emit event: topics = (metadata_updated, offering_id, caller), data = (old, new)
        env.events().publish(
            (
                Symbol::new(&env, "metadata_updated"),
                offering_id,
                caller,
            ),
            (old_metadata, metadata.clone()),
        );

        metadata
    }

    /// Get metadata for an offering. Returns None if no metadata is set.
    ///
    /// # Parameters
    /// - `offering_id`: Unique identifier for the offering
    ///
    /// # Returns
    /// - `Some(metadata)` if metadata exists
    /// - `None` if no metadata has been set for this offering
    pub fn get_metadata(env: Env, offering_id: String) -> Option<String> {
        let key = StorageKey::OfferingMetadata(offering_id);
        env.storage().instance().get(&key)
    }
}

#[cfg(test)]
mod test;
