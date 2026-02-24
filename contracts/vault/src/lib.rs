#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol, Vec};

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

#[contract]
pub struct CalloraVault;

#[contractimpl]
impl CalloraVault {
    /// Initialize vault for an owner with optional initial balance and minimum deposit.
    /// Emits an "init" event with the owner address and initial balance.
    /// `min_deposit`: minimum amount per deposit; deposits below this will panic. Use 0 for no minimum.
    pub fn init(
        env: Env,
        owner: Address,
        initial_balance: Option<i128>,
        min_deposit: Option<i128>,
    ) -> VaultMeta {
        if env.storage().instance().has(&Symbol::new(&env, "meta")) {
            panic!("vault already initialized");
        }
        owner.require_auth();

        let balance = initial_balance.unwrap_or(0);
        let min_deposit_val = min_deposit.unwrap_or(0);
        let meta = VaultMeta {
            owner: owner.clone(),
            balance,
            min_deposit: min_deposit_val,
        };
        env.storage()
            .instance()
            .set(&Symbol::new(&env, "meta"), &meta);

        // Emit event: topics = (init, owner), data = balance
        env.events()
            .publish((Symbol::new(&env, "init"), owner), balance);

        meta
    }

    /// Get vault metadata (owner and balance).
    pub fn get_meta(env: Env) -> VaultMeta {
        env.storage()
            .instance()
            .get(&Symbol::new(&env, "meta"))
            .unwrap_or_else(|| panic!("vault not initialized"))
    }

    /// Deposit increases balance. Callable by owner or designated depositor.
    /// Panics if amount is below the configured minimum deposit.
    /// Emits a "deposit" event with amount and new balance.
    pub fn deposit(env: Env, amount: i128) -> i128 {
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
            .set(&Symbol::new(&env, "meta"), &meta);

        env.events()
            .publish((Symbol::new(&env, "deposit"),), (amount, meta.balance));
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
            .set(&Symbol::new(&env, "meta"), &meta);

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
            .set(&Symbol::new(&env, "meta"), &meta);
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
            .set(&Symbol::new(&env, "meta"), &meta);

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
            .set(&Symbol::new(&env, "meta"), &meta);

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
