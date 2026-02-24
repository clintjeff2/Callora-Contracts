# Event Schema (Vault Contract)

Events emitted by the Callora vault contract for indexers and frontends. All topic/data types refer to Soroban/Stellar XDR values.

## Contract: Callora Vault

### `init`

Emitted when the vault is initialized.

| Field   | Location | Type   | Description           |
|---------|----------|--------|-----------------------|
| topic 0 | topics   | Symbol | `"init"`              |
| topic 1 | topics   | Address| vault owner           |
| data    | data     | i128   | initial balance       |

---

### `deposit`

Emitted when balance is increased via `deposit(amount)`.

| Field   | Location | Type   | Description   |
|---------|----------|--------|---------------|
| topic 0 | topics   | Symbol | `"deposit"`   |
| data    | data     | (i128, i128) | (amount, new_balance) |

---

### `deduct`

Emitted on each deduction: single `deduct(amount)` or each item in `batch_deduct(items)`.

| Field   | Location | Type   | Description   |
|---------|----------|--------|---------------|
| topic 0 | topics   | Symbol | `"deduct"`    |
| topic 1 | topics   | Address| caller        |
| topic 2 | topics   | Symbol | optional request_id (empty symbol if none) |
| data    | data     | (i128, i128) | (amount, new_balance) |

---

### `withdraw`

Emitted when the owner withdraws via `withdraw(amount)`.

| Field   | Location | Type   | Description   |
|---------|----------|--------|---------------|
| topic 0 | topics   | Symbol | `"withdraw"`  |
| topic 1 | topics   | Address| vault owner   |
| data    | data     | (i128, i128) | (amount, new_balance) |

---

### `withdraw_to`

Emitted when the owner withdraws to a designated address via `withdraw_to(to, amount)`.

| Field   | Location | Type   | Description   |
|---------|----------|--------|---------------|
| topic 0 | topics   | Symbol | `"withdraw_to"` |
| topic 1 | topics   | Address| vault owner   |
| topic 2 | topics   | Address| recipient `to` |
| data    | data     | (i128, i128) | (amount, new_balance) |

---

## Not yet implemented

- **OwnershipTransfer**: not present in current vault; would list old_owner, new_owner.
- **Pause**: not present in current vault; would indicate pause state change.

Settlement or other contracts in this repo will have their events documented here as they are added.
