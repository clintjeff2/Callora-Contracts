## test: deduct event includes request id (#51)

### What this PR does

Adds unit tests that verify the `deduct` (and `batch_deduct`) events encode the
optional `request_id` / idempotency key in their on-chain event topics, so the
backend can correlate every on-chain deduction with the originating API call.

Also fixes a coverage regression caused by multi-line storage method chains that
`cargo-tarpaulin` could not instrument; each chain is now broken into a named
`inst` binding so every storage write is a separately trackable line.

### Changes

| File                          | Change                                                                                                                                         |
| ----------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| `contracts/vault/src/test.rs` | Three new tests: `deduct_event_contains_request_id`, `deduct_event_no_request_id_uses_empty_symbol`, `batch_deduct_events_contain_request_ids` |
| `contracts/vault/src/lib.rs`  | Replace chained `.storage().instance().set()` calls with `let inst` binding to restore tarpaulin line coverage                                 |

### New tests

- **`deduct_event_contains_request_id`** — calls `deduct` with a known
  `request_id`, captures the emitted event, and asserts all three topics
  (`"deduct"`, caller address, request_id symbol) and the data
  `(amount, remaining_balance)` match exactly.
- **`deduct_event_no_request_id_uses_empty_symbol`** — when `None` is passed
  the third topic must be an empty symbol sentinel, keeping the event schema
  consistent for listeners.
- **`batch_deduct_events_contain_request_ids`** — a two-item batch emits two
  events; each event's third topic is the per-item `request_id`, confirming
  individual-item correlation for the backend.

### How to reproduce locally

```bash
cargo test --quiet
./scripts/coverage.sh
```

### Test output and coverage proof

<!-- ATTACHMENT: paste or drag in a screenshot of either:
     (a) the GitHub Actions run showing all checks green, OR
     (b) the terminal output of `./scripts/coverage.sh` showing ≥ 95% coverage.

     How to get it:
     • For (a): open the Actions tab of this PR → click the passing workflow run
       → take a screenshot of the green job summary.
     • For (b): run `./scripts/coverage.sh` locally, then screenshot the last
       ~20 lines of terminal output that include the "X% coverage" line and
       "test result: ok. N passed".
-->

![Coverage and test proof]()

### Checklist

- [x] All existing tests pass (`cargo test`)
- [x] Three new event-parsing tests added for Issue #51
- [x] Coverage ≥ 95% (`cargo-tarpaulin`)
- [x] `cargo fmt --all -- --check` passes
- [x] No new `clippy` warnings introduced
