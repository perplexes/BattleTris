# Weapons & the bazaar economy

BattleTris is Tetris with a war chest. Clearing lines earns **funds**; at every
bazaar you spend those funds on **weapons** that sabotage your opponent (or, for a
few, swap state between the two boards). This page documents the *system* — the
economy, the arsenal, how a launch is routed, and how the server resolves the
cross-player weapons.

For the per-weapon catalog (every weapon's flavor text, exact mechanic, and the
1994 American-politics in-joke it's named after), see the project dossier:
[`screenshots/weapons-codex.html`](../../screenshots/weapons-codex.html) and the
labeled showcase replays in [`screenshots/weapon-showcase.html`](../../screenshots/weapon-showcase.html).
Faithfulness notes (what the port changed vs the 1994 original) live in
[`faithfulness.md`](faithfulness.md).

Source of truth: [`rust/bt-core/src/weapons.rs`](../bt-core/src/weapons.rs) (the
table), [`rust/bt-core/src/game.rs`](../bt-core/src/game.rs) (per-board effects),
[`rust/bt-core/src/versus.rs`](../bt-core/src/versus.rs) (cross-player routing),
and [`rust/bt-server/src/bout.rs`](../bt-server/src/bout.rs) (authoritative
resolution). All numbers are transcribed verbatim from the original
`usr/src/share/btweapons.db` / `btweaponsp.db`.

## The economy

### Funds

You bank funds by clearing lines. The formula is faithful to
`BTBoardManager::checkLines`:

```
funds earned = (Σ pip values across the cleared rows) × (number of lines cleared)
```

The pip value of a row comes from the special blocks that land in it:

- **The die piece** (`BT_DIE_PIECE`) shows **1–6 pips**; a die block in a cleared
  row contributes its face value (`BT_DIE_1..BT_DIE_6`).
- **The happy face** (`BT_HAPPY`) is worth **150** (`BT_HAPPY_VAL = 150`) — *if* it
  lands in a row that actually clears. If a happy face locks without clearing its
  row, it becomes a **frown** (`BT_UNHAPPY`) and is worth nothing. ("Why give your
  opponent the opportunity to make an extra 150 beans?" — the Have-a-Nice-Day
  flavor text.)
- Ordinary blocks contribute their normal value.

### The bazaar trigger

The bazaar opens on **combined lines**: `BT_LINES_TIL_BAZ = 20`. `lines_til_bazaar`
counts down across *both* players' line clears (your clears plus the opponent's,
relayed via `receive_op_score`), so every 20 combined lines both sides are sent
shopping. The bazaar is a **synchronized barrier**: while either side is in the
bazaar the whole match freezes (gravity stops, inputs other than buy/sell/leave are
rejected) until both sides leave. The barrier and its netcode subtleties (the
latency deadlock, the snap-back fix) are covered in
[`architecture-netcode.md`](architecture-netcode.md).

### Durations are measured in lines

A weapon's `duration` is **lines, not seconds**. A duration-`N` weapon stays active
until `N` more lines pass under it, then expires (the active flag flips back off).
`duration: 0` means an **instant** effect (it fires once and is done — no ongoing
flag). Spy durations also count in lines, decrementing on the *opponent's* clears.

### Prices, taxes, inflation

- Each weapon has a fixed `price` in funds (e.g. Swap Meet 1200, The Gimp 25).
- **Carter Years** doubles the *victim's* bazaar prices while active; the UI reads
  the effective price through `bazaar_price(token)`, which honors Carter.
- **Mondale '96** taxes the victim 30% (`BT_MONDALE_RATE = 0.30`) of newly-banked
  funds; **Keating Five** seizes all of the victim's funds. Both now also *credit
  the attacker* (see "launcher-side effects" below).

## The 34-weapon system

There are exactly **34 weapons**, `WeaponToken` 0..=33 (`BT_MAX_WEAPONS = 34`). The
enum order is **load-bearing** — the discriminant is the protocol index used on the
wire, to index the arsenal, and to index the `BTActive[]` active-flag array — so it
matches the original `BTWeaponToken` enum byte-for-byte:

