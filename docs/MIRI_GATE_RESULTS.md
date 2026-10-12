# Miri gate results

Run on 2026-10-12 with `scripts/miri_gate.sh` on an Apple Silicon Mac, nightly Miri (miri 0.1.0, 2026-05-16).
Two passes per crate: default Miri, then `MIRIFLAGS=-Zmiri-strict-provenance`.

**Verdict: pass=12, incompatible=0, FAIL=0. No undefined behaviour found.**

| crate | tests under Miri | default | strict-provenance |
|---|---|---|---|
| `petr-core` | 20 | PASS | PASS |
| `petr-collections` | 28 | PASS | PASS |
| `petr-dispatch` | 13 | PASS | PASS |
| `petr-resource` | 9 | PASS | PASS |
| `petr-math` | 3 | PASS | PASS |
| `petr-spatial` | 3 | PASS | PASS |

Whole gate: about two minutes of wall clock (first log 17:10:07, last 17:12:19).
The "finished in" seconds printed inside each Miri run come from Miri's simulated clock, not real time.

What this proves: the keyed and tagged words hold their invariants under the UB checker, including
strict provenance. Wrong keys never reveal an address, forged generations and check words are rejected,
dirty words are never dereferenced, stale handles are caught, and tag mutation touches only the requested bits.
