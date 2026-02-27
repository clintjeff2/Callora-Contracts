//! # Callora Vault Contract
//!
//! ## Access Control
//!
//! The vault implements role-based access control for deposits:
//!
//! - **Owner**: Set at initialization, immutable. Always permitted to deposit.
//! - **Allowed Depositors**: Optional addresses (e.g., backend service) that can be
//!   explicitly approved by the owner. Can be set, changed, or cleared at any time.
//! - **Other addresses**: Rejected with an authorization error.
//!
//! ### Production Usage
//!
//! In production, the owner typically represents the end user's account, while the
//! allowed depositors are backend services that handle automated deposits on behalf
//! of the user.
//!
//! ### Managing the Allowed Depositors
//!
//! - Add or update: `set_allowed_depositor(Some(address))` adds the address if not present
//! - Clear (revoke all access): `set_allowed_depositor(None)`
//! - Only the owner can call `set_allowed_depositor`
//!
//! ### Security Model
//!
//! - The owner has full control over who can deposit
//! - The allowed depositors are trusted addresses (typically backend services)
//! - Access can be revoked at any time by the owner
//! - All deposit attempts are authenticated against the caller's address

#![no_std]

/// Single item for batch deduct: amount and optional request id for idempotency/tracking.
#[contracttype]
#[derive(Clone)]
pub struct DeductItem {
    pub amount: i128,
    pub request_id: Option<Symbol>,
}

#[contracttype]
#[derive(Clone)]
pub struct VaultMeta {
    pub owner: Address,
    pub balance: i128,
}

#[contracttype]
pub enum StorageKey {
    Meta,
    AllowedDepositors,
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
}

#[contract]
pub struct CalloraVault;

