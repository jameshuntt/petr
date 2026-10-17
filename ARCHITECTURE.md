# PETR architecture

PETR is a set of independent systems primitives with an optional composition
layer. The design rule is that a higher-level policy may *use* a lower-level
primitive, but it should not absorb that primitive's standalone API.

## Dependency shape

```text
                              +------------------+
                              |       petr       |
                              |      facade      |
                              +--------+---------+
                                       |
         +-----------------------------+-----------------------------+
         |            |            |          |          |           |
    petr-core   petr-dispatch  collections  resource  protection  math/spatial
         |            |                                  |
         +---- temporal                                   +-------> petr-hw
         |                                                |
         +---- affinity                                   |
              \                                           |
               +------------------+------------------------+
                                  |
                         +--------v---------+
                         |  petr-execution  |
                         |   composition    |
                         +--------+---------+
                                  |
                                scope

    allot -----------------> petr-allot <---------------- petr-hw/protection
     owner                    optional bridge
```

The diagram shows conceptual ownership, not a requirement that every application
use every crate. `petr-execution` can be used without its adapter features.

`allot` sits beside PETR rather than below it. V6 adds `petr-allot` as an
explicit bridge so neither owner acquires a mandatory dependency on the other.

## Ownership rules

### `petr-core`

Owns the portable pointer/word representation rules: low-bit tagging, clean
words, keyed masking/checks, selectors, prophecy bits, residual classes,
immediate encodings, split tags, and atomic tagged words. Dirty/tagged words
are not dereferenced directly.

### `petr-dispatch`

Owns selector-driven dispatch. It remains independently usable. Temporal
eligibility is an optional gate supplied by `petr-execution`; it is not a
mandatory property of dispatch itself.

### `petr-collections`

Owns containers that understand PETR handles: keyed arenas, paged arenas,
encoded generation-checked handles, and tagged/keyed vectors. Containers retain
real pointer provenance and validate handles before access.

### `petr-resource`

Owns abstract resource admission and generation-checked resource handles. It
has no knowledge of wall-clock time, task runtimes, or OS CPU affinity.

### `petr-protection`

Owns the portable vocabulary for protection requirements and backend
capabilities. It performs no pointer operations and owns no allocator. Execution
profiles can state requirements in this vocabulary, while `petr-hw` and memory
bridges report whether they can satisfy them.

### `petr-temporal`

Owns logical temporal eligibility: ticks, windows, phase clocks, skipped-slot
policies, cyclic execution, generation expiry, and temporal pointer authority.
Logical time is the replayable contract; a physical clock may produce ticks but
does not become the scheduling semantics.

### `petr-affinity`

Owns deterministic CPU-set placement. Actual OS thread pinning is behind the
`core-affinity` feature so applications that only want placement policy do not
acquire a platform affinity dependency.

### `petr-hw`

Owns opt-in hardware experiments. PAC and MTE never redefine PETR's portable pointer word. Native PAC signatures are treated as check tokens while PETR retains the clean pointer for provenance. MTE has a portable tag/generation model plus Linux/AArch64 instruction helpers. The logical top byte in `SplitTaggedNonNull` is software metadata and is stripped before entering either hardware path.

### `petr-execution`

Owns *local execution composition*, not a replacement runtime. It combines
logical eligibility, CPU placement, abstract resource admission, dependency
state, profile transitions, optional protection contracts, per-admission
identity, per-handler runtime authority, and `scope`-bound handler lifetimes.
Logical ticks remain replayable scheduling input; a caller-supplied
`AdmissionEpoch` names one concrete admission attempt without changing
placement. An optional opaque `EffectIdentity` can remain stable across those
attempts without making PETR the owner of receipts or idempotency policy.

`Completed`, `Cancelled`, and `Revoked` are distinct terminal meanings.
Revocation ends only the current admission, so higher layers may defer, migrate,
or later re-admit work without PETR learning why.

PETR does not own retry/resurrection policy, checkpoints, persistence,
hot/warm/cold residency, SLA/deadline interpretation, backpressure, node/cell
placement, service modes, crash recovery, or migration policy. Optional
adapters bridge to existing dispatch, wake-protocol, observable, and future
higher-layer owners without moving those policies into PETR.

### `scope`, `waker_tables`, `waker_protocol`, `ob-serve-a-bowl`

These remain separate owners of structured task lifetime, raw waker machinery,
wake/QoS protocol, and observable event delivery. PETR does not duplicate those
systems.

### `allot`

Owns concrete memory domains, allocation accounting, raw buffers, arenas,
reusable pages, trace events, and optional paging value types. It does not own
execution policy.

### `petr-allot` bridge

Lives outside both owner crates as an opt-in adapter. It combines an
`allot::MemoryOrchestrator` allocation with a `petr-protection` contract and
`petr-hw` proofs. This keeps concrete allocation policy out of PETR and keeps
hardware/pointer policy out of `allot`. With the optional execution bridge,
protected allocations consume `petr-execution::ExecutionContext` and bind
handler id, logical tick, and admission epoch into the authentication context.
Native MTE remains unavailable here until the allocation comes from a real
MTE-capable mapping.

### `recipe`, `assurance`, `classified`

They remain independent. `recipe` currently describes deterministic data/schema
composition rather than PETR execution plans; `assurance` is an attestation
boundary; `classified` owns secret-bearing containers. PETR exposes inspection
and construction seams instead of taking direct dependencies on them.

## Optional feature matrix

| Feature | Crate | Adds | Does not replace |
|---|---|---|---|
| `dispatch` | `petr-execution` | temporal gate around PETR dispatch | `petr-dispatch` |
| `waker-protocol` | `petr-execution` | QoS/profile translation | `waker_protocol` scheduler |
| `observable` | `petr-execution` | execution wake-key encoding | `ob-serve-a-bowl` runtime |
| `adapters` | `petr-execution` | all three bridges above | any owner |
| `core-affinity` | `petr-affinity`, `petr-execution`, facade | current-thread OS pinning | logical placement |
| `serde` | facade/core/collections | serialization support | core representation |
| `hardware-experiments` | facade | `petr-hw` re-export | portable PETR representation |
| `aarch64-pac` | `petr-hw` | native PACIA/AUTIA token checks | software PAC model |
| `aarch64-mte` | `petr-hw` | Linux/AArch64 MTE instruction helpers | software MTE model |

## Determinism boundary

PETR promises deterministic *logical admission* for the same immutable plan,
state, tick, resource state, and CPU set. It does not claim deterministic OS
thread scheduling or hard real-time deadlines. Those are deliberately outside
the portable core.
