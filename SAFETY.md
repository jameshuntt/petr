# Safety contract

This workspace is an experimental capability/dispatch-handle system. The safe
parts are built around one rule: **word metadata is validation data, not pointer
authority**.

## Core rules

- Raw words are not safe pointers.
- Masked/keyed words are verification/capability checks, not dereference authority.
- Handles are valid only inside the arena/collection that created them.
- The arena keeps provenance-carrying pointers internally and validates handles before access.
- Generation changes invalidate old handles.
- Selector and prophecy metadata are part of validation when they are carried by a handle.
- Low-bit tags require allocation alignment of at least `2^LOW_BITS`.
- Top-byte tagging is experimental and should remain behind explicit features.
- Residual/modulo routing is a validation/classification layer, not hardware alignment.
- Selector-table dispatch is table-driven / branch-reduced; it is not literal branchless CPU execution.

## Serialization rule

`EncodedArenaHandle` intentionally serializes only index/generation/check/selector/prophecy/tag metadata.
It never serializes a raw address. A decoded handle must be validated by the owning arena before use.

## Miri posture

The pointer-heavy crates are expected to pass Miri, including strict provenance mode:

```bash
MIRIFLAGS="-Zmiri-strict-provenance" cargo +nightly miri test -p petr_core
MIRIFLAGS="-Zmiri-strict-provenance" cargo +nightly miri test -p petr_collections
```

If those fail, treat the failure as a hard correctness issue until proven otherwise.

## Step 67I paged keyed arena

`PagedKeyedArena` never reconstructs or dereferences a pointer from the masked integer check word. Live access comes from ordinary references into stable boxed pages. The keyed word is used only to validate that the revealed integer address equals the address of the currently borrowed in-page value.

Pages are individually boxed before entries are installed. Growth of the outer `Vec<Box<Page<_>>>` moves box handles but does not move page allocations. Slot removal and reuse refreshes the generation. Tag, selector, or prophecy transitions return a new handle and invalidate the previous metadata envelope.

The division operator requires `DivisionRecord` to be 16-byte aligned because four low bits are reserved. Insertion fails through tag validation if a generic paged-arena value does not satisfy its requested low-bit alignment.
