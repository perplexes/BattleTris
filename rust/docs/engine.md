# The engine: `bt-core` internals

`bt-core` is a **faithful, deterministic port** of the 1994 BattleTris game logic
(the C++ under `usr/src/game/`, by Bryan Cantrill et al.). It has no platform, UI,
or network dependencies — it is the pure rules engine consumed by the WASM
front-end ([`bt-wasm`](../bt-wasm)), the AI ([`bt-ai`](../bt-ai)), the netcode
([`bt-netcode`](../bt-netcode)), and the replay layer ([`bt-replay`](../bt-replay)).
It is intentionally **dependency-free**: even serialization is a hand-rolled `i64`
codec, no serde in core.

This page covers the engine internals. See also:
[architecture-netcode.md](architecture-netcode.md) (how the netcode drives this
engine and reconciles it), [replays.md](replays.md) (the seed-replay contract this
engine's determinism makes possible), [testing.md](testing.md) (the proptest/oracle
suite), `docs/weapons.md` (the 34-weapon system), and `ARCHITECTURE.md` (the crate
map).

The module map mirrors the original C++ classes (from
[`bt-core/src/lib.rs`](../bt-core/src/lib.rs)):

| Module | Ports |
|---|---|
| [`constants`](../bt-core/src/constants.rs) | `BTConstants.H` |
| [`cell`](../bt-core/src/cell.rs) | `BTBox` + subclasses |
| [`piece`](../bt-core/src/piece.rs) | `BTPiece` + subclasses |
| [`piece_manager`](../bt-core/src/piece_manager.rs) | `BTPieceManager` |
| [`board`](../bt-core/src/board.rs) | `BTBoardManager` |
| [`weapons`](../bt-core/src/weapons.rs) | `BTWeaponToken`, `BTActive[]`, `BTWeapon` |
| [`rng`](../bt-core/src/rng.rs) | `rand` / `drand48` / `lrand48` |
| [`game`](../bt-core/src/game.rs) | `BTGame` (+ `BTScore`) |
| [`versus`](../bt-core/src/versus.rs) | the cross-player relay (`BTCommManager`) |
| [`arsenal`](../bt-core/src/arsenal.rs) | the per-player weapon arsenal |

---

## Determinism: the explicit virtual clock

The original is driven by **Xt timeouts** (`BT_DROP_TIMEOUT`, `BT_SLIDE_TIMEOUT`,
…) — a real-time, event-loop timer. For a headless, reproducible engine the port
replaces that timer loop with an explicit **virtual clock**:
[`Game::tick(dt_ms)`](../bt-core/src/game.rs) advances the clock by `dt_ms` and
fires drop/slide steps as their intervals elapse. Each frame the host calls `tick`
and feeds input events.

The engine is therefore:

- **Seedable** — `Game::new(seed)` fully determines the run (see the RNG below).
- **Side-effect-free** — no I/O, no wall clock, no threads. The host owns the clock.
- **Fixed-timestep** — faithful replay requires advancing in constant `dt_ms`
  steps (an accumulator loop), not wall-clock frame deltas. The host's step is
  [`bt_wasm::FIXED_DT_MS`](../bt-wasm/src/lib.rs)` = 16` (and the server's
  [`bout::TICK_MS`](../bt-server/src/bout.rs)` = 16`), so one real interval = one
  deterministic step.

`tick` is a no-op while paused, in the bazaar, after game over, or for
`dt_ms <= 0`. Inside, it accumulates `dt_ms` and steps drop/slide once per elapsed
interval (the `while`/`loop` accumulator guards against zero/negative intervals;
slide time can legitimately be 0 under the **No Slide** weapon, which locks
immediately).

The phase enum is `Falling` / `Sliding` / `Over` — only one of the drop or slide
timers is "live" for the falling piece at a time, matching the original's separate
`BT_DROP_TIMEOUT` / `BT_SLIDE_TIMEOUT`.

---

## The POSIX RNG port

[`rng.rs`](../bt-core/src/rng.rs) reproduces the POSIX `drand48` family and `rand()`
the original draws from, so the piece stream and all randomized weapon effects are
bit-identical across platforms. It is a **48-bit linear congruential generator**:

- state update `X = (A·X + C) mod 2^48`, with `A = 0x5DEECE66D`, `C = 0xB`;
- `srand48(seed)` semantics: the high 32 bits of state come from the low 32 of
  `seed`, the low 16 are `0x330E`;
