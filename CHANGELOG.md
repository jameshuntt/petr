# Changelog

## 0.2.0 (2026-10-16)

The V13.1 source. Every PETR crate shares the workspace version, so every manifest says 0.2.0.

- Published from tag `v0.2.0`: `petr-core`, `petr-dispatch`, `petr-collections`, `petr-resource`,
  `petr-math` and `petr-spatial` (0.1.0 before), and the first releases of `petr-protection`,
  `petr-temporal`, `petr-affinity` and `petr-hw`.
- Not published in 0.2.0: `petr-execution` and the `petr` facade are held (the facade depends on
  `petr-execution`, so crates.io keeps `petr` 0.1.0). `petr-py-bridge` and `petr-examples` stay
  `publish = false`.
- CI: `.github/workflows/ci.yml` runs the V13.1 gates (tests, hardening, all-target/all-feature
  checks, rustfmt, the boundary check, package build, Loom, Miri, fuzz) and publishes on a `v*` tag.

### Added

- `petr-temporal` for logical ticks, windows, cyclic execution, skipped-slot
  policy, generation expiry, and temporal pointer authority.
- `petr-affinity` for deterministic CPU placement and optional OS thread pinning.
- `petr-execution` as an optional composition layer over temporal eligibility,
  affinity, resource admission, dependencies, profile transitions, and `scope`.
- Optional execution adapters for PETR dispatch, `waker_protocol`, and
  `ob-serve-a-bowl`.
- Execution hardening for rollback, cancellation/panic reclamation, dependency
  fan-in, deterministic replay, priority preservation, and CPU collision stress.
- Architecture and release documentation for the expanded crate graph.
- Per-handler `ExecutionAuthority` lifecycle tokens for opt-in downstream capabilities, with explicit completion/cancellation and plan-drop revocation.
- V11 boundary stabilization: per-admission `AdmissionEpoch` semantics, opaque stable `EffectIdentity` metadata, and explicit `revoke_handler` distinct from completion/cancellation.

### Preserved

- Existing `petr-dispatch` remains usable without temporal scheduling.
- Existing core/collection pointer provenance and handle-validation contracts are
  unchanged by the execution additions.
- Affinity, execution adapters, and serialization remain opt-in features.