#[contractimpl]
impl CalloraVault {
    /// Initialize vault for an owner with optional initial balance.
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
            panic!("vault already initialized");
        }
        let balance = initial_balance.unwrap_or(0);
        assert!(balance >= 0, "initial balance must be non-negative");
        let meta = VaultMeta {
            owner: owner.clone(),
            balance,
        };
        env.storage().instance().set(&StorageKey::Meta, &meta);
            authorized_caller,
            min_deposit: min_deposit_val,
        };
        let inst = env.storage().instance();
        inst.set(&Symbol::new(&env, "meta"), &meta);
        inst.set(&Symbol::new(&env, META_KEY), &meta);
        inst.set(&Symbol::new(&env, USDC_KEY), &usdc_token);
        inst.set(&Symbol::new(&env, ADMIN_KEY), &owner);
        if let Some(pool) = revenue_pool {
            inst.set(&Symbol::new(&env, REVENUE_POOL_KEY), &pool);
        }
        inst.set(&Symbol::new(&env, MAX_DEDUCT_KEY), &max_deduct_val);

        // Emit event: topics = (init, owner), data = balance
        env.events()
            .publish((Symbol::new(&env, "init"), owner.clone()), balance);
        meta
    }

    /// Check if the caller is authorized to deposit (owner or allowed depositor).
    pub fn is_authorized_depositor(env: Env, caller: Address) -> bool {
    /// Return the current admin address.
    pub fn get_admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&Symbol::new(&env, ADMIN_KEY))
            .expect("vault not initialized")
    }

    /// Replace the current admin. Only the existing admin may call this.
    pub fn set_admin(env: Env, caller: Address, new_admin: Address) {
        caller.require_auth();
        let current_admin = Self::get_admin(env.clone());
        if caller != current_admin {
            panic!("unauthorized: caller is not admin");
        }
        let inst = env.storage().instance();
        inst.set(&Symbol::new(&env, ADMIN_KEY), &new_admin);
    }

        // Check if caller is in the allowed depositors
        let allowed: Vec<Address> = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, MAX_DEDUCT_KEY))
            .expect("vault not initialized")
            .get(&StorageKey::AllowedDepositors)
            .unwrap_or(Vec::new(&env));
        allowed.contains(&caller)
    }

    /// Require that the caller is the owner, panic otherwise.
    pub fn require_owner(env: Env, caller: Address) {
        let meta = Self::get_meta(env.clone());
        assert!(caller == meta.owner, "unauthorized: owner only");
    }

    /// Distribute accumulated USDC to a single developer address.
    /// Get vault metadata (owner and balance).
    ///
    /// # Panics
    /// - If the vault has not been initialized
    /// * `"unauthorized: caller is not admin"` – caller is not the admin.
    /// * `"amount must be positive"`           – amount is zero or negative.
    /// * `"insufficient USDC balance"`         – vault holds less than amount.
    ///
    /// # Events
    /// Emits topic `("distribute", to)` with data `amount` on success.
    pub fn distribute(env: Env, caller: Address, to: Address, amount: i128) {
        caller.require_auth();

        let admin = Self::get_admin(env.clone());
        if caller != admin {
            panic!("unauthorized: caller is not admin");
        }

        if amount <= 0 {
            panic!("amount must be positive");
        }

        let usdc_address: Address = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, USDC_KEY))
            .expect("vault not initialized");
        let usdc = token::Client::new(&env, &usdc_address);

        let vault_balance = usdc.balance(&env.current_contract_address());
        if vault_balance < amount {
            panic!("insufficient USDC balance");
        }

        usdc.transfer(&env.current_contract_address(), &to, &amount);

        env.events()
            .publish((Symbol::new(&env, "distribute"), to), amount);
    }

    /// Get vault metadata (owner and balance).
    pub fn get_meta(env: Env) -> VaultMeta {
        env.storage()
            .instance()
            .get(&Symbol::new(&env, META_KEY))
            .expect("vault not initialized")
            .get(&StorageKey::Meta)
            .unwrap_or_else(|| panic!("vault not initialized"))
    }

    /// Add or clear allowed depositors. Owner-only.
    /// Pass `None` to clear all allowed depositors, `Some(address)` to add the address if not already present.
    pub fn set_allowed_depositor(env: Env, caller: Address, depositor: Option<Address>) {
        caller.require_auth();
        Self::require_owner(env.clone(), caller.clone());

        match depositor {
            Some(addr) => {
                let mut allowed: Vec<Address> = env
                    .storage()
                    .instance()
                    .get(&StorageKey::AllowedDepositors)
                    .unwrap_or(Vec::new(&env));
                if !allowed.contains(&addr) {
                    allowed.push_back(addr);
                }
                env.storage()
                    .instance()
                    .set(&StorageKey::AllowedDepositors, &allowed);
            }
            None => {
                env.storage()
                    .instance()
                    .remove(&StorageKey::AllowedDepositors);
            }
        }
    }

    /// Deposit increases balance. Callable by owner or designated depositor.
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
    pub fn deposit(env: Env, from: Address, amount: i128) -> i128 {
        from.require_auth();
    /// Caller must have authorized the transfer (token transfer_from). Supports multiple depositors.
    /// Emits a "deposit" event with the depositor address and amount.
    pub fn deposit(env: Env, caller: Address, amount: i128) -> i128 {
        caller.require_auth();
        assert!(amount > 0, "amount must be positive");

        assert!(
            Self::is_authorized_depositor(env.clone(), caller.clone()),
            "unauthorized: only owner or allowed depositor can deposit"
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

        let mut meta = Self::get_meta(env.clone());
        meta.balance += amount;
        env.storage().instance().set(&StorageKey::Meta, &meta);
        let inst = env.storage().instance();
        inst.set(&Symbol::new(&env, "meta"), &meta);

        env.events()
            .publish((Symbol::new(&env, "deposit"), caller), amount);
        meta.balance
    }

    /// Deduct balance for an API call. Only owner/authorized caller in production.
    pub fn deduct(env: Env, caller: Address, amount: i128) -> i128 {
        caller.require_auth();
        Self::require_owner(env.clone(), caller);

        let mut meta = Self::get_meta(env.clone());
        assert!(amount > 0, "amount must be positive");
        assert!(meta.balance >= amount, "insufficient balance");
        meta.balance -= amount;
        env.storage().instance().set(&StorageKey::Meta, &meta);
        meta.balance
    }

    /// Deduct balance for an API call.
    /// Return current balance.
    pub fn balance(env: Env) -> i128 {
        Self::get_meta(env).balance
    }

    pub fn transfer_ownership(env: Env, new_owner: Address) {
        let mut meta = Self::get_meta(env.clone());
        meta.owner.require_auth();
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
            let within_limit = item.amount <= max_deduct;
            assert!(within_limit, "deduct amount exceeds max_deduct");
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

    /// Withdraw from vault. Callable only by the vault owner.
    pub fn withdraw(env: Env, amount: i128) -> i128 {
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

        // Validate new_owner is not the same as current owner
        assert!(
            new_owner != meta.owner,
            "new_owner must be different from current owner"
        );
        meta.balance
    }

    /// Withdraw from vault to a designated address. Owner-only.
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

        // Emit event before changing the owner, so we have the old owner
        // topics = (transfer_ownership, old_owner, new_owner)
        env.events().publish(
            (
                Symbol::new(&env, "transfer_ownership"),
                meta.owner.clone(),
                new_owner.clone(),
            ),
            (),
        );

        meta.owner = new_owner;
        env.storage().instance().set(&StorageKey::Meta, &meta);
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
