# petr

Tagged pointer words for Rust, and the tooling that grows around them.

The portable core is low-bit tagging: when an allocation is aligned to `2^N`,
the lowest `N` address bits are known to be zero and can carry metadata while
the pointer is stored as a plain word. Everything in this repository follows
one rule: a tagged or keyed word is never dereferenced directly. It is cleaned,
checked and resolved through the owning container first.

Miri clean, including `-Zmiri-strict-provenance`. See `docs/MIRI_GATE_RESULTS.md`.

## Crates

| crate | what it is |
|---|---|
| `petr` | facade re-exporting the crates below as `petr::core`, `petr::dispatch`, ... |
| `petr-core` | low-bit tag layouts, keyed check words, selector words, residual routing, immediate values |
| `petr-dispatch` | selector-driven operation dispatch, TOML configuration, shuffled bytecode maps, continuation-style programs |
| `petr-collections` | keyed arenas, paged keyed arenas, encoded generation-checked handles |
| `petr-resource` | chunk-oriented resource scheduler driven by selector words |
| `petr-math` | matrix and math demos using selector-style active/nop dispatch |
| `petr-spatial` | spatial routing: arithmetic quadrant routing and Morton codes |
| `petr-examples` | runnable examples, one binary per idea (not published) |
| `petr-py-bridge` | optional PyO3 skeleton over `petr-dispatch` (not published) |

```toml
[dependencies]
petr = "0.1"
```

or depend on the individual crates.

## Safety posture

Read `SAFETY.md` and `HARDENING.md`. In short: handles are opaque and carry a
slot index, a generation, a masked check word, a selector and prophecy bits;
the collection validates all of them before returning a reference. Real
`NonNull<T>` provenance stays inside the container. Keys come from OS entropy
and zeroize on drop. Top-byte tagging is experimental and not part of the
portable core.

## Testing

```bash
cargo test --workspace          # unit and invariant tests
scripts/miri_gate.sh            # Miri, default and strict-provenance passes (nightly)
scripts/fuzz_smoke.sh           # cargo-fuzz, ten seconds per target (nightly)
```

`TESTING.md` has the details and the last recorded numbers.

## License

MIT or Apache-2.0, at your option.
