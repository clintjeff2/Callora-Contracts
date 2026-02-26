//! Vault contract unit tests (deposits, access control, API pricing).

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

#[test]
fn vault_full_lifecycle() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let caller = Address::generate(&env);
    let recipient = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();

    fund_vault(&usdc_admin, &contract_id, 500);
    let meta = client.init(&owner, &usdc, &Some(500), &Some(10), &None, &None);
    assert_eq!(meta.balance, 500);
    assert_eq!(meta.owner, owner);
    assert_eq!(client.balance(), 500);
    assert_eq!(client.get_admin(), owner);

    let depositor = Address::generate(&env);
    fund_vault(&usdc_admin, &depositor, 200);
    let usdc_client = token::Client::new(&env, &usdc);
    usdc_client.approve(&depositor, &contract_id, &200, &1000);
    let after_deposit = client.deposit(&depositor, &200);
    assert_eq!(after_deposit, 700);
    assert_eq!(client.balance(), 700);

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
    assert_eq!(after_batch, 525);
    assert_eq!(client.balance(), 525);

    let after_deduct = client.deduct(&caller, &25, &Some(Symbol::new(&env, "r4")));
    assert_eq!(after_deduct, 500);

    client.set_admin(&owner, &new_admin);
    assert_eq!(client.get_admin(), new_admin);

    let after_withdraw = client.withdraw_to(&recipient, &100);
    assert_eq!(after_withdraw, 400);
    assert_eq!(client.balance(), 400);

    let after_withdraw2 = client.withdraw(&50);
    assert_eq!(after_withdraw2, 350);
    assert_eq!(client.balance(), 350);

    let final_meta = client.get_meta();
    assert_eq!(final_meta.balance, 350);
    assert_eq!(final_meta.owner, owner);
    assert_eq!(final_meta.min_deposit, 10);
}

#[test]
fn init_with_balance_emits_event() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 1000);

    env.mock_all_auths();
    // Call init directly inside as_contract so events are captured
    let events = env.as_contract(&contract_id, || {
        CalloraVault::init(
            env.clone(),
            owner.clone(),
            usdc_token.clone(),
            Some(1000),
            None,
            None,
            None,
        );
        env.events().all()
    });

    let client = CalloraVaultClient::new(&env, &contract_id);
    assert_eq!(client.balance(), 1000);

    let last_event = events.last().expect("expected at least one event");
    assert_eq!(last_event.0, contract_id);

    let topics = &last_event.1;
    assert_eq!(topics.len(), 2);
    let topic0: Symbol = topics.get(0).unwrap().into_val(&env);
    let topic1: Address = topics.get(1).unwrap().into_val(&env);
    assert_eq!(topic0, Symbol::new(&env, "init"));
    assert_eq!(topic1, owner);

    let data: i128 = last_event.2.into_val(&env);
    assert_eq!(data, 1000);
}

#[test]
fn init_defaults_balance_to_zero() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, _) = create_usdc(&env, &owner);

    env.mock_all_auths();
    client.init(&owner, &usdc_token, &None, &None, &None, &None);
    assert_eq!(client.balance(), 0);
}

#[test]
fn get_meta_returns_owner_and_balance() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    client.init(&owner, &Some(100));

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 500);
    client.init(&owner, &usdc_token, &Some(500), &None, &None, &None);
    let meta = client.get_meta();

    assert_eq!(meta.owner, owner);
    assert_eq!(meta.balance, 500);
}

#[test]
fn get_meta_before_init_fails() {
    let env = Env::default();
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    let result = client.try_get_meta();
    assert!(
        result.is_err(),
        "expected error when vault is uninitialised"
    );
}

#[test]
fn deposit_and_balance_match() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let depositor = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 100);
    client.init(&owner, &usdc_token, &Some(100), &None, &None, &None);

    fund_vault(&usdc_admin, &depositor, 200);
    let usdc_client = token::Client::new(&env, &usdc_token);
    usdc_client.approve(&depositor, &contract_id, &200, &1000);
    let returned = client.deposit(&depositor, &200);

    assert_eq!(
        returned, 300,
        "deposit should return the new running balance"
    );
    assert_eq!(client.balance(), 300);
}

#[test]
fn deduct_reduces_balance() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc, _, usdc_admin) = create_usdc(&env, &owner);
    let caller = Address::generate(&env);
    let depositor = Address::generate(&env);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 100);
    client.init(&owner, &usdc, &Some(100), &None, &None, &None);

    fund_vault(&usdc_admin, &depositor, 200);
    let usdc_client = token::Client::new(&env, &usdc);
    usdc_client.approve(&depositor, &contract_id, &200, &1000);
    client.deposit(&depositor, &200);
    assert_eq!(client.balance(), 300);

    let returned = client.deduct(&caller, &50, &None);
    assert_eq!(returned, 250, "deduct should return the remaining balance");
    assert_eq!(client.balance(), 250);
}

