## test: large balance and large deduct

### Summary

Adds unit tests that exercise vault arithmetic at the boundaries of `i128` to ensure no overflow or underflow occurs with large numbers.

### Changes

- **`contracts/vault/src/test.rs`** — 7 new test cases under the _"large balance and large deduct"_ section:

| Test                                   | What it covers                                                               |
| -------------------------------------- | ---------------------------------------------------------------------------- |
| `large_balance_init_and_deduct`        | Init with `i128::MAX / 2`, deduct `i128::MAX / 4`, assert remaining          |
| `large_balance_deduct_entire_balance`  | Init with `i128::MAX`, deduct full amount, assert zero                       |
| `large_balance_sequential_deducts`     | Init with 1e18, two sequential deducts draining to zero                      |
| `large_batch_deduct_correctness`       | Batch deduct three equal large chunks, verify remainder                      |
| `deposit_overflow_panics`              | Init near `i128::MAX`, attempt deposit that would overflow — expects failure |
| `large_deduct_exceeding_balance_fails` | Deduct more than a large balance — expects failure, balance unchanged        |

### How it was tested

All tests pass locally:

```
cargo test --package callora-vault
```

<!-- PASTE TEST OUTPUT / BUILD SCREENSHOT BELOW -->

![Proof of successful build](<!-- REPLACE WITH YOUR IMAGE URL OR PATH -->)

### Checklist

- [x] Tests pass with `cargo test`
- [x] No new warnings
- [x] Covers overflow and underflow edge cases
- [x] Follows existing test style and conventions

Closes #32
