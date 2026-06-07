# Glossary

BattleTris carries a lot of idiosyncratic vocabulary — some inherited verbatim
from the 1994 C++ (`die`, `happy`, `idiot`), some coined for the port's netcode
(`keyframe`, `snap-back`, `bazaar barrier`). This page is the canonical
definition for each. Terms are cross-linked from the other docs; when in doubt,
this file wins.

See also: [`overview.md`](overview.md) for the big picture,
[`ARCHITECTURE.md`](../ARCHITECTURE.md) for how the pieces connect.

---

### ack
The highest input `seq` the authoritative server has *processed* for a client
(not necessarily *applied* — a barrier-rejected input still advances `ack`).
Rides on every [snapshot](#snapshot). The [predictor](#predictor) drops every
unacked input with `seq <= ack`. "Processed up to N", not "applied N".

### arsenal
A player's owned-but-unlaunched weapons. Ten slots
(`BTArsenal`); buying a weapon in the [bazaar](#bazaar) stacks it into a slot,
and a number key launches the weapon in that slot at the opponent. Ports the
C++ `BTArsenal`.

### bazaar
The weapon shop. Triggered when the two boards' combined cleared-line count
crosses a threshold (20 lines); both players enter, spend [funds](#funds) on
weapons, and the match resumes when both press Done. Ports `BTBazaar`. While
either side shops, the match is frozen — see [bazaar barrier](#bazaar-barrier).

### bazaar barrier
The synchronization rule that **freezes the whole match while *either* side is
in the [bazaar](#bazaar)** — gravity stops and non-shopping inputs (move /
rotate / launch) from *both* sides are rejected by the server until both leave.
A barrier in the concurrency sense. Getting this right under latency was the
crux of the netcode work: an in-flight gameplay input that crossed the bazaar
boundary used to be rejected *and* never acked, hanging the match forever (the
fix: a fresh legal input advances [`ack`](#ack) the moment it is *seen*, before
the barrier check). See [`architecture-netcode.md`](architecture-netcode.md).

### Bert
The aggressive region [bot](#bot) persona: the strong line-clearing eval plus
smart weapon play (times board-raisers to when its [spy](#spy) reveals the
opponent stacked high). Counterpart to the easy-going [Ernie](#ernie).

### bot
A headless, networked player (`bt-bot`) that speaks the exact same websocket
protocol a browser does. Deployed per-fly-region over [6PN](#6pn) so the lobby
always has an opponent and the netcode gets exercised under real cross-geo
latency. Personas: [Bert](#bert), [Ernie](#ernie), [The Count](#the-count).

### bout
A single server-authoritative online match between a matched pair — a
`bt_server::bout::Bout` wrapping a [`Versus`](#versus) (both boards). The server
owns the only simulation; clients send [inputs](#input) and reconcile against
[snapshots](#snapshot).

### challenge
A *directed* invitation to a specific named player in the lobby (the
`{"type":"challenge","target":"bob"}` message), as opposed to "Find Match"
auto-pairing. The target accepts or declines (with a timeout); one in-flight
challenge per challenger. Bots are challengeable, and [The Count](#the-count)
plays primarily by issuing challenges.

### conformance trace
A behaviour sequence emitted by the TLA+/Apalache model and *replayed against
the real Rust `apply_input`* to prove the implementation matches the model.
Bridges the formal spec and the running code. See [`tla/README.md`](../../tla/README.md).

### The Count
A roaming [bot](#bot) persona (`BT_BOT_PERSONA=count`) that does not sit
passively in the lobby but issues directed [challenges](#challenge) — preferring
humans, backing off anyone who declines, dueling the regional bots when bored —
and dials its skill to each opponent's [Elo](#elo) for an even match.

### die / happy / frown
The two special falling pieces inherited from the original. A **die** lands
showing 1–6 pips and credits that many pip-values toward [funds](#funds). A
**happy** (smiley) piece is worth 150 if it lands while completing a line; if it
lands *without* clearing, it becomes a **frown** and is worth 0. Ports the
`Die` / `Happy` piece shapes.

### Elo
A single-number styling of a player's [TrueSkill](#trueskill-2) rating
(`μ − 3σ`, scaled), carried on `matchStart` so a [bot](#bot) (notably
[The Count](#the-count)) can dial its difficulty to the opponent.

### engine_sha
The git SHA of the engine build that produced a [replay](#replay). Recorded in
the replay object so playback can flag a mismatch — a [seed replay](#seed-replay)
is bit-faithful only on the *same* engine build, which is exactly what makes it
a regression test ("does this bug still reproduce on commit X?").

### Ernie
Two things, disambiguated by context: (1) the **single-player computer
opponent** — the faithful `BTComputer` port in `bt-ai`; (2) the easy-going
region [bot](#bot) persona (faithful placement, slower, no weapons), counterpart
to [Bert](#bert).

### funds
A player's money, spent in the [bazaar](#bazaar). Earned by clearing lines:
`funds = (Σ pip values across cleared rows) × (number of lines)`, exactly as
`BTBoardManager::checkLines`. [Die](#die-happy-frown) pieces add 1–6 pips; a
[happy](#die-happy-frown) piece adds 150.

### idiot
The original's name for a cell that was **filled this turn** (this lock) — used
to decide which cells participate in line/idiot bookkeeping. The C++ compared
pointer identity; the port compares filled-this-turn board indices. The flag is
flushed after `checkLines`.

### input
A single gameplay action (move / rotate / drop / launch / enter-or-leave
bazaar), the wire-level `bt_replay::Input`. Clients send only inputs; the
authoritative server validates each with `is_legal_client_input` and applies it,
which is what makes the system anti-cheat (no client can inject board state,
weapons, or [funds](#funds)).

### keep-probability
The per-piece probability that the [piece selector](#piece-manager) *keeps* a
randomly drawn piece rather than re-drawing (rejection sampling), reproducing the
original `BTPieceManager`'s non-uniform piece distribution. Transcribed verbatim.

### keyframe
An authoritative full-state restore point inside a [snapshot](#snapshot). On a
keyframe the [predictor](#predictor) overwrites its local state with the server's
and then replays its still-[unacked tail](#unacked-tail) on top. Distinct from a
plain per-tick snapshot, which only carries [`ack`](#ack) and HUD data.

### op-score
The opponent-score relay value — the channel by which one board's weapon/score
effects reach the other. In the authoritative model the server resolves every
cross-player effect; `receive_op_score` / `GameEvent::FundsStolen` are the
plumbing.

### piece-manager
`bt_core::piece_manager` (ports `BTPieceManager`): the piece selector. Uses
rejection sampling with per-piece [keep-probabilities](#keep-probability) over
the [RNG](#rng) stream to pick the next of 18 piece kinds.

### predictor
`bt_netcode::Predictor`: the shared client-side prediction + reconciliation
core, run by *both* the browser ([`WasmClient`](#wasmgame-wasmvscomputer-wasmclient))
and the [bot](#bot). `predict` applies an input locally and queues it
[unacked](#unacked-tail); `on_snapshot` drops acked inputs and, on a
[keyframe](#keyframe), restores-then-replays. One implementation, property-tested
once. See [`architecture-netcode.md`](architecture-netcode.md).

### quiesce-in-place
The deploy strategy: drain the single always-on server (stop accepting new
matches), wait — uncapped — for live [bouts](#bout) to finish, then swap the
binary. No blue-green, because the server has a single attached volume.
See [`deployment.md`](deployment.md) and `deploy-quiesce.sh`.

### RNG
The deterministic random source: a faithful port of the POSIX
`drand48` / `lrand48` / `rand` LCG (`bt_core::rng`). Seedable, side-effect-free —
the foundation of [determinism](#seed-replay) and replays.

### seed replay
A [replay](#replay) that records *only* the seed plus the timestamped input
sequence (`{seed, mode, ai_level, dt_ms, engine_sha, frames}`) and regenerates
everything else (gravity, Ernie's moves, RNG) by re-running the deterministic
engine. Faithful only on the same [`engine_sha`](#engine_sha). See
[`replays.md`](replays.md).

### replay
A recorded game (a [seed replay](#seed-replay)) that triples as a bug-report
trace, replay-library content, and a deterministic test case. Online
[bouts](#bout) are recordable because the server holds a totally-ordered input
log. See [`replays.md`](replays.md).

### snapshot
The authoritative frame the server ships to each client each tick:
`{tick, ack, you, opp, result, keyframe?}`. Carries the [`ack`](#ack), both
sides' HUD state, and — periodically — a [keyframe](#keyframe) for full
reconciliation.

### snap-back
The desync bug where a predicted-but-not-yet-acked input was *lost* when the
client reconciled to a [keyframe](#keyframe) — the piece visibly snapped back to
an older position. Fixed by replaying the [unacked tail](#unacked-tail) on top of
the restored keyframe state, so the predicted move survives the restore.

### spy
A reconnaissance weapon (William Ames / Ace of Spies / The Condor) that reveals
the opponent's board for a duration. In the authoritative model the *server*
decides what each client sees, so a spy is server-enforced.

### TrueSkill 2
The Bayesian rating system (`bt-trueskill`). A rating is a Gaussian belief over
latent skill, `μ ± σ`; the 1v1 win/loss update is the classic TrueSkill
closed form, with the TrueSkill-2 additions that apply to a 1v1 single-mode game
(an experience offset, a lines-cleared performance signal, a quit penalty). The
lobby [Elo](#elo) is `μ − 3σ` styled. Defaults `μ=25`, `σ=25/3`.

### unacked tail
The queue of [inputs](#input) a client has predicted locally but the server has
not yet [acked](#ack). Preserved across a [keyframe](#keyframe) restore (replayed
on top) — that replay is precisely what prevents the [snap-back](#snap-back).

### Versus
`bt_core::Versus`: the two-board container holding both players' `Game`s plus the
cross-player resolution (deliver weapon, spy launches, dirty flag). The
authoritative [bout](#bout) owns a `Versus`; it is also what the versus replay
player reconstructs.

### WasmGame / WasmVsComputer / WasmClient
The three wasm-bindgen surfaces (`bt-wasm/src/lib.rs`) the browser drives:
**`WasmGame`** wraps a single `bt_core::Game` (Practice, 2-tab relay);
**`WasmVsComputer`** wraps `bt_ai::VsComputer` (vs-Computer — 100% client-side,
no server); **`WasmClient`** runs the [`Predictor`](#predictor) for online play.
(Plus `WasmReplayPlayer` / `WasmVersusReplayPlayer` for playback.)

### WPN_ON / WPN_OFF
The original C++ flags marking a weapon effect as active / expired on a board.
The port carries the same on/off lifecycle (a twice-launched weapon must not
"stick"); weapon durations are counted in lines, not wall-clock.

### 6PN
Fly.io's private IPv6 network ("six private network"). The region [bots](#bot)
reach the server over 6PN, which is why the server binds `[::]`. Shared
`BT_JWT_SECRET` across the server and the bots app.
