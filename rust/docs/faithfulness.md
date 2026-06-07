# Faithfulness to the 1994 original

BattleTris is a port of the 1994 Brown CS32 networked Tetris-battler from its
original pre-standard C++/X11/Motif source (under [`usr/src/`](../../usr/src)) to
Rust + WebAssembly. The guiding contract is:

> **Port the game *logic* verbatim; reimagine the *platform*.**

The board geometry, piece shapes and rotation, the keep-probability piece selection,
the funds/die/happy economy, the line-clear recheck, and the weapon roster are a
direct, byte-faithful port of `usr/src/game/`. The things *around* the game — the
display, the network, the rating store — are deliberately re-platformed:

| Original (1994) | This port |
|-----------------|-----------|
| X11 / OSF/Motif widgets | HTML5 Canvas + a Motif-faithful CSS skin |
| Xt timeout event loop | explicit `Game::tick(dt_ms)` fixed-timestep virtual clock |
| TCP peer-to-peer game relay (`BTCommManager`) | a server-**authoritative** simulation over a WebSocket |
| `btserverd`/`btslaved` lobby daemons | the `bt-server` axum lobby/matchmaker |
| flat-file ELO high-score DB | TrueSkill 2 ratings (SQLite) |
| sampled audio (`BTSoundManager`) | synthesized Web Audio chiptune blips |

The long-form rationale for these choices is in the project dossier:
[`screenshots/redesign-plan.html`](../../screenshots/redesign-plan.html),
[`screenshots/motif-references.html`](../../screenshots/motif-references.html), and
the deferred-decisions writeup
[`screenshots/decisions-d.html`](../../screenshots/decisions-d.html). For the build
of the original C++ and its class map, see the root
[`PORTING.md`](../../PORTING.md).

## One deliberate departure: server-authoritative, not peer relay

The 1994 game's *gameplay* was actually **peer-to-peer relay** — the daemons did
lobby/matchmaking/high-scores only and never processed a game token; the challenger
opened a direct socket to the opponent and weapons/scores/boards/arsenals were sent
straight over it (`BTNetManager.C`, `BTCommManager.C`, `BTSlave.C`). So a P2P port
would have been the *faithful* model.

We chose **server-authoritative + client prediction** anyway — a conscious
modernization, not an accident — for three things peer relay can't give:

1. **Anti-cheat.** Clients send only legal player inputs; the server resolves every
   cross-player effect. A modified client can't inject weapons, funds, board state,
   or request a board it didn't earn.
2. **A totally-ordered event log**, which *is* the canonical online replay (closed
   the long-standing "online games aren't recordable" gap, D5).
3. Dropping WebRTC entirely.

This is the one place the port knowingly diverges from the original's network model.
The reconciliation netcode is documented in
[`architecture-netcode.md`](architecture-netcode.md).

## The codex audit and its fixes

A codex audit compared the port line-by-line to the C++ and confirmed faithful board
geometry, piece shapes + rotation, spawn offset, keep probabilities, the line-clear
recheck, and the funds economy. It also surfaced real divergences, all since fixed:

- **Slide Denied (No Slide)** now locks instantly (was allowing the slide it's meant
  to deny).
- **Weapon-active flags are boolean (0/1)**, not a stack counter — a twice-launched
  duration weapon no longer sticks active forever.
- **Slick Willy** is suspended during a hard-drop / slide (it no longer fights the
  drop).
- The **idiot flag** (a filled-this-turn marker) is flushed after `checkLines`.
- **Mondale '96** applies its victim-side tax (and now also credits the attacker).
- The bazaar shows the **Carter-doubled price** through `bazaar_price`.

A later adversarial weapon-PBT pass found and fixed two more *correctness* (not
faithfulness) bugs: a Speedy/Meadow gravity-stacking leak (relative speed-up
re-firing on each launch but reverting only once) and Bottle silently destroying the
funds/lines from a row its neck walls completed.

## Cross-player weapons & attacker economics — now CLOSED

> Earlier drafts of this document (and the old `rust/README.md`) listed the
> board/arsenal-exchange weapons and launcher-side economics as *known gaps*. They
> have since shipped. **Do not treat the items below as open.**

Reconciled against the project's current-state ledger, these are **done and
deployed**:

- **Swap Meet, Lazy Susan, Mirror Mirror, and the spies (Ames / Ace / Condor)** — all
  six cross-player weapons are fully implemented, including online, resolved
  authoritatively by `bt_core::Versus` and the server `Bout`. The spies render a
  server-degraded opponent board (Ames 50% / Ace 15% / Condor 0% hidden). See
  [`weapons.md`](weapons.md).
- **Mirror is offensive** (deploy-at-opponent curse with the nine-weapon nullify set
  and backfire-on-cursed-launcher), matching the original and the deployer's-POV
  flavor text — not the earlier defensive self-buff.
- **Launcher-side economics** — Mondale's 30% cut and Keating's seizure now *credit
  the attacker* via `GameEvent::FundsStolen` + `add_funds`, not just debit the
  victim.
- **Online replays** record the full match (the server's ordered input log).

## Known remaining divergences

These are the divergences that *are* still open or are deliberate. Most are small and
documented in code.

- **Mondale funds rounding.** With full match state on one host we conserve money
  exactly (the attacker gains what the victim lost). The original P2P relay sent a
  truncated funds delta and could *destroy* up to 2 funds per clear. We diverge from
  the binary by ≤2 funds/clear, by design (`credit_clear_funds`).
- **Weapon-timing model.** A Mirror backfire and a Keating seizure apply at the
  launcher's **next lock**, not immediately — consistent with the port's
  queue-at-lock (`weapq_`) event model. Both code reviewers agreed this is timing,
  not a correctness bug.
- **Lawyer's Delite** raises the board per opponent line (the faithful *effect*) but
  is not the exact piece-aware lock/slide of `BTGame::lawyers()`.
- **Board codec is id-only** (decision D1, KEPT). The original's lossy id-only board
  encoding was a 1994 bandwidth limit; we keep a deliberately simple flat-i32 codec
  for the same render-view purpose and note it as an intentional divergence.
- **A persistent opponent board** (the vs-Computer / online second board) is a port
  addition — the original only ever showed your *own* board, revealing the
  opponent's briefly via a spy weapon.
- **AI (`bt-ai`) for single-player Ernie is a faithful-in-spirit approximation.**
  `eval_board`'s dominant terms are faithful but it omits the happy-piece bonus,
  baseline-delta, and weapon-flag inputs; placement is a column×orientation
  simulation rather than the reachable-move DFS; bazaar buying is a greedy heuristic
  rather than a port of `goShopping`/`BTCOrders`. An exact AI eval-differential is
  **deferred on purpose** — it would diverge by design — and would only be revisited
  if the eval were made exactly faithful (a separate decision). (The *region bots*
  use a separate, intentionally stronger non-faithful eval; see
  [`deployment.md`](deployment.md).)

## UI geometry: native 1:1, no upscaling

Faithful rendering means the board is drawn at its **native 1:1 size**, never
upscaled. From the original X11/Motif source:

- `BT_BOARD_WTH = 10`, `BT_BOARD_HGT = 28`, `BT_BOX_WTH = BT_BOX_HGT = 23`,
  `BT_BOX_BRDR = 3` (`BTConstants.H`).
- `BTGame.C` sizes the board drawing area to exactly `23 × 10 = 230` px wide by
  `23 × 28 = 644` px tall; `BTBox.C` blits **23-px cells with no scaling**. So the
  on-screen board is **230 × 644**.
- The original game window is 670 × 700, two columns: **board on the left**, score
  box above arsenal box on the right.

**Rule:** render the well at native 23-px cells; do **not** upscale it. On a 2×
display the browser's own device scaling already matches the original's apparent
size. (The desktop board had wrongly been upscaled 1.6× at one point; that was
reverted to native 1.0×, capped — it only shrinks *below* native when a short
viewport would push the footer counter below the fold.) The faithful Motif widget
styling (computed gray-on-gray bevels from OSF/Motif `Xm/Color.c`, the custom
`XmScrolledList` scrollbar) is detailed in
[`screenshots/motif-references.html`](../../screenshots/motif-references.html) and in
[`frontend.md`](frontend.md).

## Related

- [`weapons.md`](weapons.md) — the weapon system the cross-player closures landed in.
- [`frontend.md`](frontend.md) — the Canvas/Motif render path that realizes this
  geometry.
- [`architecture-netcode.md`](architecture-netcode.md) — the authoritative model that
  replaced the peer relay.
- [`engine.md`](engine.md) — the deterministic `bt-core` engine the faithful logic
  lives in.
