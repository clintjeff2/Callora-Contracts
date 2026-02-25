extern crate std;

use super::*;
use soroban_sdk::testutils::{Address as _, Events as _};
use soroban_sdk::{token, IntoVal, Symbol};

fn create_usdc<'a>(
    env: &'a Env,
    admin: &Address,
) -> (Address, token::Client<'a>, token::StellarAssetClient<'a>) {
    let contract_address = env.register_stellar_asset_contract_v2(admin.clone());
    let address = contract_address.address();
    let client = token::Client::new(env, &address);
    let admin_client = token::StellarAssetClient::new(env, &address);
    (address, client, admin_client)
}

fn create_vault(env: &Env) -> (Address, CalloraVaultClient<'_>) {
    let address = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(env, &address);
    (address, client)
}

fn fund_vault(
    usdc_admin_client: &token::StellarAssetClient,
    vault_address: &Address,
    amount: i128,
) {
    usdc_admin_client.mint(vault_address, &amount);
}

/// Full vault lifecycle integration test: init → deposit → batch_deduct →
/// set_admin → withdraw_to, verifying state at each step.
#[test]
fn vault_full_lifecycle() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let caller = Address::generate(&env);
    let recipient = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc, _, _) = create_usdc(&env, &owner);

    env.mock_all_auths();

    // 1. Initialise with 500 balance and min_deposit of 10.
    let meta = client.init(&owner, &usdc, &Some(500), &Some(10));
    assert_eq!(meta.balance, 500);
    assert_eq!(meta.owner, owner);
    assert_eq!(client.balance(), 500);
    assert_eq!(client.get_admin(), owner);

    // 2. Deposit – must be ≥ min_deposit.
    let after_deposit = client.deposit(&200);
    assert_eq!(after_deposit, 700);
    assert_eq!(client.balance(), 700);

    // 3. Batch deduct three items in one call.
    let items = soroban_sdk::vec![
        &env,
        DeductItem {
            amount: 100,
            request_id: Some(Symbol::new(&env, "r1")),
        },
        DeductItem {
            amount: 50,
            request_id: None,
        },
        DeductItem {
            amount: 25,
            request_id: Some(Symbol::new(&env, "r3")),
        },
    ];
    let after_batch = client.batch_deduct(&caller, &items);
    assert_eq!(after_batch, 525); // 700 - 175
    assert_eq!(client.balance(), 525);

    // 4. Single deduct.
    let after_deduct = client.deduct(&caller, &25, &Some(Symbol::new(&env, "r4")));
    assert_eq!(after_deduct, 500);

    // 5. Transfer admin to new_admin, then verify.
    client.set_admin(&owner, &new_admin);
    assert_eq!(client.get_admin(), new_admin);

    // 6. Withdraw to a recipient address.
    let after_withdraw = client.withdraw_to(&recipient, &100);
    assert_eq!(after_withdraw, 400);
    assert_eq!(client.balance(), 400);

    // 7. Direct withdraw (to owner).
    let after_withdraw2 = client.withdraw(&50);
    assert_eq!(after_withdraw2, 350);
    assert_eq!(client.balance(), 350);

    // 8. get_meta round-trip.
    let final_meta = client.get_meta();
    assert_eq!(final_meta.balance, 350);
    assert_eq!(final_meta.owner, owner);
    assert_eq!(final_meta.min_deposit, 10);
}

// ---------------------------------------------------------------------------
// init / balance
// ---------------------------------------------------------------------------

/// Initialising with an explicit balance stores that value and emits the event.
#[test]
fn init_with_balance_emits_event() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());

    let usdc_token = Address::generate(&env);

    // Mock all auth checks so init can proceed without signatures
    env.mock_all_auths();

    // Invoke init inside as_contract so the SDK captures the published event.
    let events = env.as_contract(&contract_id, || {
        CalloraVault::init(
            env.clone(),
            owner.clone(),
            usdc_token.clone(),
            Some(1000),
            None,
        );
        env.events().all()
    });

    // Balance must reflect the initial value.
    let client = CalloraVaultClient::new(&env, &contract_id);
    assert_eq!(client.balance(), 1000);

    // Exactly one event should have been emitted.
    let last_event = events.last().expect("expected at least one event");

    // Emitting contract must be our vault.
    assert_eq!(last_event.0, contract_id);

    // Topics: (Symbol("init"), owner)
    let topics = &last_event.1;
    assert_eq!(topics.len(), 2);
    let topic0: Symbol = topics.get(0).unwrap().into_val(&env);
    let topic1: Address = topics.get(1).unwrap().into_val(&env);
    assert_eq!(topic0, Symbol::new(&env, "init"));
    assert_eq!(topic1, owner);

    // Event data carries the starting balance.
    let data: i128 = last_event.2.into_val(&env);
    assert_eq!(data, 1000);
}

