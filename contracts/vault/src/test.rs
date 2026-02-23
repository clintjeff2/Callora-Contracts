extern crate std;

use super::*;
use soroban_sdk::testutils::{Address as _, Events as _};
use soroban_sdk::{token, vec, IntoVal, Symbol};

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
    _env: &Env,
    usdc_admin_client: &token::StellarAssetClient,
    vault_address: &Address,
    amount: i128,
) {
    usdc_admin_client.mint(vault_address, &amount);
}

/// Logs approximate CPU/instruction and fee for init, deposit, deduct, and balance.
/// Run with: cargo test --ignored vault_operation_costs -- --nocapture
/// Requires invocation cost metering; may panic on default test env.
#[test]
#[ignore]
fn vault_operation_costs() {
    let env = Env::default();
    let owner = Address::generate(&env);
    // Register contract instance with a unique salt (owner) to avoid address reuse
    let contract_id = env.register(CalloraVault {}, (owner.clone(),));
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc, _, _) = create_usdc(&env, &owner);

    env.mock_all_auths();

    client.init(&owner, &usdc, &Some(0), &None);
    let res = env.cost_estimate().resources();
    let fee = env.cost_estimate().fee();
    std::println!(
        "init: instructions={} fee_total={}",
        res.instructions,
        fee.total
    );

    client.deposit(&owner, &100);
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

    // Initialize via client so events are captured and auth can be mocked
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc, _, _) = create_usdc(&env, &owner);
    env.mock_all_auths();
    client.init(&owner, &usdc, &Some(1000), &None);
    let _events = env.events().all();

    // Verify balance through client
    assert_eq!(client.balance(), 1000);

    // Note: event emission for `init` is validated in other tests.
}

#[test]
fn deposit_and_deduct() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    let (usdc, _, _) = create_usdc(&env, &owner);
    env.mock_all_auths();
    client.init(&owner, &usdc, &Some(100), &None);
    client.deposit(&owner, &200);
    assert_eq!(client.balance(), 300);
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
    let (usdc_address, _, _) = create_usdc(&env, &owner);
    client.init(&owner, &usdc_address, &Some(500), &None);

    // Verify consistency after initialization
    let meta = client.get_meta();
    let balance = client.balance();
    assert_eq!(meta.balance, balance, "balance mismatch after init");
    assert_eq!(meta.owner, owner, "owner changed after init");
    assert_eq!(balance, 500, "incorrect balance after init");

    // Deposit and verify consistency
    client.deposit(&owner, &300);
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
    client.deposit(&owner, &100);
    client.deduct(&owner, &50, &None);
    client.deposit(&owner, &25);
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

    let (usdc_address, _, _) = create_usdc(&env, &owner);
    env.mock_all_auths();
    client.init(&owner, &usdc_address, &Some(100), &None);
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

    let (usdc_address, _, _) = create_usdc(&env, &owner);
    env.mock_all_auths();
    client.init(&owner, &usdc_address, &Some(1000), &None);
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
fn test_init_success() {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let (_, vault) = create_vault(&env);
    let (usdc_address, _, _) = create_usdc(&env, &owner);

    let meta = vault.init(&owner, &usdc_address, &None, &None);

    assert_eq!(meta.owner, owner);
    assert_eq!(meta.balance, 0);
}

#[test]
#[should_panic(expected = "vault already initialized")]
fn test_init_double_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let (_, vault) = create_vault(&env);
    let (usdc_address, _, _) = create_usdc(&env, &owner);

    vault.init(&owner, &usdc_address, &None, &None);
    vault.init(&owner, &usdc_address, &None, &None);
}

#[test]
fn test_distribute_success() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let developer = Address::generate(&env);
    let (vault_address, vault) = create_vault(&env);
    let (usdc_address, usdc_client, usdc_admin_client) = create_usdc(&env, &admin);

    vault.init(&admin, &usdc_address, &None, &None);
    fund_vault(&env, &usdc_admin_client, &vault_address, 1_000);
    vault.distribute(&admin, &developer, &400);

    assert_eq!(usdc_client.balance(&vault_address), 600);
    assert_eq!(usdc_client.balance(&developer), 400);
}