#[test]
fn deduct_with_request_id() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc, _, usdc_admin) = create_usdc(&env, &owner);
    let caller = Address::generate(&env);

    // Initialize vault with initial balance
    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 1000);
    client.init(&owner, &usdc, &Some(1000), &None, &None, &None);

    let request_id = Symbol::new(&env, "req123");
    let remaining = client.deduct(&caller, &100, &Some(request_id));
    assert_eq!(remaining, 900);
}

#[test]
fn deduct_insufficient_balance_fails() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);
    let caller = Address::generate(&env);
    env.mock_all_auths();
    client.init(&owner, &Some(100));

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 10);
    client.init(&owner, &usdc_token, &Some(10), &None, &None, &None);

    let result = client.try_deduct(&caller, &100, &None);
    assert!(result.is_err(), "expected error for insufficient balance");
}

#[test]
fn deduct_exact_balance_succeeds() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);
    let caller = Address::generate(&env);

    env.mock_all_auths();
    client.init(&owner, &Some(100));

    // Try to deposit as unauthorized address (should panic)
    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 75);
    client.init(&owner, &usdc_token, &Some(75), &None, &None, &None);
    let remaining = client.deduct(&caller, &75, &None);

    assert_eq!(remaining, 0);
    assert_eq!(client.balance(), 0);
}

#[test]
fn deduct_event_contains_request_id() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let caller = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    client.init(&owner, &Some(100));

    // Owner sets allowed depositor
    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 500);
    client.init(&owner, &usdc_token, &Some(500), &None, &None, &None);

    let request_id = Symbol::new(&env, "api_call_42");
    client.deduct(&caller, &150, &Some(request_id.clone()));

    let events = env.events().all();
    let ev = events.last().expect("expected deduct event");

    let topic0: Symbol = ev.1.get(0).unwrap().into_val(&env);
    let topic1: Address = ev.1.get(1).unwrap().into_val(&env);
    let topic2: Symbol = ev.1.get(2).unwrap().into_val(&env);

    assert_eq!(topic0, Symbol::new(&env, "deduct"));
    assert_eq!(topic1, caller);
    assert_eq!(topic2, request_id);

    let (emitted_amount, remaining): (i128, i128) = ev.2.into_val(&env);
    assert_eq!(emitted_amount, 150);
    assert_eq!(remaining, 350);
}

#[test]
fn deduct_event_no_request_id_uses_empty_symbol() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let caller = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 300);
    client.init(&owner, &usdc_token, &Some(300), &None, &None, &None);
    client.deduct(&caller, &100, &None);

    let events = env.events().all();
    let ev = events.last().expect("expected deduct event");

    let topic0: Symbol = ev.1.get(0).unwrap().into_val(&env);
    let topic2: Symbol = ev.1.get(2).unwrap().into_val(&env);

    assert_eq!(topic0, Symbol::new(&env, "deduct"));
    assert_eq!(topic2, Symbol::new(&env, ""));
}

#[test]
fn batch_deduct_events_contain_request_ids() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let caller = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);
    env.mock_all_auths();
    client.init(&owner, &Some(100));

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 1000);
    client.init(&owner, &usdc_token, &Some(1000), &None, &None, &None);

    let rid_a = Symbol::new(&env, "batch_a");
    let rid_b = Symbol::new(&env, "batch_b");

    let items = soroban_sdk::vec![
        &env,
        DeductItem {
            amount: 200,
            request_id: Some(rid_a.clone()),
        },
        DeductItem {
            amount: 300,
            request_id: Some(rid_b.clone()),
        },
    ];
    client.batch_deduct(&caller, &items);

    let all_events = env.events().all();
    assert_eq!(all_events.len(), 2);

    let ev_a = all_events.get(0).unwrap();
    let ev_b = all_events.get(1).unwrap();

    let req_a: Symbol = ev_a.1.get(2).unwrap().into_val(&env);
    let req_b: Symbol = ev_b.1.get(2).unwrap().into_val(&env);
    assert_eq!(req_a, rid_a);
    assert_eq!(req_b, rid_b);

    let (amt_a, _): (i128, i128) = ev_a.2.into_val(&env);
    let (amt_b, _): (i128, i128) = ev_b.2.into_val(&env);
    assert_eq!(amt_a, 200);
    assert_eq!(amt_b, 300);
}

#[test]
fn get_admin_returns_correct_address() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 100);
    client.init(&owner, &usdc_token, &Some(100), &None, &None, &None);

    let admin = client.get_admin();
    assert_eq!(admin, owner);
}

#[test]
fn set_admin_updates_admin() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);
    client.init(&owner, &Some(100));

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 100);
    client.init(&owner, &usdc_token, &Some(100), &None, &None, &None);

    client.set_admin(&owner, &new_admin);
    assert_eq!(client.get_admin(), new_admin);
}

#[test]
fn set_admin_unauthorized_fails() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let intruder = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 100);
    client.init(&owner, &usdc_token, &Some(100), &None, &None, &None);

    let result = client.try_set_admin(&intruder, &new_admin);
    assert!(
        result.is_err(),
        "expected error when non-admin tries to set admin"
    );
}