| # | Token | Name | Price | Duration (lines) |
|--:|-------|------|------:|------:|
| 0 | FearedWeird | The Feared Weird | 400 | 3 |
| 1 | FourByFour | Four-by-Four | 425 | 10 |
| 2 | Hatter | The Mad Hatter | 375 | 5 |
| 3 | Upbyside | Upbyside-down | 125 | 10 |
| 4 | FallOut | Fallout | 250 | 10 |
| 5 | Swap | Swap meet | 1200 | 0 |
| 6 | Lawyers | Lawyer's delite | 350 | 5 |
| 7 | RiseUp | Rise up | 75 | 0 |
| 8 | FlipOut | Flip out | 15 | 0 |
| 9 | Speedy | Speedy Gonzales | 275 | 10 |
| 10 | Missing | Missing Pieces | 50 | 0 |
| 11 | PieceIt | Piece It Together | 100 | 0 |
| 12 | Blind | The Blind Cleric | 400 | 0 |
| 13 | Mondale | Mondale '96 | 150 | 50 |
| 14 | Keating | Keating Five | 425 | 0 |
| 15 | Carter | Carter Years | 250 | 20 |
| 16 | Reagan | Reagan Era | 425 | 0 |
| 17 | Ames | William Ames | 50 | 20 |
| 18 | Ace | Ace of Spies | 100 | 30 |
| 19 | Condor | The Condor | 225 | 40 |
| 20 | NiceDay | Have a Nice Day | 50 | 0 |
| 21 | SoLong | So Long | 100 | 10 |
| 22 | NoDice | No Dice | 600 | 35 |
| 23 | Bug | Bug Report | 320 | 0 |
| 24 | Bottle | Bottle neck | 150 | 10 |
| 25 | NoSlide | Slide Denied | 125 | 10 |
| 26 | Susan | Lazy Susan | 600 | 0 |
| 27 | Meadow | Meadow | 475 | 10 |
| 28 | Mirror | Mirror Mirror | 500 | 10 |
| 29 | Twilight | The Twilight Zone | 450 | 0 |
| 30 | Slick | Slick Willy | 650 | 3 |
| 31 | Broken | Broken Record | 325 | 5 |
| 32 | Force | The Force | 325 | 5 |
| 33 | Gimp | The Gimp | 25 | 0 |

(Effects and the political joke behind each name are in
[`screenshots/weapons-codex.html`](../../screenshots/weapons-codex.html).)

## The arsenal: 10 slots, launch by number

Your **arsenal** (`BTArsenal`, `BT_ARSENAL_SIZE = 10`) holds up to **10 distinct
weapon kinds**. Buying the same weapon again **stacks** the quantity in its existing
slot rather than taking a new slot; a slot empties (and frees up) when its last copy
is used. Selling (the bazaar "Remove" button) refunds the *effective* price (Carter
included) and removes one copy.

In play you **launch by slot number** — `launch_weapon(slot)` consumes one copy of
the weapon in that slot and fires it at the opponent (`BTWeaponManager::launchWeapon`).
You can't launch while in the bazaar or after game over.

## Active flags: 0/1, not a counter

Each board keeps the original's `BTActive[]` array (`ActiveFlags`): "is weapon `T`
currently in effect on me?" The original sets this to **1 on `BT_WPN_ON`** and **0
on `BT_WPN_OFF`** — it is a boolean, *not* a stack count. Launching the same
duration weapon twice and letting it expire once must leave it **inactive** (this
was a real bug — a twice-launched weapon stuck active forever — and is now pinned by
a regression test). The raw counts are serialized into the keyframe so a reconnecting
client restores every active effect exactly.

## Victim-side vs launcher-side effects

Most weapons are **victim-side**: the effect lands on the opponent's board. Delivery
is deferred to the victim's **next lock** (the port's `weapq_` model — a launched
weapon is queued and applied when the target's next piece locks), not applied
mid-air.

A few effects are **launcher-side** (they pay or affect the player who fired):

