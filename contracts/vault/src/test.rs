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
    usdc_admin_client: &token::StellarAssetClient,
    vault_address: &Address,
    amount: i128,
) {
    usdc_admin_client.mint(vault_address, &amount);
}

fn fund_user(usdc_admin_client: &token::StellarAssetClient, user: &Address, amount: i128) {
    usdc_admin_client.mint(user, &amount);
}

/// Approve spender to transfer amount from from (for deposit tests; from must have auth).
fn approve_spend(
    _env: &Env,
    usdc_client: &token::Client,
    from: &Address,
    spender: &Address,
    amount: i128,
) {
    // expiration_ledger 0 = no expiration in Stellar Asset Contract
    usdc_client.approve(from, spender, &amount, &0u32);
}

/// Lifecycle test: init → deposit → deduct → balance.
#[test]
fn vault_operation_costs() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_user(&usdc_admin, &owner, 200);

    client.init(&owner, &usdc, &Some(0), &None, &None, &None);
    client.deposit(&owner, &100);
    client.deduct(&owner, &50, &None);
    let bal = client.balance();
    assert_eq!(bal, 50);
}

#[test]
fn init_and_balance() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());

    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc, _, usdc_admin) = create_usdc(&env, &owner);
    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 1000);
    client.init(&owner, &usdc, &Some(1000), &None, &None, &None);
    let _events = env.events().all();

    assert_eq!(client.balance(), 1000);
}

#[test]
fn deposit_and_deduct() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    let (usdc, usdc_client, usdc_admin) = create_usdc(&env, &owner);
    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 100);
    client.init(&owner, &usdc, &Some(100), &None, &None, &None);
    fund_user(&usdc_admin, &owner, 200);
    approve_spend(&env, &usdc_client, &owner, &contract_id, 200);
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
    let (usdc_address, usdc_client, usdc_admin) = create_usdc(&env, &owner);
    fund_vault(&usdc_admin, &contract_id, 500);
    client.init(&owner, &usdc_address, &Some(500), &None, &None, &None);

    let meta = client.get_meta();
    let balance = client.balance();
    assert_eq!(meta.balance, balance, "balance mismatch after init");
    assert_eq!(meta.owner, owner, "owner changed after init");
    assert_eq!(balance, 500, "incorrect balance after init");

    fund_user(&usdc_admin, &owner, 425);
    approve_spend(&env, &usdc_client, &owner, &contract_id, 425);
    client.deposit(&owner, &300);
    let meta = client.get_meta();
    let balance = client.balance();
    assert_eq!(meta.balance, balance, "balance mismatch after deposit");
    assert_eq!(balance, 800, "incorrect balance after deposit");

    client.deduct(&owner, &150, &None);
    let meta = client.get_meta();
    let balance = client.balance();
    assert_eq!(meta.balance, balance, "balance mismatch after deduct");
    assert_eq!(balance, 650, "incorrect balance after deduct");

    fund_user(&usdc_admin, &owner, 125);
    approve_spend(&env, &usdc_client, &owner, &contract_id, 125);
    client.deposit(&owner, &100);
    client.deduct(&owner, &50, &None);
    client.deposit(&owner, &25);
    let meta = client.get_meta();
    let balance = client.balance();
    assert_eq!(
        meta.balance, balance,
        "balance mismatch after multiple operations"
    );
    assert_eq!(balance, 725, "incorrect final balance");
}

#[test]
#[should_panic(expected = "insufficient balance")]
fn deduct_exact_balance_and_panic() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);
    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 100);
    client.init(&owner, &usdc_address, &Some(100), &None, &None, &None);
    assert_eq!(client.balance(), 100);

    client.deduct(&owner, &100, &None);
    assert_eq!(client.balance(), 0);

    client.deduct(&owner, &1, &None);
}

#[test]
fn deduct_event_emission() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let caller = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);
    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 1000);
    client.init(&owner, &usdc_address, &Some(1000), &None, &None, &None);
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

    let meta = vault.init(&owner, &usdc_address, &None, &None, &None, &None);

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

    vault.init(&owner, &usdc_address, &None, &None, &None, &None);
    vault.init(&owner, &usdc_address, &None, &None, &None, &None);
}