#[test]
fn distribute_transfers_usdc_to_developer() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let developer = Address::generate(&env);
    let (vault_address, client) = create_vault(&env);
    let (usdc, usdc_client, usdc_admin_client) = create_usdc(&env, &admin);

    env.mock_all_auths();

    fund_vault(&usdc_admin_client, &vault_address, 1000);
    client.init(&admin, &usdc, &Some(0), &None, &None, &None);

    client.distribute(&admin, &developer, &300);

    assert_eq!(usdc_client.balance(&developer), 300);
    assert_eq!(usdc_client.balance(&vault_address), 700);
}

#[test]
fn distribute_unauthorized_fails() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let intruder = Address::generate(&env);
    let developer = Address::generate(&env);
    let (vault_address, client) = create_vault(&env);
    let (usdc, _, usdc_admin_client) = create_usdc(&env, &admin);

    env.mock_all_auths();

    fund_vault(&usdc_admin_client, &vault_address, 1000);
    client.init(&admin, &usdc, &Some(0), &None, &None, &None);

    let result = client.try_distribute(&intruder, &developer, &300);
    assert!(
        result.is_err(),
        "expected error when non-admin tries to distribute"
    );
}

#[test]
fn distribute_insufficient_usdc_fails() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let developer = Address::generate(&env);
    let (vault_address, client) = create_vault(&env);
    let (usdc, _, usdc_admin_client) = create_usdc(&env, &admin);

    env.mock_all_auths();

    fund_vault(&usdc_admin_client, &vault_address, 100);
    client.init(&admin, &usdc, &Some(0), &None, &None, &None);

    let result = client.try_distribute(&admin, &developer, &500);
    assert!(
        result.is_err(),
        "expected error for insufficient USDC balance"
    );
}

#[test]
fn distribute_zero_amount_fails() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let developer = Address::generate(&env);
    let (vault_address, client) = create_vault(&env);
    let (usdc, _, usdc_admin_client) = create_usdc(&env, &admin);
    client.init(&owner, &Some(100));

    env.mock_all_auths();

    fund_vault(&usdc_admin_client, &vault_address, 1000);
    client.init(&admin, &usdc, &Some(0), &None, &None, &None);

    let result = client.try_distribute(&admin, &developer, &0);
    assert!(result.is_err(), "expected error for zero amount");
}

// ============================================================================
// Offering Metadata Tests
// ============================================================================

#[test]
fn set_and_retrieve_metadata() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    env.mock_all_auths();

    // Set metadata for an offering
    let offering_id = String::from_str(&env, "offering-001");
    let metadata = String::from_str(&env, "QmXoypizjW3WknFiJnKLwHCnL72vedxjQkDDP1mXWo6uco");

    let result = client.set_metadata(&owner, &offering_id, &metadata);
    assert_eq!(result, metadata);

    // Retrieve metadata
    let retrieved = client.get_metadata(&offering_id);
    assert_eq!(retrieved, Some(metadata));
}

#[test]
fn set_metadata_emits_event() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());

    env.mock_all_auths();

    // Initialize first
    env.as_contract(&contract_id, || {
        CalloraVault::init(env.clone(), owner.clone(), Some(100));
    });

    let offering_id = String::from_str(&env, "offering-002");
    let metadata = String::from_str(
        &env,
        "bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi",
    );

    // Call set_metadata inside as_contract to capture events
    let events = env.as_contract(&contract_id, || {
        CalloraVault::set_metadata(
            env.clone(),
            owner.clone(),
            offering_id.clone(),
            metadata.clone(),
        );
        env.events().all()
    });

    // Verify event was emitted
    let last_event = events.last().expect("expected metadata_set event");

    // Verify event structure
    assert_eq!(last_event.0, contract_id);

    let topics = &last_event.1;
    assert_eq!(topics.len(), 3);

    let topic0: Symbol = topics.get(0).unwrap().into_val(&env);
    let topic1: String = topics.get(1).unwrap().into_val(&env);
    let topic2: Address = topics.get(2).unwrap().into_val(&env);

    assert_eq!(topic0, Symbol::new(&env, "metadata_set"));
    assert_eq!(topic1, offering_id);
    assert_eq!(topic2, owner);

    // Data should be the metadata string
    let data: String = last_event.2.into_val(&env);
    assert_eq!(data, metadata);
}

#[test]
fn update_metadata_and_verify() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    env.mock_all_auths();

    let offering_id = String::from_str(&env, "offering-003");
    let old_metadata = String::from_str(&env, "QmOldMetadata123");
    let new_metadata = String::from_str(&env, "QmNewMetadata456");

    // Set initial metadata
    client.set_metadata(&owner, &offering_id, &old_metadata);

    // Update metadata
    let result = client.update_metadata(&owner, &offering_id, &new_metadata);
    assert_eq!(result, new_metadata);

    // Verify updated metadata
    let retrieved = client.get_metadata(&offering_id);
    assert_eq!(retrieved, Some(new_metadata));
}

#[test]
fn batch_deduct_multiple_items() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let caller = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 1000);
    client.init(&owner, &usdc_token, &Some(1000), &None, &None, &None);

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
    assert_eq!(remaining, 650);
    assert_eq!(client.balance(), 650);
}

