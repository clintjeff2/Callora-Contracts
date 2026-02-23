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
  - `batch_deduct(items)` — multiple deducts in one transaction (reverts entire batch if any would exceed balance)
  - `withdraw(amount)` — owner-only; decreases balance (USDC transfer when integrated)
  - `withdraw_to(to, amount)` — owner-only; withdraw to a designated address
  - `balance()` — current balance

Events are emitted for init, deposit, deduct, withdraw, and withdraw_to. See [EVENT_SCHEMA.md](EVENT_SCHEMA.md) for indexer/frontend use.

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
├── .github/workflows/
│   └── ci.yml              # CI: fmt, clippy, test, WASM build
├── Cargo.toml              # Workspace and release profile
├── EVENT_SCHEMA.md         # Event names, topics, and payload types
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
