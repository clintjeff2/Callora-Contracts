//! Vault contract unit tests (deposits, access control, API pricing).

extern crate std;

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::testutils::Events as _;
use soroban_sdk::Env;
use soroban_sdk::{IntoVal, Symbol};

#[test]
fn init_and_balance() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());

    env.mock_all_auths();
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
fn init_default_zero_balance() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &None);
    assert_eq!(client.balance(), 0);
}

#[test]
fn deposit_and_deduct() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(100));

    env.mock_all_auths();
    client.deposit(&owner, &200);
    assert_eq!(client.balance(), 300);

    client.deduct(&owner, &50);
    assert_eq!(client.balance(), 250);
}

#[test]
fn owner_can_deposit() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    // Initialize vault with initial balance
    env.mock_all_auths();
    client.init(&owner, &Some(500));

    let meta = client.get_meta();
    let balance = client.balance();
    assert_eq!(meta.balance, balance, "balance mismatch after init");
    assert_eq!(meta.owner, owner, "owner changed after init");
    assert_eq!(balance, 500, "incorrect balance after init");

    client.deposit(&owner, &300);
    let meta = client.get_meta();
    let balance = client.balance();
    assert_eq!(meta.balance, balance, "balance mismatch after deposit");
    assert_eq!(balance, 800, "incorrect balance after deposit");

    // Deduct and verify consistency
    client.deduct(&owner, &150);
    client.deduct(&owner, &150);
    let meta = client.get_meta();
    let balance = client.balance();
    assert_eq!(meta.balance, balance, "balance mismatch after deduct");
    assert_eq!(balance, 500, "incorrect balance after deduct");

    // Perform multiple operations and verify final state
    client.deposit(&owner, &100);
    client.deduct(&owner, &50);
    client.deposit(&owner, &25);
    client.deposit(&owner, &100);
    client.deduct(&owner, &50);
    client.deposit(&owner, &25);
    let meta = client.get_meta();
    let balance = client.balance();
    assert_eq!(
        meta.balance, balance,
        "balance mismatch after multiple operations"
    );
    assert_eq!(balance, 650, "incorrect final balance");
}

#[test]
#[should_panic(expected = "insufficient balance")]
fn deduct_exact_balance_and_panic() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(100));
    assert_eq!(client.balance(), 100);

    // Deduct exact balance
    client.deduct(&owner, &100);
    assert_eq!(client.balance(), 0);

    // Further deduct should panic
    client.deduct(&owner, &1);
}

#[test]
fn allowed_depositor_can_deposit() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let depositor = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
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
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
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
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
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
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(100));

    env.mock_all_auths();

    // Set depositor
    client.set_allowed_depositor(&owner, &Some(depositor.clone()));
    client.deposit(&depositor, &50);
    assert_eq!(client.balance(), 150);

    // Clear depositor
    client.set_allowed_depositor(&owner, &None);

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
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
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
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(100));

    env.mock_all_auths();

    // Set and then clear depositor
    client.set_allowed_depositor(&owner, &Some(depositor.clone()));
    client.set_allowed_depositor(&owner, &None);

    // Depositor should no longer be able to deposit
    client.deposit(&depositor, &50);
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
#[should_panic(expected = "amount must be positive")]
fn deposit_zero_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(1000));
    client.deposit(&owner, &0);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn deposit_negative_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(100));
    client.deposit(&owner, &-100);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn deduct_zero_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(500));
    client.deduct(&owner, &0);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn deduct_negative_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(100));
    client.deduct(&owner, &-50);
}

#[test]
#[should_panic(expected = "insufficient balance")]
fn deduct_exceeds_balance_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(50));
    client.deduct(&owner, &100);
}