#[test]
fn batch_deduct_insufficient_balance_fails() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let caller = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 100);
    client.init(&owner, &usdc_token, &Some(100), &None, &None, &None);

    let items = soroban_sdk::vec![
        &env,
        DeductItem {
            amount: 50,
            request_id: None
        },
        DeductItem {
            amount: 80,
            request_id: None
        }
    ];

    let result = client.try_batch_deduct(&caller, &items);
    assert!(result.is_err(), "expected error for batch overdraw");
    assert_eq!(client.balance(), 100);
}

#[test]
fn batch_deduct_empty_fails() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let caller = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 100);
    client.init(&owner, &usdc_token, &Some(100), &None, &None, &None);

    let items: soroban_sdk::Vec<DeductItem> = soroban_sdk::vec![&env];

    let result = client.try_batch_deduct(&caller, &items);
    assert!(result.is_err(), "expected error for empty batch");
}

#[test]
fn batch_deduct_zero_amount_fails() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let caller = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 100);
    client.init(&owner, &usdc_token, &Some(100), &None, &None, &None);

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

#[test]
fn withdraw_reduces_balance() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 500);
    client.init(&owner, &usdc_token, &Some(500), &None, &None, &None);

    let remaining = client.withdraw(&200);
    assert_eq!(remaining, 300);
    assert_eq!(client.balance(), 300);
}

#[test]
fn withdraw_insufficient_balance_fails() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 100);
    client.init(&owner, &usdc_token, &Some(100), &None, &None, &None);

    let result = client.try_withdraw(&500);
    assert!(result.is_err(), "expected error for insufficient balance");
}

#[test]
fn withdraw_zero_fails() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 100);
    client.init(&owner, &usdc_token, &Some(100), &None, &None, &None);

    let result = client.try_withdraw(&0);
    assert!(result.is_err(), "expected error for zero amount");
}
    env.mock_all_auths();

    // transfer ownership via client
    // Owner authorizes transfer (require_auth in contract)
    client.transfer_ownership(&new_owner);

#[test]
fn withdraw_to_reduces_balance() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let recipient = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 500);
    client.init(&owner, &usdc_token, &Some(500), &None, &None, &None);

    let remaining = client.withdraw_to(&recipient, &150);
    assert_eq!(remaining, 350);
    assert_eq!(client.balance(), 350);
}

#[test]
fn withdraw_to_insufficient_balance_fails() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let recipient = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 100);
    client.init(&owner, &usdc_token, &Some(100), &None, &None, &None);

    let result = client.try_withdraw_to(&recipient, &500);
    assert!(result.is_err(), "expected error for insufficient balance");
}

#[test]
fn deposit_below_minimum_fails() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let depositor = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
fn allowed_depositor_can_set_price() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let depositor = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    let api_id = Symbol::new(&env, "backend_api");

    env.mock_all_auths();
    client.set_allowed_depositor(&owner, &Some(depositor.clone()));

    client.set_price(&depositor, &api_id, &25);

    let price = client.get_price(&api_id);
    assert_eq!(price, Some(25));
}

#[test]
#[should_panic(expected = "unauthorized: only owner or allowed depositor can set price")]
fn unauthorized_cannot_set_price() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let unauthorized = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 100);
    client.init(&owner, &usdc_token, &Some(100), &Some(50), &None, &None);

    fund_vault(&usdc_admin, &depositor, 30);
    let usdc_client = token::Client::new(&env, &usdc_token);
    usdc_client.approve(&depositor, &contract_id, &30, &1000);
    let result = client.try_deposit(&depositor, &30);
    assert!(result.is_err(), "expected error for deposit below minimum");
}

#[test]
fn deposit_at_minimum_succeeds() {
    client.init(&owner, &Some(100));

    let api_id = Symbol::new(&env, "restricted_api");

    env.mock_all_auths();
    client.set_price(&unauthorized, &api_id, &5);
}

#[test]
fn update_metadata_emits_event() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());

    env.mock_all_auths();

    // Initialize first
    env.as_contract(&contract_id, || {
        CalloraVault::init(env.clone(), owner.clone(), Some(100));
    });

    let offering_id = String::from_str(&env, "offering-004");
    let old_metadata = String::from_str(&env, "https://example.com/old.json");
    let new_metadata = String::from_str(&env, "https://example.com/new.json");

    // Set initial metadata
    env.as_contract(&contract_id, || {
        CalloraVault::set_metadata(
            env.clone(),
            owner.clone(),
            offering_id.clone(),
            old_metadata.clone(),
        );
    });

    // Update and capture events
    let events = env.as_contract(&contract_id, || {
        CalloraVault::update_metadata(
            env.clone(),
            owner.clone(),
            offering_id.clone(),
            new_metadata.clone(),
        );
        env.events().all()
    });

    // Verify event was emitted
    let last_event = events.last().expect("expected metadata_updated event");

    // Verify event structure
    assert_eq!(last_event.0, contract_id);

    let topics = &last_event.1;
    assert_eq!(topics.len(), 3);

    let topic0: Symbol = topics.get(0).unwrap().into_val(&env);
    let topic1: String = topics.get(1).unwrap().into_val(&env);
    let topic2: Address = topics.get(2).unwrap().into_val(&env);

    assert_eq!(topic0, Symbol::new(&env, "metadata_updated"));
    assert_eq!(topic1, offering_id);
    assert_eq!(topic2, owner);

    // Data should be tuple of (old_metadata, new_metadata)
    let data: (String, String) = last_event.2.into_val(&env);
    assert_eq!(data.0, old_metadata);
    assert_eq!(data.1, new_metadata);
}

