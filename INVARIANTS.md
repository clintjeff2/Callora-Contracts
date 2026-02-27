## Vault Balance Invariant

**Invariant**: For every reachable state of the `CalloraVault` contract, the stored balance in `VaultMeta.balance` is always **greater than or equal to 0**.

- **Storage field**: `VaultMeta.balance : i128`
- **Accessors**:
  - `get_meta(env: Env) -> VaultMeta`
  - `balance(env: Env) -> i128`
- **Guarantee**: Any value returned by `get_meta(env).balance` or `balance(env)` is **never negative**.

This document lists all functions that can change the stored balance and the pre-/post-conditions that preserve this invariant.

---

## Functions That Modify Balance

Only the following functions mutate `VaultMeta.balance`:

- `init(env, owner, usdc_token, initial_balance, min_deposit, revenue_pool, max_deduct)`
- `deposit(env, from, amount)`
- `deduct(env, caller, amount, request_id)`
- `batch_deduct(env, caller, items: Vec<DeductItem>)`
- `withdraw(env, amount)`
- `withdraw_to(env, to, amount)`

Helper and view functions such as `get_meta`, `get_max_deduct`, `get_revenue_pool`, `get_admin`, and `balance` **do not** modify balance.

---

### `init`

**Effect on balance**  
- Sets `VaultMeta.balance` to `initial_balance.unwrap_or(0)`.

**Pre-conditions**
- Vault is not already initialized:
  - `!env.storage().instance().has(META_KEY)`
- `max_deduct.unwrap_or(DEFAULT_MAX_DEDUCT) > 0`
- If `initial_balance > 0`, the contract already holds at least that much USDC:
  - `usdc.balance(current_contract_address) >= initial_balance`

**Post-conditions**
- `VaultMeta.balance == initial_balance.unwrap_or(0)`
- `VaultMeta.balance >= 0` (since `initial_balance` is an `i128` and enforced via the token-balance check).

---

### `deposit`

**Effect on balance**  
- Increases `VaultMeta.balance` by `amount`:
  - `balance' = balance + amount`

**Pre-conditions**
- Caller is authorized:
  - `from.require_auth()`
- Vault is initialized (via `get_meta` and USDC address lookup).
- Vault is **not paused**:
  - `is_paused(env) == false` (deposit aborts with `"vault is paused"` if paused).
- Amount satisfies the minimum deposit:
  - `amount >= meta.min_deposit`
- USDC transfer-from must succeed:
  - Token contract must allow `current_contract_address` to transfer `amount` from `from` to `current_contract_address`.

**Post-conditions**
- `VaultMeta.balance' = balance + amount`
- Because `amount >= 0` in practice (negative amounts are not useful and would fail at the token layer) and `balance` is already non-negative, we maintain:
  - `VaultMeta.balance' >= 0`

---

### `deduct`

**Effect on balance**  
- Decreases `VaultMeta.balance` by `amount`:
  - `balance' = balance - amount`

**Pre-conditions**
- Caller is authorized:
  - `caller.require_auth()`
- Vault is initialized.
- Amount constraints:
  - `amount > 0`
  - `amount <= get_max_deduct(env)`
- Sufficient balance:
  - `meta.balance >= amount`

**Post-conditions**
- `VaultMeta.balance' = balance - amount`
- Because of the `meta.balance >= amount` assertion and `amount > 0`, we have:
  - `VaultMeta.balance' >= 0`

---

### `batch_deduct`

**Effect on balance**  
- For each `DeductItem { amount, .. }`, decreases `VaultMeta.balance` by `amount`, applied in sequence.
- Total change: `balance' = balance - sum_i(amount_i)`.

**Pre-conditions**
- Caller is authorized:
  - `caller.require_auth()`
- Vault is initialized.
- Items constraints:
  - `items.len() > 0`
  - For every item:
    - `item.amount > 0`
    - `item.amount <= get_max_deduct(env)`
- Sufficient balance across the entire batch:
  - The loop uses a `running` variable and asserts `running >= item.amount` before each subtraction.
  - This ensures that the **cumulative** deductions never drive the interim balance negative.

**Post-conditions**
- `VaultMeta.balance' = balance - sum_i(amount_i)`
- The running-balance checks ensure:
  - `VaultMeta.balance' >= 0`
- If any pre-condition fails, the entire batch reverts and the original `VaultMeta.balance` is preserved.

---

### `withdraw`

**Effect on balance**  
- Decreases `VaultMeta.balance` by `amount`:
  - `balance' = balance - amount`

**Pre-conditions**
- Vault is initialized.
- Only the owner may withdraw:
  - `meta.owner.require_auth()`
- Amount constraints:
  - `amount > 0`
  - `meta.balance >= amount`

**Post-conditions**
- `VaultMeta.balance' = balance - amount`
- From `meta.balance >= amount` and `amount > 0`:
  - `VaultMeta.balance' >= 0`

---

### `withdraw_to`

**Effect on balance**  
- Decreases `VaultMeta.balance` by `amount`:
  - `balance' = balance - amount`

**Pre-conditions**
- Vault is initialized.
- Only the owner may withdraw:
  - `meta.owner.require_auth()`
- Amount constraints:
  - `amount > 0`
  - `meta.balance >= amount`

**Post-conditions**
- `VaultMeta.balance' = balance - amount`
- From `meta.balance >= amount` and `amount > 0`:
  - `VaultMeta.balance' >= 0`

---

## How Tests Support the Invariant

The test suite in `contracts/vault/src/test.rs` provides practical evidence for the non-negative balance invariant:

- **Deterministic fuzz test** (`fuzz_deposit_and_deduct`):
  - Randomly mixes deposits and deducts, asserting after each step that:
    - `balance() >= 0`
    - `balance()` matches a locally tracked expected value.
- **Batch deduct tests**:
  - `batch_deduct_success`, `batch_deduct_all_succeed`, `batch_deduct_all_revert`, and `batch_deduct_revert_preserves_balance` all verify that:
    - Successful batches leave balance consistent with expectations.
    - Failing batches revert without corrupting balance.
- **Withdraw tests**:
  - `withdraw_owner_success`, `withdraw_exact_balance`, and `withdraw_exceeds_balance_fails` ensure that:
    - Withdrawals are only allowed up to the current balance.
    - Over-withdraw attempts panic before balance can become negative.

Together with the explicit pre-/post-conditions above, these tests help auditors and maintainers validate that **`VaultMeta.balance` is always non-negative** in all reachable states.

