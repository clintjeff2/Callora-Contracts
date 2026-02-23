# Callora Contracts

Soroban smart contracts for the Callora API marketplace: prepaid vault (USDC) and balance deduction for pay-per-call.

## Tech stack

- **Rust** with **Soroban SDK** (Stellar)
- Contract compiles to WebAssembly and deploys to Stellar/Soroban

## What’s included

- **`callora-vault`** contract:
  - `init(owner, initial_balance)` — initialize vault for an owner
  - `get_meta()` — owner and current balance
  - `deposit(amount)` — increase balance
  - `deduct(amount)` — decrease balance (e.g. per API call)
  - `balance()` — current balance

Production use would add: USDC asset, auth (only backend or owner can deduct), and events.

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

## Project layout

```
callora-contracts/
├── Cargo.toml              # Workspace and release profile
├── contracts/
│   └── vault/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs      # Contract logic
│           └── test.rs     # Unit tests
└── README.md
```

## Deployment

Use Soroban CLI or Stellar Laboratory to deploy the built WASM to testnet/mainnet and configure the vault (owner, optional initial balance). The backend will call `deduct` after metering API usage.

This repo is part of [Callora](https://github.com/your-org/callora). Frontend: `callora-frontend`. Backend: `callora-backend`.
