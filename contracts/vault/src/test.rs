extern crate std;

use super::*;
use soroban_sdk::testutils::{Address as _, Events};
use soroban_sdk::{IntoVal, Symbol};

#[test]
fn init_and_balance() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());

    // Call init directly inside as_contract so events are captured
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

    client.init(&owner, &Some(100));

    env.mock_all_auths();
    client.deposit(&owner, &200);
    assert_eq!(client.balance(), 300);

    client.deduct(&50);
    assert_eq!(client.balance(), 250);
}

#[test]
fn owner_can_deposit() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    // Mock the owner as the invoker
    env.mock_all_auths();
    client.deposit(&owner, &200);

    assert_eq!(client.balance(), 300);
}

#[test]
fn allowed_depositor_can_deposit() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let depositor = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    // Owner sets the allowed depositor
    env.mock_all_auths();
    client.set_allowed_depositor(&owner, &Some(depositor.clone()));

    // Depositor can now deposit
    client.deposit(&depositor, &50);
    assert_eq!(client.balance(), 150);
}

#[test]
#[should_panic(expected = "unauthorized: only owner or allowed depositor can deposit")]
fn unauthorized_address_cannot_deposit() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    // Try to deposit as unauthorized address (should panic)
    env.mock_all_auths();
    let unauthorized_addr = Address::generate(&env);
    client.deposit(&unauthorized_addr, &50);
}

#[test]
fn owner_can_set_allowed_depositor() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let depositor = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    // Owner sets allowed depositor
    env.mock_all_auths();
    client.set_allowed_depositor(&owner, &Some(depositor.clone()));

    // Depositor can deposit
    client.deposit(&depositor, &25);
    assert_eq!(client.balance(), 125);
}

#[test]
fn owner_can_clear_allowed_depositor() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let depositor = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    env.mock_all_auths();

    // Set depositor
    client.set_allowed_depositor(&owner, &Some(depositor.clone()));
    client.deposit(&depositor, &50);
    assert_eq!(client.balance(), 150);

    // Clear depositor
    client.set_allowed_depositor(&owner, &None);

    // Depositor can no longer deposit (would panic if attempted)
    // Owner can still deposit
    client.deposit(&owner, &25);
    assert_eq!(client.balance(), 175);
}

#[test]
#[should_panic(expected = "unauthorized: owner only")]
fn non_owner_cannot_set_allowed_depositor() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let depositor = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    // Try to set allowed depositor as non-owner (should panic)
    env.mock_all_auths();
    let non_owner_addr = Address::generate(&env);
    client.set_allowed_depositor(&non_owner_addr, &Some(depositor));
}

#[test]
#[should_panic(expected = "unauthorized: only owner or allowed depositor can deposit")]
fn deposit_after_depositor_cleared_is_rejected() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let depositor = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    env.mock_all_auths();

    // Set and then clear depositor
    client.set_allowed_depositor(&owner, &Some(depositor.clone()));
    client.set_allowed_depositor(&owner, &None);

    // Depositor should no longer be able to deposit
    client.deposit(&depositor, &50);
}

mod integration {
    //! Integration tests for vault contract with mock Stellar token.
    //!
    //! These tests demonstrate the full flow of token-based operations:
    //! - Mock token creation and minting
    //! - Vault initialization and deposits
    //! - Balance tracking and deductions
    //! - Authorization scenarios with allowed depositors

    use super::*;
    use soroban_sdk::token::{StellarAssetClient, TokenClient};

    /// Test basic vault-token integration with owner deposits.
    ///
    /// Flow: Create token → Mint to owner → Owner deposits → Deduct → Verify balances
    #[test]
    fn vault_token_integration() {
        let env = Env::default();
        env.mock_all_auths();

        // Create mock token
        let token_admin = Address::generate(&env);
        let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
        let token_id = token_contract.address();
        let token_client = TokenClient::new(&env, &token_id);
        let token_admin_client = StellarAssetClient::new(&env, &token_id);

        // Create vault
        let vault_owner = Address::generate(&env);
        let vault_id = env.register(CalloraVault {}, ());
        let vault_client = CalloraVaultClient::new(&env, &vault_id);

        // Initialize vault with zero balance
        vault_client.init(&vault_owner, &Some(0));

        // Mint tokens to vault owner
        token_admin_client.mint(&vault_owner, &1000);
        assert_eq!(token_client.balance(&vault_owner), 1000);

        // Owner deposits 500 tokens to vault
        vault_client.deposit(&vault_owner, &500);

        // Verify vault balance increased
        assert_eq!(vault_client.balance(), 500);

        // Deduct 200 from vault
        vault_client.deduct(&200);

        // Verify vault balance decreased
        assert_eq!(vault_client.balance(), 300);

        // Verify token accounting consistency
        // Note: In full implementation, tokens would be transferred to vault contract
        // and deductions would transfer tokens to revenue pool
        assert_eq!(token_client.balance(&vault_owner), 1000); // Owner still has original tokens (mock scenario)
    }

    /// Test vault-token integration with allowed depositor (backend service).
    ///
    /// Flow: Create token → Set allowed depositor → Mint to backend → Backend deposits → Multiple deductions → Verify balances
    #[test]
    fn vault_token_integration_with_allowed_depositor() {
        let env = Env::default();
        env.mock_all_auths();

        // Create mock token
        let token_admin = Address::generate(&env);
        let token_contract = env.register_stellar_asset_contract_v2(token_admin.clone());
        let token_id = token_contract.address();
        let token_client = TokenClient::new(&env, &token_id);
        let token_admin_client = StellarAssetClient::new(&env, &token_id);

        // Create vault
        let vault_owner = Address::generate(&env);
        let backend_service = Address::generate(&env);
        let vault_id = env.register(CalloraVault {}, ());
        let vault_client = CalloraVaultClient::new(&env, &vault_id);

        // Initialize vault with zero balance
        vault_client.init(&vault_owner, &Some(0));

        // Set backend service as allowed depositor
        vault_client.set_allowed_depositor(&vault_owner, &Some(backend_service.clone()));

        // Mint tokens to backend service
        token_admin_client.mint(&backend_service, &2000);
        assert_eq!(token_client.balance(&backend_service), 2000);

        // Backend service deposits 800 tokens to vault
        vault_client.deposit(&backend_service, &800);

        // Verify vault balance increased
        assert_eq!(vault_client.balance(), 800);

        // Simulate API usage - deduct 150
        vault_client.deduct(&150);
        assert_eq!(vault_client.balance(), 650);

        // Another API call - deduct 100
        vault_client.deduct(&100);
        assert_eq!(vault_client.balance(), 550);

        // Verify token accounting consistency
        assert_eq!(token_client.balance(&backend_service), 2000); // Backend still has original tokens (mock scenario)
    }
}
