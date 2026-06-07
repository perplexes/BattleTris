#!/usr/bin/env bash
#
# FAST TLA+ model checks for CI (the slow all-fixed Netcode run is gated separately,
# in the tla-full job). Each check ASSERTS its expected Apalache outcome — a buggy cfg
# MUST report an Error (its invariant violated), a fixed cfg MUST report NoError. A
# check that doesn't produce the expected outcome FAILS THE BUILD (so a model edit that
# accidentally makes a "teeth" check pass, or a fix check fail, is caught — the check
# can't pass silently).
#
# Needs apalache-mc on PATH or $APALACHE_MC pointing at the binary, + Java 17+.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$HERE"
APALACHE="${APALACHE_MC:-apalache-mc}"

fail=0

# run_check <expect: NoError|Error> <length> <cfg> <model> <human label>
run_check() {
  local expect="$1" length="$2" cfg="$3" model="$4" label="$5"
  echo "── $label  (expect $expect; --length=$length --config=$cfg $model)"
  local work out
  work="$(mktemp -d)"
  # Apalache exits nonzero when it finds a violation; we assert on the OUTCOME line,
  # not the exit code, so `|| true` keeps `set -e` from aborting on an expected Error.
  out="$("$APALACHE" check --length="$length" --out-dir="$work" \
          --config="$cfg" "$model" 2>&1)" || true
  rm -rf "$work"
  if echo "$out" | grep -q "The outcome is: $expect"; then
    echo "   OK: outcome $expect"
  else
    echo "   FAIL: expected outcome '$expect' but Apalache reported:"
    echo "$out" | grep -E "The outcome is:|violated|Found .* error" || echo "$out" | tail -3
    fail=1
  fi
}

# Bazaar teaching model: buggy policy MUST deadlock; fixed policy MUST be clean.
run_check Error   10 BazaarBuggy.cfg Bazaar.tla  "Bazaar buggy -> NoDeadlock violated"
run_check NoError 12 Bazaar.cfg      Bazaar.tla  "Bazaar fixed -> NoDeadlock holds"

# Cross generator: the trap invariant MUST fire (it emits the crossing trace fixture).
run_check Error   6  Cross.cfg       Cross.tla   "Cross generator -> NotCrossed trap fires"

# Netcode teeth: each fix OFF MUST break exactly its invariant (short length, fast).
run_check Error   10 NetcodeBugAck.cfg   Netcode.tla "Netcode ack OFF   -> NoDeadlock violated"
run_check Error   10 NetcodeBugReset.cfg Netcode.tla "Netcode reset OFF -> AckBounds violated"
run_check Error   10 NetcodeBugLeave.cfg Netcode.tla "Netcode leave OFF -> LeaveOnlyWhenReal violated"

# A REDUCED-length all-fixed Netcode check (NOT the full ~6.5-min length-14 run, which
# is gated behind tla-full): every invariant holds to a short bound. Catches an
# accidental invariant break without the full cost.
run_check NoError 6  Netcode.cfg     Netcode.tla "Netcode all-fixed (short) -> AllSafe holds"

if [[ "$fail" != "0" ]]; then
  echo "One or more TLA+ model checks did not produce the expected outcome." >&2
  exit 1
fi
echo "All TLA+ fast model checks produced their expected outcomes."
