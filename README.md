# Callora Contracts

Soroban smart contracts for the Callora API marketplace: prepaid vault (USDC) and balance deduction for pay-per-call.

## Tech stack

- **Rust** with **Soroban SDK** (Stellar)
- Contract compiles to WebAssembly and deploys to Stellar/Soroban

## What’s included

- **`callora-vault`** contract:
  - `init(owner, usdc_token, initial_balance, min_deposit, revenue_pool, max_deduct)` — initialize vault; optional revenue pool (receives USDC on deduct), optional max single deduct cap
  - `get_meta()`, `get_max_deduct()`, `get_revenue_pool()` — view config
  - `deposit(from, amount)` — user transfers USDC to contract (transfer_from); increases ledger balance; amount must be ≥ min_deposit
  - `deduct(caller, amount, request_id)` — decrease balance; amount ≤ max_deduct; if revenue_pool set, USDC is transferred to it
  - `batch_deduct(caller, items)` — batch deduct with same rules; total USDC transferred to revenue_pool if set
  - `withdraw(amount)` — owner-only; decreases balance and transfers USDC to owner
  - `withdraw_to(to, amount)` — owner-only; decreases balance and transfers USDC to `to`
  - `balance()` — current ledger balance
- **`callora-revenue-pool`** contract (settlement):
  - `init(admin, usdc_token)` — set admin and USDC token
  - `distribute(caller, to, amount)` — admin sends USDC from this contract to a developer
  - Flow: vault deduct → vault transfers USDC to revenue pool → admin calls `distribute(to, amount)`

Events are emitted for init, deposit, deduct, withdraw, and withdraw_to. See [EVENT_SCHEMA.md](EVENT_SCHEMA.md) for indexer/frontend use. Approximate gas/cost notes: [BENCHMARKS.md](BENCHMARKS.md). Upgrade and migration: [UPGRADE.md](UPGRADE.md).

We've enhanced the `CalloraVault` contract with robust input validation to prevent invalid transactions:

- **Amount Validation**: Both `deposit()` and `deduct()` now enforce `amount > 0`, rejecting zero and negative values before any state changes
- **Improved Error Messages**: Enhanced panic messages provide clear context (e.g., "insufficient balance: X requested but only Y available")
- **Early Validation**: Checks occur before storage writes, minimizing gas waste on invalid transactions
- **Comprehensive Test Coverage**: Added 5 new test cases covering edge cases:
  - `deposit_zero_panics()` — validates zero deposit rejection
  - `deposit_negative_panics()` — validates negative deposit rejection
  - `deduct_zero_panics()` — validates zero deduction rejection
  - `deduct_negative_panics()` — validates negative deduction rejection
  - `deduct_exceeds_balance_panics()` — validates insufficient balance checks with detailed error messages

All tests use `#[should_panic]` assertions for guaranteed validation. This resolves issue #9.

## Local setup

1. **Prerequisites:**
   - [Rust](https://rustup.rs/) (stable)
   - [Stellar Soroban CLI](https://developers.stellar.org/docs/smart-contracts/getting-started/setup) (`cargo install soroban-cli`)

2. **Build and test:**

   ```bash
   cd callora-contracts
   cargo build
   cargo test
   ```

3. **Build WASM (for deployment):**

   ```bash
   cd contracts/vault
   cargo build --target wasm32-unknown-unknown --release
   ```

   Or use `soroban contract build` if you use the Soroban CLI workflow.

## Development

Use one branch per issue or feature (e.g. `test/minimum-deposit-rejected`, `docs/vault-gas-notes`) to keep PRs small and reduce merge conflicts. Run `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test` before pushing.

## Project layout

```
callora-contracts/
├── .github/workflows/
│   └── ci.yml              # CI: fmt, clippy, test, WASM build
├── Cargo.toml              # Workspace and release profile
├── BENCHMARKS.md           # Vault operation gas/cost notes
├── EVENT_SCHEMA.md         # Event names, topics, and payload types
├── UPGRADE.md              # Vault upgrade and migration path
├── contracts/
│   ├── vault/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs      # Contract logic
│   │       └── test.rs     # Unit tests
│   └── revenue_pool/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs      # Settlement contract
│           └── test.rs     # Unit tests
└── README.md
```

## Security Notes

- **Checked arithmetic**: All balance mutations use `checked_add` / `checked_sub` — overflow and underflow cause an immediate panic rather than silent wrapping.
- **Input validation**: `deposit` and `deduct` reject zero and negative amounts (`amount > 0`). `init` rejects negative initial balances.
- **`overflow-checks`**: Enabled for **both** `[profile.dev]` and `[profile.release]` in the workspace `Cargo.toml`, ensuring overflow bugs are caught in tests as well as production.
- **Max balance**: `i128::MAX` (≈ 1.7 × 10³⁸ stroops). Deposits that would exceed this limit will panic.

## 🔐 Security

See [SECURITY.md](SECURITY.md) for the Vault Security Checklist and audit recommendations.

## Deployment

Use Soroban CLI or Stellar Laboratory to deploy the built WASM to testnet/mainnet and configure the vault (owner, optional initial balance). The backend will call `deduct` after metering API usage.

This repo is part of [Callora](https://github.com/your-org/callora). Frontend: `callora-frontend`. Backend: `callora-backend`.