#[test]
#[should_panic(expected = "unauthorized: owner only")]
fn unauthorized_cannot_set_metadata() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let unauthorized = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    env.mock_all_auths();

    let offering_id = String::from_str(&env, "offering-005");
    let metadata = String::from_str(&env, "QmUnauthorized");

    // Unauthorized user tries to set metadata (should panic)
    client.set_metadata(&unauthorized, &offering_id, &metadata);
}

#[test]
#[should_panic(expected = "new_owner must be different from current owner")]
fn test_transfer_ownership_same_address_fails() {
#[should_panic(expected = "insufficient balance")]
fn deduct_greater_than_balance_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let depositor = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 100);
    client.init(&owner, &usdc_token, &Some(100), &Some(50), &None, &None);

    fund_vault(&usdc_admin, &depositor, 50);
    let usdc_client = token::Client::new(&env, &usdc_token);
    usdc_client.approve(&depositor, &contract_id, &50, &1000);
    let new_balance = client.deposit(&depositor, &50);
    assert_eq!(new_balance, 150);
}

#[test]
fn double_init_fails() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);
    client.init(&owner, &Some(100));

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 100);
    client.init(&owner, &usdc_token, &Some(100), &None, &None, &None);

    let result = client.try_init(&owner, &usdc_token, &Some(200), &None, &None, &None);
    assert!(result.is_err(), "expected error for double init");
}

#[test]
fn init_insufficient_usdc_balance_fails() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    // Initialize with balance of 100
    client.init(&owner, &Some(100));
    assert_eq!(client.balance(), 100);

    // Mock the owner as the invoker
    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 50);

    let result = client.try_init(&owner, &usdc_token, &Some(100), &None, &None, &None);
    assert!(
        result.is_err(),
        "expected error when initial_balance exceeds contract USDC"
    );
}
    env.mock_all_auths();

    // This should panic because new_owner is the same as current owner
    client.transfer_ownership(&owner);
    // Attempt to deduct more than balance, which should panic
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.deduct(&owner, &101);
    }));

#[test]
fn init_with_zero_max_deduct_fails() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, _) = create_usdc(&env, &owner);

    env.mock_all_auths();

    let result = client.try_init(&owner, &usdc_token, &None, &None, &None, &Some(0));
    assert!(result.is_err(), "expected error for max_deduct <= 0");
}

#[test]
fn init_with_revenue_pool_and_get_revenue_pool() {
#[should_panic(expected = "unauthorized: owner only")]
fn unauthorized_cannot_update_metadata() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let unauthorized = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    env.mock_all_auths();

    let offering_id = String::from_str(&env, "offering-006");
    let metadata = String::from_str(&env, "QmInitial");

    // Owner sets metadata
    client.set_metadata(&owner, &offering_id, &metadata);

    // Unauthorized user tries to update (should panic)
    let new_metadata = String::from_str(&env, "QmUnauthorized");
    client.update_metadata(&unauthorized, &offering_id, &new_metadata);
}

#[test]
fn empty_metadata_is_allowed() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    env.mock_all_auths();

    let offering_id = String::from_str(&env, "offering-007");
    let empty_metadata = String::from_str(&env, "");

    // Empty string should be allowed
    client.set_metadata(&owner, &offering_id, &empty_metadata);

    let retrieved = client.get_metadata(&offering_id);
    assert_eq!(retrieved, Some(empty_metadata));
}

#[test]
#[should_panic(expected = "metadata exceeds maximum length")]
fn oversized_metadata_is_rejected() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    env.mock_all_auths();

    let offering_id = String::from_str(&env, "offering-008");

    // Create a string that exceeds MAX_METADATA_LENGTH (256 chars)
    let oversized = "a".repeat(257);
    let oversized_metadata = String::from_str(&env, &oversized);

    // Should panic due to length constraint
    client.set_metadata(&owner, &offering_id, &oversized_metadata);
}

#[test]
#[should_panic(expected = "metadata exceeds maximum length")]
fn oversized_update_is_rejected() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    env.mock_all_auths();

    let offering_id = String::from_str(&env, "offering-009");
    let initial_metadata = String::from_str(&env, "QmInitial");

    // Set initial metadata
    client.set_metadata(&owner, &offering_id, &initial_metadata);

    // Try to update with oversized metadata
    let oversized = "b".repeat(257);
    let oversized_metadata = String::from_str(&env, &oversized);

    // Should panic due to length constraint
    client.update_metadata(&owner, &offering_id, &oversized_metadata);
}

