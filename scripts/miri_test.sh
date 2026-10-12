#!/usr/bin/env bash
set -euo pipefail

# Miri is normally a nightly component. Keep the project itself stable,
# but run the memory-model checks under nightly.
rustup install nightly >/dev/null
rustup +nightly component add miri >/dev/null
cargo +nightly miri setup >/dev/null

# Keep proptest from trying to write/read persistence files under Miri.
export PROPTEST_DISABLE_FAILURE_PERSISTENCE=1

cargo +nightly miri test -p petr-core
cargo +nightly miri test -p petr-collections

# Strict provenance is the important hardening pass for pointer/tag code.
MIRIFLAGS="-Zmiri-strict-provenance" cargo +nightly miri test -p petr-core
MIRIFLAGS="-Zmiri-strict-provenance" cargo +nightly miri test -p petr-collections
