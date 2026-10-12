# Publishing to crates.io

Seven crates publish. `petr-examples` and `petr-py-bridge` are `publish = false`.

Order matters: each crate must be on crates.io before anything that depends on it.
Wait for the index to pick up each one (usually under a minute) before the next.

```bash
cargo publish -p petr-core
cargo publish -p petr-dispatch
cargo publish -p petr-collections
cargo publish -p petr-resource
cargo publish -p petr-math
cargo publish -p petr-spatial
cargo publish -p petr
```

Before publishing, the whole set was checked with
`cargo package --no-verify --workspace --exclude petr-examples --exclude petr-py-bridge`
and `cargo publish --dry-run -p petr-core`. Dependents cannot be dry-run until
their dependencies exist on crates.io; that is a crates.io rule, not a defect.

Versions are set once in the workspace `Cargo.toml` (`workspace.package.version`).
Bump there, and the `version = "0.1.0"` on each internal path dependency, together.
