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

# run_check <expect: NoError|Error> <invariant> <length> <cfg> <model> <human label>
#
# Asserts BOTH:
#   (1) Apalache actually LOADED the expected invariant — `found INVARIANTS: <inv>` —
#       so a malformed cfg, a missing/renamed INVARIANT line, a type error, or a tool
#       crash (none of which load that invariant) can NOT pass as a "teeth" Error; and
#   (2) the OUTCOME is the expected one (`Error` => the invariant was VIOLATED, the real
#       teeth; `NoError` => it held).
# A bare "the run errored" is NOT accepted — we require the specific invariant we meant
# to check was the one that fired.
run_check() {
  local expect="$1" inv="$2" length="$3" cfg="$4" model="$5" label="$6"
  echo "── $label  (expect $expect on $inv; --length=$length --config=$cfg $model)"
  local work out
  work="$(mktemp -d)"
  # Apalache exits nonzero when it finds a violation; we assert on the OUTCOME line,
  # not the exit code, so `|| true` keeps `set -e` from aborting on an expected Error.
  out="$("$APALACHE" check --length="$length" --out-dir="$work" \
          --config="$cfg" "$model" 2>&1)" || true
  rm -rf "$work"

  # (0) Reject Apalache INPUT/TOOL failures outright (parse/type/config/IO). These never
  # reach an invariant check, so they must NEVER be mistaken for a teeth "Error". (They
  # also print no "The outcome is:" line, but we reject them explicitly + loudly.)
  if echo "$out" | grep -qE "type input error|Error parsing|config file not found|Parsing error|SemanticError|Fatal error"; then
    echo "   FAIL: Apalache reported an input/tool error (not an invariant result):"
    echo "$out" | grep -E "type input error|Error parsing|config file not found|Parsing error|SemanticError|Fatal error" | head -2
    fail=1
    return
  fi
  # (1) The intended invariant must have been the one Apalache checked.
  if ! echo "$out" | grep -qE "found INVARIANTS:.*\b$inv\b"; then
    echo "   FAIL: Apalache did not load the expected invariant '$inv' (malformed cfg/model?):"
    echo "$out" | grep -E "found INVARIANTS:|type input error|parsing|not found|Error" | head -3 \
      || echo "$out" | tail -3
    fail=1
    return
  fi
  # (2) ...with the expected outcome — AND, for an expected Error, the genuine
  # invariant-VIOLATION markers (so a non-violation that still prints "outcome: Error"
  # can't pass as teeth). For an expected NoError we additionally require there was no
  # violation reported.
  if ! echo "$out" | grep -q "The outcome is: $expect"; then
    echo "   FAIL: expected outcome '$expect' on '$inv' but Apalache reported:"
    echo "$out" | grep -E "The outcome is:|violated|Found .* error" || echo "$out" | tail -3
    fail=1
    return
  fi
  if [[ "$expect" == "Error" ]]; then
    if echo "$out" | grep -qE "invariant [0-9]+ violated" && echo "$out" | grep -q "Checker has found an error"; then
      echo "   OK: $inv -> invariant violated (genuine teeth)"
    else
      echo "   FAIL: '$expect' on '$inv' but no genuine invariant-violation marker — \
the Error may be a tool/parse failure, not the invariant having teeth:"
      echo "$out" | grep -E "The outcome is:|invariant .* violated|Checker has found|error" | head -3
      fail=1
    fi
  else
    # NoError: there must be NO violation marker at all.
    if echo "$out" | grep -qE "invariant [0-9]+ violated"; then
      echo "   FAIL: expected NoError on '$inv' but a violation was reported."
      fail=1
    else
      echo "   OK: $inv -> outcome NoError (holds)"
    fi
  fi
}

# Bazaar teaching model: buggy policy MUST deadlock; fixed policy MUST be clean.
run_check Error   NoDeadlock 10 BazaarBuggy.cfg Bazaar.tla  "Bazaar buggy -> NoDeadlock violated"
run_check NoError NoDeadlock 12 Bazaar.cfg      Bazaar.tla  "Bazaar fixed -> NoDeadlock holds"

# Cross generator: the trap invariant MUST fire (it emits the crossing trace fixture).
run_check Error   NotCrossed 6  Cross.cfg       Cross.tla   "Cross generator -> NotCrossed trap fires"

# Netcode teeth: each fix OFF MUST break EXACTLY its named invariant (short length, fast).
run_check Error   NoDeadlock        10 NetcodeBugAck.cfg   Netcode.tla "Netcode ack OFF   -> NoDeadlock violated"
run_check Error   AckBounds         10 NetcodeBugReset.cfg Netcode.tla "Netcode reset OFF -> AckBounds violated"
run_check Error   LeaveOnlyWhenReal 10 NetcodeBugLeave.cfg Netcode.tla "Netcode leave OFF -> LeaveOnlyWhenReal violated"

# A REDUCED-length all-fixed Netcode check (NOT the full ~6.5-min length-14 run, which
# is gated behind tla-full): every invariant holds to a short bound. Catches an
# accidental invariant break without the full cost.
run_check NoError AllSafe 6  Netcode.cfg     Netcode.tla "Netcode all-fixed (short) -> AllSafe holds"

if [[ "$fail" != "0" ]]; then
  echo "One or more TLA+ model checks did not produce the expected outcome." >&2
  exit 1
fi
echo "All TLA+ fast model checks produced their expected outcomes."