- **Mondale '96** — the 30% tax the victim loses is *credited back to the attacker*
  (`GameEvent::FundsStolen` → the relay calls `add_funds` on the launcher). The
  victim re-grosses from its already-truncated funds delta, faithful to
  `BTScoreManager.C` to within ≤2 funds per clear (a documented rounding divergence;
  see [`faithfulness.md`](faithfulness.md)).
- **Keating Five** — the victim's funds are seized *and handed to the attacker* (same
  `FundsStolen` credit path).
- **The spies** (Ames / Ace / Condor) reveal the *opponent's* board **to the
  launcher** — they don't damage the opponent at all (see below).

## The 6 cross-player weapons

Six weapons can't be expressed as "a flag on one board" — they move state *between*
the two boards. These were the last gap to close (they were token-only no-ops at one
point); all six are now fully implemented, including online:

| Weapon | What crosses |
|--------|--------------|
| **Swap Meet** (5) | Exchanges the two boards wholesale (and clears Bottle/Upbyside on both). |
| **Lazy Susan** (26) | Swaps the two players' *arsenals*. |
| **Mirror Mirror** (28) | Curses the opponent so their own launches backfire (see nullify/reflect below). |
| **William Ames** (17) | Spy: reveals ~50% of the opponent's board to you. |
| **Ace of Spies** (18) | Spy: reveals ~85% of the opponent's board (15% hidden). |
| **The Condor** (19) | Spy: reveals the opponent's board with perfect accuracy. |

### How the authoritative server resolves them

In the 1994 game these were exchanged peer-to-peer between the two clients. This port
is **server-authoritative**: there is no client-to-client channel, so a client can
*never* request a board it didn't earn or mutate the opponent's state. The whole
cross-player relay lives in one place — `bt_core::Versus`, which owns both boards and
ticks them in lockstep — driven by the server's `Bout`.

- **Swap / Susan** are symmetric exchanges applied directly to both `Game`s
  (`swap_board_with` / `swap_arsenal_with`). An initiator guard (`swapper_`) stops a
  simultaneous double-swap from racing, and the server's total input ordering
  serializes them anyway.
- **Mirror** is routed through `deliver_weapon`: launching Mirror normally curses the
  opponent; while a player is mirror-cursed, every weapon *they* launch is caught by
  their own curse (see the nullify/reflect set).
- **The spies** are resolved entirely server-side. A spy launch is picked up by the
  `Bout`, which then includes a **degraded** copy of the opponent's board in the
  launcher's snapshot — degraded to the spy's accuracy *on the server* (`Ames` hides
  50% of cells, `Ace` hides 15%, `Condor` hides 0%), so a modified client can't read
  the cells the spy didn't reveal. Spy duration is line-based (it decrements on the
  *opponent's* clears) and accumulates if you relaunch. This is what makes the old
  unauthenticated spy-request attack (decision D4) moot.

### Mirror's nullify / reflect set

`mirror_nullifies(token)` is the original `BTWeaponManager.C:204-216` switch. When
the launcher is mirror-cursed:

- **Nullified (fizzle — do nothing):** the nine weapons that have no sensible
  self-target — **Swap Meet, Mondale '96, Keating Five, Ames, Ace, Condor, Have a
  Nice Day, Lazy Susan, and Mirror itself** (Mirror is on the list so a curse can't
  ping-pong; the three spies are on it per decision D6).
- **Reflected (backfire onto the cursed launcher):** *every other* weapon — the
  launcher's own attack lands on themselves at their next lock.

An un-cursed launch always hits the opponent normally.

## Where this is exercised

- Per-board effects + durations: `bt-core` weapon PBTs (`pbt_weapons.rs`).
- Cross-player routing (Swap/Susan/Mirror nullify-set, spy fizzle): `versus.rs` and
  `bt-ai/src/vs.rs` tests.
- Authoritative resolution + anti-cheat (a client can't inject `ReceiveWeapon` /
  funds / board state): `bout.rs` proptests.
- See [`testing.md`](testing.md) for the full four-layer suite.
