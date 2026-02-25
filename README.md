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
<<<<<<< HEAD

## Test coverage

The project enforces a **minimum of 95 % line coverage** on every push and pull-request via GitHub Actions.

### Run coverage locally

```bash
# First time only — the script auto-installs cargo-tarpaulin if absent
./scripts/coverage.sh
```

The script will:

1. Check for `cargo-tarpaulin`; install it automatically if it is missing.
2. Run all tests with instrumentation according to `tarpaulin.toml`.
3. Exit with a non-zero code if coverage drops below 95 %.
4. Write reports to the `coverage/` directory (git-ignored).

| Report file                      | Description                                     |
| -------------------------------- | ----------------------------------------------- |
| `coverage/tarpaulin-report.html` | Interactive per-file view — open in any browser |
| `coverage/cobertura.xml`         | Cobertura XML consumed by CI                    |

> **Tip:** You can also run `cargo tarpaulin` directly from the workspace root;
> the settings in `tarpaulin.toml` are picked up automatically.

### CI enforcement

`.github/workflows/coverage.yml` runs on every push and pull-request.
It installs tarpaulin, runs coverage, uploads the HTML report as a downloadable
artefact, and posts a coverage summary table as a PR comment.
A result below 95 % causes the workflow — and the required status check — to fail.
=======
>>>>>>> b0229e42e4d4517da9f548ea3e374a5886304bf2

## Project layout

```
callora-contracts/
├── .github/workflows/
│   └── ci.yml              # CI: fmt, clippy, test, WASM build
<<<<<<< HEAD
├── Cargo.toml                        # Workspace and release profile
├── BENCHMARKS.md           # Vault operation gas/cost notes
├── EVENT_SCHEMA.md         # Event names, topics, and payload types
├── UPGRADE.md              # Vault upgrade and migration path
├── tarpaulin.toml                    # cargo-tarpaulin config (≥ 95 % enforced)
├── scripts/
│   └── coverage.sh                   # One-command local coverage runner
├── .github/
│   └── workflows/
│       └── coverage.yml              # CI: enforces 95 % on every push / PR
└── contracts/
    └── vault/
        ├── Cargo.toml
        └── src/
            ├── lib.rs                # Contract logic
            └── test.rs               # Unit tests (covers all code paths)
=======
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
>>>>>>> b0229e42e4d4517da9f548ea3e374a5886304bf2
```

## Security Notes

- **Checked arithmetic**: All balance mutations use `checked_add` / `checked_sub` — overflow and underflow cause an immediate panic rather than silent wrapping.
- **Input validation**: `deposit` and `deduct` reject zero and negative amounts (`amount > 0`). `init` rejects negative initial balances.
- **`overflow-checks`**: Enabled for **both** `[profile.dev]` and `[profile.release]` in the workspace `Cargo.toml`, ensuring overflow bugs are caught in tests as well as production.
- **Max balance**: `i128::MAX` (≈ 1.7 × 10³⁸ stroops). Deposits that would exceed this limit will panic.

## Deployment

Use Soroban CLI or Stellar Laboratory to deploy the built WASM to testnet/mainnet and configure the vault (owner, optional initial balance). The backend will call `deduct` after metering API usage.

This repo is part of [Callora](https://github.com/your-org/callora). Frontend: `callora-frontend`. Backend: `callora-backend`.
