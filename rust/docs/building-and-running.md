# Building & running

Every way to build and run BattleTris, plus the environment variables the server
and bots read. All commands are run from the `rust/` workspace root unless noted.

> The original `rust/README.md` "Play modes" section is stale — it describes a
> WebRTC peer-to-peer path that no longer exists. The real online path is an
> **authoritative axum server** (`cargo run -p bt-server`) that browsers and
> bots reach over a `/ws` websocket. Use this document, not that section.

## Prerequisites

| Tool | Needed for | Install |
|------|-----------|---------|
| Rust (stable) | everything | <https://rustup.rs> |
| `wasm-pack` | the browser wasm client | `cargo install wasm-pack` |
| `wasm32-unknown-unknown` target | the browser wasm client | `rustup target add wasm32-unknown-unknown` |
| Node + `tsc` (TypeScript) | compiling `www/*.ts` | `npm install` in `bt-wasm/` (pulls `typescript`) |
| Python 3 | the static dev server (vs-Computer) | system Python |
| Apalache + Java 17 | the TLA+ model checks (optional) | see [`tla/README.md`](../../tla/README.md) |

There is no toolchain pin file; the CI builds on `dtolnay/rust-toolchain@stable`.

## Building

### The engine + host crates (no wasm toolchain)

`bt-wasm` only targets wasm32, so it is excluded from the default workspace
members. A plain build/test of the engine, AI, replay, netcode, identity, and
TrueSkill crates needs no wasm tooling:

```sh
cargo build --workspace   # the default members (engine + host crates)
cargo test  --workspace   # run the full host test suite
```

### The browser client (wasm + TypeScript)

The page is shipped as **plain ES modules — there is no bundler**. `tsc` is the
only build step for the TypeScript, and `wasm-pack` produces the wasm glue the
page imports via `../pkg` (see the `Dockerfile` comments, which document this
no-bundler design). From `rust/`:

```sh
# wasm-bindgen glue -> bt-wasm/pkg/
wasm-pack build bt-wasm --target web --out-dir pkg --dev   # --release for production

# www/*.ts -> www/*.js  (run AFTER wasm-pack: tsc types the ../pkg import
# against the generated pkg/bt_wasm.d.ts)
cd bt-wasm && npm install && npm run build:ts
```

The `bt-wasm/package.json` scripts wrap these:

| Script | Does |
|--------|------|
| `npm run build:wasm` | `wasm-pack build . --target web --out-dir pkg --dev` |
| `npm run build:ts` | `tsc -p tsconfig.json` |
| `npm run build` | `build:wasm` then `build:ts` |
| `npm run typecheck` | `tsc --noEmit` (no output) |
| `npm run test:unit` | Node `--test` over `www/*.test.ts` |
| `npm run test:e2e` | Playwright e2e (`tests/*.spec.js`) |
| `npm test` | `build` then Playwright e2e |

A `BT_GIT_SHA` env var passed to the wasm build is stamped into recordings at
compile time (via `option_env!`); it defaults to `dev`.

## Run modes

### Practice / vs-Computer — 100% client-side (no server)

These two modes run entirely in the browser; nothing is sent over the network.
Build the wasm + TypeScript (above), then serve the static files with anything —
the e2e suite uses a bare Python server over `bt-wasm/`
([`playwright.config.js`](../bt-wasm/playwright.config.js) proves no `bt-server`
is required):

```sh
cd bt-wasm && python3 -m http.server 4173
# open http://localhost:4173/www/
#   Practice    -> solo
#   Play Ernie  -> battle the bt-ai opponent (label tracks the difficulty slider)
```

### Online — the authoritative server

Online matches go through `bt-server`: an axum process that does in-process
matchmaking, runs each match as a server-authoritative `Bout` on `/ws`, keeps
the leaderboard, and stores replays. It **also serves the static site**, so a
single process gives you the whole app. The client derives its websocket URL
from `location.host`, so just open the page the server serves:

```sh
# Serve the already-built www/ + pkg/ and the /ws endpoint.
# Defaults: STATIC_DIR=bt-wasm, PORT=8080.
STATIC_DIR=bt-wasm cargo run -p bt-server
# open http://localhost:8080/  (redirects to /www/), click "Find Match"
```

To play a real online match against yourself, open the page in two browser
windows and have both click **Find Match**.

### Run a region bot locally