/// When no initial balance is provided the vault should default to zero.
#[test]
fn init_defaults_balance_to_zero() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    let usdc_token = Address::generate(&env);

    env.mock_all_auths();

    // Pass None — exercises the `unwrap_or(0)` branch in lib.rs.
    client.init(&owner, &usdc_token, &None, &None);
    assert_eq!(client.balance(), 0);
}

// ---------------------------------------------------------------------------
// get_meta
// ---------------------------------------------------------------------------

/// get_meta returns both the stored owner address and balance correctly.
#[test]
fn get_meta_returns_owner_and_balance() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    let usdc_token = Address::generate(&env);

    env.mock_all_auths();

    client.init(&owner, &usdc_token, &Some(500), &None);
    let meta = client.get_meta();

    assert_eq!(meta.owner, owner);
    assert_eq!(meta.balance, 500);
}

/// Calling get_meta before init must return an error (not a panic that kills
/// the test process) — exercises the `unwrap_or_else(panic)` error path.
#[test]
fn get_meta_before_init_fails() {
    let env = Env::default();
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    // try_get_meta() is the Result-returning variant generated by the SDK.
    let result = client.try_get_meta();
    assert!(
        result.is_err(),
        "expected error when vault is uninitialised"
    );
}

// ---------------------------------------------------------------------------
// deposit
// ---------------------------------------------------------------------------

/// Depositing accumulates correctly and the returned value matches balance().
#[test]
fn deposit_and_balance_match() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    let usdc_token = Address::generate(&env);

    env.mock_all_auths();

    client.init(&owner, &usdc_token, &Some(100), &None);
    let returned = client.deposit(&200);

    assert_eq!(
        returned, 300,
        "deposit should return the new running balance"
    );
    assert_eq!(client.balance(), 300);
}

// ---------------------------------------------------------------------------
// deduct
// ---------------------------------------------------------------------------

/// A valid deduction reduces the balance by exactly the requested amount.
#[test]
fn deduct_reduces_balance() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    let (usdc, _, _) = create_usdc(&env, &owner);
    let caller = Address::generate(&env);

    env.mock_all_auths();
    client.init(&owner, &usdc, &Some(100), &None);
    client.deposit(&200);
    assert_eq!(client.balance(), 300);

    let returned = client.deduct(&caller, &50, &None);
    assert_eq!(returned, 250, "deduct should return the remaining balance");
    assert_eq!(client.balance(), 250);
}

/// Deduct with request_id for idempotency tracking.
#[test]
fn deduct_with_request_id() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    let (usdc, _, _) = create_usdc(&env, &owner);
    let caller = Address::generate(&env);

    env.mock_all_auths();
    client.init(&owner, &usdc, &Some(1000), &None);

    let request_id = Symbol::new(&env, "req123");
    let remaining = client.deduct(&caller, &100, &Some(request_id));
    assert_eq!(remaining, 900);
}

/// Deducting more than the available balance must be rejected — exercises the
/// `assert!(meta.balance >= amount, "insufficient balance")` guard.
#[test]
fn deduct_insufficient_balance_fails() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    let usdc_token = Address::generate(&env);
    let caller = Address::generate(&env);

    env.mock_all_auths();

    client.init(&owner, &usdc_token, &Some(10), &None);

    // try_deduct() returns Result so we can assert on the error without
    // unwinding the test runner.
    let result = client.try_deduct(&caller, &100, &None);
    assert!(result.is_err(), "expected error for insufficient balance");
}

/// Deducting exactly the full balance should succeed and leave zero.
#[test]
fn deduct_exact_balance_succeeds() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    let usdc_token = Address::generate(&env);
    let caller = Address::generate(&env);

    env.mock_all_auths();

    client.init(&owner, &usdc_token, &Some(75), &None);
    let remaining = client.deduct(&caller, &75, &None);

    assert_eq!(remaining, 0);
    assert_eq!(client.balance(), 0);
}

// ---------------------------------------------------------------------------
// admin management
// ---------------------------------------------------------------------------

/// get_admin returns the admin address set during init.
#[test]
fn get_admin_returns_correct_address() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let usdc_token = Address::generate(&env);

    env.mock_all_auths();
    client.init(&owner, &usdc_token, &Some(100), &None);

    let admin = client.get_admin();
    assert_eq!(admin, owner);
}

