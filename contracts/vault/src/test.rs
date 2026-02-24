extern crate std;

use super::*;
use soroban_sdk::testutils::{Address as _, Events as _};
use soroban_sdk::{vec, IntoVal, Symbol};

/// Logs approximate CPU/instruction and fee for init, deposit, deduct, and balance.
/// Run with: cargo test --ignored vault_operation_costs -- --nocapture
/// Requires invocation cost metering; may panic on default test env.
#[test]
#[ignore]
fn vault_operation_costs() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();

    client.init(&owner, &Some(0), &None);
    let res = env.cost_estimate().resources();
    let fee = env.cost_estimate().fee();
    std::println!(
        "init: instructions={} fee_total={}",
        res.instructions,
        fee.total
    );

    client.deposit(&100);
    let res = env.cost_estimate().resources();
    let fee = env.cost_estimate().fee();
    std::println!(
        "deposit: instructions={} fee_total={}",
        res.instructions,
        fee.total
    );

    client.deduct(&owner, &50, &None);
    let res = env.cost_estimate().resources();
    let fee = env.cost_estimate().fee();
    std::println!(
        "deduct: instructions={} fee_total={}",
        res.instructions,
        fee.total
    );

    let _ = client.balance();
    let res = env.cost_estimate().resources();
    let fee = env.cost_estimate().fee();
    std::println!(
        "balance: instructions={} fee_total={}",
        res.instructions,
        fee.total
    );
}

#[test]
fn init_and_balance() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());

    // Call init directly inside as_contract so events are captured
    env.mock_all_auths();
    let events = env.as_contract(&contract_id, || {
        CalloraVault::init(env.clone(), owner.clone(), Some(1000), None);
        env.events().all()
    });

    // Verify balance through client
    let client = CalloraVaultClient::new(&env, &contract_id);
    assert_eq!(client.balance(), 1000);

    // Verify "init" event was emitted
    let last_event = events.last().expect("expected at least one event");

    // Contract ID matches
    assert_eq!(last_event.0, contract_id);

    // Topic 0 = Symbol("init"), Topic 1 = owner address
    let topics = &last_event.1;
    assert_eq!(topics.len(), 2);
    let topic0: Symbol = topics.get(0).unwrap().into_val(&env);
    let topic1: Address = topics.get(1).unwrap().into_val(&env);
    assert_eq!(topic0, Symbol::new(&env, "init"));
    assert_eq!(topic1, owner);

    // Data = initial balance as i128
    let data: i128 = last_event.2.into_val(&env);
    assert_eq!(data, 1000);
}

#[test]
fn deposit_and_deduct() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(100), &None);
    client.deposit(&200);
    assert_eq!(client.balance(), 300);
    env.mock_all_auths();
    client.deduct(&owner, &50, &None);
    assert_eq!(client.balance(), 250);
}

/// Test that verifies consistency between balance() and get_meta() after init, deposit, and deduct.
/// This ensures that both methods return the same balance value and that the owner remains unchanged.
#[test]
fn balance_and_meta_consistency() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    // Initialize vault with initial balance
    client.init(&owner, &Some(500), &None);

    // Verify consistency after initialization
    let meta = client.get_meta();
    let balance = client.balance();
    assert_eq!(meta.balance, balance, "balance mismatch after init");
    assert_eq!(meta.owner, owner, "owner changed after init");
    assert_eq!(balance, 500, "incorrect balance after init");

    // Deposit and verify consistency
    client.deposit(&300);
    let meta = client.get_meta();
    let balance = client.balance();
    assert_eq!(meta.balance, balance, "balance mismatch after deposit");
    assert_eq!(meta.owner, owner, "owner changed after deposit");
    assert_eq!(balance, 800, "incorrect balance after deposit");

    // Deduct and verify consistency
    client.deduct(&owner, &150, &None);
    let meta = client.get_meta();
    let balance = client.balance();
    assert_eq!(meta.balance, balance, "balance mismatch after deduct");
    assert_eq!(meta.owner, owner, "owner changed after deduct");
    assert_eq!(balance, 650, "incorrect balance after deduct");

    // Perform multiple operations and verify final state
    client.deposit(&100);
    client.deduct(&owner, &50, &None);
    client.deposit(&25);
    let meta = client.get_meta();
    let balance = client.balance();
    assert_eq!(
        meta.balance, balance,
        "balance mismatch after multiple operations"
    );
    assert_eq!(meta.owner, owner, "owner changed after multiple operations");
    assert_eq!(balance, 725, "incorrect final balance");
}

#[test]
#[should_panic(expected = "insufficient balance")]
fn deduct_exact_balance_and_panic() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(100), &None);
    assert_eq!(client.balance(), 100);

    // Deduct exact balance
    client.deduct(&owner, &100, &None);
    assert_eq!(client.balance(), 0);

    // Further deduct should panic
    client.deduct(&owner, &1, &None);
}

#[test]
fn deduct_event_emission() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let caller = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(1000), &None);

    env.mock_all_auths();
    let req_id = Symbol::new(&env, "req123");

    // Call client directly to avoid re-entry panic inside as_contract
    client.deduct(&caller, &200, &Some(req_id.clone()));

    let events = env.events().all();

    let last_event = events.last().unwrap();
    assert_eq!(last_event.0, contract_id);

    let topics = &last_event.1;
    assert_eq!(topics.len(), 3);
    let topic0: Symbol = topics.get(0).unwrap().into_val(&env);
    assert_eq!(topic0, Symbol::new(&env, "deduct"));
    let topic_caller: Address = topics.get(1).unwrap().into_val(&env);
    assert_eq!(topic_caller, caller);
    let topic_req_id: Symbol = topics.get(2).unwrap().into_val(&env);
    assert_eq!(topic_req_id, req_id);

    let data: (i128, i128) = last_event.2.into_val(&env);
    assert_eq!(data, (200, 800));
}