#[test]
fn repeated_updates_to_same_offering() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    env.mock_all_auths();

    let offering_id = String::from_str(&env, "offering-010");

    // Set initial metadata
    let metadata1 = String::from_str(&env, "QmVersion1");
    client.set_metadata(&owner, &offering_id, &metadata1);
    assert_eq!(client.get_metadata(&offering_id), Some(metadata1));

    // Update multiple times
    let metadata2 = String::from_str(&env, "QmVersion2");
    client.update_metadata(&owner, &offering_id, &metadata2);
    assert_eq!(client.get_metadata(&offering_id), Some(metadata2));

    let metadata3 = String::from_str(&env, "QmVersion3");
    client.update_metadata(&owner, &offering_id, &metadata3);
    assert_eq!(client.get_metadata(&offering_id), Some(metadata3));

    let metadata4 = String::from_str(&env, "QmVersion4");
    client.update_metadata(&owner, &offering_id, &metadata4);
    assert_eq!(client.get_metadata(&offering_id), Some(metadata4));
}

#[test]
#[should_panic(expected = "metadata already exists for this offering")]
fn cannot_set_metadata_twice() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    env.mock_all_auths();

    let offering_id = String::from_str(&env, "offering-011");
    let metadata1 = String::from_str(&env, "QmFirst");
    let metadata2 = String::from_str(&env, "QmSecond");

    // Set metadata
    client.set_metadata(&owner, &offering_id, &metadata1);

    // Try to set again (should panic)
    client.set_metadata(&owner, &offering_id, &metadata2);
}

#[test]
#[should_panic(expected = "no metadata exists for this offering")]
fn cannot_update_nonexistent_metadata() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    env.mock_all_auths();

    let offering_id = String::from_str(&env, "offering-012");
    let metadata = String::from_str(&env, "QmNonexistent");

    // Try to update without setting first (should panic)
    client.update_metadata(&owner, &offering_id, &metadata);
}

#[test]
fn get_nonexistent_metadata_returns_none() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    let offering_id = String::from_str(&env, "offering-nonexistent");

    // Should return None for nonexistent metadata
    let retrieved = client.get_metadata(&offering_id);
    assert_eq!(retrieved, None);
}

#[test]
fn metadata_at_max_length_is_accepted() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    env.mock_all_auths();

    let offering_id = String::from_str(&env, "offering-013");

    // Create a string exactly at MAX_METADATA_LENGTH (256 chars)
    let max_length = "x".repeat(256);
    let max_metadata = String::from_str(&env, &max_length);

    // Should succeed
    client.set_metadata(&owner, &offering_id, &max_metadata);

    let retrieved = client.get_metadata(&offering_id);
    assert_eq!(retrieved, Some(max_metadata));
}

#[test]
fn multiple_offerings_can_have_metadata() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    env.mock_all_auths();

    // Set metadata for multiple offerings
    let offering1 = String::from_str(&env, "offering-A");
    let metadata1 = String::from_str(&env, "QmMetadataA");
    client.set_metadata(&owner, &offering1, &metadata1);

    let offering2 = String::from_str(&env, "offering-B");
    let metadata2 = String::from_str(&env, "QmMetadataB");
    client.set_metadata(&owner, &offering2, &metadata2);

    let offering3 = String::from_str(&env, "offering-C");
    let metadata3 = String::from_str(&env, "QmMetadataC");
    client.set_metadata(&owner, &offering3, &metadata3);

    // Verify all metadata is stored independently
    assert_eq!(client.get_metadata(&offering1), Some(metadata1));
    assert_eq!(client.get_metadata(&offering2), Some(metadata2));
    assert_eq!(client.get_metadata(&offering3), Some(metadata3));
}

#[test]
#[should_panic]
fn test_transfer_ownership_not_owner() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let revenue_pool = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, _) = create_usdc(&env, &owner);

    env.mock_all_auths();
    client.init(
        &owner,
        &usdc_token,
        &None,
        &None,
        &Some(revenue_pool.clone()),
        &None,
    );

    let retrieved_pool = client.get_revenue_pool();
    assert_eq!(retrieved_pool, Some(revenue_pool));
}

#[test]
fn get_revenue_pool_returns_none_when_not_set() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, _) = create_usdc(&env, &owner);

    env.mock_all_auths();
    client.init(&owner, &usdc_token, &None, &None, &None, &None);

    let retrieved_pool = client.get_revenue_pool();
    assert_eq!(retrieved_pool, None);
    client.init(&owner, &Some(100));

    // No auth for owner — transfer_ownership requires current owner to authorize
    env.mock_auths(&[]);
    client.transfer_ownership(&new_owner);
}

#[test]
fn get_max_deduct_returns_configured_value() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, _) = create_usdc(&env, &owner);

    env.mock_all_auths();
    client.init(&owner, &usdc_token, &None, &None, &None, &Some(5000));

    let max_deduct = client.get_max_deduct();
    assert_eq!(max_deduct, 5000);
}

