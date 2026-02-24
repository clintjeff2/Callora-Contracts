extern crate std;

use super::*;
use soroban_sdk::testutils::{Address as _, Events as _};
use soroban_sdk::{IntoVal, Symbol};

#[test]
fn init_and_balance() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());

    // Call init directly inside as_contract so events are captured
    env.mock_all_auths();
    let events = env.as_contract(&contract_id, || {
        CalloraVault::init(env.clone(), owner.clone(), Some(1000));
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
    client.init(&owner, &Some(100));
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

    // Initialize vault with initial balance
    client.init(&owner, &Some(500));

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
    client.deduct(&150);
    let meta = client.get_meta();
    let balance = client.balance();
    assert_eq!(meta.balance, balance, "balance mismatch after deduct");
    assert_eq!(meta.owner, owner, "owner changed after deduct");
    assert_eq!(balance, 650, "incorrect balance after deduct");

    // Perform multiple operations and verify final state
    client.deposit(&100);
    client.deduct(&50);
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

    client.init(&owner, &Some(100));
    assert_eq!(client.balance(), 100);

    // Deduct exact balance
    client.deduct(&100);
    assert_eq!(client.balance(), 0);

    // Further deduct should panic
    client.deduct(&1);
}

#[test]
fn deduct_event_emission() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let caller = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(1000));

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
#[should_panic(expected = "deposit amount must be positive")]
fn deposit_zero_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));
    client.deposit(&0);
}

#[test]
#[should_panic(expected = "deposit amount must be positive")]
fn deposit_negative_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(100));
    client.deposit(&-100);
}

#[test]
#[should_panic(expected = "deduct amount must be positive")]
fn deduct_zero_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));
    client.deduct(&0);
}

#[test]
#[should_panic(expected = "deduct amount must be positive")]
fn deduct_negative_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));
    client.deduct(&-50);
}

#[test]
#[should_panic(expected = "insufficient balance")]
fn deduct_exceeds_balance_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    // Need to mock auth just for init, then disable it or let withdraw fail.
    // However mock_all_auths applies to the whole test unless explicitly managed.
    // Instead, we can just mock_all_auths, init, then clear mock auths.
    env.mock_all_auths();
    client.init(&owner, &Some(100));
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
            args: (&owner, Some(100i128)).into_val(&env),
            sub_invokes: &[],
        },
    }]);

    client.init(&owner, &Some(100));
    client.deduct(&200);
}

#[test]
#[should_panic(expected = "vault already initialized")]
fn init_already_initialized_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(100));
    client.init(&owner, &Some(200)); // Should panic
}
