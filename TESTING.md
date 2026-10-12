# Testing

Three gates. Run them from the repository root.

## Unit and invariant tests

```bash
cargo test --workspace
```

Last recorded: 78 passed, 0 failed. The invariant tests live in each crate's
`tests/` directory: `core_invariants`, `dispatch_invariants`,
`typed_dispatch_invariants`, `collection_invariants`, `paged_arena_invariants`,
`resource_scheduler_invariants`, `math_invariants`, `spatial_invariants`.

## Miri gate

```bash
scripts/miri_gate.sh
```

Runs every crate under Miri twice: default, then `-Zmiri-strict-provenance`,
which is the pass that matters for pointer and tag code. Needs the nightly
toolchain with the `miri` component; the script installs it. Logs go to
`$MIRI_GATE_LOGDIR` (default `/tmp/miri-gate-logs`). The narrower
`scripts/miri_test.sh` covers only `petr-core` and `petr-collections`.

Last recorded result is in `docs/MIRI_GATE_RESULTS.md`.

Timings printed inside Miri runs come from Miri's simulated clock, not wall
time. The whole gate takes a few minutes of real time on an Apple Silicon Mac.

## Fuzz smoke

```bash
cargo install cargo-fuzz
scripts/fuzz_smoke.sh
```

Three libFuzzer targets in `fuzz/fuzz_targets`: `selector_word_decode`,
`toml_operation_config`, `encoded_handle_decode`. Ten seconds each. Crashes
land in `fuzz/artifacts/`. Set `PROPTEST_DISABLE_FAILURE_PERSISTENCE=1` if you
run proptest-backed tests under Miri by hand.

## Benchmarks

```bash
cargo bench -p petr-dispatch
```

Compares match dispatch, option-table dispatch and dense selector-table
dispatch.