/// Fuzz test: random deposit/deduct sequence asserting balance >= 0 and matches expected.
/// Run with: cargo test --package callora-vault fuzz_deposit_and_deduct -- --nocapture
#[test]
fn fuzz_deposit_and_deduct() {
    use rand::Rng;

    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let (vault_address, vault) = create_vault(&env);
    let (usdc_address, usdc_client, usdc_admin) = create_usdc(&env, &owner);

    let initial_balance: i128 = 1_000;
    fund_vault(&usdc_admin, &vault_address, initial_balance);
    // Pre-fund owner for deposits in the loop
    usdc_admin.mint(&owner, &250_000);
    usdc_client.approve(&owner, &vault_address, &250_000, &10_000);
    vault.init(
        &owner,
        &usdc_address,
        &Some(initial_balance),
        &None,
        &None,
        &None,
    );
    let mut expected = initial_balance;
    let mut rng = rand::thread_rng();

    for _ in 0..500 {
        if rng.gen_bool(0.5) {
            let amount = rng.gen_range(1..=500);
            vault.deposit(&owner, &amount);
            expected += amount;
        } else if expected > 0 {
            let amount = rng.gen_range(1..=expected.min(500));
            vault.deduct(&owner, &amount, &None);
            expected -= amount;
        }

        let balance = vault.balance();
        assert!(balance >= 0, "balance went negative: {}", balance);
        assert_eq!(
            balance, expected,
            "balance mismatch: got {}, expected {}",
            balance, expected
        );
    }

    assert_eq!(vault.balance(), expected);
}

#[test]
fn deduct_returns_new_balance() {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let (vault_address, vault) = create_vault(&env);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);

    fund_vault(&usdc_admin, &vault_address, 100);
    vault.init(&owner, &usdc_address, &Some(100), &None, &None, &None);
    let new_balance = vault.deduct(&owner, &30, &None);
    assert_eq!(new_balance, 70);
    assert_eq!(vault.balance(), 70);
}

/// Fuzz test (seeded): deterministic deposit/deduct sequence asserting balance >= 0 and matches expected.
#[test]
fn fuzz_deposit_and_deduct_seeded() {
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let (vault_address, vault) = create_vault(&env);
    let (usdc_address, usdc_client, usdc_admin) = create_usdc(&env, &owner);

    // Pre-fund owner for deposits in the loop
    usdc_admin.mint(&owner, &5_000_000);
    usdc_client.approve(&owner, &vault_address, &5_000_000, &10_000);
    vault.init(&owner, &usdc_address, &Some(0), &None, &None, &None);
    let mut expected: i128 = 0;
    let mut rng = StdRng::seed_from_u64(42);

    for _ in 0..500 {
        let action: u8 = rng.gen_range(0..2);

        if action == 0 {
            let amount: i128 = rng.gen_range(1..=10_000);
            vault.deposit(&owner, &amount);
            expected += amount;
        } else if expected > 0 {
            let amount: i128 = rng.gen_range(1..=expected);
            vault.deduct(&owner, &amount, &None);
            expected -= amount;
        }

        assert!(expected >= 0, "balance went negative");
        assert_eq!(vault.balance(), expected, "balance mismatch at iteration");
    }
}

#[test]
fn batch_deduct_all_succeed() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 60);
    client.init(&owner, &usdc_address, &Some(60), &None, &None, &None);
    let items = soroban_sdk::vec![
        &env,
        DeductItem {
            amount: 10,
            request_id: None,
        },
        DeductItem {
            amount: 20,
            request_id: None,
        },
        DeductItem {
            amount: 30,
            request_id: None,
        },
    ];
    let caller = Address::generate(&env);
    env.mock_all_auths();
    let new_balance = client.batch_deduct(&caller, &items);
    assert_eq!(new_balance, 0);
    assert_eq!(client.balance(), 0);
}

#[test]
#[should_panic(expected = "insufficient balance")]
fn batch_deduct_all_revert() {
    let env = Env::default();

    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 25);
    client.init(&owner, &usdc_address, &Some(25), &None, &None, &None);
    assert_eq!(client.balance(), 25);
    let items = soroban_sdk::vec![
        &env,
        DeductItem {
            amount: 10,
            request_id: None,
        },
        DeductItem {
            amount: 20,
            request_id: None,
        },
        DeductItem {
            amount: 30,
            request_id: None,
        },
    ];
    let caller = Address::generate(&env);
    env.mock_all_auths();
    client.batch_deduct(&caller, &items);
}

#[test]
fn batch_deduct_revert_preserves_balance() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 25);
    client.init(&owner, &usdc_address, &Some(25), &None, &None, &None);
    assert_eq!(client.balance(), 25);
    let items = soroban_sdk::vec![
        &env,
        DeductItem {
            amount: 10,
            request_id: None,
        },
        DeductItem {
            amount: 20,
            request_id: None,
        },
        DeductItem {
            amount: 30,
            request_id: None,
        },
    ];
    let caller = Address::generate(&env);
    env.mock_all_auths();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.batch_deduct(&caller, &items);
    }));

    assert!(result.is_err());
    assert_eq!(client.balance(), 25);
}

#[test]
#[should_panic(expected = "vault is paused")]
fn test_deduct_when_paused_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(500));
    env.mock_all_auths();
    client.pause(&owner);
    client.deduct(&owner, &100);
}

