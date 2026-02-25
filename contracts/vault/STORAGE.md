# Vault Storage Layout

This document describes the storage layout of the Callora Vault contract, including storage keys, data types, and upgrade implications.

## Storage Overview

The Callora Vault contract uses Soroban's instance storage to persist contract state. Data is stored under a small set of typed keys defined by the `StorageKey` enum.

## Storage Keys

### Instance Storage

The contract defines the following storage keys:

```rust
#[contracttype]
pub enum StorageKey {
    Meta,
    AllowedDepositor,
    ApiPrice(Symbol),
}
```

They are used as follows:

| Key Variant | Value Type | Description | Usage |
|------------|-----------|-------------|-------|
| `Meta` | `VaultMeta` | Primary vault metadata (owner, balance) | Core vault state |
| `AllowedDepositor` | `Address` | Optional backend/admin address allowed to deposit and manage pricing | Access control |
| `ApiPrice(Symbol(api_id))` | `i128` | Price per API call (smallest USDC units) for a given `api_id` | API pricing |

### Data Structures

#### VaultMeta

```rust
#[contracttype]
#[derive(Clone)]
pub struct VaultMeta {
    pub owner: Address,
    pub balance: i128,
}
```

**Fields:**
- `owner`: `Address` - The address that owns the vault and can perform operations
- `balance`: `i128` - Current vault balance, can be positive or zero

## Storage Operations

### Write Operations
- `init()`: Creates initial `VaultMeta` and stores under `StorageKey::Meta`
- `set_allowed_depositor()`: Sets or clears `StorageKey::AllowedDepositor`
- `deposit()`: Reads `VaultMeta`, updates balance, writes back under `StorageKey::Meta`
- `deduct()`: Reads `VaultMeta`, validates balance, updates, writes back under `StorageKey::Meta`
- `set_price()`: Writes the price for a given `api_id` under `StorageKey::ApiPrice(api_id)`

### Read Operations
- `get_meta()`: Reads and returns `VaultMeta` from `StorageKey::Meta`
- `balance()`: Reads `VaultMeta` and returns balance field
- `get_price()`: Reads and returns the price for a given `api_id` from `StorageKey::ApiPrice(api_id)` (or `None` if unset)

## Storage Layout Visualization

```
Instance Storage
├── StorageKey::Meta
│   └── VaultMeta
│       ├── owner: Address
│       └── balance: i128
├── StorageKey::AllowedDepositor
│   └── Address (optional key; present only when set)
└── StorageKey::ApiPrice(Symbol(api_id))
    └── i128 (price per call in smallest USDC units)
```

## Upgrade Implications

### Current Layout Considerations
- **Single Key Design**: All vault state is consolidated under one storage key, simplifying migrations
- **Immutable Structure**: `VaultMeta` structure fields are not optional, ensuring data consistency
- **Type Safety**: Strong typing prevents data corruption

### Potential Upgrade Paths

#### 1. Adding New Fields
To add new fields to `VaultMeta`:
```rust
#[contracttype]
#[derive(Clone)]
pub struct VaultMetaV2 {
    pub owner: Address,
    pub balance: i128,
    pub new_field: SomeType,  // New field
}
```

**Migration Strategy:**
- Read existing `VaultMeta` from `"meta"` key
- Transform to `VaultMetaV2` with default values for new fields
- Write back to same `"meta"` key
- Update contract code to use `VaultMetaV2`

#### 2. Adding New Storage Keys
For additional data that doesn't fit in `VaultMeta`:
```rust
// New storage key for additional data
env.storage().instance().set(&Symbol::new(&env, "new_key"), &new_data);
```

**Benefits:**
- Non-breaking change to existing `VaultMeta`
- Allows separation of concerns
- Easier to manage optional data

#### 3. Storage Key Renaming
If renaming storage keys becomes necessary:
```rust
// Migration pattern
let old_data = env.storage().instance().get(&Symbol::new(&env, "old_key"));
env.storage().instance().set(&Symbol::new(&env, "new_key"), &old_data);
env.storage().instance().remove(&Symbol::new(&env, "old_key"));
```

## Security Considerations

### Access Control
- Storage operations are protected by contract functions
- `deduct()` includes balance validation to prevent underflow
- No direct storage access from external callers

### Data Integrity
- `VaultMeta` is atomic - all fields updated together
- Balance operations include assertions to prevent invalid states
- Storage writes are transactional within Soroban

## Gas Efficiency

### Current Optimizations
- Single storage key reduces read/write overhead
- Composite structure minimizes storage entries
- Efficient `i128` type for balance calculations

### Potential Optimizations
- Consider packed storage for very large-scale deployments
- Evaluate temporary vs. persistent storage for frequently accessed data
- Batch operations where possible

## Testing Considerations

### Storage Tests
Current test suite covers:
- Initial storage setup in `init_and_balance()`
- Storage updates in `deposit_and_deduct()`
- Event emission verification

### Recommended Additional Tests
- Storage migration scenarios
- Edge cases (maximum balance, zero balance)
- Storage upgrade/downgrade compatibility
- Gas usage benchmarks for storage operations

## Monitoring and Debugging

### Storage Inspection
Use Soroban CLI to inspect storage:
```bash
soroban contract storage \
  --contract-id <CONTRACT_ID> \
  --key "meta" \
  --output json
```

### Event Monitoring
Monitor storage-related events:
- `init` events for vault creation
- Future events could track significant balance changes

## Version History

| Version | Storage Layout | Changes |
|---------|----------------|---------|
| 1.0 | Single `"meta"` key with `VaultMeta` | Initial implementation |

## Future Considerations

### Scalability
- Current design suitable for single-tenant vaults
- Multi-tenant support would require storage key redesign
- Consider sharding strategies for high-volume deployments

### Compliance
- Storage layout supports audit trails through events
- Transparent state structure for regulatory compliance
- Upgrade paths maintain data integrity

---

**Note**: This storage layout documentation should be updated whenever contract storage is modified. Always test storage migrations thoroughly before deployment.