#[test]
#[should_panic(expected = "insufficient USDC balance")]
fn test_distribute_excess_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let developer = Address::generate(&env);
    let (vault_address, vault) = create_vault(&env);
    let (usdc_address, _, usdc_admin_client) = create_usdc(&env, &admin);

    vault.init(&admin, &usdc_address, &None, &None);
    fund_vault(&env, &usdc_admin_client, &vault_address, 100);
    vault.distribute(&admin, &developer, &101);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn test_distribute_zero_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let developer = Address::generate(&env);
    let (_, vault) = create_vault(&env);
    let (usdc_address, _, _) = create_usdc(&env, &admin);

    vault.init(&admin, &usdc_address, &None, &None);
    vault.distribute(&admin, &developer, &0);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn test_distribute_negative_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let developer = Address::generate(&env);
    let (_, vault) = create_vault(&env);
    let (usdc_address, _, _) = create_usdc(&env, &admin);

    vault.init(&admin, &usdc_address, &None, &None);
    vault.distribute(&admin, &developer, &-1);
}

#[test]
#[should_panic(expected = "unauthorized: caller is not admin")]
fn test_distribute_unauthorized_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let attacker = Address::generate(&env);
    let developer = Address::generate(&env);
    let (vault_address, vault) = create_vault(&env);
    let (usdc_address, _, usdc_admin_client) = create_usdc(&env, &admin);

    vault.init(&admin, &usdc_address, &None, &None);
    fund_vault(&env, &usdc_admin_client, &vault_address, 1_000);
    vault.distribute(&attacker, &developer, &500);
}

#[test]
fn test_distribute_full_balance() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let developer = Address::generate(&env);
    let (vault_address, vault) = create_vault(&env);
    let (usdc_address, usdc_client, usdc_admin_client) = create_usdc(&env, &admin);

    vault.init(&admin, &usdc_address, &None, &None);
    fund_vault(&env, &usdc_admin_client, &vault_address, 777);
    vault.distribute(&admin, &developer, &777);

    assert_eq!(usdc_client.balance(&vault_address), 0);
    assert_eq!(usdc_client.balance(&developer), 777);
}

#[test]
fn test_distribute_multiple_times() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let dev_a = Address::generate(&env);
    let dev_b = Address::generate(&env);
    let (vault_address, vault) = create_vault(&env);
    let (usdc_address, usdc_client, usdc_admin_client) = create_usdc(&env, &admin);

    vault.init(&admin, &usdc_address, &None, &None);
    fund_vault(&env, &usdc_admin_client, &vault_address, 1_000);
    vault.distribute(&admin, &dev_a, &300);
    vault.distribute(&admin, &dev_b, &200);

    assert_eq!(usdc_client.balance(&vault_address), 500);
    assert_eq!(usdc_client.balance(&dev_a), 300);
    assert_eq!(usdc_client.balance(&dev_b), 200);
}

#[test]
fn test_set_admin_transfers_control() {
    let env = Env::default();
    env.mock_all_auths();

    let original_admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let developer = Address::generate(&env);
    let (vault_address, vault) = create_vault(&env);
    let (usdc_address, usdc_client, usdc_admin_client) = create_usdc(&env, &original_admin);

    vault.init(&original_admin, &usdc_address, &None, &None);
    fund_vault(&env, &usdc_admin_client, &vault_address, 500);
    vault.set_admin(&original_admin, &new_admin);

    assert_eq!(vault.get_admin(), new_admin);

    vault.distribute(&new_admin, &developer, &100);
    assert_eq!(usdc_client.balance(&developer), 100);
}

#[test]
#[should_panic(expected = "unauthorized: caller is not admin")]
fn test_old_admin_cannot_distribute_after_transfer() {
    let env = Env::default();
    env.mock_all_auths();

    let original_admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let developer = Address::generate(&env);
    let (vault_address, vault) = create_vault(&env);
    let (usdc_address, _, usdc_admin_client) = create_usdc(&env, &original_admin);

    vault.init(&original_admin, &usdc_address, &None, &None);
    fund_vault(&env, &usdc_admin_client, &vault_address, 500);
    vault.set_admin(&original_admin, &new_admin);
    vault.distribute(&original_admin, &developer, &100);
}

#[test]
fn test_deposit_and_balance() {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let (_, vault) = create_vault(&env);
    let (usdc_address, _, _) = create_usdc(&env, &owner);

    vault.init(&owner, &usdc_address, &Some(0), &None);
    vault.deposit(&owner, &200);
    assert_eq!(vault.balance(), 200);
    vault.deposit(&owner, &50);
    assert_eq!(vault.balance(), 250);
}

#[test]
fn test_deduct_success() {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let (_, vault) = create_vault(&env);
    let (usdc_address, _, _) = create_usdc(&env, &owner);

    vault.init(&owner, &usdc_address, &Some(300), &None);
    vault.deduct(&owner, &100, &None);
    assert_eq!(vault.balance(), 200);
}

