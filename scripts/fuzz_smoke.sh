#!/usr/bin/env bash
set -euo pipefail
if ! cargo +nightly fuzz --help >/dev/null 2>&1; then
  echo "cargo-fuzz is not installed. Run: cargo install cargo-fuzz" >&2
  exit 1
fi
cargo +nightly fuzz run selector_word_decode -- -max_total_time=10
cargo +nightly fuzz run toml_operation_config -- -max_total_time=10
cargo +nightly fuzz run encoded_handle_decode -- -max_total_time=10
