# Callora Contracts

Soroban smart contracts for the Callora API marketplace: prepaid vault (USDC) and balance deduction for pay-per-call.

## Tech stack

- **Rust** with **Soroban SDK** (Stellar)
- Contract compiles to WebAssembly and deploys to Stellar/Soroban

## What’s included

- **`callora-vault`** contract:
  - `init(owner, usdc_token, initial_balance, min_deposit)` — initialize vault for an owner; optional minimum deposit (0 = none)
  - `get_meta()` — owner, current balance, and min_deposit
  - `deposit(from, amount)` — increase balance (supports multiple authorized depositors; panics if amount < min_deposit)
  - `deduct(caller, amount, request_id)` — decrease balance (e.g. per API call)
  - `batch_deduct(caller, items)` — multiple deducts in one transaction (reverts entire batch if any would exceed balance)
  - `withdraw(amount)` — owner-only; decreases balance (USDC transfer when integrated)
  - `withdraw_to(to, amount)` — owner-only; withdraw to a designated address
  - `balance()` — current balance

Events are emitted for init, deposit, deduct, withdraw, and withdraw_to. See [EVENT_SCHEMA.md](EVENT_SCHEMA.md) for indexer/frontend use. Approximate gas/cost notes: [BENCHMARKS.md](BENCHMARKS.md). Upgrade and migration: [UPGRADE.md](UPGRADE.md).

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
│   └── vault/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs      # Contract logic
│           └── test.rs     # Unit tests
└── README.md
```

## Security Notes

- **Checked arithmetic**: All balance mutations use `checked_add` / `checked_sub` — overflow and underflow cause an immediate panic rather than silent wrapping.
- **Input validation**: `deposit` and `deduct` reject zero and negative amounts (`amount > 0`). `init` rejects negative initial balances.
- **`overflow-checks`**: Enabled for **both** `[profile.dev]` and `[profile.release]` in the workspace `Cargo.toml`, ensuring overflow bugs are caught in tests as well as production.
- **Max balance**: `i128::MAX` (≈ 1.7 × 10³⁸ stroops). Deposits that would exceed this limit will panic.

## Deployment

Use Soroban CLI or Stellar Laboratory to deploy the built WASM to testnet/mainnet and configure the vault (owner, optional initial balance). The backend will call `deduct` after metering API usage.

This repo is part of [Callora](https://github.com/your-org/callora). Frontend: `callora-frontend`. Backend: `callora-backend`.