/// Only the current admin can update the admin address.
#[test]
fn set_admin_updates_admin() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let usdc_token = Address::generate(&env);

    env.mock_all_auths();
    client.init(&owner, &usdc_token, &Some(100), &None);

    client.set_admin(&owner, &new_admin);
    assert_eq!(client.get_admin(), new_admin);
}

/// Non-admin callers cannot change the admin.
#[test]
fn set_admin_unauthorized_fails() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let intruder = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let usdc_token = Address::generate(&env);

    env.mock_all_auths();
    client.init(&owner, &usdc_token, &Some(100), &None);

    let result = client.try_set_admin(&intruder, &new_admin);
    assert!(
        result.is_err(),
        "expected error when non-admin tries to set admin"
    );
}

// ---------------------------------------------------------------------------
// distribute
// ---------------------------------------------------------------------------

/// Admin can distribute USDC from vault to a developer.
#[test]
fn distribute_transfers_usdc_to_developer() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let developer = Address::generate(&env);
    let (vault_address, client) = create_vault(&env);
    let (usdc, _, usdc_admin_client) = create_usdc(&env, &admin);

    env.mock_all_auths();

    client.init(&admin, &usdc, &Some(0), &None);

    // Mint 1000 USDC into the vault
    fund_vault(&env, &usdc_admin_client, &vault_address, 1000);

    // Distribute 300 to developer
    client.distribute(&admin, &developer, &300);

    // Verify developer received the funds
    let usdc_client = token::Client::new(&env, &usdc);
    assert_eq!(usdc_client.balance(&developer), 300);
    assert_eq!(usdc_client.balance(&vault_address), 700);
}

/// Non-admin cannot distribute funds.
#[test]
fn distribute_unauthorized_fails() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let intruder = Address::generate(&env);
    let developer = Address::generate(&env);
    let (vault_address, client) = create_vault(&env);
    let (usdc, _, usdc_admin_client) = create_usdc(&env, &admin);

    env.mock_all_auths();

    client.init(&admin, &usdc, &Some(0), &None);
    fund_vault(&env, &usdc_admin_client, &vault_address, 1000);

    let result = client.try_distribute(&intruder, &developer, &300);
    assert!(
        result.is_err(),
        "expected error when non-admin tries to distribute"
    );
}

/// Distributing more than vault balance fails.
#[test]
fn distribute_insufficient_usdc_fails() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let developer = Address::generate(&env);
    let (vault_address, client) = create_vault(&env);
    let (usdc, _, usdc_admin_client) = create_usdc(&env, &admin);

    env.mock_all_auths();

    client.init(&admin, &usdc, &Some(0), &None);
    fund_vault(&env, &usdc_admin_client, &vault_address, 100);

    let result = client.try_distribute(&admin, &developer, &500);
    assert!(
        result.is_err(),
        "expected error for insufficient USDC balance"
    );
}

/// Distributing zero or negative amount fails.
#[test]
fn distribute_zero_amount_fails() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let developer = Address::generate(&env);
    let (vault_address, client) = create_vault(&env);
    let (usdc, _, usdc_admin_client) = create_usdc(&env, &admin);

    env.mock_all_auths();

    client.init(&admin, &usdc, &Some(0), &None);
    fund_vault(&env, &usdc_admin_client, &vault_address, 1000);

    let result = client.try_distribute(&admin, &developer, &0);
    assert!(result.is_err(), "expected error for zero amount");
}

// ---------------------------------------------------------------------------
// batch_deduct
// ---------------------------------------------------------------------------

/// Batch deduct processes multiple items and emits multiple events.
#[test]
fn batch_deduct_multiple_items() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let caller = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let usdc_token = Address::generate(&env);

    env.mock_all_auths();
    client.init(&owner, &usdc_token, &Some(1000), &None);

    let items = soroban_sdk::vec![
        &env,
        DeductItem {
            amount: 100,
            request_id: Some(Symbol::new(&env, "req1"))
        },
        DeductItem {
            amount: 200,
            request_id: None
        },
        DeductItem {
            amount: 50,
            request_id: Some(Symbol::new(&env, "req2"))
        }
    ];

    let remaining = client.batch_deduct(&caller, &items);
    assert_eq!(remaining, 650); // 1000 - 100 - 200 - 50
    assert_eq!(client.balance(), 650);
}

/// Batch deduct fails if any item would overdraw.
#[test]
fn batch_deduct_insufficient_balance_fails() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let caller = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let usdc_token = Address::generate(&env);

    env.mock_all_auths();
    client.init(&owner, &usdc_token, &Some(100), &None);

    let items = soroban_sdk::vec![
        &env,
        DeductItem {
            amount: 50,
            request_id: None
        },
        DeductItem {
            amount: 80, // would overdraw
            request_id: None
        }
    ];

    let result = client.try_batch_deduct(&caller, &items);
    assert!(result.is_err(), "expected error for batch overdraw");
    // Balance should remain unchanged after failed batch
    assert_eq!(client.balance(), 100);
}

