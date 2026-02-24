#![no_std]

<<<<<<< feature/zero-negative-amount-guard
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol};
=======
use soroban_sdk::{contract, contractimpl, contracttype, token, Address, Env, Symbol, Vec};

/// Single item for batch deduct: amount and optional request id for idempotency/tracking.
#[contracttype]
#[derive(Clone)]
pub struct DeductItem {
    pub amount: i128,
    pub request_id: Option<Symbol>,
}
>>>>>>> main

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
        let min_deposit_val = min_deposit.unwrap_or(0);
        let meta = VaultMeta {
            owner: owner.clone(),
            balance,
            min_deposit: min_deposit_val,
        };
        env.storage()
            .instance()
            .set(&Symbol::new(&env, "meta"), &meta);
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
            .publish((Symbol::new(&env, "init"), owner.clone()), balance);

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
    pub fn get_meta(env: Env) -> VaultMeta {
        env.storage()
            .instance()
            .get(&Symbol::new(&env, "meta"))
            .unwrap_or_else(|| panic!("vault not initialized"))
    }

    /// Deposit increases balance. Callable by owner or designated depositor.
<<<<<<< feature/zero-negative-amount-guard
    /// Requires amount > 0 to prevent no-op transactions and accidental negative flows.
=======
    /// Panics if amount is below the configured minimum deposit.
    /// Emits a "deposit" event with amount and new balance.
>>>>>>> main
    pub fn deposit(env: Env, amount: i128) -> i128 {
        assert!(
            amount > 0,
            "deposit amount must be positive; received: {}",
            amount
        );
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
        meta.balance
    }

    /// Deduct balance for an API call. Only backend/authorized caller in production.
    /// Requires amount > 0 to prevent no-op transactions and accidental negative flows.
    pub fn deduct(env: Env, amount: i128) -> i128 {
        assert!(
            amount > 0,
            "deduct amount must be positive; received: {}",
            amount
        );
        let mut meta = Self::get_meta(env.clone());
        assert!(
            meta.balance >= amount,
            "insufficient balance: {} requested but only {} available",
            amount,
            meta.balance
        );
        meta.balance -= amount;
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