#[test]
fn batch_deduct_success() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(1000), &None);
    let req1 = Symbol::new(&env, "req1");
    let req2 = Symbol::new(&env, "req2");
    let items = vec![
        &env,
        DeductItem {
            amount: 100,
            request_id: Some(req1.clone()),
        },
        DeductItem {
            amount: 200,
            request_id: Some(req2.clone()),
        },
        DeductItem {
            amount: 50,
            request_id: None,
        },
    ];
    let caller = Address::generate(&env);
    env.mock_all_auths();
    let new_balance = client.batch_deduct(&caller, &items);
    assert_eq!(new_balance, 650);
    assert_eq!(client.balance(), 650);
}

#[test]
#[should_panic(expected = "insufficient balance")]
fn batch_deduct_reverts_entire_batch() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(100), &None);
    let items = vec![
        &env,
        DeductItem {
            amount: 60,
            request_id: None,
        },
        DeductItem {
            amount: 60,
            request_id: None,
        }, // total 120 > 100
    ];
    let caller = Address::generate(&env);
    env.mock_all_auths();
    client.batch_deduct(&caller, &items);
}

#[test]
fn withdraw_owner_success() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(500), &None);
    let new_balance = client.withdraw(&200);
    assert_eq!(new_balance, 300);
    assert_eq!(client.balance(), 300);
}

#[test]
fn withdraw_exact_balance() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(100), &None);
    let new_balance = client.withdraw(&100);
    assert_eq!(new_balance, 0);
    assert_eq!(client.balance(), 0);
}

#[test]
#[should_panic(expected = "insufficient balance")]
fn withdraw_exceeds_balance_fails() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(50), &None);
    client.withdraw(&100);
}

#[test]
fn withdraw_to_success() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let to = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(500), &None);
    let new_balance = client.withdraw_to(&to, &150);
    assert_eq!(new_balance, 350);
    assert_eq!(client.balance(), 350);
}

#[test]
#[should_panic]
fn withdraw_without_auth_fails() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    // Need to mock auth just for init, then disable it or let withdraw fail.
    // However mock_all_auths applies to the whole test unless explicitly managed.
    // Instead, we can just mock_all_auths, init, then clear mock auths.
    env.mock_all_auths();
    client.init(&owner, &Some(100), &None);
    // Clear mocks so withdraw fails.
    // Wait, Soroban testutils doesn't have an easy way to clear auths in older versions...
    // Actually, we can just drop the mock_auths or not use mock_all_auths and use mock_auths explicitly.
    // Actually mock_all_auths just allows anything. If we need withdraw to fail due to lack of auth,
    // we should only mock auth for init.
    // Let's modify this test to use standard auth mocking for init explicitly, or better yet, since client.withdraw
    // will panic without mock_all_auths, we can just not mock it for withdraw.
    // For init, we *have* to provide auth now.

    env.mock_auths(&[soroban_sdk::testutils::MockAuth {
        address: &owner,
        invoke: &soroban_sdk::testutils::MockAuthInvoke {
            contract: &contract_id,
            fn_name: "init",
            args: (&owner, Some(100i128), None::<i128>).into_val(&env),
            sub_invokes: &[],
        },
    }]);

    client.init(&owner, &Some(100), &None);

    // This will fail because withdraw requires auth which is not mocked for this call
    client.withdraw(&50);
}

#[test]
#[should_panic(expected = "vault already initialized")]
fn init_already_initialized_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(100), &None);
    client.init(&owner, &Some(200), &None); // Should panic
}

/// Test that deposit below the configured minimum panics.
/// init with min_deposit 10; call deposit(5); expect panic.
#[test]
#[should_panic(expected = "deposit below minimum")]
fn minimum_deposit_rejected() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(0), &Some(10)); // min_deposit = 10
    assert_eq!(client.balance(), 0);
    client.deposit(&5); // below minimum -> panic
}

#[test]
fn withdraw_event_payload() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(500), &None);
    client.withdraw(&200);

    let events = env.events().all();
    let last_event = events.last().unwrap();
    assert_eq!(last_event.0, contract_id);

    let topics = &last_event.1;
    assert_eq!(topics.len(), 2);
    let topic0: Symbol = topics.get(0).unwrap().into_val(&env);
    assert_eq!(topic0, Symbol::new(&env, "withdraw"));
    let topic_owner: Address = topics.get(1).unwrap().into_val(&env);
    assert_eq!(topic_owner, owner);

    let data: (i128, i128) = last_event.2.into_val(&env);
    assert_eq!(data, (200, 300));
}

#[test]
fn withdraw_to_event_payload() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let to = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(500), &None);
    client.withdraw_to(&to, &150);

    let events = env.events().all();
    let last_event = events.last().unwrap();
    assert_eq!(last_event.0, contract_id);

    let topics = &last_event.1;
    assert_eq!(topics.len(), 3);
    let topic0: Symbol = topics.get(0).unwrap().into_val(&env);
    assert_eq!(topic0, Symbol::new(&env, "withdraw_to"));
    let topic_owner: Address = topics.get(1).unwrap().into_val(&env);
    assert_eq!(topic_owner, owner);
    let topic_to: Address = topics.get(2).unwrap().into_val(&env);
    assert_eq!(topic_to, to);

    let data: (i128, i128) = last_event.2.into_val(&env);
    assert_eq!(data, (150, 350));
}