#[test]
fn owner_unchanged_after_deposit_and_deduct() {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_address, usdc_client, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 100);
    // Fund owner for the deposit call
    usdc_admin.mint(&owner, &50);
    usdc_client.approve(&owner, &contract_id, &50, &10_000);
    client.init(&owner, &usdc_address, &Some(100), &None, &None, &None);
    client.deposit(&owner, &50);
    client.deduct(&owner, &30, &None);

    assert_eq!(client.get_meta().owner, owner);
}

#[test]
fn batch_deduct_exceeds_max_deduct_fails() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let caller = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 1000);
    client.init(&owner, &usdc_token, &Some(1000), &None, &None, &Some(50));

    let items = soroban_sdk::vec![
        &env,
        DeductItem {
            amount: 100,
            request_id: None,
        },
    ];

    let result = client.try_batch_deduct(&caller, &items);
    assert!(
        result.is_err(),
        "expected error for amount exceeding max_deduct"
    );
}

// ---------------------------------------------------------------------------
// large balance and large deduct
// ---------------------------------------------------------------------------

#[test]
fn large_balance_init_and_deduct() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let caller = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();

    let large_balance: i128 = i128::MAX / 2;
    fund_vault(&usdc_admin, &contract_id, large_balance);
    client.init(
        &owner,
        &usdc_token,
        &Some(large_balance),
        &None,
        &None,
        &None,
    );
    assert_eq!(client.balance(), large_balance);

    let deduct_amount: i128 = i128::MAX / 4;
    let remaining = client.deduct(&caller, &deduct_amount, &None);
    let expected = large_balance - deduct_amount;
    assert_eq!(remaining, expected);
    assert_eq!(client.balance(), expected);
}

#[test]
fn large_balance_deduct_entire_balance() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let caller = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();

    let large_balance: i128 = i128::MAX;
    fund_vault(&usdc_admin, &contract_id, large_balance);
    client.init(
        &owner,
        &usdc_token,
        &Some(large_balance),
        &None,
        &None,
        &None,
    );
    assert_eq!(client.balance(), large_balance);

    let remaining = client.deduct(&caller, &large_balance, &None);
    assert_eq!(remaining, 0);
    assert_eq!(client.balance(), 0);
}

#[test]
fn large_balance_sequential_deducts() {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let caller = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();

    let large_balance: i128 = 1_000_000_000_000_000_000;
    fund_vault(&usdc_admin, &contract_id, large_balance);
    client.init(
        &owner,
        &usdc_token,
        &Some(large_balance),
        &None,
        &None,
        &None,
    );

    let first = client.deduct(&caller, &400_000_000_000_000_000, &None);
    assert_eq!(first, 600_000_000_000_000_000);

    let second = client.deduct(&caller, &600_000_000_000_000_000, &None);
    assert_eq!(second, 0);
    assert_eq!(client.balance(), 0);
}

#[test]
fn large_batch_deduct_correctness() {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let caller = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();

    let large_balance: i128 = i128::MAX / 2;
    fund_vault(&usdc_admin, &contract_id, large_balance);
    client.init(
        &owner,
        &usdc_token,
        &Some(large_balance),
        &None,
        &None,
        &None,
    );

    let chunk = large_balance / 3;
    let items = soroban_sdk::vec![
        &env,
        DeductItem {
            amount: chunk,
            request_id: None
        },
        DeductItem {
            amount: chunk,
            request_id: None
        },
        DeductItem {
            amount: chunk,
            request_id: None
        },
    ];

    let remaining = client.batch_deduct(&caller, &items);
    let expected = large_balance - (chunk * 3);
    assert_eq!(remaining, expected);
    assert_eq!(client.balance(), expected);
}

#[test]
fn deposit_overflow_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let depositor = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();

    let near_max: i128 = i128::MAX - 10;
    fund_vault(&usdc_admin, &contract_id, near_max);
    client.init(&owner, &usdc_token, &Some(near_max), &None, &None, &None);

    fund_vault(&usdc_admin, &depositor, 100);
    let usdc_client = token::Client::new(&env, &usdc_token);
    usdc_client.approve(&depositor, &contract_id, &100, &1000);
    let result = client.try_deposit(&depositor, &100);
    assert!(
        result.is_err(),
        "expected overflow on deposit exceeding i128::MAX"
    );
}

#[test]
fn large_deduct_exceeding_balance_fails() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let caller = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_token, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();

    let large_balance: i128 = i128::MAX / 2;
    fund_vault(&usdc_admin, &contract_id, large_balance);
    client.init(
        &owner,
        &usdc_token,
        &Some(large_balance),
        &None,
        &None,
        &None,
    );

    let result = client.try_deduct(&caller, &(large_balance + 1), &None);
    assert!(
        result.is_err(),
        "expected error when deducting more than large balance"
    );
    assert_eq!(client.balance(), large_balance);
#[should_panic]
fn init_unauthorized_owner_panics() {
    let env = Env::default();
    let owner = Address::generate(&env); // Represents an arbitrary/zero/unset address that didn't sign
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    // Call init without mocking authorization for `owner`.
    // It should panic at `owner.require_auth()`, preventing unauthorized or zero-address initialization.
    client.init(&owner, &Some(100));
}