- `rand()` / `lrand48()` return the top 31 bits (`X >> 17`); `drand48()` returns
  `X / 2^48` in `[0, 1)`; `rand_below(n)` is the C++ `rand() % n` idiom.

The exact LCG step is pinned by `test_lcg_step_verification` (seeding with 0 must
land state on `0x330E`, then the first step must equal `(A·0x330E + C) & (2^48-1)`).
The raw state is exposed via [`Rng::raw`](../bt-core/src/rng.rs)/`from_raw` so the
keyframe codec can serialize it — without it, a restored game would draw a different
piece stream and diverge (the whole reason keyframes capture the RNG, see below).

The original's RNG consumption order is preserved exactly — which draw happens when
matters, because consuming an extra `drand48()` would desync every subsequent piece.

---

## The board model

[`Board`](../bt-core/src/board.rs) is the standard **10×28** playfield:
`Vec<Option<Cell>>`, row-major (`index = y*width + x`, `None` = empty). It carries
its own `BTActive[]` flags (consulted by the FallOut/Bottle/Force/Upbyside
mechanics), the Upbyside flip (`upside`), the computer-board flag (`computer`), and
the idiot bad-move latch (`idiot`/`reason`). A `Cell` is a tagged value
(`BTBox` + subclasses); the die cell carries a pip value 1–6.

Line clears go through [`Board::check_lines`](../bt-core/src/board.rs), which returns
a [`LineClear`](../bt-core/src/board.rs) `{ lines, value, funds }` — the number of
cleared rows, the summed pip value, and the funds earned.

---

## Pieces and selection

There are **18 piece kinds** (1..=18): the 7 standard tetromino-likes, the die, the
happy piece, the "weird" set (Dog, RevDog, Cap, Wall, Tower, Star, WeirdLong), the
four-by-four, and the long-dong. [`PieceManager`](../bt-core/src/piece_manager.rs)
ports `BTPieceManager` and selects the next piece by **rejection sampling against
per-piece keep probabilities** (`BTPieceManager::create`):

```text
if !hap_on && (!broken || lrand48() % BT_BROKEN_PROB == 0):
    loop { i = rand_below(BT_MAX_PIECES) + 1; if drand48() < keep_prob[i] { break } }
else if !hap_on && broken:   i = old_piece          # Broken Record repeats
else:                        hap_on -= 1; i = HAP    # Have a Nice Day forces happy
```

The default keep-probs (`new`/`reset`): standard pieces `0.21`
([`BT_DEFAULT_KEEP_PROB`](../bt-core/src/constants.rs)), the die `1.0`, happy and
long-dong `0.02` (`BT_EXOTIC_KEEP_PROB`), weird/4×4 `0.0` (never drawn by default).
Piece-stream weapons (Feared Weird, Four-by-Four, No Dice, So Long, Have a Nice Day,
Broken Record) flip these keep-probs in `weapon_on`/`weapon_off`. The die's pip
value is `rand_below(6) + 1`, drawn **only** when the selected piece is the die —
preserving the original's RNG-consumption order. A guard handles the degenerate case
where Broken Record activates before any piece has spawned (`old_piece == 0`);
unreachable in real play, so it never perturbs RNG order in a live game.

---

## The lock / line-clear / funds path

When a slide expires and the piece can't move down, [`Game::place`](../bt-core/src/game.rs)
runs the lock sequence, faithfully ordered to match the original's `BT_LINE`
manager-ring (`BTGame.C:400-406`):

1. **Airslide** detection (a fast drop that slid into place without being able to
   move back up — `BT_AIRSLIDE`).
2. **Lock** the piece into the board (`p.land`), which fills cells and sets the idiot
   bad-move flag.
3. **`check_lines`** → add to the line count and **credit the funds**
   ([`credit_clear_funds`](../bt-core/src/game.rs)).
4. Flush the idiot flag *after* `check_lines` (a cleared line un-flags "idiot";
   near-death/missed-smiley are set by `check_lines` itself).
5. If lines cleared: **open the bazaar first** ([`update_bazaar`](../bt-core/src/game.rs)),
   **then expire weapon durations** ([`tick_durations`](../bt-core/src/game.rs)). The
   order matters — `BTScoreManager` (the bazaar trigger) precedes `BTWeaponManager`
   (duration expiry) in the original's manager ring, so a single line that both
   opens the bazaar and would expire Carter charges the still-doubled Carter price.