#[test]
fn test_distribute_success() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let developer = Address::generate(&env);
    let (vault_address, vault) = create_vault(&env);
    let (usdc_address, usdc_client, usdc_admin_client) = create_usdc(&env, &admin);

    fund_vault(&usdc_admin_client, &vault_address, 1_000);
    vault.init(&admin, &usdc_address, &None, &None, &None, &None);
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

    fund_vault(&usdc_admin_client, &vault_address, 100);
    vault.init(&admin, &usdc_address, &None, &None, &None, &None);
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

    vault.init(&admin, &usdc_address, &None, &None, &None, &None);
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

    vault.init(&admin, &usdc_address, &None, &None, &None, &None);
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

    fund_vault(&usdc_admin_client, &vault_address, 1_000);
    vault.init(&admin, &usdc_address, &None, &None, &None, &None);
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

    fund_vault(&usdc_admin_client, &vault_address, 777);
    vault.init(&admin, &usdc_address, &None, &None, &None, &None);
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

    fund_vault(&usdc_admin_client, &vault_address, 1_000);
    vault.init(&admin, &usdc_address, &None, &None, &None, &None);
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

    fund_vault(&usdc_admin_client, &vault_address, 500);
    vault.init(&original_admin, &usdc_address, &None, &None, &None, &None);
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

    fund_vault(&usdc_admin_client, &vault_address, 500);
    vault.init(&original_admin, &usdc_address, &None, &None, &None, &None);
    vault.set_admin(&original_admin, &new_admin);
    vault.distribute(&original_admin, &developer, &100);
}

#[test]
fn test_deposit_and_balance() {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let (vault_address, vault) = create_vault(&env);
    let (usdc_address, usdc_client, usdc_admin) = create_usdc(&env, &owner);

    vault.init(&owner, &usdc_address, &Some(0), &None, &None, &None);
    fund_user(&usdc_admin, &owner, 250);
    approve_spend(&env, &usdc_client, &owner, &vault_address, 250);
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
    let (vault_address, vault) = create_vault(&env);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);

    fund_vault(&usdc_admin, &vault_address, 300);
    vault.init(&owner, &usdc_address, &Some(300), &None, &None, &None);
    vault.deduct(&owner, &100, &None);
    assert_eq!(vault.balance(), 200);
}

#[test]
#[should_panic(expected = "deduct amount exceeds max_deduct")]
fn test_deduct_above_max_deduct_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let (vault_address, vault) = create_vault(&env);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);

    fund_vault(&usdc_admin, &vault_address, 10_000);
    vault.init(
        &owner,
        &usdc_address,
        &Some(10_000),
        &None,
        &None,
        &Some(100),
    );
    assert_eq!(vault.get_max_deduct(), 100);
    vault.deduct(&owner, &100, &None);
    assert_eq!(vault.balance(), 9_900);
    vault.deduct(&owner, &101, &None);
}

#[test]
#[should_panic(expected = "insufficient balance")]
fn test_deduct_excess_panics() {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let (vault_address, vault) = create_vault(&env);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);

    fund_vault(&usdc_admin, &vault_address, 50);
    vault.init(&owner, &usdc_address, &Some(50), &None, &None, &None);
    vault.deduct(&owner, &100, &None);
}

#[test]
fn test_get_meta_returns_correct_values() {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let (vault_address, vault) = create_vault(&env);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);

    fund_vault(&usdc_admin, &vault_address, 999);
    vault.init(&owner, &usdc_address, &Some(999), &None, &None, &None);
    let meta = vault.get_meta();
    assert_eq!(meta.owner, owner);
    assert_eq!(meta.balance, 999);
}