#[test]
fn test_transfer_ownership() {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let new_owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    env.mock_all_auths();

    // transfer ownership via client
    // Owner authorizes transfer (require_auth in contract)
    client.transfer_ownership(&new_owner);

    let transfer_event = env
        .events()
        .all()
        .into_iter()
        .find(|e| {
            e.0 == contract_id && {
                let topics = &e.1;
                if !topics.is_empty() {
                    let topic_name: Symbol = topics.get(0).unwrap().into_val(&env);
                    topic_name == Symbol::new(&env, "transfer_ownership")
                } else {
                    false
                }
            }
        })
        .expect("expected transfer event");

    let topics = &transfer_event.1;
    let topic_old_owner: Address = topics.get(1).unwrap().into_val(&env);
    assert_eq!(topic_old_owner, owner);

    let topic_new_owner: Address = topics.get(2).unwrap().into_val(&env);
    assert_eq!(topic_new_owner, new_owner);
}

#[test]
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
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(100));

    // Mock the owner as the invoker
    env.mock_all_auths();

    // This should panic with "insufficient balance"
    client.deduct(&owner, &101);
}

#[test]
fn balance_unchanged_after_failed_deduct() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault {}, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    // Initialize with balance of 100
    client.init(&owner, &Some(100));
    assert_eq!(client.balance(), 100);

    // Mock the owner as the invoker
    env.mock_all_auths();

    env.mock_all_auths();

    // This should panic because new_owner is the same as current owner
    client.transfer_ownership(&owner);
    // Attempt to deduct more than balance, which should panic
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.deduct(&owner, &101);
    }));

    // Verify the operation panicked
    assert!(result.is_err());

    // Verify balance is still 100 (unchanged after the failed deduct)
    assert_eq!(client.balance(), 100);
}

#[test]
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
    let new_owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(100));

    // No auth for owner — transfer_ownership requires current owner to authorize
    env.mock_auths(&[]);
    client.transfer_ownership(&new_owner);
}

#[test]
#[should_panic(expected = "vault already initialized")]
fn init_already_initialized_panics() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(100));
    client.init(&owner, &Some(200)); // Should panic
}

/// Fuzz test: random deposit/deduct sequence asserting balance >= 0 and matches expected.
/// Run with: cargo test --package callora-vault fuzz_deposit_and_deduct -- --nocapture
#[test]
fn fuzz_deposit_and_deduct() {
    use rand::Rng;

    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    let initial_balance: i128 = 1_000;
    client.init(&owner, &Some(initial_balance));

    let mut expected = initial_balance;
    let mut rng = rand::thread_rng();

    for _ in 0..500 {
        if rng.gen_bool(0.5) {
            let amount = rng.gen_range(1..=500);
            client.deposit(&owner, &amount);
            expected += amount;
        } else if expected > 0 {
            let amount = rng.gen_range(1..=expected.min(500));
            client.deduct(&owner, &amount);
            expected -= amount;
        }

        let balance = client.balance();
        assert!(balance >= 0, "balance went negative: {}", balance);
        assert_eq!(
            balance, expected,
            "balance mismatch: got {}, expected {}",
            balance, expected
        );
    }

    assert_eq!(client.balance(), expected);
}

#[test]
fn deduct_returns_new_balance() {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));
    let new_balance = client.deduct(&owner, &30);
    assert_eq!(new_balance, 70);
    assert_eq!(client.balance(), 70);
}

#[test]
fn test_concurrent_deposits() {
    let env = Env::default();
    env.mock_all_auths();

    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    client.init(&owner, &Some(100));

    let dep1 = Address::generate(&env);
    let dep2 = Address::generate(&env);

    client.set_allowed_depositor(&owner, &Some(dep1.clone()));
    client.set_allowed_depositor(&owner, &Some(dep2.clone()));

    // Concurrent deposits
    client.deposit(&dep1, &200);
    client.deposit(&dep2, &300);

    assert_eq!(client.balance(), 600);
}

#[test]
fn init_twice_panics_on_reinit() {
    let env = Env::default();
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(25));
    assert_eq!(client.balance(), 25);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        client.init(&owner, &Some(50));
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
    let owner = Address::generate(&env);
    let contract_id = env.register(CalloraVault, ());
    let client = CalloraVaultClient::new(&env, &contract_id);

    env.mock_all_auths();
    client.init(&owner, &Some(100));
    client.deposit(&owner, &50);
    client.deduct(&owner, &30);
    assert_eq!(client.get_meta().owner, owner);
}

#[test]
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
