# Publishing PETR to crates.io

Twelve crates in this workspace are publishable. `petr-examples` and
`petr-py-bridge` are intentionally `publish = false`.

Publishing is dependency-ordered because crates.io must know every internal
version before a dependent crate can be published.

## Dependency order

1. `petr-core`
2. `petr-protection`
3. `petr-dispatch`
4. `petr-collections`
5. `petr-resource`
6. `petr-temporal`
7. `petr-affinity`
8. `petr-math`
9. `petr-spatial`
10. `petr-hw`
11. `petr-execution`
12. `petr`

`petr-protection` is intentionally small and portable. `petr-hw` follows both
protection and core. `petr-execution` follows protection, temporal, affinity,
resource, and dispatch. The facade is last because it depends on every
publishable PETR crate. The standalone `petr-allot` bridge is not part of this
release sequence yet (`publish = false`).

## Before publishing

From the ecosystem bundle, run:

```bash
./TEST_RELEASE.command
```

That gate includes V3 hardening plus package construction for every publishable
PETR crate. It does not publish anything.

The package-only commands are equivalent to:

```bash
cargo package --manifest-path petr/Cargo.toml --workspace --exclude petr-py-bridge --exclude petr-examples --allow-dirty --no-verify --registry crates-io
```

`--no-verify` is used for the pre-publication package pass because verification
of a dependent package resolves its internal PETR dependencies through the
registry. Before those versions are published, that registry resolution cannot
succeed. After each dependency is live on crates.io, normal `cargo publish`
performs the registry-backed verification.

## Publish

Wait for the crates.io index to expose each crate/version before publishing the
next dependent crate:

```bash
cargo publish -p petr-core
cargo publish -p petr-protection
cargo publish -p petr-dispatch
cargo publish -p petr-collections
cargo publish -p petr-resource
cargo publish -p petr-temporal
cargo publish -p petr-affinity
cargo publish -p petr-math
cargo publish -p petr-spatial
cargo publish -p petr-hw
cargo publish -p petr-execution
cargo publish -p petr
```

## Versioning

The workspace version is declared once in `petr/Cargo.toml` under
`workspace.package.version`. Internal path dependencies also contain an explicit
registry version, currently `0.1.0`. When the release version changes, update
both the workspace version and every internal `version = "..."` requirement in
the same change.

Do not publish merely because the release gate passes. The gate establishes
build/package readiness; choosing a public version and performing the actual
publication remain explicit release actions.

## External adapters

`petr-execution` uses published `scope`, `waker_protocol`, and
`ob-serve-a-bowl` versions for its optional adapters. The ecosystem integration
harness additionally patches those dependencies to the supplied local source
repositories so their current implementations are tested together. The local
patch is a validation configuration, not a requirement that PETR own or publish
those projects.
