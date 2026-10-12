# Hardening Pass

This revision moves the project from demo-only pointer tricks toward a safer experimental systems design.

## What changed

### Provenance-preserving keyed containers

`KeyedDispatchVec<T, LOW_BITS>` keeps the real `NonNull<T>` inside the collection and uses the IV/key-masked word as a **capability/check word**, not as the only source of dereference. That matters for Rust strict provenance: an integer-looking address should not be treated as the sole authority for recreating a valid pointer.

### Opaque handles

`KeyedDispatchHandle<T, LOW_BITS>` carries:

- slot index
- generation
- masked check word
- selector word
- prophecy bits

The handle is copyable, but it cannot dereference by itself. The collection validates index, generation, selector, check word, and keyed reveal before returning a reference.

### Better key generation

`Key::random()` now uses OS entropy through `getrandom` when available. `KeyMaterial` exists for owned key material that zeroizes its mask on drop. Deterministic `Key::from_seed` remains for tests.


### KeyedArena

`KeyedArena<T, LOW_BITS>` is the generalized hardened storage layer. It keeps stable boxed values, real `NonNull<T>` provenance internally, keyed check words, low tags, selectors, prophecy bits, and generation-checked opaque handles. It supports insertion, removal/reuse, `get`, `get_mut`, metadata updates, explicit generation refresh, live handle enumeration, and dispatch through `OperationTable`.

This is the preferred foundation for future arena/slab/resource policies because it centralizes the handle validation model instead of duplicating it in every collection.

### Resource handles

`ResourceManager` adds generation-checked opaque `ResourceHandle`s for RAM/storage/file/network chunk allocations. This is still a model, not a real OS allocator, but it now has stale-handle detection and keyed check words.

### Tests

New tests exercise:

- keyed dispatch handle validation
- forged check word rejection
- stale generation rejection
- tag refresh invalidating old handles
- resource handle generation checks
- property tests for keyed mask/reveal roundtrips

### Miri/fuzz/bench scaffolding

- `scripts/miri_test.sh` runs Miri over the pointer-heavy crates.
- `fuzz/` includes `cargo-fuzz` targets for selector decoding and TOML operation config parsing.
- `petr-dispatch/benches/dispatch_bench.rs` compares match dispatch, option-table dispatch, and dense selector-table dispatch.

## Important honesty

The dispatch path is **branch-reduced**, not magically branchless. A function pointer call is still an indirect branch. The value is that the runtime avoids a large dynamic `match`/`if` decision tree and routes through compact selector words.

Top-byte tagging remains experimental. The portable core is low-bit alignment tagging plus opaque handles and validation.

## Strict provenance hardening

The primitive pointer wrappers now separate two cases:

- values built from real `NonNull<T>` keep that `NonNull<T>` internally and use it for dereference;
- values built from raw words are address/check-word views and are not treated as proof of dereference validity.

`TaggedNonNull` and `SplitTaggedNonNull` preserve provenance on the normal `new`/`from_mut` paths. `KeyedTaggedNonNull::new` also keeps the original pointer provenance, while `from_stored_word` is intentionally documented as a raw/check-word constructor.

This means the collection layer can remain strict-provenance clean: hardened containers dereference their owned `Box<T>`/`NonNull<T>` storage and use masked words only for validation.