6. Publish the `Scored` event, **flush pending weapons** the opponent launched, then
   **spawn** the next piece (or top out).

### The Mondale tax: a deliberate correctness fix

`credit_clear_funds` applies the Mondale tax (the victim keeps `floor(70%)`,
[`BT_MONDALE_RATE`](../bt-core/src/constants.rs)` = 0.30`) and emits a
[`GameEvent::FundsStolen`](../bt-core/src/game.rs) for the exact remainder. The
1994 original reconstructed the attacker's cut from the victim's already-truncated
funds delta sent over the P2P wire — a second independent truncation that **destroys
up to 2 funds per clear** (the victim loses more than the attacker gains). With full
information at the authoritative relay, the port transfers the **exact remainder**
instead, so the tax *conserves* money. A handful of these correctness-over-
faithfulness fixes are documented inline in [`game.rs`](../bt-core/src/game.rs) and
in `docs/faithfulness.md` (e.g. the Speedy/Meadow drop-time lifecycle leak made
idempotent on the active flag).

---

## The game-vs-piece position-sync invariant (the mid-air-lock footgun)

The single subtlest engine invariant: the game tracks the falling piece's position
in **two places** that must stay in lockstep — `self.x`/`self.y` (read by
collision/lock logic) and the piece's own `p.x`/`p.y` (read by render and `p.land`).

The footgun lives in the "slid off the edge in time" branch of
[`Game::place`](../bt-core/src/game.rs): when the lock-delay expires but the piece
can still move down (it was slid over a hole during the 150 ms slide window), the
piece **resumes falling** instead of locking. That branch must advance *both*
positions:

```rust
self.y += self.delta_y;
p.x = self.x;
p.y = self.y;   // omitting this let a piece lock one row above where it rendered
```

Omitting the `p.y` sync let a piece lock one row *above* where it rendered —
resting in mid-air. The guarantee ("nothing locks with empty space under every
cell") is pinned by the unit test
`a_piece_that_can_still_fall_resumes_falling_not_locks_midair` in
[`game.rs`](../bt-core/src/game.rs), and by a property test in the suite that scans
every frame asserting `(game.x, game.y) == (piece.x, piece.y)` whenever a piece is
falling. The bug was originally chased down with `dump_replay` (below).

---

## Keyframes: the full-game codec

Server-authoritative online play needs a **complete** engine snapshot the client can
restore and re-simulate its unacked inputs on top of (see
[architecture-netcode.md](architecture-netcode.md)). A board grid alone is only a
render view — it omits the falling piece, phase/timers, the RNG + piece-manager
state, and the weapon flags/pending queue, all of which drive the deterministic
stream.

[`Game::snapshot`](../bt-core/src/game.rs)/[`restore`](../bt-core/src/game.rs)
capture the whole `Game` as a flat `i64` stream (versioned `KEYFRAME_VERSION`),
with `snapshot_bytes`/`restore_bytes` as the little-endian wire form (the stream
includes `keep_prob` f64 bit-patterns that exceed 2^53, so it can't ride as JSON
numbers — bytes round-trip exactly). `restore` reads into locals via a
bounds-checked cursor and only commits if the keyframe is well-formed and fully
consumed, so a malformed/truncated keyframe leaves the game **untouched** (returns
`false`, never panics). [`client_keyframe_bytes`](../bt-core/src/game.rs) is the
client-visible form with `op_funds` redacted (the opponent's funds must not leak
through reconciliation). Round-trip and deterministic-continuation guarantees are
pinned by `keyframe_round_trips_exactly` and
`keyframe_enables_deterministic_continuation`.

---

## `dump_replay`: the replay debugger

[`bt-replay/examples/dump_replay.rs`](../bt-replay/examples/dump_replay.rs) is a
diagnostic tool built while chasing the mid-air-lock bug and kept as a reusable
debugger. Given a stored `VersusReplay` JSON, it can:

- **Dump both boards at a tick** (`#` = locked, `O` = the falling piece), labeling a
  position desync inline:
  ```sh
  cargo run -p bt-replay --example dump_replay -- /tmp/r.json 231
  ```
- **Scan every tick** for a game-vs-piece position desync — the exact class of bug
  that lets a piece lock floating:
  ```sh
  cargo run -p bt-replay --example dump_replay -- /tmp/r.json
  ```

Get a stored replay's JSON from the live server:
`curl https://battletris.fly.dev/api/replays/<id> -o /tmp/r.json`. The replay format
and routes are documented in [replays.md](replays.md).