#[test]
#[should_panic(expected = "insufficient balance")]
fn test_deduct_excess_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let (_, vault) = create_vault(&env);
    let (usdc_address, _, _) = create_usdc(&env, &owner);

    vault.init(&owner, &usdc_address, &Some(50), &None);
    vault.deduct(&owner, &100, &None);
}

#[test]
fn test_get_meta_returns_correct_values() {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let (_, vault) = create_vault(&env);
    let (usdc_address, _, _) = create_usdc(&env, &owner);

    vault.init(&owner, &usdc_address, &Some(999), &None);
    let meta = vault.get_meta();
    assert_eq!(meta.owner, owner);
    assert_eq!(meta.balance, 999);
}

#[test]
fn test_multiple_depositors() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_address, _, _) = create_usdc(&env, &owner);
    env.mock_all_auths();

    let dep1 = Address::generate(&env);
    let dep2 = Address::generate(&env);

    // Use as_contract to call the functions directly and capture events
    let contract_events = env.as_contract(&contract_id, || {
        CalloraVault::init(env.clone(), owner.clone(), usdc_address.clone(), None, None);
        CalloraVault::deposit(env.clone(), dep1.clone(), 100);
        CalloraVault::deposit(env.clone(), dep2.clone(), 200);

        env.events().all()
    });

    assert_eq!(client.balance(), 300);

    // Verify events: init + 2 deposits
    assert_eq!(contract_events.len(), 3);

    // Event 1: Init event
    let event0 = contract_events.get(0).unwrap();
    let topic0_0: Symbol = event0.1.get(0).unwrap().into_val(&env);
    assert_eq!(topic0_0, Symbol::new(&env, "init"));

    // Event 2: deposit from dep1
    let event1 = contract_events.get(1).unwrap();
    let topic1_0: Symbol = event1.1.get(0).unwrap().into_val(&env);
    let topic1_1: Address = event1.1.get(1).unwrap().into_val(&env);
    let data1: i128 = event1.2.into_val(&env);
    assert_eq!(topic1_0, Symbol::new(&env, "deposit"));
    assert_eq!(topic1_1, dep1);
    assert_eq!(data1, 100);

    // Event 3: deposit from dep2
    let event2 = contract_events.get(2).unwrap();
    let topic2_0: Symbol = event2.1.get(0).unwrap().into_val(&env);
    let topic2_1: Address = event2.1.get(1).unwrap().into_val(&env);
    let data2: i128 = event2.2.into_val(&env);
    assert_eq!(topic2_0, Symbol::new(&env, "deposit"));
    assert_eq!(topic2_1, dep2);
    assert_eq!(data2, 200);
}

#[test]
fn batch_deduct_success() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_address, _, _) = create_usdc(&env, &owner);

    env.mock_all_auths();
    client.init(&owner, &usdc_address, &Some(1000), &None);
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
    let (usdc_address, _, _) = create_usdc(&env, &owner);

    env.mock_all_auths();
    client.init(&owner, &usdc_address, &Some(100), &None);
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
    let (usdc_address, _, _) = create_usdc(&env, &owner);

    env.mock_all_auths();
    client.init(&owner, &usdc_address, &Some(500), &None);
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
    let (usdc_address, _, _) = create_usdc(&env, &owner);

    env.mock_all_auths();
    client.init(&owner, &usdc_address, &Some(100), &None);
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
    let (usdc_address, _, _) = create_usdc(&env, &owner);

    env.mock_all_auths();
    client.init(&owner, &usdc_address, &Some(50), &None);
    client.withdraw(&100);
}

#[test]
fn withdraw_to_success() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let to = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_address, _, _) = create_usdc(&env, &owner);

    env.mock_all_auths();
    client.init(&owner, &usdc_address, &Some(500), &None);
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
    let (usdc_address, _, _) = create_usdc(&env, &owner);

    // Mock auth ONLY for init
    env.mock_auths(&[soroban_sdk::testutils::MockAuth {
        address: &owner,
        invoke: &soroban_sdk::testutils::MockAuthInvoke {
            contract: &contract_id,
            fn_name: "init",
            args: (&owner, &usdc_address, Some(100i128), Option::<i128>::None).into_val(&env),
            sub_invokes: &[],
        },
    }]);

    client.init(&owner, &usdc_address, &Some(100), &None);

    // This should now panic because owner.require_auth() is called in withdraw but not mocked here
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
    let (usdc_address, _, _) = create_usdc(&env, &owner);
    client.init(&owner, &usdc_address, &Some(100), &None);
    client.init(&owner, &usdc_address, &Some(200), &None); // Should panic
}
