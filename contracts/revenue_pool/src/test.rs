extern crate std;

use super::*;
use soroban_sdk::testutils::{Address as _, Events as _};
use soroban_sdk::token;

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

fn create_pool(env: &Env) -> (Address, RevenuePoolClient<'_>) {
    let address = env.register(RevenuePool, ());
    let client = RevenuePoolClient::new(env, &address);
    (address, client)
}

fn fund_pool(usdc_admin_client: &token::StellarAssetClient, pool_address: &Address, amount: i128) {
    usdc_admin_client.mint(pool_address, &amount);
}

#[test]
fn init_success() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let (_pool_addr, client) = create_pool(&env);
    let (usdc, _, _) = create_usdc(&env, &admin);

    client.init(&admin, &usdc);
    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.balance(), 0);
}

#[test]
#[should_panic(expected = "revenue pool already initialized")]
fn init_double_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let (_, client) = create_pool(&env);
    let (usdc, _, _) = create_usdc(&env, &admin);

    client.init(&admin, &usdc);
    client.init(&admin, &usdc);
}

#[test]
fn distribute_success() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let developer = Address::generate(&env);
    let (pool_addr, client) = create_pool(&env);
    let (usdc_address, usdc_client, usdc_admin) = create_usdc(&env, &admin);

    client.init(&admin, &usdc_address);
    fund_pool(&usdc_admin, &pool_addr, 1_000);
    client.distribute(&admin, &developer, &400);

    assert_eq!(usdc_client.balance(&pool_addr), 600);
    assert_eq!(usdc_client.balance(&developer), 400);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn distribute_zero_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let developer = Address::generate(&env);
    let (_, client) = create_pool(&env);
    let (usdc, _, _) = create_usdc(&env, &admin);

    client.init(&admin, &usdc);
    client.distribute(&admin, &developer, &0);
}

#[test]
#[should_panic(expected = "insufficient USDC balance")]
fn distribute_excess_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let developer = Address::generate(&env);
    let (pool_addr, client) = create_pool(&env);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &admin);

    client.init(&admin, &usdc_address);
    fund_pool(&usdc_admin, &pool_addr, 100);
    client.distribute(&admin, &developer, &101);
}

#[test]
#[should_panic(expected = "unauthorized: caller is not admin")]
fn distribute_unauthorized_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let attacker = Address::generate(&env);
    let developer = Address::generate(&env);
    let (pool_addr, client) = create_pool(&env);
    let (usdc_address, _, usdc_admin) = create_usdc(&env, &admin);

    client.init(&admin, &usdc_address);
    fund_pool(&usdc_admin, &pool_addr, 500);
    client.distribute(&attacker, &developer, &100);
}

#[test]
fn set_admin_transfers_control() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let developer = Address::generate(&env);
    let (pool_addr, client) = create_pool(&env);
    let (usdc_address, usdc_client, usdc_admin) = create_usdc(&env, &admin);

    client.init(&admin, &usdc_address);
    fund_pool(&usdc_admin, &pool_addr, 300);
    client.set_admin(&admin, &new_admin);
    assert_eq!(client.get_admin(), new_admin);

    client.distribute(&new_admin, &developer, &100);
    assert_eq!(usdc_client.balance(&developer), 100);
}

#[test]
fn receive_payment_emits_event() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let (_, client) = create_pool(&env);
    let (usdc, _, _) = create_usdc(&env, &admin);

    client.init(&admin, &usdc);
    client.receive_payment(&admin, &500, &true);
    let events = env.events().all();
    assert!(!events.is_empty());
}

// ---------------------------------------------------------------------------
// Additional coverage tests
// ---------------------------------------------------------------------------

/// set_admin with a non-admin caller panics — covers line 50.
#[test]
#[should_panic(expected = "unauthorized: caller is not admin")]
fn set_admin_unauthorized_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let intruder = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let (_, client) = create_pool(&env);
    let (usdc, _, _) = create_usdc(&env, &admin);

    client.init(&admin, &usdc);
    client.set_admin(&intruder, &new_admin);
}

