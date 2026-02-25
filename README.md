# Callora Contracts

Soroban smart contracts for the Callora API marketplace: prepaid vault (USDC) and balance deduction for pay-per-call.

## Tech stack

- **Rust** with **Soroban SDK** (Stellar)
- Contract compiles to WebAssembly and deploys to Stellar/Soroban

## What’s included

- **`callora-vault`** contract:
  - `init(owner, initial_balance)` — initialize vault for an owner with optional initial balance
  - `get_meta()` — view config (owner and current balance)
  - `set_allowed_depositor(caller, depositor)` — owner-only; set or clear a backend/allowed depositor that can deposit and manage pricing
  - `deposit(caller, amount)` — owner or allowed depositor increases ledger balance
  - `deduct(amount)` — decrease balance for an API call (backend uses this after metering usage)
  - `balance()` — current ledger balance
  - `set_price(caller, api_id, price)` — owner or allowed depositor sets the **price per API call** for `api_id` in smallest USDC units (e.g. 1 = 1 cent)
  - `get_price(api_id)` — returns `Option<i128>` with the configured price per call for `api_id`

### API pricing resolution

The backend resolves `(vault_id, api_id) -> price` as follows:

1. Use the vault contract address as `vault_id`.
2. Call `get_price(api_id)` on that vault.
3. If a price is returned, use it as the per-call price (in smallest USDC units) before calling `deduct(amount)`.

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

## Deployment

Use Soroban CLI or Stellar Laboratory to deploy the built WASM to testnet/mainnet and configure the vault (owner, optional initial balance and optional pricing). The backend will:

1. Call `get_price(api_id)` on the vault (`vault_id`) to fetch the price per call.
2. Multiply by the number of billable calls to get the total amount.
3. Call `deduct(amount)` on the same vault to charge the user.

This repo is part of [Callora](https://github.com/your-org/callora). Frontend: `callora-frontend`. Backend: `callora-backend`.