#[test]
fn test_multiple_depositors() {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_address, usdc_client, usdc_admin) = create_usdc(&env, &owner);
    env.mock_all_auths();

    let dep1 = Address::generate(&env);
    let dep2 = Address::generate(&env);
    fund_user(&usdc_admin, &dep1, 100);
    fund_user(&usdc_admin, &dep2, 200);
    approve_spend(&env, &usdc_client, &dep1, &contract_id, 100);
    approve_spend(&env, &usdc_client, &dep2, &contract_id, 200);

    let all_events = env.as_contract(&contract_id, || {
        CalloraVault::init(
            env.clone(),
            owner.clone(),
            usdc_address.clone(),
            None,
            None,
            None,
            None,
        );
        CalloraVault::deposit(env.clone(), dep1.clone(), 100);
        CalloraVault::deposit(env.clone(), dep2.clone(), 200);

        env.events().all()
    });
    let contract_events: std::vec::Vec<_> =
        all_events.iter().filter(|e| e.0 == contract_id).collect();

    assert_eq!(client.balance(), 300);

    assert_eq!(
        contract_events.len(),
        3,
        "vault should emit init + 2 deposits"
    );

    // Event 1: Init event
    let event0 = contract_events.first().unwrap();
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
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 1000);
    client.init(&owner, &usdc_address, &Some(1000), &None, &None, &None);
    let req1 = Symbol::new(&env, "req1");
    let req2 = Symbol::new(&env, "req2");
    let items = soroban_sdk::vec![
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
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 100);
    client.init(&owner, &usdc_address, &Some(100), &None, &None, &None);
    let items = soroban_sdk::vec![
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
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 500);
    client.init(&owner, &usdc_address, &Some(500), &None, &None, &None);
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
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 100);
    client.init(&owner, &usdc_address, &Some(100), &None, &None, &None);
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
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 50);
    client.init(&owner, &usdc_address, &Some(50), &None, &None, &None);
    client.withdraw(&100);
}

#[test]
fn withdraw_to_success() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let to = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 500);
    client.init(&owner, &usdc_address, &Some(500), &None, &None, &None);
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
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);

    fund_vault(&usdc_admin, &contract_id, 100);
    env.mock_auths(&[soroban_sdk::testutils::MockAuth {
        address: &owner,
        invoke: &soroban_sdk::testutils::MockAuthInvoke {
            contract: &contract_id,
            fn_name: "init",
            args: (
                &owner,
                &usdc_address,
                Some(100i128),
                Option::<i128>::None,
                Option::<Address>::None,
                Option::<i128>::None,
            )
                .into_val(&env),
            sub_invokes: &[],
        },
    }]);

    client.init(&owner, &usdc_address, &Some(100), &None, &None, &None);

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
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);
    fund_vault(&usdc_admin, &contract_id, 100);
    client.init(&owner, &usdc_address, &Some(100), &None, &None, &None);
    client.init(&owner, &usdc_address, &Some(200), &None, &None, &None);
}

#[test]
fn deduct_returns_new_balance() {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let (vault_addr, vault) = create_vault(&env);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);

    usdc_admin.mint(&vault_addr, &100);
    vault.init(&owner, &usdc_address, &Some(100), &None, &None, &None);
    let new_balance = vault.deduct(&owner, &30, &None);
    assert_eq!(new_balance, 70);
    assert_eq!(vault.balance(), 70);
}

/// Fuzz test: random deposit/deduct sequence asserting balance >= 0 and matches expected.
#[test]
fn fuzz_deposit_and_deduct() {
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};

    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let (vault_addr, vault) = create_vault(&env);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);

    vault.init(&owner, &usdc_address, &Some(0), &None, &None, &None);
    usdc_admin.mint(&owner, &(i128::MAX / 2));
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
    usdc_admin.mint(&contract_id, &60);
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
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    usdc_admin.mint(&contract_id, &25);
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
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    usdc_admin.mint(&contract_id, &25);
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
fn owner_unchanged_after_deposit_and_deduct() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    usdc_admin.mint(&contract_id, &100);
    client.init(&owner, &usdc_address, &Some(100), &None, &None, &None);
    usdc_admin.mint(&owner, &50);
    client.deposit(&owner, &50);
    client.deduct(&owner, &30, &None);

    assert_eq!(client.get_meta().owner, owner);
}

