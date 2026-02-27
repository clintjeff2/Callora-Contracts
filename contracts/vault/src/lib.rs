#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, Symbol, Vec};

/// Single item for batch deduct: amount and optional request id for idempotency/tracking.
#[contracttype]
#[derive(Clone)]
pub struct DeductItem {
    pub amount: i128,
    pub request_id: Option<Symbol>,
}
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String, Symbol};
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol, Vec};

#[contracttype]
#[derive(Clone)]
pub struct VaultMeta {
    pub owner: Address,
    pub balance: i128,
    pub authorized_caller: Option<Address>,
    /// Minimum amount required per deposit; deposits below this panic.
    pub min_deposit: i128,
}

const META_KEY: &str = "meta";
const USDC_KEY: &str = "usdc";
const ADMIN_KEY: &str = "admin";
const SETTLEMENT_KEY: &str = "settlement";
const REVENUE_POOL_KEY: &str = "revenue_pool";
const MAX_DEDUCT_KEY: &str = "max_deduct";

/// Default maximum single deduct amount when not set at init (no cap).
pub const DEFAULT_MAX_DEDUCT: i128 = i128::MAX;

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct DistributeEvent {
    pub to: Address,
    pub amount: i128,
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
    AllowedDepositors,
    ApiPrice(Symbol),
    Paused,
}

#[contract]
pub struct CalloraVault;