/// Empty batch fails.
#[test]
fn batch_deduct_empty_fails() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let caller = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let usdc_token = Address::generate(&env);

    env.mock_all_auths();
    client.init(&owner, &usdc_token, &Some(100), &None);

    let items: soroban_sdk::Vec<DeductItem> = soroban_sdk::vec![&env];

    let result = client.try_batch_deduct(&caller, &items);
    assert!(result.is_err(), "expected error for empty batch");
}

/// Batch deduct with zero amount fails.
#[test]
fn batch_deduct_zero_amount_fails() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let caller = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let usdc_token = Address::generate(&env);

    env.mock_all_auths();
    client.init(&owner, &usdc_token, &Some(100), &None);

    let items = soroban_sdk::vec![
        &env,
        DeductItem {
            amount: 0,
            request_id: None
        }
    ];

    let result = client.try_batch_deduct(&caller, &items);
    assert!(result.is_err(), "expected error for zero amount");
}

// ---------------------------------------------------------------------------
// withdraw
// ---------------------------------------------------------------------------

/// Owner can withdraw funds.
#[test]
fn withdraw_reduces_balance() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let usdc_token = Address::generate(&env);

    env.mock_all_auths();
    client.init(&owner, &usdc_token, &Some(500), &None);

    let remaining = client.withdraw(&200);
    assert_eq!(remaining, 300);
    assert_eq!(client.balance(), 300);
}

/// Withdrawing more than balance fails.
#[test]
fn withdraw_insufficient_balance_fails() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let usdc_token = Address::generate(&env);

    env.mock_all_auths();
    client.init(&owner, &usdc_token, &Some(100), &None);

    let result = client.try_withdraw(&500);
    assert!(result.is_err(), "expected error for insufficient balance");
}

/// Withdrawing zero or negative fails.
#[test]
fn withdraw_zero_fails() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let usdc_token = Address::generate(&env);

    env.mock_all_auths();
    client.init(&owner, &usdc_token, &Some(100), &None);

    let result = client.try_withdraw(&0);
    assert!(result.is_err(), "expected error for zero amount");
}

// ---------------------------------------------------------------------------
// withdraw_to
// ---------------------------------------------------------------------------

/// Owner can withdraw to a specified address.
#[test]
fn withdraw_to_reduces_balance() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let recipient = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let usdc_token = Address::generate(&env);

    env.mock_all_auths();
    client.init(&owner, &usdc_token, &Some(500), &None);

    let remaining = client.withdraw_to(&recipient, &150);
    assert_eq!(remaining, 350);
    assert_eq!(client.balance(), 350);
}

/// Withdrawing to address with insufficient balance fails.
#[test]
fn withdraw_to_insufficient_balance_fails() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let recipient = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let usdc_token = Address::generate(&env);

    env.mock_all_auths();
    client.init(&owner, &usdc_token, &Some(100), &None);

    let result = client.try_withdraw_to(&recipient, &500);
    assert!(result.is_err(), "expected error for insufficient balance");
}

// ---------------------------------------------------------------------------
// min_deposit
// ---------------------------------------------------------------------------

/// Deposits below min_deposit are rejected.
#[test]
fn deposit_below_minimum_fails() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let usdc_token = Address::generate(&env);

    env.mock_all_auths();
    client.init(&owner, &usdc_token, &Some(100), &Some(50)); // min_deposit = 50

    let result = client.try_deposit(&30); // below minimum
    assert!(result.is_err(), "expected error for deposit below minimum");
}

/// Deposits at or above min_deposit succeed.
#[test]
fn deposit_at_minimum_succeeds() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let usdc_token = Address::generate(&env);

    env.mock_all_auths();
    client.init(&owner, &usdc_token, &Some(100), &Some(50)); // min_deposit = 50

    let new_balance = client.deposit(&50);
    assert_eq!(new_balance, 150);
}

// ---------------------------------------------------------------------------
// double init guard
// ---------------------------------------------------------------------------

/// Calling init twice fails.
#[test]
fn double_init_fails() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let usdc_token = Address::generate(&env);

    env.mock_all_auths();
    client.init(&owner, &usdc_token, &Some(100), &None);

    let result = client.try_init(&owner, &usdc_token, &Some(200), &None);
    assert!(result.is_err(), "expected error for double init");
}