/// set_admin success path — covers the storage set on line 54.
#[test]
fn set_admin_success() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let (_, client) = create_pool(&env);
    let (usdc, _, _) = create_usdc(&env, &admin);

    client.init(&admin, &usdc);
    assert_eq!(client.get_admin(), admin);
    client.set_admin(&admin, &new_admin);
    assert_eq!(client.get_admin(), new_admin);
}

/// balance() on an initialised pool with no funds returns 0 — covers line 118.
#[test]
fn balance_returns_zero_when_empty() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let (_, client) = create_pool(&env);
    let (usdc, _, _) = create_usdc(&env, &admin);

    client.init(&admin, &usdc);
    assert_eq!(client.balance(), 0);
}

/// balance() after funding returns the correct amount — covers line 118 success path.
#[test]
fn balance_returns_correct_amount_after_funding() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let (pool_addr, client) = create_pool(&env);
    let (usdc, usdc_client, usdc_admin) = create_usdc(&env, &admin);

    client.init(&admin, &usdc);
    fund_pool(&usdc_admin, &pool_addr, 750);
    assert_eq!(client.balance(), 750);
    // partial distribute then re-check
    let developer = Address::generate(&env);
    client.distribute(&admin, &developer, &250);
    assert_eq!(client.balance(), 500);
    assert_eq!(usdc_client.balance(&developer), 250);
}

/// distribute with a negative amount panics — covers the amount <= 0 branch.
#[test]
#[should_panic(expected = "amount must be positive")]
fn distribute_negative_panics() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let developer = Address::generate(&env);
    let (_, client) = create_pool(&env);
    let (usdc, _, _) = create_usdc(&env, &admin);

    client.init(&admin, &usdc);
    client.distribute(&admin, &developer, &-1);
}

// ---------------------------------------------------------------------------
// Direct-call (as_contract) tests — force tarpaulin to instrument the
// chained-method lines that the Soroban host hides when called via client.
// ---------------------------------------------------------------------------

/// Covers init() .set() lines (28, 31) directly.
#[test]
fn direct_init_storage_lines() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let pool_id = env.register(RevenuePool, ());
    let (usdc_address, _, _) = create_usdc(&env, &admin);
    env.mock_all_auths();

    env.as_contract(&pool_id, || {
        RevenuePool::init(env.clone(), admin.clone(), usdc_address.clone());
    });

    let client = RevenuePoolClient::new(&env, &pool_id);
    assert_eq!(client.get_admin(), admin);
    assert_eq!(client.balance(), 0);
}

/// Covers set_admin() .set() line (54) directly.
#[test]
fn direct_set_admin_storage_line() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let new_admin = Address::generate(&env);
    let (pool_id, client) = create_pool(&env);
    let (usdc_address, _, _) = create_usdc(&env, &admin);
    env.mock_all_auths();

    // Init via client so admin's require_auth() is consumed in its own frame.
    client.init(&admin, &usdc_address);

    // Enter the contract directly only for set_admin — covers line 54.
    env.as_contract(&pool_id, || {
        RevenuePool::set_admin(env.clone(), admin.clone(), new_admin.clone());
    });

    assert_eq!(client.get_admin(), new_admin);
}

/// Covers distribute() USDC-lookup line (99) and balance() USDC-lookup (118) directly.
#[test]
fn direct_distribute_and_balance_lines() {
    let env = Env::default();
    let admin = Address::generate(&env);
    let developer = Address::generate(&env);
    let (pool_id, client) = create_pool(&env);
    let (usdc_address, usdc_client, usdc_admin) = create_usdc(&env, &admin);
    env.mock_all_auths();

    // Init via client so admin's require_auth() is consumed in its own frame.
    client.init(&admin, &usdc_address);
    fund_pool(&usdc_admin, &pool_id, 1_000);

    // Enter the contract directly for balance + distribute — covers lines 99, 118.
    env.as_contract(&pool_id, || {
        let b = RevenuePool::balance(env.clone());
        assert_eq!(b, 1_000);
        RevenuePool::distribute(env.clone(), admin.clone(), developer.clone(), 400);
    });

    assert_eq!(usdc_client.balance(&developer), 400);
    assert_eq!(usdc_client.balance(&pool_id), 600);
}