#[test]
#[should_panic(expected = "insufficient USDC in contract for initial_balance")]
fn init_insufficient_usdc_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    let (_, vault) = create_vault(&env);
    let (usdc, _, _) = create_usdc(&env, &owner);
    // No USDC funded to vault — should panic
    vault.init(&owner, &usdc, &Some(1000), &None, &None, &None);
}

#[test]
#[should_panic(expected = "max_deduct must be positive")]
fn init_max_deduct_zero_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    let (_, vault) = create_vault(&env);
    let (usdc, _, _) = create_usdc(&env, &owner);
    vault.init(&owner, &usdc, &None, &None, &None, &Some(0));
}

#[test]
#[should_panic(expected = "max_deduct must be positive")]
fn init_max_deduct_negative_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    let (_, vault) = create_vault(&env);
    let (usdc, _, _) = create_usdc(&env, &owner);
    vault.init(&owner, &usdc, &None, &None, &None, &Some(-5));
}

#[test]
fn init_stores_revenue_pool_and_max_deduct() {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    let rev_pool = Address::generate(&env);
    let (_, vault) = create_vault(&env);
    let (usdc, _, _) = create_usdc(&env, &owner);

    vault.init(
        &owner,
        &usdc,
        &None,
        &None,
        &Some(rev_pool.clone()),
        &Some(500),
    );

    assert_eq!(vault.get_max_deduct(), 500);
    assert_eq!(vault.get_revenue_pool(), Some(rev_pool));
    assert_eq!(vault.get_admin(), owner);
    assert_eq!(vault.balance(), 0);
}

#[test]
fn get_revenue_pool_none_when_not_set() {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    let (_, vault) = create_vault(&env);
    let (usdc, _, _) = create_usdc(&env, &owner);

    vault.init(&owner, &usdc, &None, &None, &None, &None);
    assert_eq!(vault.get_revenue_pool(), None);
}

#[test]
#[should_panic(expected = "unauthorized: caller is not admin")]
fn set_admin_unauthorized_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    let attacker = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let (_, vault) = create_vault(&env);
    let (usdc, _, _) = create_usdc(&env, &owner);

    vault.init(&owner, &usdc, &None, &None, &None, &None);
    vault.set_admin(&attacker, &new_admin);
}

#[test]
fn deduct_with_revenue_pool_transfers_usdc() {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    let rev_pool = Address::generate(&env);
    let (vault_addr, vault) = create_vault(&env);
    let (usdc, usdc_client, usdc_admin) = create_usdc(&env, &owner);

    fund_vault(&usdc_admin, &vault_addr, 1000);
    vault.init(
        &owner,
        &usdc,
        &Some(1000),
        &None,
        &Some(rev_pool.clone()),
        &None,
    );

    let caller = Address::generate(&env);
    vault.deduct(&caller, &200, &Some(Symbol::new(&env, "req_pool")));

    assert_eq!(vault.balance(), 800);
    assert_eq!(usdc_client.balance(&rev_pool), 200);
    assert_eq!(usdc_client.balance(&vault_addr), 800);
}

#[test]
fn deduct_without_request_id_event() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let caller = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);
    let (usdc, _, usdc_admin) = create_usdc(&env, &owner);

    env.mock_all_auths();
    fund_vault(&usdc_admin, &contract_id, 500);
    client.init(&owner, &usdc, &Some(500), &None, &None, &None);

    // Deduct without request_id
    client.deduct(&caller, &100, &None);

    let events = env.events().all();
    let last_event = events.last().unwrap();
    assert_eq!(last_event.0, contract_id);

    let topics = &last_event.1;
    assert_eq!(topics.len(), 3);
    let topic0: Symbol = topics.get(0).unwrap().into_val(&env);
    assert_eq!(topic0, Symbol::new(&env, "deduct"));
    // topic[2] should be empty symbol when no request_id
    let topic2: Symbol = topics.get(2).unwrap().into_val(&env);
    assert_eq!(topic2, Symbol::new(&env, ""));

    // Verify data: (amount, new_balance)
    let data: (i128, i128) = last_event.2.into_val(&env);
    assert_eq!(data, (100, 400));
}

