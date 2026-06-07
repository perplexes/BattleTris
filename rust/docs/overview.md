# Overview

## What BattleTris is

BattleTris is a two-player networked Tetris-battler. You drop pieces and clear
lines like Tetris, but every cleared line earns **funds**, and when the two
boards' combined line count crosses a threshold the match pauses at a weapons
**bazaar** where both players spend those funds on weapons. You then launch
weapons at your opponent — flip their screen, swap boards, spy on them, hand them
disjointed pieces, raise their stack — to bury them first. First to top out
loses. (You can also play the computer for practice, unranked.)

It was written at Brown University as a CS32 final project in spring 1994 by
Bryan Cantrill, Charlie Hoecker, and Mike Shapiro, for Solaris/SPARC over X11 and
Motif, with TCP daemons for the lobby and a flat-file rankings database. See the
[root README](../../README.md) for the full lineage and Cantrill's
[reunion blog post](https://bcantrill.dtrace.org/2026/05/25/a-portentous-reunion/);
the original C++ lives under [`usr/src/`](../../usr/src) and
[`PORTING.md`](../../PORTING.md) covers building it.

## The port's thesis

This is a faithful Rust + WebAssembly port with one clean split:

- **The game *logic* is ported faithfully.** Board geometry, the 18 piece shapes
  and their rotation, the piece-selection keep-probabilities, the
  funds/[die](glossary.md#die-happy-frown)/[happy](glossary.md#die-happy-frown)
  economy, the 20-combined-line bazaar trigger, the line-clear recheck, and the
  34-weapon roster are transcribed from the 1994 source. Each Rust module names
  the C++ class it ports (`board.rs` ⇐ `BTBoardManager`), and constants come
  verbatim from `BTConstants.H`. Where the port diverges, it is documented — see
  [`faithfulness.md`](faithfulness.md).

- **The *platform* is modernized.** X11/Motif becomes an HTML5 Canvas front-end
  (WASM-driven); the TCP daemons become an [authoritative WebSocket
  server](architecture-netcode.md); the flat-file ELO database becomes
  [TrueSkill 2](glossary.md#trueskill-2) matchmaking. One deliberate departure
  from the original is online play: the 1994 game relayed gameplay peer-to-peer,
  while this port makes the **server authoritative** — a conscious modernization
  for anti-cheat and a totally-ordered, replayable event log, not a faithfulness
  goal.

The enabling idea under both halves is **determinism**: the engine
([`bt-core`](engine.md)) is seeded and side-effect-free, advanced by an explicit
virtual `tick(dt)` clock instead of an X11 timeout loop. That single property
buys reproducible games, [replays](replays.md), property-tested netcode, and
TLA+ conformance.

## Where to go next

- New to the codebase? Read [`ARCHITECTURE.md`](../ARCHITECTURE.md) — the crate
  map and the "where to start reading" entry points.
- Want to play or run it? [`quickstart.md`](quickstart.md).
- The vocabulary (bazaar barrier, keyframe, snap-back, op-score, idiot…) is in
  the [glossary](glossary.md).
- The long-form, illustrated write-ups (netcode, weapons codex, TLA+ explainer,
  the faithful-Motif redesign) are collected in the **project dossier**,
  `screenshots/index.html`, served over Tailscale.

It is live at **<https://battletris.fly.dev>**.
