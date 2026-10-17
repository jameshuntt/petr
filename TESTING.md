# Testing

PETR uses layered validation. The bundle-level commands are the canonical gates
for an ecosystem/release candidate.

## Ordinary workspace tests

```bash
cargo test --workspace
```

These cover pointer/word invariants, collections, resources, temporal windows,
affinity, execution admission, dependency state, authority, math, and spatial
routing.

## Ecosystem evolution

```bash
./TEST_EVOLUTION.command
```

Runs the current PETR/allot integration matrix and confirms the local runtime
composition.

## Hostile verification

```bash
./TEST_VERIFICATION.command
```

This composes four gates:

- `TEST_EVOLUTION.command` — ordinary/integration behavior
- `TEST_LOOM.command` — PETR authority interleavings plus existing scope and
  ob-serve-a-bowl concurrency models
- `TEST_MIRI.command` — focused unsafe/provenance-sensitive tests, including a
  strict-provenance arena check
- `TEST_FUZZ.command` — five cargo-fuzz targets covering selector decoding, TOML
  operations, encoded handles, execution-authority sequences, and resource-handle
  sequences

A successful run ends with `VERIFICATION PASS`. `PETR_FUZZ_SECONDS` controls the
per-target fuzz-smoke duration.

## Release/package gate

```bash
./TEST_RELEASE.command
```

Runs hardening, the PETR feature/target matrix, examples, and package construction
for all publishable PETR crates plus `allot`. It does not publish anything. A
successful run ends with `RELEASE PASS`.

## Final current-scope closeout

```bash
./TEST_CLOSEOUT.command
```

This is the final pre-publication gate. It reruns hostile verification and the
release/package gate, then checks rustfmt, all PETR targets/features, Cargo
metadata, and the architecture dependency boundary. A successful run ends with
`CLOSEOUT PASS`.

`CLOSEOUT PASS` closes PETR's current architectural scope; it does not prohibit
future bug fixes, performance work, portability, hardware PAC/MTE work, stronger
validation, or narrowly required adapters.
