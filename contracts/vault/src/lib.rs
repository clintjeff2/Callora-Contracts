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
    pub fn init(env: Env, owner: Address, initial_balance: Option<i128>) -> VaultMeta {
        let balance = initial_balance.unwrap_or(0);
        let meta = VaultMeta { owner: owner.clone(), balance };
        env.storage().instance().set(&Symbol::new(&env, "meta"), &meta);

        // Emit event: topics = (init, owner), data = balance
        env.events().publish(
            (Symbol::new(&env, "init"), owner),
            balance,
        );

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
    pub fn deposit(env: Env, amount: i128) -> i128 {
        let mut meta = Self::get_meta(env.clone());
        meta.balance += amount;
        env.storage().instance().set(&Symbol::new(&env, "meta"), &meta);
        meta.balance
    }

    /// Deduct balance for an API call. Only backend/authorized caller in production.
    pub fn deduct(env: Env, amount: i128) -> i128 {
        let mut meta = Self::get_meta(env.clone());
        assert!(meta.balance >= amount, "insufficient balance");
        meta.balance -= amount;
        env.storage().instance().set(&Symbol::new(&env, "meta"), &meta);
        meta.balance
    }

    /// Return current balance.
    pub fn balance(env: Env) -> i128 {
        Self::get_meta(env).balance
    }
}

#[cfg(test)]
mod test;
