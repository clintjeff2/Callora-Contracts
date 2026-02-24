# Vault Operation Gas / Cost Notes

Approximate resource usage for Callora Vault operations to guide integration and capacity planning. Soroban uses resource metering (CPU instructions, ledger reads/writes, events). Exact numbers depend on network fee configuration and should be validated on testnet or via `soroban contract invoke` simulation.

## Relative Cost (typical order)

| Operation    | Relative cost | Notes |
|-------------|---------------|--------|
| `balance()` | Lowest        | Single instance read, no writes, no event. |
| `get_meta()`| Low           | Same as balance (reads full meta). |
| `deposit`   | Medium        | One read, one write, one event. |
| `deduct`    | Medium        | One read, one write, one event. |
| `withdraw`  | Medium        | One read, one write, one event. |
| `withdraw_to` | Medium      | One read, one write, one event. |
| `batch_deduct` | Medium–High | One read, one write, N events (one per item). |
| `init`      | Highest       | First write (create instance), one event; requires auth. |

## Obtaining Exact Numbers

- **Testnet**: Deploy the vault and invoke each operation; inspect transaction meta for instructions and fee.
- **CLI**: Use `soroban contract invoke` with `--simulate` (or equivalent) and check returned resource/fee info.
- **Test env**: Run the optional benchmark test: `cargo test --ignored vault_operation_costs -- --nocapture`. This logs CPU/instruction and fee estimates per operation when invocation cost metering is enabled in the test environment.

## Fee Configuration

Soroban fees are configured per network (e.g. Pubnet). They are applied to:

- CPU instructions (per increment)
- Ledger entry reads and writes
- Event size
- Transaction size
- Rent for persistent/temporary storage

See [Stellar documentation](https://developers.stellar.org/docs) for current fee parameters.
