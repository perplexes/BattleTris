#!/usr/bin/env bash
#
# Quiesce-in-place deploy for the battletris server.
#
# A plain `flyctl deploy` replaces the single machine immediately, severing any live
# game's websocket. This instead:
#   1. builds + pushes the new image while the server keeps serving normally,
#   2. pauses NEW matchmaking on the running machine (POST /admin/drain),
#   3. waits — UNCAPPED — for every in-flight bout to finish (matches can run long),
#   4. swaps the machine to the new image in place (0 bouts now → no game is killed).
# The fresh boot clears the drain flag, so matchmaking resumes on the new version.
#
# The only cost: NEW matches are paused for the duration of the longest in-flight game
# (usually none/short). Existing games always finish.
#
# Why no second machine / true blue-green: the server is stateful on a single-attach
# fly volume (/data: replays.db + ratings), which two machines can't share. Quiesce-
# in-place keeps one machine and one DB, so there's no data divergence to reconcile.
#
# Requires: flyctl (authed), curl, jq, and BT_ADMIN_TOKEN matching the server's
# `BT_ADMIN_TOKEN` secret. Run from the `rust/` dir (where fly.toml lives).
set -euo pipefail

APP="${FLY_APP:-battletris}"
HOST="${BT_HOST:-https://battletris.fly.dev}"
POLL_SECS="${DRAIN_POLL_SECS:-10}"
# A unique tag per run so we deploy exactly the image we just built.
LABEL="quiesce-${GITHUB_RUN_ID:-local-$$}"
IMAGE="registry.fly.io/${APP}:${LABEL}"
# Stamp the build with the commit so recordings carry a real engine_sha (matches the
# old `flyctl deploy --build-arg BT_GIT_SHA=…`). Falls back to the local HEAD.
SHA="${BT_GIT_SHA:-$(git rev-parse HEAD 2>/dev/null || echo unknown)}"
SHA="${SHA:0:7}"

: "${BT_ADMIN_TOKEN:?BT_ADMIN_TOKEN must be set (matches the server secret) — refusing to deploy without drain}"

admin() { curl -fsS -X POST "$HOST/admin/$1" -H "x-admin-token: $BT_ADMIN_TOKEN"; }
bouts() { curl -fsS "$HOST/api/debug/matches" | jq '.matches | length'; }

# If we fail AFTER pausing but BEFORE the in-place swap completes, un-pause matchmaking
# so the lobby isn't left stuck. A successful swap replaces the machine (fresh boot =
# not draining), so `swapped` short-circuits this.
swapped=0
on_exit() {
  local rc=$?
  if [ "$rc" -ne 0 ] && [ "$swapped" -eq 0 ]; then
    echo ">> deploy aborted (rc=$rc) — resuming matchmaking on the live machine"
    admin resume || true
  fi
}
trap on_exit EXIT

echo "1/4  building + pushing $IMAGE (server stays live during the build)…"
flyctl deploy -a "$APP" --build-only --push --remote-only \
  --image-label "$LABEL" --build-arg BT_GIT_SHA="$SHA"

echo "2/4  pausing new matches…"
admin drain

echo "3/4  waiting for in-flight bouts to finish (uncapped)…"
while :; do
  n="$(bouts)"
  echo "       $n bout(s) still in flight"
  [ "$n" -eq 0 ] && break
  sleep "$POLL_SECS"
done

echo "4/4  swapping to $IMAGE in place (0 bouts → no game cut)…"
flyctl deploy -a "$APP" --image "$IMAGE" --strategy immediate --ha=false
swapped=1
echo "done — matchmaking resumes on the new version."
