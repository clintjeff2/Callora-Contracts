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
    _env: &Env,
    usdc_admin_client: &token::StellarAssetClient,
    vault_address: &Address,
    amount: i128,
) {
    usdc_admin_client.mint(vault_address, &amount);
}

#[test]
fn init_and_balance() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());

    // Call init directly inside as_contract so events are captured
    let events = env.as_contract(&contract_id, || {
        let (usdc, _, _) = create_usdc(&env, &owner);
        CalloraVault::init(env.clone(), owner.clone(), usdc, Some(1000));
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

    let (usdc, _, _) = create_usdc(&env, &owner);
    client.init(&owner, &usdc, &Some(100));
    client.deposit(&200);
    assert_eq!(client.balance(), 300);
    client.deduct(&50);
    assert_eq!(client.balance(), 250);
}

#[test]
fn test_init_success() {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let (_, vault) = create_vault(&env);
    let (usdc_address, _, _) = create_usdc(&env, &owner);

    let meta = vault.init(&owner, &usdc_address, &None);

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

    vault.init(&owner, &usdc_address, &None);
    vault.init(&owner, &usdc_address, &None);
}

#[test]
fn test_distribute_success() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let developer = Address::generate(&env);
    let (vault_address, vault) = create_vault(&env);
    let (usdc_address, usdc_client, usdc_admin_client) = create_usdc(&env, &admin);

    vault.init(&admin, &usdc_address, &None);
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

    vault.init(&admin, &usdc_address, &None);
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

    vault.init(&admin, &usdc_address, &None);
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

    vault.init(&admin, &usdc_address, &None);
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

    vault.init(&admin, &usdc_address, &None);
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

    vault.init(&admin, &usdc_address, &None);
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

    vault.init(&admin, &usdc_address, &None);
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

    vault.init(&original_admin, &usdc_address, &None);
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

    vault.init(&original_admin, &usdc_address, &None);
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

    vault.init(&owner, &usdc_address, &Some(0));
    vault.deposit(&200);
    assert_eq!(vault.balance(), 200);
    vault.deposit(&50);
    assert_eq!(vault.balance(), 250);
}

#[test]
fn test_deduct_success() {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let (_, vault) = create_vault(&env);
    let (usdc_address, _, _) = create_usdc(&env, &owner);

    vault.init(&owner, &usdc_address, &Some(300));
    vault.deduct(&100);
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

    vault.init(&owner, &usdc_address, &Some(50));
    vault.deduct(&100);
}

#[test]
fn test_get_meta_returns_correct_values() {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let (_, vault) = create_vault(&env);
    let (usdc_address, _, _) = create_usdc(&env, &owner);

    vault.init(&owner, &usdc_address, &Some(999));
    let meta = vault.get_meta();
    assert_eq!(meta.owner, owner);
    assert_eq!(meta.balance, 999);
}
