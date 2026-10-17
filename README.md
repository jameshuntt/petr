# petr

Tagged pointer words for Rust, and the independently usable systems primitives that grow around them.

The portable core is low-bit tagging: when an allocation is aligned to `2^N`,
the lowest `N` address bits are known to be zero and can carry metadata while
the pointer is stored as a plain word. Everything in this repository follows
one rule: a tagged or keyed word is never dereferenced directly. It is cleaned,
checked and resolved through the owning container first.

Miri clean, including `-Zmiri-strict-provenance`. See `docs/MIRI_GATE_RESULTS.md`.

## Crates

| crate | what it is |
|---|---|
| `petr` | facade re-exporting the crates below as `petr::core`, `petr::dispatch`, `petr::temporal`, `petr::affinity`, `petr::execution`, ... |
| `petr-core` | low-bit tag layouts, keyed check words, selector words, residual routing, immediate values |
| `petr-dispatch` | selector-driven operation dispatch, TOML configuration, shuffled bytecode maps, continuation-style programs |
| `petr-collections` | keyed arenas, paged keyed arenas, encoded generation-checked handles |
| `petr-resource` | chunk-oriented resource scheduler driven by selector words |
| `petr-protection` | portable protection requirements/capabilities shared by execution, hardware and memory bridges |
| `petr-temporal` | temporal windows, monotonic phase clocks, cyclic execution and temporal pointer authority |
| `petr-affinity` | deterministic CPU-set placement with optional OS thread pinning |
| `petr-execution` | optional composition layer: lifetime-bound handlers, dependencies, profile transitions, temporal/affinity/resource admission and `scope` |
| `petr-hw` | experimental PAC/MTE adapters plus backend capability discovery; never part of the default pointer representation |
| `petr-math` | matrix and math demos using selector-style active/nop dispatch |
| `petr-spatial` | spatial routing: arithmetic quadrant routing and Morton codes |
| `petr-examples` | runnable examples, one binary per idea (not published) |
| `petr-py-bridge` | optional PyO3 skeleton over `petr-dispatch` (not published) |

```toml
[dependencies]
petr = "0.1"
```

or depend on the individual crates.

## Execution composition

`petr-execution` is deliberately optional. `petr-dispatch`, `petr-temporal`,
`petr-affinity`, `petr-resource` and `scope` remain independently usable. The
execution crate adds multi-part dependencies, logical-tick profile transitions
and lifetime-bound handler leases. Optional features bridge to existing systems
without taking them over:

- `dispatch`: gate an existing PETR dispatch by a temporal window.
- `waker-protocol`: translate handler QoS into `waker_protocol::TaskPolicy`.
- `observable`: encode execution eligibility/profile events as `ob-serve-a-bowl` wake keys.
- `core-affinity`: enable actual current-thread pinning through `petr-affinity`.
- capability-aware admission can require `petr-protection` authentication and/or memory-tagging semantics without forcing a concrete hardware backend.

The execution contract is logical time. Wall-clock phase clocks may produce
logical ticks, but plan dependencies and transitions are expressed in ticks so
the same plan can be simulated and replayed deterministically.

The ownership and dependency rules are spelled out in [`ARCHITECTURE.md`](ARCHITECTURE.md).
The important rule is that composition stays optional: dispatch remains dispatch,
affinity remains affinity, temporal policy remains temporal policy, and
`petr-execution` is the layer that chooses to combine them.

### Feature matrix

| Feature | Enables |
|---|---|
| `serde` | serialization support in core/collections |
| `core-affinity` | OS current-thread pinning through `petr-affinity` |
| `execution-adapters` | dispatch, waker-protocol, and observable bridges |
| `hardware-experiments` | re-export `petr-hw` PAC/MTE experiments from the facade |

For direct `petr-execution` use, the adapter features can also be enabled
individually as `dispatch`, `waker-protocol`, or `observable`.

```toml
[dependencies]
petr = { version = "0.1", features = ["execution-adapters"] }
```


## Protected memory bridge

The ecosystem bundle also contains the standalone `petr-allot` bridge. It is
not required by PETR or by `allot`: applications opt into it when a tracked
`allot` allocation should carry the same protection contract as an execution
profile. The bridge provides RAII reclamation, generation rotation, software
PAC/MTE validation on portable hosts, and an explicit refusal to treat ordinary
heap memory as a native MTE mapping.

## Safety posture

Read `SAFETY.md` and `HARDENING.md`. In short: handles are opaque and carry a
slot index, a generation, a masked check word, a selector and prophecy bits;
the collection validates all of them before returning a reference. Real
`NonNull<T>` provenance stays inside the container. Keys come from OS entropy
and zeroize on drop. Top-byte tagging is experimental and not part of the
portable core. `petr-hw` keeps software top-byte metadata separate from PAC/MTE-owned
architectural bits and returns preserved clean-pointer provenance only after checks.

## Testing

```bash
cargo test --workspace          # unit and invariant tests
```

For the integrated ecosystem candidate, the canonical gates are:

```bash
./TEST_VERIFICATION.command      # evolution + Loom + Miri + five fuzz targets
./TEST_RELEASE.command           # feature/target matrix + package construction
./TEST_CLOSEOUT.command          # final current-scope pre-publication gate
```

`TESTING.md` has the full breakdown. These gates do not publish anything.

## License

MIT or Apache-2.0, at your option.
