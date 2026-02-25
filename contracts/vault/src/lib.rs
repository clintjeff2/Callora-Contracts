#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, Symbol, Vec};

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
    /// Minimum amount required per deposit; deposits below this panic.
    pub min_deposit: i128,
}

const META_KEY: &str = "meta";
const USDC_KEY: &str = "usdc";
const ADMIN_KEY: &str = "admin";

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
    /// Initialize vault for an owner with optional initial balance and minimum deposit.
    /// Emits an "init" event with the owner address and initial balance.
    ///
    /// # Panics
    /// - If `initial_balance` is negative
    pub fn init(
        env: Env,
        owner: Address,
        usdc_token: Address,
        initial_balance: Option<i128>,
        min_deposit: Option<i128>,
    ) -> VaultMeta {
        owner.require_auth();
        if env.storage().instance().has(&Symbol::new(&env, META_KEY)) {
            panic!("vault already initialized");
        }
        let balance = initial_balance.unwrap_or(0);
        assert!(balance >= 0, "initial balance must be non-negative");
        let min_deposit_val = min_deposit.unwrap_or(0);
        let meta = VaultMeta {
            owner: owner.clone(),
            balance,
            min_deposit: min_deposit_val,
        };
        env.storage()
            .instance()
            .set(&Symbol::new(&env, META_KEY), &meta);
        env.storage()
            .instance()
            .set(&Symbol::new(&env, USDC_KEY), &usdc_token);
        env.storage()
            .instance()
            .set(&Symbol::new(&env, ADMIN_KEY), &owner);

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
        env.storage()
            .instance()
            .set(&Symbol::new(&env, ADMIN_KEY), &new_admin);
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
        let usdc_address: Address = env
            .storage()
            .instance()
            .get(&Symbol::new(&env, USDC_KEY))
            .unwrap_or_else(|| panic!("vault not initialized"));

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
    ///
    /// # Panics
    /// - If the vault has not been initialized
    pub fn get_meta(env: Env) -> VaultMeta {
        env.storage()
            .instance()
            .get(&Symbol::new(&env, META_KEY))
            .unwrap_or_else(|| panic!("vault not initialized"))
    }

    /// Deposit increases balance. Supports multiple depositors: any authorized user can deposit.
    /// Emits a "deposit" event with the depositor address and amount.
    pub fn deposit(env: Env, from: Address, amount: i128) -> i128 {
        from.require_auth();
        assert!(amount > 0, "amount must be positive");

        let mut meta = Self::get_meta(env.clone());
        assert!(
            amount >= meta.min_deposit,
            "deposit below minimum: {} < {}",
            amount,
            meta.min_deposit
        );
        meta.balance += amount;
        env.storage()
            .instance()
            .set(&Symbol::new(&env, META_KEY), &meta);

        // Emit event: topics = (deposit, from), data = amount
        env.events()
            .publish((Symbol::new(&env, "deposit"), from), amount);

        meta.balance
    }

    /// Deduct balance for an API call. Callable by authorized caller (e.g. backend/deployer).
    /// Emits a "deduct" event with caller, optional request_id, amount, and new balance.
    pub fn deduct(env: Env, caller: Address, amount: i128, request_id: Option<Symbol>) -> i128 {
        caller.require_auth();
        let mut meta = Self::get_meta(env.clone());
        assert!(meta.balance >= amount, "insufficient balance");
        meta.balance -= amount;
        env.storage()
            .instance()
            .set(&Symbol::new(&env, META_KEY), &meta);

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
    pub fn batch_deduct(env: Env, caller: Address, items: Vec<DeductItem>) -> i128 {
        caller.require_auth();
        let mut meta = Self::get_meta(env.clone());
        let n = items.len();
        assert!(n > 0, "batch_deduct requires at least one item");

        // Validate: running balance must never go negative
        let mut running = meta.balance;
        for item in items.iter() {
            assert!(item.amount > 0, "amount must be positive");
            assert!(running >= item.amount, "insufficient balance");
            running -= item.amount;
        }

        // Apply all deductions and emit one event per deduct
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
            .set(&Symbol::new(&env, META_KEY), &meta);
        meta.balance
    }

    /// Withdraw from vault. Callable only by the vault owner; reduces balance.
    /// When USDC is integrated, funds will be transferred to the owner.
    pub fn withdraw(env: Env, amount: i128) -> i128 {
        let mut meta = Self::get_meta(env.clone());
        meta.owner.require_auth();
        assert!(amount > 0, "amount must be positive");
        assert!(meta.balance >= amount, "insufficient balance");
        meta.balance -= amount;
        env.storage()
            .instance()
            .set(&Symbol::new(&env, META_KEY), &meta);

        env.events().publish(
            (Symbol::new(&env, "withdraw"), meta.owner.clone()),
            (amount, meta.balance),
        );
        meta.balance
    }

    /// Withdraw from vault to a designated address. Owner-only.
    /// When USDC is integrated, funds will be transferred to `to`.
    pub fn withdraw_to(env: Env, to: Address, amount: i128) -> i128 {
        let mut meta = Self::get_meta(env.clone());
        meta.owner.require_auth();
        assert!(amount > 0, "amount must be positive");
        assert!(meta.balance >= amount, "insufficient balance");
        meta.balance -= amount;
        env.storage()
            .instance()
            .set(&Symbol::new(&env, META_KEY), &meta);

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
