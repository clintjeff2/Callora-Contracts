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

## Project layout

```
callora-contracts/
├── Cargo.toml                        # Workspace and release profile
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
```

## Deployment

Use Soroban CLI or Stellar Laboratory to deploy the built WASM to testnet/mainnet and configure the vault (owner, optional initial balance). The backend will call `deduct` after metering API usage.

This repo is part of [Callora](https://github.com/your-org/callora). Frontend: `callora-frontend`. Backend: `callora-backend`.