#[test]
fn batch_deduct_with_revenue_pool_transfers_usdc() {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    let rev_pool = Address::generate(&env);
    let (vault_addr, vault) = create_vault(&env);
    let (usdc, usdc_client, usdc_admin) = create_usdc(&env, &owner);

    fund_vault(&usdc_admin, &vault_addr, 1000);
    vault.init(
        &owner,
        &usdc,
        &Some(1000),
        &None,
        &Some(rev_pool.clone()),
        &None,
    );

    let caller = Address::generate(&env);
    let items = soroban_sdk::vec![
        &env,
        DeductItem {
            amount: 100,
            request_id: Some(Symbol::new(&env, "r1")),
        },
        DeductItem {
            amount: 200,
            request_id: None,
        },
    ];
    let new_balance = vault.batch_deduct(&caller, &items);

    assert_eq!(new_balance, 700);
    assert_eq!(usdc_client.balance(&rev_pool), 300);
    assert_eq!(usdc_client.balance(&vault_addr), 700);
}

#[test]
#[should_panic(expected = "deduct amount exceeds max_deduct")]
fn batch_deduct_exceeds_max_deduct_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    let (vault_addr, vault) = create_vault(&env);
    let (usdc, _, usdc_admin) = create_usdc(&env, &owner);

    fund_vault(&usdc_admin, &vault_addr, 10_000);
    vault.init(
        &owner,
        &usdc,
        &Some(10_000),
        &None,
        &None,
        &Some(100),
    );

    let caller = Address::generate(&env);
    let items = soroban_sdk::vec![
        &env,
        DeductItem {
            amount: 101,
            request_id: None,
        },
    ];
    vault.batch_deduct(&caller, &items);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn batch_deduct_zero_amount_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    let (vault_addr, vault) = create_vault(&env);
    let (usdc, _, usdc_admin) = create_usdc(&env, &owner);

    fund_vault(&usdc_admin, &vault_addr, 100);
    vault.init(&owner, &usdc, &Some(100), &None, &None, &None);

    let caller = Address::generate(&env);
    let items = soroban_sdk::vec![
        &env,
        DeductItem {
            amount: 0,
            request_id: None,
        },
    ];
    vault.batch_deduct(&caller, &items);
}

#[test]
#[should_panic(expected = "batch_deduct requires at least one item")]
fn batch_deduct_empty_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    let (vault_addr, vault) = create_vault(&env);
    let (usdc, _, usdc_admin) = create_usdc(&env, &owner);

    fund_vault(&usdc_admin, &vault_addr, 100);
    vault.init(&owner, &usdc, &Some(100), &None, &None, &None);

    let caller = Address::generate(&env);
    let items: soroban_sdk::Vec<DeductItem> = soroban_sdk::vec![&env];
    vault.batch_deduct(&caller, &items);
}

#[test]
#[should_panic(expected = "deposit below minimum")]
fn deposit_below_minimum_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    let (_, vault) = create_vault(&env);
    let (usdc, _, usdc_admin) = create_usdc(&env, &owner);

    // min_deposit = 100
    vault.init(&owner, &usdc, &None, &Some(100), &None, &None);
    fund_user(&usdc_admin, &owner, 50);
    vault.deposit(&owner, &50);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn deduct_zero_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    let (vault_addr, vault) = create_vault(&env);
    let (usdc, _, usdc_admin) = create_usdc(&env, &owner);

    fund_vault(&usdc_admin, &vault_addr, 100);
    vault.init(&owner, &usdc, &Some(100), &None, &None, &None);
    vault.deduct(&owner, &0, &None);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn withdraw_zero_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    let (vault_addr, vault) = create_vault(&env);
    let (usdc, _, usdc_admin) = create_usdc(&env, &owner);

    fund_vault(&usdc_admin, &vault_addr, 100);
    vault.init(&owner, &usdc, &Some(100), &None, &None, &None);
    vault.withdraw(&0);
}

