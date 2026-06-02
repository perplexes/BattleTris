# BattleTris — Rust + WASM port

A faithful port of [BattleTris](../README.md) (Brown CS32, 1994) — the 2‑player
networked Tetris‑battler — from its original pre‑standard C++/X11/Motif source
(under [`usr/src/`](../usr/src)) to Rust, targeting the browser via WebAssembly,
with **TrueSkill 2** matchmaking.

The port preserves the original *game logic* byte‑for‑byte where it matters
(board geometry, piece shapes & rotation, the funds/die/happy economy, the
20‑combined‑line bazaar trigger, weapon roster) while replacing the platform
layers: X11/Motif → HTML5 Canvas, the TCP master/slave daemons → WebRTC P2P,
the flat‑file ELO DB → TrueSkill 2.

## Workspace layout

```
rust/
  bt-core/       Pure, deterministic game logic (no platform/UI/net deps)
  bt-trueskill/  TrueSkill / TrueSkill 2 ratings + matchmaking
  bt-ai/         (planned) BTComputer opponent port
  bt-wasm/       (planned) wasm-bindgen glue + Canvas front-end
```

Each Rust module names the C++ class it ports (e.g. `board.rs` ⇐
`BTBoardManager`), and constants are transcribed verbatim from `BTConstants.H`.

## Status

| Area | Crate / module | State | Tests |
|------|----------------|-------|-------|
| Constants | `bt-core::constants` | ✅ verbatim from `BTConstants.H` | — |
| RNG (`drand48`/`lrand48`/`rand`) | `bt-core::rng` | ✅ POSIX LCG, deterministic | 8 |
| Box/cell semantics (`BTBox`) | `bt-core::cell` | ✅ value/id/removable/hidden | — |
| 18 pieces + rotation (`BTPiece`) | `bt-core::piece` | ✅ incl. Wall/Star/WeirdLong state machines | 19 |
| Board (`BTBoardManager`) | `bt-core::board` | ✅ collision, line‑clear+funds, idiot, fall‑out | 9 |
| Weapon data (34, `btweapons.db`) | `bt-core::weapons` | ✅ table generated from the DB | 2 |
| Piece selection (`BTPieceManager`) | `bt-core::piece_manager` | ✅ rejection sampling + keep‑probs | 6 |
| Game loop (`BTGame`) | `bt-core::game` | ✅ deterministic `tick`, spawn→fall→slide→lock→clear→spawn→top‑out | 9 |
| Classic TrueSkill 1v1 | `bt-trueskill` | ✅ matches reference values | 6 |
| Normal math (`erfc`/probit/`v`/`w`) | `bt-trueskill::math` | ✅ | 5 |
| TrueSkill 2 (experience/lines/quit) | `bt-trueskill::ts2` | ✅ EP-consistent lines factor (reduces to classic at λ=0) | 6 |
| Arsenal (`BTArsenal`) | `bt-core::arsenal` | ✅ stack/empty buy, use | 3 |
| Weapon effects + relay | `bt-core::{board,game}` | ✅ all WPN_ON/OFF effects, durations, launch, op‑score, bazaar | 3 |
| AI (`BTComputer` + `BTCBoard`) | `bt-ai` | ✅ eval heuristic + placement search + driver | 5 |
| Canvas front‑end + WASM | `bt-wasm` | ✅ retro Canvas, arsenal, bazaar; Practice / vs Computer / 2‑tab / Online | — |
| Matchmaking + WebRTC signaling + ratings | `bt-server` | ✅ WS server, TrueSkill match quality, signaling relay, rating persistence | 2 |

Total: **83 tests passing** (81 host + 2 server).

## Play modes (all verified in Chrome via CDP)

Build & serve:
```sh
cd rust
wasm-pack build bt-wasm --target web --out-dir pkg --dev   # build the wasm
cargo run -p bt-server                                      # online matchmaking/rating server (ws://127.0.0.1:9000)
cd bt-wasm && python3 -m http.server 8000                   # then open http://localhost:8000/www/
```

- **Practice** — solo play.
- **vs Computer** — battle Ernie (the `bt-ai` opponent); his board shows alongside yours.
- **vs Player (2 tabs)** — two same‑origin tabs battle via `BroadcastChannel`.
- **Online** — WebRTC P2P data‑channel play; the server matchmakes by TrueSkill
  quality and updates/persists ratings on the result.

## Design notes

- **Determinism.** `bt-core` is seedable and side‑effect‑free; the X11/Xt
  timeout loop is replaced by an explicit `Game::tick(dt_ms)` virtual clock, so
  games are reproducible (important for replays and tests). See
  `tests/game_loop.rs`.
- **Board model.** `map_[x][y]` (a `BTBox*`‑or‑null grid) becomes
  `Vec<Option<Cell>>`; idiot detection compares filled‑this‑turn board indices
  instead of pointer identity.
- **Funds economy.** `funds = (Σ pip values across cleared rows) × (#lines)`,
  exactly as `BTBoardManager::checkLines`. Die = 1‑6 pips, happy = 150 (0 if
  it lands without clearing → frown).
- **TrueSkill 2.** A rating stays a single `(μ, σ)`; TS2 only changes inference.
  The 1v1 win/loss update *is* the classic EP closed form. The applicable TS2
  additions for a 1v1 single‑mode game are the experience offset (eq 8), the
  individual‑statistic signal (eq 9 → lines cleared), and the quit penalty
  (eq 12‑13). The paper gives no closed‑form for eq 9 and Microsoft released no
  code, so the lines signal is an explicitly bounded approximation pending a
  factor‑graph treatment.

## Build & test

```sh
cd rust
cargo test                 # whole workspace
cargo test -p bt-core      # game logic
cargo test -p bt-trueskill # ratings
rustup target add wasm32-unknown-unknown   # for the future wasm front-end
```
