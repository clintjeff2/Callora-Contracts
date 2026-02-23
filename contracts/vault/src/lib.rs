#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol};

#[contracttype]
#[derive(Clone)]
pub struct VaultMeta {
    pub owner: Address,
    pub balance: i128,
}

#[contract]
pub struct CalloraVault;

#[contractimpl]
impl CalloraVault {
    /// Initialize vault for an owner with optional initial balance.
    /// Emits an "init" event with the owner address and initial balance.
    ///
    /// # Panics
    /// - If `initial_balance` is negative
    pub fn init(env: Env, owner: Address, initial_balance: Option<i128>) -> VaultMeta {
        let balance = initial_balance.unwrap_or(0);
        assert!(balance >= 0, "initial balance must be non-negative");
        let meta = VaultMeta {
            owner: owner.clone(),
            balance,
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
    ///
    /// # Panics
    /// - If the vault has not been initialized
    pub fn get_meta(env: Env) -> VaultMeta {
        env.storage()
            .instance()
            .get(&Symbol::new(&env, "meta"))
            .unwrap_or_else(|| panic!("vault not initialized"))
    }

    /// Deposit increases balance. Callable by owner or designated depositor.
    ///
    /// # Safety
    /// - `amount` must be strictly positive (> 0)
    /// - Use `checked_add` to prevent overflow + Panics on overflow
    ///
    /// # Panics
    /// - If `amount <= 0`
    /// - If adding `amount` to the current balance would overflow
    pub fn deposit(env: Env, amount: i128) -> i128 {
        assert!(amount > 0, "amount must be positive");
        let mut meta = Self::get_meta(env.clone());
        meta.balance = meta
            .balance
            .checked_add(amount)
            .expect("deposit overflow: balance would exceed i128::MAX");
        env.storage()
            .instance()
            .set(&Symbol::new(&env, "meta"), &meta);
        meta.balance
    }

    /// Deduct balance for an API call. Only backend/authorized caller in production.
    ///
    /// # Safety
    /// - `amount` must be strictly positive (> 0).
    /// - Uses `checked_sub` to prevent underflow. Panics on underflow.
    ///
    /// # Panics
    /// - If `amount <= 0`.
    /// - If `amount` exceeds the current balance (insufficient balance).
    pub fn deduct(env: Env, amount: i128) -> i128 {
        assert!(amount > 0, "amount must be positive");
        let mut meta = Self::get_meta(env.clone());
        assert!(meta.balance >= amount, "insufficient balance");
        meta.balance = meta
            .balance
            .checked_sub(amount)
            .expect("deduct underflow: balance would go below zero");
        env.storage()
            .instance()
            .set(&Symbol::new(&env, "meta"), &meta);
        meta.balance
    }

    /// Return current balance.
    pub fn balance(env: Env) -> i128 {
        Self::get_meta(env).balance
    }
}

#[cfg(test)]
mod test;