`bt-bot` is a headless player that speaks the same `/ws` protocol a browser does
(it connects, announces "open to matches", and plays a server-authoritative
match driven by the same `bt-ai` search). Point it at a running server with
`BT_BOT_WS` (it defaults to `ws://127.0.0.1:8088/ws`) and pick a persona with
`BT_BOT_PERSONA`:

```sh
# Terminal 1: the server (must share BT_JWT_SECRET with the bot — see below).
BT_JWT_SECRET=dev-secret STATIC_DIR=bt-wasm cargo run -p bt-server

# Terminal 2: an aggressive "Bert" bot dialed at this server's port.
BT_JWT_SECRET=dev-secret \
BT_BOT_WS=ws://127.0.0.1:8080/ws \
BT_BOT_PERSONA=bert \
cargo run -p bt-bot
```

Personas (from `BT_BOT_PERSONA`, else the fly process group, else Ernie):

- `bert` / `strong` / `hard` — aggressive: the strong line-clearing eval + smart
  weapons, brisk pace.
- `ernie` (default) — easy-going: faithful (weaker) placement, slower, no weapons.
- `count` / `roam` — **The Count**, a roaming challenger that issues directed
  challenges and dials its skill to each opponent's rating.

The bot self-mints its identity token, so the server and bot **must share the
same `BT_JWT_SECRET`** or the token won't verify (the bot warns loudly when the
secret is unset).

## Environment variables

`bt-server` and `bt-bot` read these via `std::env::var`. `BT_GIT_SHA` is read at
compile time (`option_env!`) by both `bt-wasm` and `bt-server`.

| Variable | Read by | Default | Purpose |
|----------|---------|---------|---------|
| `BT_GIT_SHA` | bt-wasm, bt-server (compile-time `option_env!`) | `dev` | Commit stamped into recordings as the engine SHA. |
| `STATIC_DIR` | bt-server | `bt-wasm` | Directory served as static (holds `www/` + `pkg/`); the Docker image sets `/app/site`. |
| `PORT` | bt-server | `8080` | HTTP/websocket listen port. |
| `RATINGS_FILE` | bt-server | `ratings.json` | Path to the persisted TrueSkill ratings (a fly volume `/data` in prod). |
| `REPLAY_DB` | bt-server | `replays.db` | SQLite database for stored replays. |
| `REPLAYS_DIR` | bt-server | `replays` | Legacy JSON replay dir, imported into the DB once at startup if present. |
| `BT_JWT_SECRET` | bt-server, bt-bot, bt-identity | per-process random (logged) | HS256 identity-token secret; **server and bots must share it** or bot tokens won't verify. |
| `BT_ADMIN_TOKEN` | bt-server | unset (admin gated, fail-closed) | Token gating the `/admin/*` endpoints. |
| `SENTRY_DSN` | bt-server | unset (no Sentry) | Optional error reporting. |
| `BT_BOT_WS` | bt-bot | `ws://127.0.0.1:8088/ws` | Server websocket the bot dials. |
| `BT_BOT_PERSONA` | bt-bot | (falls back to `FLY_PROCESS_GROUP`, else Ernie) | `bert` / `ernie` / `count` persona select. |
| `BT_BOT_NAME` | bt-bot | derived from region + persona | Override the bot's display name. |
| `BT_BOT_GEO` | bt-bot | derived from `FLY_REGION` | Override the bot's geo label. |
| `BT_BOT_ANNOUNCE_BOT` | bt-bot | `true` | Set falsey to make the bot auto-pair as a "human" (testing escape hatch). |
| `FLY_REGION` | bt-bot | — | Set by fly; maps to a friendly city name. |
| `FLY_PROCESS_GROUP` | bt-bot | — | Set by fly; persona fallback when `BT_BOT_PERSONA` is unset. |

## Production build (Docker)

The `Dockerfile` builds the release wasm + TypeScript and a fully-static (musl)
`bt-server` binary, then ships it on Chainguard's distroless `static` image
(no shell, minimal CVE surface). `Dockerfile.bot` builds the static `bt-bot`.
Deployment topology, the quiesce-in-place drain, and the region-bots app are
covered in the project dossier (`screenshots/*.html`) and the fly configs
(`fly.toml`, `fly.bots.toml`).

## See also

- [quickstart.md](quickstart.md) — the fastest path to seeing it run.
- [../ARCHITECTURE.md](../ARCHITECTURE.md) — crate map + data-flow paths.
- [../CONTRIBUTING.md](../CONTRIBUTING.md) — toolchain, test loop, house rules.