#[test]
#[should_panic(expected = "insufficient balance")]
fn withdraw_to_exceeds_balance_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    let to = Address::generate(&env);
    let (vault_addr, vault) = create_vault(&env);
    let (usdc, _, usdc_admin) = create_usdc(&env, &owner);

    fund_vault(&usdc_admin, &vault_addr, 50);
    vault.init(&owner, &usdc, &Some(50), &None, &None, &None);
    vault.withdraw_to(&to, &100);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn withdraw_to_zero_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    let to = Address::generate(&env);
    let (vault_addr, vault) = create_vault(&env);
    let (usdc, _, usdc_admin) = create_usdc(&env, &owner);

    fund_vault(&usdc_admin, &vault_addr, 100);
    vault.init(&owner, &usdc, &Some(100), &None, &None, &None);
    vault.withdraw_to(&to, &0);
}

/// Full lifecycle test exercising init (with revenue_pool + max_deduct), deposit, deduct
/// (with & without request_id), batch_deduct, withdraw, withdraw_to, distribute, and all getters.
#[test]
fn full_lifecycle_with_revenue_pool() {
    let env = Env::default();
    env.mock_all_auths();
    let owner = Address::generate(&env);
    let rev_pool = Address::generate(&env);
    let (vault_addr, vault) = create_vault(&env);
    let (usdc, usdc_client, usdc_admin) = create_usdc(&env, &owner);

    // Fund vault and owner
    fund_vault(&usdc_admin, &vault_addr, 5000);
    fund_user(&usdc_admin, &owner, 2000);

    // Init with revenue_pool and max_deduct
    let meta = vault.init(
        &owner,
        &usdc,
        &Some(5000),
        &Some(10),
        &Some(rev_pool.clone()),
        &Some(1000),
    );
    assert_eq!(meta.balance, 5000);
    assert_eq!(meta.owner, owner);
    assert_eq!(meta.min_deposit, 10);

    // Verify getters
    assert_eq!(vault.get_admin(), owner);
    assert_eq!(vault.get_max_deduct(), 1000);
    assert_eq!(vault.get_revenue_pool(), Some(rev_pool.clone()));

    // Deposit
    approve_spend(&env, &usdc_client, &owner, &vault_addr, 2000);
    let bal = vault.deposit(&owner, &500);
    assert_eq!(bal, 5500);

    // Deduct with request_id → transfers to revenue_pool
    let caller = Address::generate(&env);
    vault.deduct(&caller, &200, &Some(Symbol::new(&env, "full_req")));
    assert_eq!(vault.balance(), 5300);
    assert_eq!(usdc_client.balance(&rev_pool), 200);

    // Deduct without request_id
    vault.deduct(&caller, &100, &None);
    assert_eq!(vault.balance(), 5200);
    assert_eq!(usdc_client.balance(&rev_pool), 300);

    // Batch deduct with revenue_pool
    let items = soroban_sdk::vec![
        &env,
        DeductItem {
            amount: 50,
            request_id: Some(Symbol::new(&env, "batch1")),
        },
        DeductItem {
            amount: 75,
            request_id: None,
        },
    ];
    let bal = vault.batch_deduct(&caller, &items);
    assert_eq!(bal, 5075);
    assert_eq!(usdc_client.balance(&rev_pool), 425);

    // Withdraw
    let bal = vault.withdraw(&100);
    assert_eq!(bal, 4975);

    // Withdraw to
    let to = Address::generate(&env);
    let bal = vault.withdraw_to(&to, &200);
    assert_eq!(bal, 4775);
    assert_eq!(usdc_client.balance(&to), 200);

    // Distribute
    let dev = Address::generate(&env);
    vault.distribute(&owner, &dev, &500);
    assert_eq!(usdc_client.balance(&dev), 500);

    // set_admin
    let new_admin = Address::generate(&env);
    vault.set_admin(&owner, &new_admin);
    assert_eq!(vault.get_admin(), new_admin);

    // get_meta
    let meta = vault.get_meta();
    assert_eq!(meta.balance, 4775);
    assert_eq!(vault.balance(), 4775);
}