#[contractimpl]
impl CalloraVault {
    /// Initialize vault for an owner with optional initial balance and minimum deposit.
    /// If initial_balance > 0, the contract must already hold at least that much USDC (e.g. deployer transferred in first).
    /// Emits an "init" event with the owner address and initial balance.
    pub fn init(
        env: Env,
        owner: Address,
        initial_balance: Option<i128>,
        authorized_caller: Option<Address>,
    ) -> VaultMeta {
    ///
    /// # Arguments
    /// * `revenue_pool` – Optional address to receive USDC on each deduct (e.g. settlement contract). If None, USDC stays in vault.
    /// * `max_deduct` – Optional cap per single deduct; if None, uses DEFAULT_MAX_DEDUCT (no cap).
    pub fn init(
        env: Env,
        owner: Address,
        usdc_token: Address,
        initial_balance: Option<i128>,
        min_deposit: Option<i128>,
        revenue_pool: Option<Address>,
        max_deduct: Option<i128>,
    ) -> VaultMeta {
        owner.require_auth();
        if env.storage().instance().has(&Symbol::new(&env, META_KEY)) {
    /// # Security Note
    /// The `owner` address is required to authorize the initialization transaction via `owner.require_auth()`.
    /// This prevents unauthorized parties from initializing the vault with a "zero" or unauthenticated owner.
    ///
    /// # Panics
    /// - If the vault is already initialized
    /// - If `initial_balance` is negative
    pub fn init(env: Env, owner: Address, initial_balance: Option<i128>) -> VaultMeta {
        owner.require_auth();
        if env.storage().instance().has(&StorageKey::Meta) {
            panic!("vault already initialized");
        }
        let balance = initial_balance.unwrap_or(0);
        if balance > 0 {
            let usdc = token::Client::new(&env, &usdc_token);
            let contract_balance = usdc.balance(&env.current_contract_address());
            if contract_balance < balance {
                panic!("insufficient USDC in contract for initial_balance");
            }
        }
        let min_deposit_val = min_deposit.unwrap_or(0);
        let max_deduct_val = max_deduct.unwrap_or(DEFAULT_MAX_DEDUCT);
        if max_deduct_val <= 0 {
            panic!("max_deduct must be positive");
        }
        let meta = VaultMeta {
            owner: owner.clone(),
            balance,
            authorized_caller,
            min_deposit: min_deposit_val,
        };
        // Persist metadata under both the literal key and the constant for safety.
        let inst = env.storage().instance();
        inst.set(&Symbol::new(&env, "meta"), &meta);
        inst.set(&Symbol::new(&env, META_KEY), &meta);
        inst.set(&Symbol::new(&env, USDC_KEY), &usdc_token);
        inst.set(&Symbol::new(&env, ADMIN_KEY), &owner);
        if let Some(pool) = revenue_pool {
            inst.set(&Symbol::new(&env, REVENUE_POOL_KEY), &pool);
        }
        inst.set(&Symbol::new(&env, MAX_DEDUCT_KEY), &max_deduct_val);

        env.events()
            .publish((Symbol::new(&env, "init"), owner), balance);

        meta
    }

    /// Return the current admin address.
    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&Symbol::new(&env, ADMIN_KEY))
            .unwrap_or_else(|| panic!("vault not initialized"))
    }

    /// Replace the current admin. Only the existing admin may call this.
    pub fn set_admin(env: Env, caller: Address, new_admin: Address) {
        caller.require_auth();
        let current_admin = Self::get_admin(env.clone());
        if caller != current_admin {
            panic!("unauthorized: caller is not admin");
    /// Check if the caller is authorized to deposit (owner or allowed depositor).
    fn is_authorized_depositor(env: Env, caller: Address) -> bool {
        let meta = Self::get_meta(env.clone());
        // Owner is always authorized
        if caller == meta.owner {
            return true;
        }
        let inst = env.storage().instance();
        inst.set(&Symbol::new(&env, ADMIN_KEY), &new_admin);
    }

    /// Return the maximum allowed amount for a single deduct (configurable at init).
    pub fn get_max_deduct(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&Symbol::new(&env, MAX_DEDUCT_KEY))
            .unwrap_or_else(|| panic!("vault not initialized"))
    }

    /// Return the revenue pool address if set (receives USDC on deduct).
    pub fn get_revenue_pool(env: Env) -> Option<Address> {
        env.storage()
            .instance()
            .get(&Symbol::new(&env, REVENUE_POOL_KEY))
            .unwrap_or(None)
    }

    /// Distribute accumulated USDC to a single developer address.
    ///
    /// # Access control
    /// Only the admin (backend / multisig) may call this.
    ///
    /// # Arguments
    /// * `caller` – Must be the current admin address.
    /// * `to`     – Developer wallet to receive the USDC.
    /// * `amount` – Amount in USDC micro-units (must be > 0 and ≤ vault balance).
    ///
    /// # Panics
    /// * `"unauthorized: caller is not admin"` – caller is not the admin.
    /// * `"amount must be positive"`           – amount is zero or negative.
    /// * `"insufficient USDC balance"`         – vault holds less than amount.
    ///
    /// # Events
    /// Emits topic `("distribute", to)` with data `amount` on success.
    pub fn distribute(env: Env, caller: Address, to: Address, amount: i128) {
        // 1. Require on-chain signature from caller.
        caller.require_auth();

        // 2. Only the registered admin may distribute.
        let admin = Self::get_admin(env.clone());
        if caller != admin {
            panic!("unauthorized: caller is not admin");
        }

        // 3. Amount must be positive.
        if amount <= 0 {
            panic!("amount must be positive");
        }

        // 4. Load the USDC token address.
        let usdc_opt: Option<Address> = env.storage().instance().get(&Symbol::new(&env, USDC_KEY));
        let usdc_address: Address = usdc_opt.unwrap_or_else(|| panic!("vault not initialized"));

        let usdc = token::Client::new(&env, &usdc_address);

        // 5. Check vault has enough USDC.
        let vault_balance = usdc.balance(&env.current_contract_address());
        if vault_balance < amount {
            panic!("insufficient USDC balance");
        }

        // 6. Transfer USDC from vault to developer.
        usdc.transfer(&env.current_contract_address(), &to, &amount);

        // 7. Emit distribute event.
        env.events()
            .publish((Symbol::new(&env, "distribute"), to), amount);
    }

    /// Get vault metadata (owner and balance).
    pub fn get_meta(env: Env) -> VaultMeta {
        env.storage()
            .instance()
            .get(&Symbol::new(&env, META_KEY))
            .unwrap_or_else(|| panic!("vault not initialized"))
    }

    /// Set or update the authorized caller for deduction. Only callable by the vault owner.
    pub fn set_authorized_caller(env: Env, caller: Address) {
        let mut meta = Self::get_meta(env.clone());
        meta.owner.require_auth();

        meta.authorized_caller = Some(caller.clone());
        env.storage()
            .instance()
            .set(&Symbol::new(&env, "meta"), &meta);

        env.events().publish(
            (Symbol::new(&env, "set_auth_caller"), meta.owner.clone()),
            caller,
        );
    }

    /// Deposit increases balance. Callable by owner or designated depositor.
    /// Emits a "deposit" event with amount and new balance.
    pub fn deposit(env: Env, amount: i128) -> i128 {
    /// Deposit: user transfers USDC to the contract; contract increases internal balance.
    /// Caller must have authorized the transfer (token transfer_from). Supports multiple depositors.
    /// Emits a "deposit" event with the depositor address and amount.
    pub fn deposit(env: Env, from: Address, amount: i128) -> i128 {
        from.require_auth();

        let mut meta = Self::get_meta(env.clone());
        assert!(
            amount >= meta.min_deposit,
            "deposit below minimum: {} < {}",
            amount,
            meta.min_deposit
        );

        let usdc_address: Address = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, USDC_KEY))
            .expect("vault not initialized");
        let usdc = token::Client::new(&env, &usdc_address);
        usdc.transfer_from(
            &env.current_contract_address(),
            &from,
            &env.current_contract_address(),
            &amount,
        );

        meta.balance += amount;
        let inst = env.storage().instance();
        inst.set(&Symbol::new(&env, "meta"), &meta);

        env.events()
            .publish((Symbol::new(&env, "deposit"), from), amount);

        meta.balance
    }

    /// Deduct balance for an API call. Only authorized caller or owner.
    /// Emits a "deduct" event with amount and new balance.
    pub fn deduct(env: Env, caller: Address, amount: i128) -> i128 {
    /// Deduct balance for an API call. Callable by authorized caller (e.g. backend).
    /// Amount must not exceed max single deduct (see init / get_max_deduct).
    /// If revenue pool is set, USDC is transferred to it; otherwise it remains in the vault.
    /// Emits a "deduct" event with caller, optional request_id, amount, and new balance.
    /// Automatically transfers USDC to settlement contract for revenue settlement.
    pub fn deduct(env: Env, caller: Address, amount: i128, request_id: Option<Symbol>) -> i128 {
        caller.require_auth();
        let max_deduct = Self::get_max_deduct(env.clone());
        assert!(amount > 0, "amount must be positive");
        assert!(amount <= max_deduct, "deduct amount exceeds max_deduct");
    /// Pause the vault. Only the owner may call this.
    pub fn pause(env: Env, caller: Address) {
        caller.require_auth();
        Self::require_owner(env.clone(), caller);
        env.storage().instance().set(&StorageKey::Paused, &true);
    }

    /// Unpause the vault. Only the owner may call this.
    pub fn unpause(env: Env, caller: Address) {
        caller.require_auth();
        Self::require_owner(env.clone(), caller);
        env.storage().instance().set(&StorageKey::Paused, &false);
    }

    /// Return whether the vault is currently paused.
    pub fn paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&StorageKey::Paused)
            .unwrap_or(false)
    }

    /// Deduct balance for an API call. Only owner/authorized caller in production.
    /// Panics if the vault is paused.
    pub fn deduct(env: Env, caller: Address, amount: i128) -> i128 {
        caller.require_auth();
        Self::require_owner(env.clone(), caller);
        assert!(!Self::paused(env.clone()), "vault is paused");

        let mut meta = Self::get_meta(env.clone());

        // Ensure the caller corresponds to the address signing the transaction.
        caller.require_auth();

        // Check authorization: must be either the authorized_caller if set, or the owner.
        let authorized = match &meta.authorized_caller {
            Some(auth_caller) => caller == *auth_caller || caller == meta.owner,
            None => caller == meta.owner,
        };
        assert!(authorized, "unauthorized caller");

        assert!(meta.balance >= amount, "insufficient balance");

        meta.balance -= amount;
        let inst = env.storage().instance();
        inst.set(&Symbol::new(&env, "meta"), &meta);

        // Transfer USDC to settlement contract for revenue settlement
        Self::transfer_to_settlement(env.clone(), amount);

        let topics = match &request_id {
            Some(rid) => (Symbol::new(&env, "deduct"), caller.clone(), rid.clone()),
            None => (
                Symbol::new(&env, "deduct"),
                caller.clone(),
                Symbol::new(&env, ""),
            ),
        };
        env.events().publish(topics, (amount, meta.balance));
        meta.balance
    }

    /// Batch deduct: multiple (amount, optional request_id) in one transaction.
    /// Reverts the entire batch if any single deduct would exceed balance.
    /// Emits one "deduct" event per item (same shape as single deduct).
    /// Automatically transfers total USDC amount to settlement contract for revenue settlement.
    pub fn batch_deduct(env: Env, caller: Address, items: Vec<DeductItem>) -> i128 {
    /// Each amount must not exceed max_deduct. Reverts entire batch if any check fails.
    /// If revenue pool is set, total deducted USDC is transferred to it once.
    /// Emits one "deduct" event per item.
    pub fn batch_deduct(env: Env, caller: Address, items: Vec<DeductItem>) -> i128 {
        caller.require_auth();
        let max_deduct = Self::get_max_deduct(env.clone());
        let mut meta = Self::get_meta(env.clone());

        // Ensure the caller corresponds to the address signing the transaction.
        caller.require_auth();

        // Check authorization: must be either the authorized_caller if set, or the owner.
        let authorized = match &meta.authorized_caller {
            Some(auth_caller) => caller == *auth_caller || caller == meta.owner,
            None => caller == meta.owner,
        };
        assert!(authorized, "unauthorized caller");

        let n = items.len();
        assert!(n > 0, "batch_deduct requires at least one item");

        let mut running = meta.balance;
        let mut total_amount = 0i128;
        for item in items.iter() {
            assert!(item.amount > 0, "amount must be positive");
            assert!(
                item.amount <= max_deduct,
                "deduct amount exceeds max_deduct"
            );
            assert!(running >= item.amount, "insufficient balance");
            running -= item.amount;
            total_amount += item.amount;
        }

        let mut balance = meta.balance;
        for item in items.iter() {
            balance -= item.amount;
            let topics = match &item.request_id {
                Some(rid) => (Symbol::new(&env, "deduct"), caller.clone(), rid.clone()),
                None => (
                    Symbol::new(&env, "deduct"),
                    caller.clone(),
                    Symbol::new(&env, ""),
                ),
            };
            env.events().publish(topics, (item.amount, balance));
        }

        meta.balance = balance;
        env.storage()
            .instance()
            .set(&Symbol::new(&env, "meta"), &meta);

        // Transfer total USDC amount to settlement contract for revenue settlement
        Self::transfer_to_settlement(env.clone(), total_amount);
        
        let inst = env.storage().instance();
        inst.set(&Symbol::new(&env, "meta"), &meta);
        meta.balance
    }

    /// Withdraw from vault. Callable only by the vault owner; reduces balance and transfers USDC to owner.
    pub fn withdraw(env: Env, amount: i128) -> i128 {
    /// Set the price per API call (in smallest USDC units) for a given API ID.
    /// Callable by the owner or allowed depositor (backend/admin).
    pub fn set_price(env: Env, caller: Address, api_id: Symbol, price: i128) {
        caller.require_auth();

        assert!(
            Self::is_authorized_depositor(env.clone(), caller.clone()),
            "unauthorized: only owner or allowed depositor can set price"
        );

        env.storage()
            .instance()
            .set(&StorageKey::ApiPrice(api_id), &price);
    }

    /// Get the configured price per API call (in smallest USDC units) for a given API ID.
    /// Returns `None` if no price has been set for this API.
    pub fn get_price(env: Env, api_id: Symbol) -> Option<i128> {
        env.storage()
            .instance()
            .get::<StorageKey, i128>(&StorageKey::ApiPrice(api_id))
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
            (Symbol::new(&env, "metadata_set"), offering_id, caller),
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
        let old_metadata: String = env.storage().instance().get(&key).unwrap_or_else(|| {
            panic!("no metadata exists for this offering; use set_metadata first")
        });

        // Update metadata
        env.storage().instance().set(&key, &metadata);

        // Emit event: topics = (metadata_updated, offering_id, caller), data = (old, new)
        env.events().publish(
            (Symbol::new(&env, "metadata_updated"), offering_id, caller),
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

    pub fn transfer_ownership(env: Env, new_owner: Address) {
        let mut meta = Self::get_meta(env.clone());
        meta.owner.require_auth();
        assert!(amount > 0, "amount must be positive");
        assert!(meta.balance >= amount, "insufficient balance");

        let usdc_address: Address = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, USDC_KEY))
            .expect("vault not initialized");
        let usdc = token::Client::new(&env, &usdc_address);
        usdc.transfer(&env.current_contract_address(), &meta.owner, &amount);

        meta.balance -= amount;
        let inst = env.storage().instance();
        inst.set(&Symbol::new(&env, "meta"), &meta);

        env.events().publish(
            (Symbol::new(&env, "withdraw"), meta.owner.clone()),
            (amount, meta.balance),
        );
        meta.balance
    }

    /// Withdraw from vault to a designated address. Owner-only; transfers USDC to `to`.
    pub fn withdraw_to(env: Env, to: Address, amount: i128) -> i128 {
        let mut meta = Self::get_meta(env.clone());
        meta.owner.require_auth();
        assert!(amount > 0, "amount must be positive");
        assert!(meta.balance >= amount, "insufficient balance");

        let usdc_address: Address = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, USDC_KEY))
            .expect("vault not initialized");
        let usdc = token::Client::new(&env, &usdc_address);
        usdc.transfer(&env.current_contract_address(), &to, &amount);

        meta.balance -= amount;
        let inst = env.storage().instance();
        inst.set(&Symbol::new(&env, "meta"), &meta);

        env.events().publish(
            (
                Symbol::new(&env, "withdraw_to"),
                meta.owner.clone(),
                to.clone(),
            ),
            (amount, meta.balance),
        );
        meta.balance
    }

    /// Return current balance.
    pub fn balance(env: Env) -> i128 {
        Self::get_meta(env).balance
    }

    /// Set settlement contract address (admin only)
    pub fn set_settlement(env: Env, caller: Address, settlement_address: Address) {
        caller.require_auth();
        let current_admin = Self::get_admin(env.clone());
        if caller != current_admin {
            panic!("unauthorized: caller is not admin");
        }
        env.storage()
            .instance()
            .set(&Symbol::new(&env, SETTLEMENT_KEY), &settlement_address);
    }

    /// Get settlement contract address
    pub fn get_settlement(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&Symbol::new(&env, SETTLEMENT_KEY))
            .unwrap_or_else(|| panic!("settlement address not set"))
    }

    /// Transfer USDC to settlement contract (internal function)
    /// Used by deduct functions to automatically transfer revenue to settlement
    fn transfer_to_settlement(env: Env, amount: i128) {
        let settlement_address = Self::get_settlement(env.clone());
        let usdc_address: Address = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, USDC_KEY))
            .unwrap_or_else(|| panic!("vault not initialized"));

        let usdc = token::Client::new(&env, &usdc_address);
        
        // Transfer USDC to settlement contract
        usdc.transfer(&env.current_contract_address(), &settlement_address, &amount);

        // Emit transfer event
        env.events()
            .publish((Symbol::new(&env, "transfer_to_settlement"), settlement_address), amount);
    }
}

#[cfg(test)]
mod test;
