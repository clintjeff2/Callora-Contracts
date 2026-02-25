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

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol};

#[contracttype]
#[derive(Clone)]
pub struct VaultMeta {
    pub owner: Address,
    pub balance: i128,
}

#[contracttype]
pub enum StorageKey {
    Meta,
    AllowedDepositor,
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

        // Emit event: topics = (init, owner), data = balance
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

        // Owner is always authorized
        if caller == &meta.owner {
            return true;
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
            .unwrap_or_else(|| panic!("vault not initialized"));
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

    /// Deduct balance for an API call. Callable by authorized caller (e.g. backend).
    /// Amount must not exceed max single deduct (see init / get_max_deduct).
    /// If revenue pool is set, USDC is transferred to it; otherwise it remains in the vault.
    /// Emits a "deduct" event with caller, optional request_id, amount, and new balance.
    pub fn deduct(env: Env, caller: Address, amount: i128, request_id: Option<Symbol>) -> i128 {
        caller.require_auth();
        Self::require_owner(&env, &caller);

        meta.balance -= amount;
        let inst = env.storage().instance();
        inst.set(&Symbol::new(&env, "meta"), &meta);

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

    /// Deposit increases balance. Callable by owner or designated depositor.
    pub fn deposit(env: Env, caller: Address, amount: i128) -> i128 {
        caller.require_auth();

        let mut running = meta.balance;
        for item in items.iter() {
            assert!(item.amount > 0, "amount must be positive");
            assert!(
                item.amount <= max_deduct,
                "deduct amount exceeds max_deduct"
            );
            assert!(running >= item.amount, "insufficient balance");
            running -= item.amount;
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
        let inst = env.storage().instance();
        inst.set(&Symbol::new(&env, "meta"), &meta);
        meta.balance
    }

        let mut meta = Self::get_meta(env.clone());
        meta.owner.require_auth();
        assert!(amount > 0, "amount must be positive");
        assert!(meta.balance >= amount, "insufficient balance");

        let usdc_address: Address = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, USDC_KEY))
            .unwrap_or_else(|| panic!("vault not initialized"));
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

    /// Deduct balance for an API call. Only backend/authorized caller in production.
    pub fn deduct(env: Env, amount: i128) -> i128 {
        let mut meta = Self::get_meta(env.clone());
        assert!(meta.balance >= amount, "insufficient balance");
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
}

#[cfg(test)]
mod test;
