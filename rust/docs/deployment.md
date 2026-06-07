# Deployment & operations

The server runs as a single always-on machine on [fly.io](https://fly.io); the
always-on lobby opponents run as a *separate* fly app. This page covers the fly
topology, the Docker builds, the region bots, the zero-cut deploy, CI, and the admin
endpoints + secrets.

Sources: [`rust/fly.toml`](../fly.toml), [`rust/fly.bots.toml`](../fly.bots.toml),
[`rust/Dockerfile`](../Dockerfile), [`rust/Dockerfile.bot`](../Dockerfile.bot),
[`rust/deploy-quiesce.sh`](../deploy-quiesce.sh),
[`.github/workflows/deploy.yml`](../../.github/workflows/deploy.yml), and
`bt-server/src/{main,metrics}.rs`.

## Fly topology: one always-on machine

Matchmaking and live-match state are held **in-process**, so all players must reach
the *same* process. That dictates the whole topology (`fly.toml`):

- **App `battletris`**, primary region `sjc`.
- **Exactly one always-on machine.** `auto_stop_machines = "off"`,
  `min_machines_running = 1`, and deploys pass `--ha=false` so fly never spins up a
  second machine. `shared-cpu-1x`, 256 MB.
- **One `/data` volume** (`source = "data"`, `destination = "/data"`). It holds the
  TrueSkill ratings (`ratings.json`) and the SQLite replay DB (`replays.db`). A fly
  volume is single-attach — only one machine can mount it — which is *why* the
  single-machine model and the in-place deploy (below) exist.
- **`[[metrics]]`** points fly's managed Grafana (`fly-metrics.net`) at `/metrics`
  (port 8080).
- The server binds **`[::]`** (dual-stack), not `0.0.0.0` — fly's private 6PN
  network is IPv6-only, and an IPv4-only bind leaves the bots with "connection
  refused".

## The Dockerfile

[`Dockerfile`](../Dockerfile) is a two-stage build → a tiny distroless image:

1. **Builder** (`rust:1-bookworm`): adds the `wasm32-unknown-unknown` and
   `x86_64-unknown-linux-musl` targets, `musl-tools`, and `nodejs`/`npm`. It then:
   - builds the browser wasm (`wasm-pack build bt-wasm --target web --out-dir pkg
     --release`), stamping `BT_GIT_SHA` (the commit, read by the engine via
     `option_env!`, default `"dev"`) into recordings;
   - compiles the browser TypeScript (`npm ci && npm run build:ts`) — run *after*
     wasm-pack so the generated `pkg/bt_wasm.d.ts` types the import (the page is
     plain ES modules, **tsc is the only build step, no bundler**);
   - builds a **fully static (musl)** `bt-server` (`CC_x86_64_unknown_linux_musl=musl-gcc`
     so the bundled SQLite — the server's one C dep — links statically; the runtime
     image stays libc-free).
2. **Runtime** (`cgr.dev/chainguard/static:latest`): distroless, no shell, minimal
   CVE surface. Copies in the static `bt-server` binary plus `www/` (page) and
   `pkg/` (wasm) under `/app/site`. Sets `STATIC_DIR=/app/site`, `PORT=8080`,
   `RATINGS_FILE=/data/ratings.json`, `REPLAY_DB=/data/replays.db`,
   `REPLAYS_DIR=/data/replays`. Runs as **root** (the only reason: chainguard's
   default nonroot can't write a fly volume).

## Region bots: a separate app

The always-on lobby opponents are real headless `bt-bot` processes — *not* part of
the server — deployed as a **separate fly app `battletris-bots`**
([`fly.bots.toml`](../fly.bots.toml), [`Dockerfile.bot`](../Dockerfile.bot)). They
populate the lobby (a fresh visitor always has someone to play) and exercise the
netcode under real cross-geo latency.

- **One image, three personas, selected at runtime** from `BT_BOT_PERSONA` (else the
  fly process-group name `FLY_PROCESS_GROUP`):
  - **Bert** — aggressive: the strong line-clearing eval + smart weapons.
  - **Ernie** — easy-going: faithful placement, slower, no weapons.
  - **The Count** (`count`) — a single roaming challenger in `sjc` that issues
    directed challenges, preferring humans, and dials its skill to each opponent's
    Elo (carried on `matchStart`) for an even match.
- **Per-region placement.** `fly.bots.toml` declares three `[processes]`
  (`bert`/`ernie`/`count`, all running `/app/bt-bot`). Each machine reads
  `FLY_REGION` for its city and `FLY_PROCESS_GROUP` for its persona, so one image
  yields e.g. Tokyo-Bert and Tokyo-Ernie. Deploy + scale:
  ```sh
  fly deploy -c fly.bots.toml --dockerfile Dockerfile.bot --remote-only
  fly scale count bert=1 ernie=1 --region nrt,lhr,gru,syd,sjc --max-per-region 1 -a battletris-bots
  fly scale count count=1 --region sjc -a battletris-bots
  ```
- **Outbound only.** The bots dial the server over 6PN at
  `BT_BOT_WS = ws://battletris.internal:8080/ws` (plaintext ws, real latency). There's
  no `[http_service]` — a worker with nothing to serve, so the static musl build has
  no C deps (no TLS stack). They never auto-pair each other (server `bot` flag);
  they're human-challengeable and human-auto-pairable.
- **Shared secret.** The bots self-mint identity tokens, so they need the **same**
  `BT_JWT_SECRET` as the server (`fly secrets set BT_JWT_SECRET=<value> -a
  battletris-bots`). See "secrets" below.

## Zero-cut deploys: quiesce-in-place

A plain `flyctl deploy` replaces the single machine *immediately*, severing any live
game's websocket. [`deploy-quiesce.sh`](../deploy-quiesce.sh) avoids that by draining
in place:

1. Build + push the new image while the server keeps serving normally.
2. `POST /admin/drain` — pause *new* matchmaking on the running machine.
3. Wait — **uncapped** — for every in-flight bout to finish (`GET /api/debug/matches`
   polled until zero; matches can run long).
4. Swap the machine to the new image in place (`--strategy immediate --ha=false`).
   Zero bouts now → no game is cut. The fresh boot clears the drain flag, so
   matchmaking resumes on the new version.

The only cost is that *new* matches are paused for the duration of the longest
in-flight game (usually none). If the script fails after draining but before the
swap, a trap calls `POST /admin/resume` so the lobby isn't left stuck.

**Why no blue-green / second machine:** the server is stateful on the single-attach
`/data` volume (`replays.db` + ratings), which two machines can't share. One machine
+ one DB means there's no data divergence to reconcile. Requires `flyctl`, `curl`,
`jq`, and `BT_ADMIN_TOKEN` matching the server's secret (the script refuses to deploy
without it).

## CI: `.github/workflows/deploy.yml`

Triggers: every push/PR to `main`, manual dispatch, and a nightly cron. Jobs:

| Job | What it does | Gates deploy? |
|-----|--------------|:---:|
| `test` | `cargo test --workspace --locked` | yes |
| `clippy` | `cargo clippy --workspace --all-targets -- -D warnings` | yes |
| `tla` | the **fast** Apalache model checks (`tla/ci-check.sh`) + a trace-fixtures-not-stale check | yes |
| `tla-full` | the slow (~6.5 min) all-fixed Netcode check (`AllSafe`, length 14) | no — **dispatch/nightly only**, never gates a PR/deploy |
| `deploy` | `needs: [test, clippy, tla]` → `flyctl deploy` | — |

`deploy` runs **only on a push or manual dispatch to `main`** — never on PRs (which
may come from forks and must not touch production or see secrets), never on the
nightly schedule. It's serialized with a `concurrency: deploy-battletris` so two
pushes can't race the single machine, and stamps the build with `BT_GIT_SHA` so
recordings carry a real `engine_sha`. (The deploy step currently runs `flyctl deploy
--remote-only --ha=false`; a code comment notes the planned switch to
`./deploy-quiesce.sh` once the drain endpoints and `BT_ADMIN_TOKEN` are live in
prod.)

## Admin & debug endpoints

Routes registered in `bt-server/src/main.rs`:

| Route | Method | Purpose |
|-------|--------|---------|
| `/admin/drain` | POST | Begin a quiesce-in-place drain — pause new matches, notify the lobby. **Admin-gated.** |
| `/admin/resume` | POST | Clear the drain flag (rollback if a deploy aborts). **Admin-gated.** |
| `/admin/grant` | POST | Gated dev tool: inject a weapon (`weapon` = wire index 0..=33) and/or `funds` into a live bout side (`match_id`, `side` = "A"/"B"). The grant is applied out-of-band inside the bout loop and is **not** recorded into the replay stream. **Admin-gated.** |
| `/api/debug/matches` | GET | Lists live bouts (match_id + names) — drives the spectator picker and the drain wait-loop. |
| `/metrics` | GET | Prometheus text (see below). |
| `/api/identity` | POST | Mint an HS256 identity token for a `{name}` (`bt_identity::issue_token`). |
| `/api/leaderboard` · `/api/player/:name` · `/api/replays`(GET/POST) · `/api/replays/:id` · `/replay/:id` | — | Leaderboard, profile, replay store/list/fetch, replay page. |
| `/ws` | GET | The websocket — lobby + authoritative matches. |
| everything else | — | static files from `STATIC_DIR` (`fallback_service(ServeDir)`). |

### `/metrics`

Pure-Rust Prometheus client (`metrics.rs`), no C deps. Exposes:
`bt_http_requests_total` (hit rate), `bt_ws_messages_total{direction}` (msgs/sec),
`bt_ws_ping_ms` histogram (p50/p95 — the same RTT shown in the lobby),
`bt_ws_connections` gauge, `bt_matches_total`.

## Secrets & env vars

| Var | Where | Notes |
|-----|-------|-------|
| `BT_ADMIN_TOKEN` | server | Gates **all** `/admin/*` endpoints (checked against the `x-admin-token` header). **Fail-closed:** if the env is unset/empty the admin endpoints return 403 — never a silently-open admin control. Must also match `deploy-quiesce.sh`. |
| `BT_JWT_SECRET` | server **and** bots | HS256 identity signing key (`bt_identity`). **Must be the same value on `battletris` and `battletris-bots`** so the bots' self-minted tokens verify (and so real users aren't logged out on every redeploy — it was per-process-random before). If unset, the server falls back to a per-process-random secret. |
| `BT_GIT_SHA` | build arg | The commit, stamped into recordings' `engine_sha`. Default `"dev"`. |
| `STATIC_DIR` | server | Static root (image sets `/app/site`). |
| `PORT` | server | Default 8080. |
| `RATINGS_FILE` | server | `/data/ratings.json`. |
| `REPLAY_DB` | server | `/data/replays.db` (the SQLite replay store — source of truth). |
| `REPLAYS_DIR` | server | `/data/replays` — legacy JSON, imported into the DB once at startup if present. |
| `BT_BOT_WS` | bots | The server ws URL (`ws://battletris.internal:8080/ws` in prod). |
| `BT_BOT_PERSONA` / `FLY_PROCESS_GROUP` | bots | Persona selection (`bert`/`ernie`/`count`). |
| `FLY_REGION` · `BT_BOT_NAME` · `BT_BOT_GEO` · `BT_BOT_ANNOUNCE_BOT` | bots | City/name/geo label; `BT_BOT_ANNOUNCE_BOT=false` lets two bots spar (otherwise they never auto-pair). |
| `SENTRY_DSN` | server (optional) | Inert until set; staged as a fly secret. |

## Related

- [`architecture-netcode.md`](architecture-netcode.md) — the authoritative `Bout` /
  rejoin-grace / quiesce interplay.
- [`testing.md`](testing.md) — the TLA+ jobs CI gates on.
- [`replays.md`](replays.md) — the replay store these endpoints serve.
- [`weapons.md`](weapons.md) — what `/admin/grant`'s `weapon` index means.
