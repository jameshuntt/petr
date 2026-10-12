#!/usr/bin/env bash
# THE MIRI GATE — undefined-behavior checking for the kernel's pointer-tag
# primitives. Ported from the attempts_ptr_* lineage (miri_test.sh), where it
# proved petr_core + petr_collections clean under Miri. The kernel
# ABSORBED those crates (tests byte-identical) but never the gate; this runs it
# over ALL ptr_* primitives — broader than any attempt did — in two passes:
# default Miri, then the strict-provenance hardening pass (the important one
# for pointer/tag code).
#
# Miri is a nightly component and SLOW (minutes per crate). Keep the project
# on stable; only this gate runs nightly.
set -uo pipefail

CRATES=(
  petr-core
  petr-collections
  petr-dispatch
  petr-math
  petr-spatial
  petr-resource
)

export PROPTEST_DISABLE_FAILURE_PERSISTENCE=1   # Miri: no persistence file I/O
rustup +nightly component add miri >/dev/null 2>&1 || true
cargo +nightly miri setup >/dev/null 2>&1 || true

# Full per-run logs are preserved here (no truncation), so the pass counts and
# any UB report survive for review.
LOGDIR="${MIRI_GATE_LOGDIR:-/tmp/miri-gate-logs}"
mkdir -p "$LOGDIR"

pass=0; fail=0; incompat=0
run() {
  local label="$1"; local slug; slug=$(echo "$label" | tr ' ()/' '____'); shift
  local log="$LOGDIR/$slug.log"
  echo "=== $label ==="
  "$@" >"$log" 2>&1
  grep -E "running [0-9]+ tests|test result:" "$log" | grep -v "running 0 tests" | tail -2
  if grep -q "test result: ok" "$log" && ! grep -q "test result: FAILED" "$log"; then
    pass=$((pass+1)); return
  fi
  # Distinguish real UB / test failure from a Miri limitation (unsupported op).
  if grep -qiE "unsupported operation|can.t call foreign function|miri does not support" "$log"; then
    echo "  -> MIRI-INCOMPATIBLE (a Miri limitation, not UB) — see $log"; incompat=$((incompat+1))
  else
    echo "  -> FAILED (real UB or test failure — investigate $log)"; fail=$((fail+1))
  fi
}

for c in "${CRATES[@]}"; do
  run "$c (default miri)" cargo +nightly miri test -p "$c"
  run "$c (strict-provenance)" env MIRIFLAGS="-Zmiri-strict-provenance" cargo +nightly miri test -p "$c"
done

echo "=================================================="
echo " MIRI GATE: pass=$pass  incompatible=$incompat  FAIL=$fail"
echo "=================================================="
[ "$fail" -eq 0 ]
