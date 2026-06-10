# BattleTris weapons: original (1994 C++) vs Rust/WASM port

A dimension-by-dimension comparison of every weapon as implemented in the
original C++ (`usr/src/game/`) and in the Rust port (`rust/`). Descriptive only:
each row states what the original does and what the port does, with source
citations. No judgement is implied by listing a difference.

Weapon tokens are indexed 0..=33 in the order of `WeaponToken` (`bt-core/src/weapons.rs`)
and `BTWeaponToken` (`usr/src/game/BTProtocol.H`). Prices and durations are the
rows of `usr/src/share/btweaponsp.db`, mirrored in `bt-core/src/weapons.rs`'s
`weapon_table()`.

## Shared mechanics

These apply to every weapon unless an entry states otherwise.

- **Active state.** The original sets a boolean flag `BTActive[token] = 1` on
  activation and `= 0` on expiry (`BTWeaponManager.C:117`). The port mirrors this
  with a boolean active flag, `weapons.set(token, true/false)` (`game.rs:1118`).
- **Duration.** Durations are counted in lines cleared. On activation the original
  does `remaining_[token] += duration` (`BTWeaponManager.C:118`); re-launching the
  same weapon while it is active adds to its remaining duration. The port does the
  same: `remaining[token] += duration` (`game.rs:1122`). One `WPN_OFF` fires when
  remaining reaches 0. A weapon with duration 0 fires once and sets no lasting
  active state.
- **RNG.** The original draws from libc `rand()`, `drand48()`, and `lrand48()`. The
  port uses a deterministic generator that reproduces the POSIX `drand48` family
  and draws at the same call sites in the same order (`rng.rs`), so probabilities
  and draw order match; the exact pseudo-random stream differs from a given
  platform's libc.
- **Cross-player delivery.** A weapon launched at the opponent is applied at the
  opponent's next piece lock. The original queues it peer-to-peer (`BTCommManager`
  `weapq_` / `flushWeapons`); the port relays it through the authoritative server
  and applies it from a pending queue at the next lock (`bt-server/src/bout.rs`,
  `game.rs` `receive_weapon`).
- **Computer player.** The original hard-blocks the computer from launching Hatter,
  FlipOut, and Speedy (`BTWeaponManager.C:194-198`) and gates its purchases and
  launches through a commando-orders engine (`BTComputer.C`). The port has two
  computer players. The vs-Computer opponent, Ernie, ports that engine
  (`bt-ai/src/ernie.rs`): the commando orders list, the purchase whitelist and gates
  (Swap held while `board_top > BT_SWAPLINE`, Susan unlocked at opponent line 50), the
  Mirror self-curse hold, and the never-launch Hatter/FlipOut/Speedy block, with the
  weapon-adaptive penalty retuning in `bt-ai/src/lib.rs`. The lobby bots keep a
  separate rating-matched policy that launches only weapons from its buy list and seed
  arsenal (`bt-ai/src/weapons.rs`), a list that includes Hatter and Speedy and declines
  several the original could use (e.g. Swap). Per-weapon entries that cite
  `bt-ai/weapons.rs` describe the lobby-bot policy, not Ernie.
- **Bazaar prices.** While Carter is active, bazaar prices are multiplied by 2
  (`price * (1 + carter_)`, with `carter_` boolean, `BTBazaar.C:393`; port
  `price *= 2`, `game.rs:619`).

---

## Per-weapon comparison

### 0. The Feared Weird (`WeaponToken::FearedWeird`)

- **Price / duration**: `400` / `3` in both. Original record in `btweaponsp.db` (price `400`, duration `3`, plus a trailing `0` the loader reads and discards via an extra `READLINE`, `BTPimp.C:70-86`); port `weapon_table` entry `price: 400, duration: 3` (`weapons.rs:231`). Duration is counted in lines.
- **Effect / mechanism**: Both rewrite the piece-selection keep-probability table. They zero the standard pieces `BT_EL_PIECE..=BT_BOX_PIECE` (ids 1..=7) and set the weird pieces `(BT_WEIRD_OFFS+1)..=BT_WLONG_PIECE` (ids 10..=16) to `BT_DEFAULT_KEEP_PROB` (0.21). The port arm (`piece_manager.rs:167-176`) is a line-for-line port of the original (`BTPieceManager.C:89-94`); constants match (`constants.rs:163,173,180,185,195`). Selection is rejection sampling: roll `rand()%BT_MAX_PIECES+1` for an id, then `drand48()` against `keep_prob[id]`, loop until kept (`BTPieceManager.C:193-197` ↔ `piece_manager.rs:123-130`). On expiry both restore standard pieces to 0.21 and re-zero ids 10..=16 (`BTPieceManager.C:126-131` ↔ `piece_manager.rs:202-210`).
- **Targeting**: Opponent (both). The launch is converted to a `BT_WPN_ON` on the opponent ring (`BTCommManager.C:115-122`) / relayed to the victim's `receive_weapon` (`versus.rs:76-87,102-113`); the keep-prob effect lands on the receiver's piece manager.
- **Trigger**: At the target's next piece lock (both). Original drains the weapon queue in `flushWeapons` inside `place`, after `checkLines` and before the next `create` (`BTGame.C:776-799`, `BTCommManager.C:573-582`). Port drains `pending` via `flush_pending` → `apply_weapon_on` inside `place`, after `check_lines`/duration ticks and before `spawn` (`game.rs:411-414`).
- **Duration & stacking**: Active flag + lines-remaining, duration 3 (both). `BT_WPN_ON` sets the boolean active flag and `remaining_ += duration` (`BTWeaponManager.C:114-119`); the port sets `weapons.set(token,true)` and `remaining += duration` (`game.rs:1120-1124`). Each cleared line subtracts from `remaining`, and a single `WPN_OFF` fires at 0 (`BTWeaponManager.C:137-151` ↔ `game.rs:1220-1232`). Re-launch accumulates `remaining` in both.
- **RNG**: The effect itself draws nothing; it only rewrites the table. While active, ordinary piece selection draws in the same order in both: id roll, then `drand48()` per rejection iteration (`BTPieceManager.C:193-197` ↔ `piece_manager.rs:123-130`). FearedWeird does not touch the die (id 8), so a kept die still draws its extra `rand()%6+1` (`BTPiece.C:274` ↔ `piece_manager.rs:154-158`); the weird pieces draw no RNG to construct.
- **Cross-player relay**: Reaches the opponent (both). FearedWeird is not a spy and not in the port's `mirror_nullifies` list (`versus.rs:56-62`), so under a Mirror curse the port backfires it onto the launcher's own next lock (`versus.rs:77-85`), matching the original's `sendPlusMe(BT_WPN_ON, wpn)` default path for non-nullified weapons (`BTWeaponManager.C:204-218`).
- **Edge cases / exact differences**:
  - Active-flag storage: the original `BTActive[token]` is an int assigned literal `1` (`BTWeaponManager.C:117`); the port stores an `i32` but sets it to `0`/`1` via `ActiveFlags::set` (`weapons.rs:186-188`). Both expose boolean active state; a re-launch leaves the flag at `1` and accumulates only `remaining`.
  - Piece-set scope is identical: ids 10..=16 enabled at 0.21, ids 1..=7 zeroed, with die (8), happy (9), 4x4 (17), long-dong (18) untouched while active.
  - No magnitude, probability, ordering, or guard differences found.

### 1. Four-by-Four (`WeaponToken::FourByFour`)

- **Price / duration**: `425` / `10` in both (`btweaponsp.db` record `425,10,0`; `weapons.rs:232`).
- **Effect / mechanism**: Both flip two keep-probability entries: `keep_prob[BT_BOX_PIECE]=0` and `keep_prob[BT_4X4_PIECE]=BT_DEFAULT_KEEP_PROB` (0.21), so the opponent's box piece (id 7) is replaced by the 4x4 piece (id 17) in the selection stream (`BTPieceManager.C:96-100` ↔ `piece_manager.rs:177-180`); on expiry both reverse the pair (`BTPieceManager.C:133-137` ↔ `piece_manager.rs:212-215`). The 4x4 piece is a hollow 4x4 ring of 12 cells in color index 8: the full top and bottom rows plus the two interior cells on each side (`BTPiece.C:640-653`, color `BT_BOX_PIECE+1=8` ↔ `piece.rs:233-251`, `Cell::color(8)`). Same geometry and color.
- **Targeting / trigger / duration / relay**: Follows the shared mechanics. Opponent-targeted; takes effect from the next spawned piece onward; duration 10 lines with the boolean active flag and accumulating `remaining`; delivered by the standard cross-player relay (not a spy), and backfires onto the launcher under a Mirror curse on both sides (`BTWeaponManager.C:204-219` ↔ `versus.rs:77-85`).
- **RNG**: The weapon draws none; it only sets `keep_prob`. The biased piece selection draws the same two values in the same order (id roll, then `drand48()` per rejection iteration) as the shared selection loop.
- **Edge cases / exact differences**: In the default (no-weapon) stream `keep_prob[BT_4X4_PIECE]` is 0 on both sides (`BTPieceManager.C:77-78` ↔ `piece_manager.rs reset`), so the ring appears only while active. No magnitude, color, geometry, timing, duration, or RNG differences found.

### 2. The Mad Hatter (`WeaponToken::Hatter`)

- **Price / duration**: `375` / `5` in both (`btweaponsp.db:11-13`; `weapons.rs:233`).
- **Effect / mechanism**: While active, the opponent's current falling piece is rotated clockwise on a fixed 20-unit timer. Original: `BT_HATTER_TIMEOUT.time_ = 20` (`BTGame.C:130`); `hattertime()` re-arms the timer and calls `current_piece_->rotate()` (default `reverse=0`, clockwise, advancing `orientation_=(orientation_+1)%4`) (`BTGame.C:311-323`, `BTPiece.C:144`). Port: `tick_weapons` rotates once per 20 accumulated `dt` via `rotate_internal()` → `p.rotate(&board, false)` (clockwise) (`game.rs:1255-1281`). The rotation is rejected when any rotated cell would hit an occupied or out-of-bounds location, so a piece pinned against a wall or floor does not rotate (`BTPiece.C:119-122` + `occupied` out-of-bounds in `BTBoardManager.H:73` ↔ `piece.rs:366-370` + `board.rs:108-110`). Cadence (20), direction (clockwise), collision-reject, and the pinned exception match.
- **Targeting / trigger / duration / relay**: Follows shared mechanics. Opponent-targeted; both apply at the opponent's next piece lock (original: the received `BT_WPN_ON` is enqueued to `weapq_` by `receiveFromSibling`, `BTCommManager.C:182`, and drained by `flushWeapons` into the local `BT_WPN_ON` receive handler inside `place()`, `BTGame.C:795`,`:544-556`; port: `receive_weapon` enqueues to `pending`, applied by `flush_pending` inside `place()`, `game.rs:411`,`:578`). Duration 5 lines, boolean active flag, accumulating `remaining`. Standard cross-player relay; backfires onto the launcher under a Mirror curse.
- **RNG**: None. Activation, the rotation timer, and `rotate()` draw nothing in either version.
- **Bazaar**: Both stop Hatter rotation during the bazaar (original pauses `BT_HATTER_TIMEOUT` while `in_baz_`, `BTGame.C:314-318`; port gates the whole match tick behind the bazaar barrier, `versus.rs:203-209`).
- **Edge cases / exact differences**:
  - AI-launch restriction: the original hard-blocks the computer from launching Hatter (and FlipOut and Speedy): `launchWeapon` no-ops when `computer_ && (token==BT_HATTER || ==BT_FLIP_OUT || ==BT_SPEEDY)` (`BTWeaponManager.C:194-198`). The port's `bt-ai` has no such block: Hatter is `WClass::Harass` and is bought and launched (`bt-ai/src/weapons.rs:49-50,72,174-177`).
  - No cadence, direction, collision, pinned-exception, duration, or RNG differences found. (Both apply at the opponent's next lock; the original receive handler that arms the timer is invoked from `flushWeapons` in `place()`, not on raw network receipt.)

### 3. Up By Side (`WeaponToken::Upbyside`)

- **Price / duration**: `125` / `10` in both (`btweaponsp.db`; `weapons.rs:234`).
- **Effect / mechanism**: Two parts. (1) Movement frame: the activation arm sets `def_y = BT_BOARD_HGT-4` (= 24, spawn at the bottom), `delta_y = -1` (gravity upward), `left_x = +1` / `right_x = -1` (inverted left/right); `def_x` is left unchanged. The same drop/move/place code then runs with flipped signs (`BTGame.C:547-552`,`:677-691`,`:759` ↔ `game.rs:1146-1151`,`:421`,`:442`). (2) Board vertical flip: a top-bottom mirror, gated to run only on the inactive→active transition and only for a non-computer board, `if !upside && !computer { flip }` (`BTBoardManager.C:284-296` ↔ `board.rs:495-500`, `flip_horiz` at `:464-470`). The upside flag is still set for a computer board, which keeps line-shift, FallOut, and idiot-detection direction consistent for the AI's unflipped board (line-shift gate `!active(Upbyside) || computer`, `BTBoardManager.C:85,168` ↔ `board.rs:323,384`; FallOut `from_bottom`, `board.rs:593`; idiot top, `board.rs:182`).
- **Targeting / trigger / duration / relay**: Follows shared mechanics. Opponent-targeted; applied at the victim's next lock; duration 10 lines, board flip idempotent on re-launch (gated on `!upside`), `remaining` accumulates; standard relay (not a spy, not mirror-nullified).
- **RNG**: None. The activation/expiry arms and the flip draw nothing.
- **Edge cases / exact differences**:
  - Computer-flip gating confirmed on both sides (activation `!upside && !computer`; expiry `!computer`), with the upside flag itself still set for the AI board (`BTBoardManager.C:285,294,464-467` ↔ `board.rs:496,499,622-624`).
  - Rotation is not reversed in either version: `rotate` uses a constant direction with no Upbyside branch (`BTGame.C:696-702` ↔ `game.rs:470-478`,`:1276-1281`). The user-facing description ("pieces rotate the opposite way", `weapons.rs:234`) describes a behavior neither code base implements.
  - Swap cancels Upbyside (and Bottle) on both boards, forcing them off and zeroing `remaining` before the grid swap (`BTGame.C:498-501,525-529` ↔ `game.rs:675-681`, `force_weapon_off` `:664-669`).
  - The expiry arm resets `def_x` to `BT_DEFAULT_X` even though activation never changed it; present identically in both (`BTGame.C:638` ↔ `game.rs:1203`).

### 4. Fallout (`WeaponToken::FallOut`)

- **Price / duration**: `250` / `10` in both (`btweaponsp.db`; `weapons.rs:235`).
- **Effect / mechanism**: Two coupled parts. (1) A one-time collapse: loop `height` times removing the bottom line (or the top line under Upbyside on a human board) over the non-ledge columns, so the middle six columns (`[BT_FALL_OUT_LEDGE, width-BT_FALL_OUT_LEDGE)` = cols 2..7 of 10) empty out (`BTBoardManager.C:410-421` ↔ `board.rs:584-598`). (2) An ongoing `occupied()` override while the active flag is set: the middle band of both the floor (`y >= height`) and the ceiling (`y < 0`) is treated as open while the two-wide ledges stay solid, so a piece dropped over the middle passes through and is discarded; the side walls (`x<0 || x>=width`) and a far overshoot (beyond `±BT_PIECE_HEIGHT=8` off the board) stay solid (`BTBoardManager.H:71-86` ↔ `board.rs:106-129`, line-for-line).
- **Targeting / trigger / duration / relay**: Follows shared mechanics. Opponent-targeted; applied at the victim's next lock; duration 10 lines; the collapse runs once on activation (the flag is already set for a re-launch), `remaining` accumulates; standard relay (not a spy, not mirror-nullified).
- **RNG**: None. The collapse and the `occupied()` override use no random draws.
- **Edge cases / exact differences**:
  - Both the collapse and the `occupied()` hole follow gravity direction: under Upbyside on a human board the top line is removed and the ceiling opens; on a computer board (never visually flipped) it stays bottom-ward. The gate `!active(Upbyside) || computer` is identical in both (`BTBoardManager.C:414,85` ↔ `board.rs:593,323`).
  - A piece that falls through is discarded by the in-bounds-only `fill` (`board.rs:133-139` ↔ the C++ off-board delete).
  - No behavioral differences found.

### 5. Swap Meet (`WeaponToken::Swap`)

- **Price / duration**: `1200` / `0` in both (`btweaponsp.db`; `weapons.rs:236`). Sets no active flag or remaining duration in either version (the port queues a board swap rather than calling `apply_weapon_on`). The effect itself lands at the next lock, not on the launch frame.
- **Effect / mechanism**: Exchange only the two board grids, after forcing Bottle and Upbyside off on both boards. Original: the launcher zeroes its own Bottle/Upbyside and sends `BT_WPN_OFF`, then sends its board; the receiver does the same, installs the incoming board via `newBoard` (which copies the cell map `rep_` only), and sends its old board back (`BTGame.C:492-533`, `newBoard` `BTBoardManager.C:627-640`). Port: at launch the relay captures each side's board (`export_board`) and queues the other side's board onto each game's `pending_board` slot (`queue_board_swap`); at each side's next lock `flush_pending` installs the queued board (`import_board`) and forces Bottle and Upbyside off (`versus.rs` `apply_weapon` Swap arm; `game.rs` `queue_board_swap` / `flush_pending`). Scores, funds, arsenals, active-weapon flags and durations, and the falling piece all stay with their owner on both sides.
- **Targeting**: Both boards (a symmetric exchange), not a single victim.
- **Trigger / timing**: Both apply the swap at each side's next piece lock, in the gap where no piece is mid-fall. Original holds the swapped board in `board_buf_` and re-emits it from `flushWeapons` inside `place()` (`BTCommManager.C:448-453,584-588`). Port installs the launch-captured board from the `pending_board` slot in `flush_pending`, also called from `place()` at the lock, matching `board_buf_`.
- **Duration & stacking**: Duration 0; no active flag (both). Re-launch captures and queues the boards again.
- **RNG**: None (board copy).
- **Cross-player relay**: Special two-board routing, not the normal victim queue. Original tags a `BT_BOARD` packet `motivation=BT_SWAP`, stashed in `board_buf_` and applied at the next flush (`BTCommManager.C:448-453,584-588`). Port special-cases `Swap`/`Susan` in `apply_weapon`: Swap captures both boards at launch and queues each onto the other side's `pending_board`, installed at that side's next lock (`versus.rs`).
- **Edge cases / exact differences**:
  - Mirror-nullify (both): a Mirror-cursed launcher's Swap fizzles to nothing (original lists `BT_SWAP` among the Mirror cases that return without re-sending, `BTWeaponManager.C:204-219`; port has `Swap` on `mirror_nullifies`, `versus.rs:60`, and a cursed launch returns with no effect, `versus.rs:78-80`).
  - AI use: the original computer can purchase and launch Swap, gated on board height (`can_purchase_[BT_SWAP]` off while `top_ > BT_SWAPLINE=5`, `BTComputer.C:552-555`). Ernie ports this gate (Swap purchasable only while `board_top <= BT_SWAPLINE`, `bt-ai/src/ernie.rs`). The lobby bots never launch Swap (classified `WClass::Skip`, `bt-ai/weapons.rs:53`). The port engine imposes no Swap launch restriction; the gating is the computer player's policy.

### 6. Lawyer's Delite (`WeaponToken::Lawyers`)

- **Price / duration**: `350` / `5` in both (`btweaponsp.db:27-29`; `weapons.rs:237`). Duration counts down by the holder's own cleared lines in both.
- **Effect / mechanism**: While active, each line the opponent clears raises the holder's own board by one garbage row: the stack pushes up and the bottom row is filled solid across the width except for one random gap column, in green (`BT_GREEN=6`). The gap column is `rand()%width` (`BTBoardManager.C:165` ↔ `board.rs:380`, `rng.rand_below(width)`). The push-up, the Bottle column-narrowing during the push, and the Upbyside branch are identical (`BTBoardManager.C:158-204` ↔ `board.rs:379-426`).
- **Targeting**: Raises the holder's (victim's) own board, in both.
- **Trigger**: Per opponent line-clear. The opponent's lock reports its score, and the holder's handler inserts `op_lines - old_op_lines` rows (`BTGame.C:477-483` ↔ `game.rs:583-592`). Same delta computation; the rows are inserted inline, not deferred to the holder's own next lock.
- **Duration & stacking**: 5, measured in the holder's own cleared lines; `remaining` accumulates on re-launch (both).
- **RNG**: One gap-column draw (`rand()%width`) per inserted row, in both.
- **Edge cases / exact differences**:
  - The original has two Lawyers paths. The human player's `BTGame::lawyers` (`BTGame.C:838-867`) is piece-aware: for each rise it inspects the falling piece, and if the piece is sliding or cannot descend one step it cancels the drop/slick/slide timeouts and force-locks the piece via `place(1)` (then skips that pass without consuming a rise); if the piece can descend exactly once it starts a slide; only then does it insert the row. The computer opponent's path (`BTComputer.C:283-292`) is a bare per-line insert loop with no piece inspection.
  - The port's `receive_op_score` (`game.rs:587-591`) inserts the rows in a plain loop and never reads, locks, or repositions the falling piece, nor touches drop/slide timing. The port's behavior matches the original computer Lawyers path; it does not reproduce the original human path's force-lock and slide handling of the currently-falling piece.

### 7. Rise Up (`WeaponToken::RiseUp`)

- **Price / duration**: `75` / `0` in both (`btweaponsp.db`; `weapons.rs:238`). Instant one-shot: with duration 0, `apply_weapon_on` adds 0 to `remaining`, so no line-counted expiry is scheduled.
- **Effect / mechanism**: Inserts exactly one garbage row via the same routine Lawyers uses (entry 6): push the stack up one and fill the new bottom row solid in green (`BT_GREEN=6`) except for one random gap column (`rand()%width`). The original `BT_RISE_UP` arm calls `insertLine()` once (`BTBoardManager.C:444-448`, commented "Trivial"); the port `RiseUp` arm calls `insert_line(rng)` once (`board.rs:610`). The difference from Lawyers is the call count (Rise Up once on activation, Lawyers once per opponent line cleared), not the routine.
- **Targeting / trigger / duration / relay**: Follows shared mechanics. Opponent-targeted; applied at the victim's next lock; duration 0 (one-shot, no ongoing effect); standard relay, backfires under Mirror.
- **RNG**: One gap-column draw (`rand()%width`) for the inserted row, in both.
- **Edge cases / exact differences**: The Upbyside (push direction and filled edge) and Bottle (neck narrowing) branches of the shared routine behave identically (`BTBoardManager.C:168,177,205-235` ↔ `board.rs:384,390,405-425`). The original disposes the rendering object scrolled off the top (`BTBoardManager.C:187-189`); the port operates on a pure cell grid with no analogous object-cleanup step, producing the same resulting cells. No other differences found.

### 8. Flip Out (`WeaponToken::FlipOut`)

- **Price / duration**: `15` / `0` in both (`btweaponsp.db`; `weapons.rs:239`). Instant, no persistent active flag.
- **Effect / mechanism**: Mirror the board left-to-right (swap column x with column width-1-x) over the full grid, including empty cells. Original `flipOnVert`: `for i in 0..width/2, for j in 0..height: swap(width-1-i, j, i, j)` (`BTBoardManager.C:246-251`, called from the `BT_FLIP_OUT` arm `:403-408`). Port `flip_vert`: the same loops and swap (`board.rs:472-479`, `FlipOut` arm `:583`). The port function is named `flip_vert` but mirrors columns; the separate `flip_horiz` is the top-bottom Upbyside mirror. Identical.
- **Targeting / trigger / duration / relay**: Follows shared mechanics. Opponent-targeted; applied at the victim's next lock; duration 0 (one-shot, no active flag); standard relay, backfires under Mirror.
- **RNG**: None.
- **Edge cases / exact differences**:
  - AI: the original engine-blocks the computer from launching FlipOut (`BTWeaponManager.C:194-198`). The port has no engine block, but its bot never acquires FlipOut: it is `WClass::Harass` (`bt-ai/weapons.rs:50`) yet absent from `BUY_PRIORITY` and the seed arsenal, so it is never in the bot's arsenal to launch. Neither AI launches FlipOut, by different routes. (Hatter and Speedy are in the port's `BUY_PRIORITY`, so the port bot does launch those two.)
  - FlipOut sets no active flag and credits no lines or funds; it does not affect FallOut direction or Bottle.

### 9. Speedy Gonzales (`WeaponToken::Speedy`)

- **Price / duration**: `275` / `10` in both (`btweaponsp.db`; `weapons.rs:240`).
- **Effect / mechanism**: Halve the opponent's gravity interval `base_drop_time` on activation (`>>= 1`) and double it back on expiry (`<<= 1`). Original `BTGame.C:563-565` (ON) / `:654-656` (OFF); port `game.rs:1160-1166` (ON) / `:1209` (OFF).
- **Targeting / trigger / relay**: Follows shared mechanics. Opponent-targeted; applied at the victim's next lock; standard relay, backfires under Mirror.
- **Duration & stacking (duration is the same, magnitude differs)**:
  - Duration: both add the duration to a running `remaining` counter on every activation, so stacked launches extend the lifetime (`remaining += duration`; `BTWeaponManager.C:118` ↔ `game.rs:1122`). The active marker is a boolean in both, not a counter (`BTActive[token]=1` ↔ `weapons.set(token,true)`).
  - Magnitude: the original applies `base_drop_time_ >>= 1` on every `BT_WPN_ON` with no guard on the already-active state, and reverts once (`<<= 1`) on the single `BT_WPN_OFF` at expiry, so two stacked launches quarter the interval but double it back only once. The port applies the `>>= 1` only on the inactive→active transition (`if !was_active`, `game.rs:1160-1166`), pairing 1:1 with the single revert, so a re-launch extends duration without re-scaling the clock.
- **RNG**: None.
- **Edge cases / exact differences**:
  - The re-launch magnitude difference above. The user-facing description ("Several of these launched at once… pretty interesting for your opponent") is present in both (`weapons.rs:240`); the port carries the same string while not compounding the magnitude.
  - AI: the original engine-blocks the computer from launching Speedy (`BTWeaponManager.C:191-198`); the port bot launches it (in `BUY_PRIORITY` `weapons.rs:67`, seeded `:246,:255`).
  - Live `drop_time` resync: the port immediately retargets the running `drop_time` to the new `base_drop_time` (clamped `>=1`) when not fast-dropping (`game.rs:1163-1165`); the original recomputes the live `*drop_time_` from `base_drop_time_` on the next piece reset (`BTGame.C:821`).

### 10. Missing (`WeaponToken::Missing`)

- **Price / duration**: `50` / `0` in both (`btweaponsp.db`; `weapons.rs:241`). Instant one-shot, no active flag.
- **Effect / mechanism**: Pick a random origin and remove exactly one removable block scanning outward from it. Both draw the origin as `x = rand()%width` then `y = rand()%height`, then scan with the row index as the outer loop (from `y`, wrapping `% height`) and the column index as the inner loop (from `x`, wrapping `% width`), removing the first cell that is occupied and removable; structure cells are skipped (`is_removable` is false only for `Structure`). Original `BTBoardManager.C:331-353` ↔ port `board.rs:534-550`; `is_removable` `cell.rs:75-77`. Same RNG draw order (x/width before y/height) and same scan order.
- **Targeting / trigger / relay**: Follows shared mechanics. Opponent-targeted; applied at the victim's next lock; standard relay, backfires under Mirror.
- **Duration & stacking**: Instant, duration 0, no active flag; each launch removes one block.
- **RNG**: Two draws per application: the column origin (`rand()%width`) then the row origin (`rand()%height`), same order in both. The scan itself draws nothing.
- **Edge cases / exact differences**: With no removable cell (empty board), both leave the board unchanged (the scan finds nothing). No differences found.

### 11. Piece It Together (`WeaponToken::PieceIt`)

- **Price / duration**: `100` / `0` in both (`btweaponsp.db`; `weapons.rs:242`). Instant one-shot, no active flag.
- **Effect / mechanism**: Place one box at a random empty cell in the middle two quarters of the board (rows `height/4 .. 3*height/4` = rows 7-20 of 28). A rejection loop draws column `i = rand()%width` and row `j = rand()%(height/2) + height/4` until the cell is empty, then places a box there (no gravity). PieceIt places a visible random color `rand()%(BT_NEUTRAL-1)+1` (= 1-8); Bug (token 23) shares this exact arm but places `BT_INVISIBLE` (-1) and draws no color. Original `BTBoardManager.C:299-323` ↔ port `board.rs:501-532` (`BT_NEUTRAL=9`, `BT_INVISIBLE=-1`, board 10x28).
- **Targeting / trigger / relay**: Follows shared mechanics. Opponent-targeted; applied at the victim's next lock; standard relay.
- **Duration & stacking**: Instant, duration 0, no active flag; one box per launch.
- **RNG**: Per rejection-loop iteration, a column draw (`rand()%width`) then a row draw (`rand()%(height/2)`); retries redraw both; then PieceIt draws a color (`rand()%(NEUTRAL-1)+1`), Bug draws none. Same draw count and order in both.
- **Edge cases / exact differences**:
  - Bug (token 23) shares the identical placement/rejection loop; only the placed value differs (PieceIt visible 1-8, Bug invisible -1).
  - Fully-packed band: the original rejection loop is unbounded and spins forever if the middle band has no empty cell (`BTBoardManager.C:307-310`). The port adds a pre-loop scan (no RNG draws): if the band has a free cell it runs the identical unbounded loop (same draws), but if the band is completely full it skips the arm with no placement and no draw (`board.rs:513-532`). In any state with a free band cell the two are identical draw-for-draw; they diverge only in the fully-packed-band state.

### 12. Blind Cleric (`WeaponToken::Blind`)

- **Price / duration**: `400` / `0` in both (`btweaponsp.db`; `weapons.rs:243`). Instant one-shot, no active flag.
- **Effect / mechanism**: Walk the entire board and, for each occupied removable cell independently, remove it with probability 1/2 via a `(rand()%2)==0` coin flip. This is a per-cell 50% removal across the whole board, not a contiguous region. Iteration order is outer=row, inner=column; one RNG draw per removable cell, taken only after the occupancy and removable tests pass. Structure cells are excluded. Original `BTBoardManager.C:358-371` (its comment: "Run through the entire board, randomly nuking half of it") ↔ port `board.rs:551-558`. Identical.
- **Targeting / trigger / relay**: Follows shared mechanics. Opponent-targeted; applied at the victim's next lock; standard relay.
- **Duration & stacking**: Instant, duration 0, no active flag.
- **RNG**: One `rand()%2` draw per occupied removable cell, in row-major order, in both.
- **Edge cases / exact differences**:
  - The in-game description ("Bombs a region…", `weapons.rs:243`) does not match the implementation (a per-cell 50% coin flip over the whole board) in either version; the port carries the same flavor text as the original.
  - No dimension differs between the two implementations.

### 13. Mondale '96 (`WeaponToken::Mondale`)

- **Price / duration**: `150` / `50` in both (`btweaponsp.db`; `weapons.rs:244`). Rate 30% (`BT_MONDALE_RATE=.30`; `BTScoreManager.C:14` ↔ `constants.rs:244`).
- **Effect / mechanism**: While Mondale is active on the victim, the funds the victim earns from a line clear are taxed: the victim keeps `floor(70%)` and the other 30% goes to the attacker. The tax is wired to the line-clear funds path, not every funds delta. The victim-kept arithmetic is identical: a truncating cast of `0.70 * gained` (`BTScoreManager.C:202` ↔ `game.rs:1101`).
- **Targeting**: Victim loses, attacker gains, in both.
- **Trigger / duration / relay**: Follows shared mechanics. Mondale is queued onto the victim and becomes active at the victim's next lock; duration 50 lines (boolean active flag plus accumulating `remaining` on the victim); the attacker is credited per victim clear while the flag is live.
- **RNG**: None.
- **Edge cases / exact differences (funds arithmetic)**:
  - Original: two independent integer truncations across the network. The victim keeps `(long)(0.70*gained)` (`BTScoreManager.C:202`); the attacker separately reconstructs the gain from the victim's reported funds *delta* and truncates again, `(long)((1/0.70 * delta) * 0.30)` (`BTScoreManager.C:158-159`), where `delta` is the victim's already-truncated funds change, guarded on a funds increase (`:157`). The two truncations need not sum to the gained amount, so up to about 2 funds vanish per clear (e.g. gained 17 → victim 11 + attacker 4 = 15; gained 7 → 4 + 1 = 5). The original also tracks Mondale's lifetime in two places: the victim's `remaining_` and the attacker's separate `tax_on_` countdown (`BTScoreManager.C:160-162`).
  - Port: one integer tax computed on the victim, `kept = floor(0.70*clear_funds)`, `tax = clear_funds - kept`, emitted as `FundsStolen(tax)` and credited to the attacker exactly via the relay (`game.rs:1100-1104`, `versus.rs:236-239` → `add_funds`). Victim-kept plus attacker-gain always equals the cleared funds; a single boolean flag plus accumulating `remaining` tracks the lifetime, with no separate attacker countdown.
  - Snapshot timing: the original's attacker credit is driven off the opponent's funds delta observed at `BT_OP_SCORE` (applied only when funds increased); the port emits the credit exactly at the victim's clear, with no delta reconstruction or `op_funds` snapshot for Mondale.

### 14. Keating Five (`WeaponToken::Keating`)

- **Price / duration**: `425` / `0` in both (`btweaponsp.db`; `weapons.rs:245`). Instant one-shot, no active flag.
- **Effect / mechanism**: Zero the victim's funds and credit the attacker the funds value as of launch. Both set the victim's funds to 0 at activation and credit the attacker a launch-time snapshot, so the two amounts can differ:
  - Original: at launch the attacker snapshots its cached view of the opponent's funds, `keating_ = rep_.op_funds_` (`BTScoreManager.C:110-111`); the victim is zeroed when the weapon activates at its next lock (`:121-123`); the attacker is credited the launch snapshot at the next incoming `OP_SCORE`, `rep_.funds_ += keating_` (`:151-153`). The attacker gets the opponent-funds value as of launch; the victim loses whatever it holds at activation.
  - Port: at launch the relay credits the attacker the victim's funds read at that moment (`attacker.add_funds(victim.score().funds)` in `apply_weapon`) and queues the seizure; the victim is zeroed only when the weapon activates at its next lock (`apply_weapon_on` sets `score.funds = 0`, with no `FundsStolen` event). The server holds ground truth, so the port's launch value is the victim's real balance at launch, where the original uses the attacker's cached mirror of it (`versus.rs` Keating arm; `game.rs` `apply_weapon_on` Keating).
- **Targeting / trigger / relay**: Follows shared mechanics. Opponent-targeted; applied at the victim's next lock; standard relay.
- **Duration & stacking**: Instant, duration 0, no active flag.
- **RNG**: None.
- **Edge cases / exact differences**:
  - Snapshot timing: both credit the attacker the funds value snapshotted at launch, independent of what the victim actually holds when zeroed, so the credited amount and the seized amount can differ when the victim's balance changes between launch and its next lock. The original snapshots its cached `op_funds` (`keating_`); the port reads the victim's real funds in the relay at launch.
  - The port zeroes the victim at its activation lock, after that lock's clear is credited, so funds banked on the triggering piece are included in what the victim loses; the credited amount stays fixed at the launch snapshot, as in the original.
  - Mirror-nullify (both): a Keating launched by a Mirror-cursed attacker fizzles rather than backfiring (original lists `BT_KEATING` among the Mirror skip cases, `BTWeaponManager.C:204-216`; port has `Keating` on `mirror_nullifies`, `versus.rs:60`).

### 15. Carter Years (`WeaponToken::Carter`)

- **Price / duration**: `250` / `20` in both (`btweaponsp.db`; `weapons.rs:246`). The description ("the prices double at your opponent's bazaar") states the 2x rate.
- **Effect / mechanism**: While Carter is active on the victim, the victim's bazaar prices are doubled, for both buying and selling. Original: `price_ * (1 + carter_)`, with `carter_` a boolean, in the displayed price (`BTBazaar.C:393`), the buy/Add path (`:415`), and the sell/Remove refund (`:458`); with `carter_ ∈ {0,1}` this is exactly 1x or 2x. Port: `buy_weapon` does `price *= 2` when Carter is active (`game.rs:619-620`), and `bazaar_price` (used by display and sell) returns `p*2` (`game.rs:1083-1086`). The port's `*2` reproduces the original's `*(1+carter_)` with boolean `carter_`.
- **Targeting**: The opponent's bazaar. The active flag lives on the victim, whose buy/sell prices rise (original `BTGame.C:592` passes `BTActive[BT_CARTER]` into the victim's bazaar; port the victim's own Carter flag governs its buy/sell).
- **Trigger / relay**: Follows shared mechanics. Cross-player; activates at the victim's next lock.
- **Duration & stacking**: Boolean active flag, 20-line duration that accumulates on re-launch (`remaining += 20` in both). It does not stack beyond 2x: the active flag is boolean and the multiplier reads it, so the price is exactly 2x while active and never higher. (An earlier audit claimed Carter stacks to 3x/4x; that is not supported by either codebase; there is no path producing more than 2x.)
- **RNG**: None.
- **Edge cases / exact differences**:
  - The 2x cap: both multiply by exactly 2 via a boolean flag; no 3x/4x exists in either codebase.
  - Sell refunds at the same Carter-doubled price as buying, in both (`BTBazaar.C:458` ↔ `game.rs:644` → `bazaar_price`).
  - Bazaar-open-before-expiry ordering: when one cleared line both opens the bazaar and expires Carter, the bazaar opens before the duration ticks, so that visit still charges doubled, on both sides (manager-ring order ↔ `update_bazaar` before `tick_durations`).
  - AI use differs: the original computer buys and launches Carter (it stays enabled in `can_purchase_` and is in the launch set, `BTComputer.C:179,595-596`). The port bot never launches Carter: it is `WClass::Economy` but absent from `BUY_PRIORITY`, so it is never bought or held (`bt-ai/weapons.rs:51`); the bot does double its own prices when Carter-cursed (`price_of(.., carter)`, `:82-90`).

### 16. Reaganomics (`WeaponToken::Reagan`)

- **Price / duration**: `425` / `0` in both (`btweaponsp.db`; `weapons.rs:247`).
- **Effect / mechanism**: Negate the victim's funds (`funds *= -1`), so a positive balance becomes negative debt, an already-negative balance flips back positive, and zero stays zero. It is an unconditional sign flip in both, with no special-casing. Original `BTScoreManager.C:125-127` (`rep_.funds_ *= -1`) ↔ port `game.rs:1189` (`self.score.funds = -self.score.funds`). Funds are a signed type in both (original `long funds_`, `BTScore.H:24`; port signed `i64`), so the negation produces real debt identically.
- **Targeting / trigger / relay**: Follows shared mechanics. Opponent-targeted; applied at the victim's next lock; standard relay.
- **Duration & stacking**: Instant, duration 0, no active flag; re-applying negates again (two applications cancel).
- **RNG**: None.
- **Edge cases / exact differences**:
  - Mirror is a backfire, not a nullify (both): Reagan is absent from both Mirror-nullify lists, so a Mirror-cursed launcher's Reagan reflects onto the launcher, negating the launcher's own funds. The original Mirror switch (`BTWeaponManager.C:204-219`) lists only Swap/Mondale/Keating/Ames/Ace/Condor/NiceDay/Susan/Mirror as no-effect cases, and `BT_REAGAN` falls to `default → sendPlusMe(BT_WPN_ON, wpn)` onto the cursed launcher; the port's `mirror_nullifies` (`versus.rs:56-62`) is exactly that same nine-token set and excludes Reagan, so `deliver_weapon` takes the backfire branch (`Recipient::Attacker`). The two nullify lists match token-for-token.
  - AI: the original computer couples Reagan to "Have a Nice Day" (after a smiley gift it sets `next_weapon_ = BT_REAGAN`, `BTComputer.C:600-622`). The port bot has no such tactic and does not launch Reagan offensively (`WClass::Economy`, not in `BUY_PRIORITY`).

### 17. Ames (`WeaponToken::Ames`)

- **Price / duration**: `50` / `20` in both (`btweaponsp.db`; `weapons.rs:248`). Duration counts down in the opponent's cleared lines.
- **Effect / mechanism**: Shows the launcher an obscured view of the opponent's board and the opponent's funds (the victim's board is not damaged):
  - Board reveal: the original shows ~50% of non-empty cells (`report_prob=.5` for Ames), re-rolling `drand48()` per cell per render, so the hidden subset flickers frame to frame (`BTRecon.C:58-71`). The port sends the launcher the full opponent board and flickers it client-side: it blanks ~50% of the cells each frame, re-rolling the hidden subset on a ~70 ms timer (`spy-degrade.ts` `rollSpyMask` / `applySpyMask`, driven from `main.ts`). The hide percentage, 50 = `1 - report_prob`, is reported by the server (`spy_hide`, `bout.rs` `spy_hide_pct`). Both flicker; the original re-rolls per render via `drand48`, the port per ~70 ms tick via `Math.random`.
  - Funds: the original displays the opponent's funds next to the launcher's, randomized per render by `±(rand()%(funds+1))` via `adjustFunds`, with a `funds==-1 -> -2` FPE guard (`BTScoreManager.C:61-62`, `BTRecon.C:94-117`). The port computes the revealed value server-side and sends only that scalar (`spy_funds`): `funds + sign*(noise % (|funds|+1))` with the same `funds==-1 -> -2` guard, the sign and magnitude drawn from a splitmix64 hash of the tick so consecutive reveals are uncorrelated (`bout.rs` `adjust_funds`). The client shows it next to its own funds while spying (`main.ts`). Computing it server-side means a modified client reads no more than the spy grants.
- **Targeting**: The launcher sees the opponent's board; the victim's board is untouched. The original draws the opponent board the opponent sends each lock; the port records the spy launcher-side and the server sends the opponent's full `render_ids` plus the hide level, never delivered to the opponent, with the degradation applied client-side for display (`bout.rs` `snapshot_for`).
- **Trigger / relay**: Cross-player, attaching to the launcher's view. Original: the opponent's game sends its board tagged with the spy token on each of the opponent's locks (`BTGame.C:785-789`), redrawn launcher-side per receive (`BTRecon.C:209-223`). Port: the spy launch is recorded host-side and the server ships the board and funds on throttled keyframe frames (`bout.rs` `snapshot_for`); the client flickers the board between frames. Board content updates at keyframe cadence (port) against per-opponent-lock (original); the flicker runs at the client's ~70 ms tick.
- **Duration & stacking**: Countdown in the opponent's cleared lines; re-launch accumulates the budget and switches accuracy to the newest spy token (`BTRecon.C:159-218` ↔ `bout.rs`).
- **RNG**: The original re-rolls live RNG per render (`drand48()` per cell for the board, `rand()` per funds redraw). The port re-rolls the board flicker with `Math.random` client-side per ~70 ms tick, and draws the funds perturbation from a splitmix64 hash of the tick server-side per keyframe. The cadences differ; both vary the reveal continuously.
- **Edge cases / exact differences**: The board now flickers in both, and both reveal funds. The remaining differences are cadence (per-render original against the ~70 ms client tick for the board, and per-render against per-keyframe for funds) and that the port computes the funds value server-side, which is the anti-cheat boundary. The full opponent board reaches the client during an active spy, an accepted exposure bounded to the spy window. These are shared by the spy family (Ames, Ace, Condor); only the reveal percentage and duration differ per spy.

### 18. Ace of Spies (`WeaponToken::Ace`)

- **Price / duration**: `100` / `30` in both (`btweaponsp.db`; `weapons.rs:249`).
- **Effect / mechanism**: A spy revealing the opponent's board at 85% (more accurate than Ames's 50%): original `report_prob=.85` (`BTRecon.C:60-61`), port `spy_hide_pct(Ace)=15` so 15% is flickered out and 85% shown (`bout.rs` `spy_hide_pct`). The shared spy machinery (the client-side flicker of the full board, the server-computed funds reveal, the keyframe board cadence, launcher-only targeting, and the opponent-line duration with newest-token accumulation) is as documented for Ames (entry 17).
- **Ace-specific funds**: in the original, Ace's `adjustFunds` shows the opponent's funds unchanged, except a one-shot `±(rand()%100)` applied on the first funds render after the opponent clears a tetris (4 lines at once; `tet_` is set when `inc==4`, `BTRecon.C:106-110,205-206`). The port reproduces this: `adjust_funds` returns the exact funds, except `funds + sign*(noise%100)` while the opponent's last tetris is within `ACE_TETRIS_WINDOW` ticks, where the original fires once on the post-tetris render (`bout.rs` `adjust_funds`, `last_opp_tetris_tick`). This is more accurate than Ames (which perturbs by `±rand%(funds+1)` every render) and less than Condor (exact). The port holds the perturbation for a short tick window because it reveals funds on throttled keyframes rather than per render.
- **Targeting / trigger / duration / relay / RNG**: As Ames (entry 17), with duration 30.
- **Edge cases / exact differences**: 85% reveal (against Ames 50%); the original's per-tetris one-shot funds perturbation, which the port reproduces as a short tick window; plus the spy-family cadence differences. Mirror-nullified in both (`BT_ACE` in the original Mirror switch `BTWeaponManager.C:204-219`; `Ace` in `mirror_nullifies` `versus.rs:60`).

### 19. The Condor (`WeaponToken::Condor`)

- **Price / duration**: `225` / `40` in both (`btweaponsp.db`; `weapons.rs:250`).
- **Effect / mechanism**: The most accurate spy, with a perfect board reveal in both: the original's `report_prob` stays 1 because Condor is not special-cased (`BTRecon.C:58,72`), and the port's `spy_hide_pct(Condor)=0` leaves the full board unflickered (the client's `spyFlicker` returns the board untouched at hide 0) (`bout.rs` `spy_hide_pct`; `main.ts`). The shared spy machinery is as documented for Ames (entry 17).
- **Funds (exact in both)**: the original reveals the opponent's funds exactly (`adjustFunds` `case BT_CONDOR: return (funds)`, `BTRecon.C:112-113`). The port does the same: `adjust_funds` returns the funds unchanged for Condor, sent as the `spy_funds` scalar (`bout.rs` `adjust_funds`).
- **Targeting / trigger / relay**: As the spy family (the launcher sees the opponent's board; not delivered to the opponent). Duration 40, opponent-line countdown with additive re-launch.
- **RNG**: None in the Condor path on either side (the original's per-cell `drand48()` is moot at `report_prob=1` since no cell is hidden; the port's hide 0 skips the flicker re-roll; the `CONDOR` funds arm draws nothing).
- **Edge cases / exact differences**:
  - Funds exact in both; no longer a difference.
  - Board flicker is moot: a perfect reveal hides nothing, so there is no per-frame re-roll flicker on either side (unlike Ames/Ace).
  - Cadence: per-opponent-board-update (original) against keyframe-throttled (port), as the spy family.
  - Mirror-nullified in both (`BT_CONDOR` in the original Mirror switch; `Condor` in `versus.rs:60`).

### 20. Have a Nice Day (`WeaponToken::NiceDay`)

- **Price / duration**: `50` / `0` in both (`btweaponsp.db`; `weapons.rs:251`).
- **Effect / mechanism**: Forces a "smiley"/happy piece (`BT_HAP_PIECE=9`) into the opponent's piece stream and lets them earn a bonus if they clear it. Activation increments a `hap_on` counter (original `hap_on_++` `BTPieceManager.C:113-114` ↔ port `hap_on += 1` `piece_manager.rs:190-192`); piece selection then forces one happy piece per `hap_on`, decrementing it and bypassing the normal rejection loop with no RNG for the forced piece (`BTPieceManager.C:184,206-216` ↔ `piece_manager.rs:123-146`). A happy cell is worth `BT_HAPPY_VAL=150` toward the line's funds value while it has not landed (`BTBox.H:100` ↔ `cell.rs:43-47`), so clearing the smiley pays the clearer +150 (line funds = value*lines). If a happy cell sits in a row that does not complete a line, it converts to a frown (value→0) and raises the idiot flag with reason `BT_MISSED_SMILEY=2` (`BTBoardManager.C:590-595` ↔ `board.rs:278-289`); clearing any line afterward un-flags idiot.
- **Targeting**: The opponent receives the forced smiley and earns the 150 if they clear it (the funds credit lands on the clearing/victim board).
- **Trigger / relay**: Follows shared mechanics. Cross-player; the forced happy piece enters the opponent's stream from the piece after their next lock.
- **Duration & stacking**: Duration 0, no active timer; the effect is the one-shot `hap_on` increment, additive (two NiceDays force two happy pieces). `hap_on` resets at game start.
- **RNG**: None for the forced happy piece (the `hap_on` branch returns `BT_HAP_PIECE` directly, drawing nothing). The keep-prob for a happy piece appearing in the random stream is `BT_EXOTIC_KEEP_PROB=0.02` in both, which governs only the random stream, not the forced piece.
- **Edge cases / exact differences**:
  - Mirror-nullified in both (NiceDay in the original Mirror switch `BTWeaponManager.C:206-214` and the port `mirror_nullifies` `versus.rs:60`).
  - The Reagan combo is emergent, not special-cased: the identical weapon description suggests following a NiceDay with a Reagan to negate the gained funds; only the original's AI launch-ordering heuristic implements the sequence (`BTComputer.C:820-826`), and no combo code exists in either engine.
  - No behavioral differences found across the dimensions checked.

### 21. So Long (`WeaponToken::SoLong`)

- **Price / duration**: `100` / `10` in both (`btweaponsp.db`; `weapons.rs:252`).
- **Effect / mechanism**: Sets the standard long piece's keep-probability to 0 (`keep_prob[BT_LONG_PIECE=5] = 0`), removing the long piece from the opponent's selection stream; expiry restores it to `BT_DEFAULT_KEEP_PROB` (0.21). Only the standard long (id 5) is zeroed; the weird-long (`BT_WLONG_PIECE=16`) is untouched. Original `BTPieceManager.C:110` (ON) / `:143` (OFF) ↔ port `piece_manager.rs:188` (ON) / `:220` (OFF). Identical.
- **Targeting / trigger / duration / relay**: Follows shared mechanics. Opponent-targeted piece-stream edit; applied at the victim's next lock; duration 10 (boolean active flag plus accumulating `remaining`); the off-arm unconditionally writes the default back. Standard relay, backfires under Mirror.
- **RNG**: None for the weapon (a table edit only); the keep-prob is read by the downstream piece-selection `drand48()` draw.
- **Edge cases / exact differences**: Only the standard long id (5) is affected (weird-long 16 untouched); restore value 0.21; no differences found.

### 22. No Dice (`WeaponToken::NoDice`)

- **Price / duration**: `600` / `35` in both (`btweaponsp.db`; `weapons.rs:253`).
- **Effect / mechanism**: Sets the die piece's keep-probability to 0 (`keep_prob[BT_DIE_PIECE=8] = 0`), removing the die from the opponent's selection stream; expiry restores it to `BT_DIE_KEEP_PROB`. The die's default keep-prob is the special always-kept value 1.0 (not the 0.21 standard default), so the cycle is 1.0 → 0 → 1.0. Original `BTPieceManager.C:105-107` (ON) / `:138-140` (OFF), `BT_DIE_KEEP_PROB=1` (`BTConstants.H:18`) ↔ port `piece_manager.rs:185` (ON) / `:217` (OFF), `BT_DIE_KEEP_PROB=1.0` (`constants.rs:201`). Identical.
- **Targeting / trigger / duration / relay**: Follows shared mechanics. Opponent-targeted piece-stream edit; applied at the victim's next lock; duration 35 (boolean active flag plus accumulating `remaining`, reverting once on expiry). Standard relay, backfires under Mirror.
- **RNG**: None for the weapon (a table edit); with keep-prob 0 the die never passes the selection test, so no die is rolled while active. (The die's pip draw `rand()%6+1` is part of die construction, untouched by the weapon.)
- **Edge cases / exact differences**: The restore is the die's always-kept 1.0, not 0.21. FearedWeird (which zeroes ids 1-7 and raises 10-16) leaves the die (id 8) untouched, so NoDice and FearedWeird act on disjoint keep-prob indices. No differences found.

### 23. Bug Report (`WeaponToken::Bug`)

- **Price / duration**: `320` / `0` in both (`btweaponsp.db`; `weapons.rs:254`). Instant one-shot.
- **Effect / mechanism**: Identical to Piece It Together (entry 11) except the placed block is invisible. Bug shares the same arm (original `case BT_PIECE_IT: case BT_BUG:` `BTBoardManager.C:299-323`; port `WeaponToken::PieceIt | WeaponToken::Bug` `board.rs:501-532`): the same column-then-row rejection loop picks a random empty cell in the middle two quarters, and Bug places a `BT_INVISIBLE` (-1) block with no color draw (PieceIt draws a visible color). `BT_INVISIBLE=-1` in both (`BTConstants.H:32` ↔ `constants.rs:40`). The invisible block behaves as an ordinary occupied square: it blocks falling pieces (`occupied` → true), counts toward filling a line while contributing `value()=0`, and is removable. Same on both sides.
- **Targeting / trigger / relay**: Follows shared mechanics. Opponent-targeted; applied at the victim's next lock.
- **Duration & stacking**: Instant, duration 0, no active flag; one invisible block per launch.
- **RNG**: Per application, a column draw (`rand()%width`) then a row draw (`rand()%(height/2)`), with retries; Bug draws no color (unlike PieceIt's extra color draw). Same as entry 11.
- **Edge cases / exact differences**:
  - The only difference from PieceIt is the placed value (`BT_INVISIBLE` -1, no color draw).
  - Representation: the original stores the block as a dedicated `BTInvisiBox` object (value 0, removable); the port stores it as a `CellKind::Color(-1)` cell (value 0, removable), not its separate `CellKind::Invisible` variant. Both render as nothing and contribute value 0.
  - `band_has_empty` guard: as PieceIt (entry 11). The port no-ops on a fully-packed middle band where the original's rejection loop would spin forever; identical draw-for-draw when the band has a free cell.

### 24. Bottle Neck (`WeaponToken::Bottle`)

- **Price / duration**: `150` / `10` in both (`btweaponsp.db`; `weapons.rs:255`). Description: "4-block wide bottle neck."
- **Effect / mechanism**: Plants un-removable structure walls `BT_BOTTLE_X=3` columns deep on each side, across the middle `±BT_BOTTLE_Y=4` rows (rows `BT_BOARD_HGT/2-4 .. +4` = rows 10-17), at columns 0-2 and 7-9, squeezing the playable width to the middle `10 - 2*3 = 4` columns (the neck). After planting, `check_lines` runs, so if the walls complete the neck row that line clears and the funds/lines are credited to the board owner (the victim). Original `BTBoardManager.C:423-442` ↔ port `board.rs:599-608`; constants `BT_BOTTLE_X=3`, `BT_BOTTLE_Y=4` (`BTBoardManager.H:12-13` ↔ `constants.rs:235-236`). Structure cells are un-removable with value 0 (`BTBox.H:121-128` ↔ `cell.rs` `Structure`). Expiry removes the walls (`BTBoardManager.C:471-488` ↔ revert `board.rs:626-633`). While active, `insert_line`/`remove_line` shift only within the neck columns `[BT_BOTTLE_X, width-BT_BOTTLE_X)` for the neck-band rows (same band tests including the `-1` upside asymmetry). Identical.
- **Targeting / trigger / duration / relay**: Follows shared mechanics. Opponent-targeted; applied at the victim's next lock; duration 10 (boolean active flag plus accumulating `remaining`); walls planted once on activation, removed once on expiry. Standard relay, backfires under Mirror.
- **RNG**: None for activation (deterministic walls); the only RNG is the narrowed `insert_line` hole (`rand()%width`) if a rise/Lawyers row is inserted while active, the same draw as a non-Bottle insert.
- **Edge cases / exact differences**:
  - Neck-row funds credit: a wall-completed neck row credits funds/lines to the board owner via the normal line-clear path; structure cells contribute value 0, so the credited value comes only from the previously placed non-structure cells (both sides).
  - Swap cancels Bottle (and Upbyside) on both boards before the grid swap (entry 5).
  - Structure walls are un-removable; only expiry or the Swap cancel removes them.
  - No differences found.

### 25. Slide Denied (`WeaponToken::NoSlide`)

- **Price / duration**: `125` / `10` in both (`btweaponsp.db`; `weapons.rs:256`).
- **Effect / mechanism**: Sets the slide (lock-delay) grace time to `BT_SLIDE_TIME * (1 - active(NoSlide))`. With NoSlide active the multiplier is 0, so the slide time is 0 and a piece that can no longer fall locks immediately with no slide grace. Original `BTGame::startSlide` `BTGame.C:747-748` (`*slide_time_ = BT_SLIDE_TIME * (1 - BTActive[BT_NO_SLIDE])`) ↔ port `start_slide` `game.rs:345` (`slide_time = BT_SLIDE_TIME * (1 - is_active(NoSlide) as i32)`). `BT_SLIDE_TIME=150` in both (`BTConstants.H:94` ↔ `constants.rs:130`). Identical.
- **Targeting / trigger / duration / relay**: Follows shared mechanics. Opponent-targeted; applied at the victim's next lock; duration 10 (boolean active flag plus accumulating `remaining`, line-counted); reverting clears the flag (no board state to restore). Standard relay, backfires under Mirror.
- **RNG**: None (no board effect; the formula is pure arithmetic).
- **Edge cases / exact differences**:
  - NoSlide affects only the slide / lock-delay grace, not the Slick auto-slide weapon (a separate token with its own timer).
  - Reverting on expiry only clears the active flag; the next slide recomputes `150 * (1-0) = 150` naturally.
  - No differences found.

### 26. Lazy Susan (`WeaponToken::Susan`)

- **Price / duration**: `600` / `0` in both (`btweaponsp.db`; `weapons.rs:257`). Instant.
- **Effect / mechanism**: Exchange only the two players' arsenals (the weapon-slot inventory). Port: `swap_arsenal_with` = `std::mem::swap(arsenal)` (`game.rs:685-687`); the `Arsenal` is `rep:[Option<WeaponToken>;10]` + `quantity:[u16;10]` (`arsenal.rs:14-19`). Original: the launcher ships its arsenal to the opponent and installs the opponent's (`BTWeaponManager.C:104-110,122-135`; `sendArsenal`/`recvArsenal`). Boards, funds, score, the current piece, and active weapon effects stay with each player in both.
- **Targeting**: Both arsenals exchanged (symmetric), not a single victim.
- **Trigger / timing**: Like Swap (entry 5). The port applies Susan immediately in the relay pass that drains the launch event (`versus.rs:108`, special-cased away from the per-victim `pending` queue); the original installs the swapped arsenal when the `BT_ARSENAL` packet is received over the comm ring (not at a piece lock).
- **Duration & stacking**: Duration 0, instant, no timed effect.
- **RNG**: None (`mem::swap` / fixed-slot arsenal copy).
- **Cross-player relay**: Special two-arsenal routing (not the ordinary queued delivery), like Swap.
- **Edge cases / exact differences**:
  - Active flag: the original sets `BTActive[BT_SUSAN]=1` via the `BT_WPN_ON` path and never clears it (duration 0 → `remaining` stays 0 → skipped by expiry), but no code reads `BTActive[BT_SUSAN]`; the port sets no active flag at all. The flag is inert, so the two are observationally identical.
  - Mirror-nullified in both (`BT_SUSAN` in the original Mirror switch; `Susan` in `versus.rs` `mirror_nullifies`).
  - AI: the original computer can buy and launch Susan (enabled after the opponent reaches 50 lines, `BTComputer.C:187-200,648,845-846`); the port bot never launches it (`WClass::Skip`, `bt-ai/weapons.rs:53`).

### 27. Meadow (`WeaponToken::Meadow`)

- **Price / duration**: `475` / `10` in both (`btweaponsp.db`; `weapons.rs:258`).
- **Effect / mechanism**: Doubles both the opponent's gravity interval `base_drop_time` and their fast-drop interval `fast_drop_time` (`<<= 1`), halving the opponent's drop speed; expiry halves both back (`>>= 1`). The description states it acts on the opponent ("the drop speed of their pieces is halved"). Original `BTGame.C:567-571` (ON) / `:658-661` (OFF) ↔ port `game.rs:1170-1171` (ON) / `:1210-1213` (OFF). The port additionally resyncs the live `drop_time` to the rescaled value when not fast-dropping (`game.rs:1172-1174`); the original arm touches only the two stored fields.
- **Targeting**: The opponent (victim). Meadow is not a spy and not mirror-nullified, so the relay routes it to `Recipient::Victim` (`versus.rs:86,111`). Slowing the opponent's drop helps the recipient, which is why the port bot treats Meadow as non-offensive.
- **Trigger / relay**: Follows shared mechanics. Cross-player; applied at the victim's next lock; standard relay, backfires under Mirror.
- **Duration & stacking (duration is the same, magnitude differs)** (same pattern as Speedy, entry 9): both accumulate `remaining += duration` on re-launch (boolean active flag). The original applies `<<= 1` on every `BT_WPN_ON` with no guard and reverts once (`>>= 1`) at expiry, so stacked launches compound the slowdown but revert only once; the port applies the doubling only on the inactive→active transition (`if !was_active`, `game.rs:1169-1175`), so a re-launch extends duration without re-scaling.
- **RNG**: None.
- **Edge cases / exact differences**:
  - The re-launch magnitude difference (as Speedy).
  - The port's ON arm resyncs the live `drop_time`; the original touches only the stored fields.
  - AI: the port bot never launches Meadow (`WClass::Skip`, `bt-ai/weapons.rs:53`, treating the opponent-slowdown as helping the victim).

### 28. Mirror Mirror (`WeaponToken::Mirror`)

- **Price / duration**: `500` / `10` in both (`btweaponsp.db`; `weapons.rs:259`). Description: "when launched, your opponent's weapons will be reflected back on to them. Note that some weapons … are simply nullified."
- **Effect / mechanism**: Launching Mirror is a normal attack that curses the opponent (not the launcher). While a player is Mirror-cursed, their own curse catches every weapon they launch: the 9 nullify-set tokens fizzle, and every other token reflects back onto the cursed launcher (backfire). An un-cursed launch (Mirror included) hits the opponent normally. Original `BTWeaponManager.C:204-219` (`if BTActive[BT_MIRROR]:` nullify-cases `break`, `default: sendPlusMe(BT_WPN_ON)` onto the cursed launcher, `return`) ↔ port `deliver_weapon` `versus.rs:76-87` (`if attacker.weapon_active(Mirror):` `mirror_nullifies` → return, else `Recipient::Attacker`; else `Recipient::Victim`). This `deliver_weapon` function is the source of the backfire/fizzle behavior referenced throughout this document.
- **Targeting**: Launching Mirror curses the opponent; the launcher is not armed by launching it (confirmed by the port test `vs.rs:332-336`). The opponent's subsequent launches then backfire or fizzle.
- **Trigger / duration / relay**: Follows shared mechanics. Cross-player; Mirror itself activates at the victim's next lock; duration 10 (boolean active flag plus accumulating `remaining`).
- **RNG**: None in the Mirror mechanism.
- **Edge cases / exact differences**:
  - The 9-token nullify set is identical token-for-token: {Swap, Mondale, Keating, Ames, Ace, Condor, NiceDay, Susan, Mirror} (`BTWeaponManager.C:206-214` ↔ `versus.rs:56-62`). Mirror is in its own set, so a Mirror-cursed player's Mirror fizzles.
  - Backfire timing: the original applies the backfire immediately and locally at launch (`sendPlusMe(BT_WPN_ON)` circulates the full ring including the sender, `BTWeaponManager.C:217`); the port queues it onto the cursed launcher's next lock (`Recipient::Attacker` → `receive_weapon` → `pending` → `flush_pending`). Same destination, different timing.
  - AI: the port bot never launches Mirror (`WClass::Skip`, `bt-ai/weapons.rs:53`). The original computer can launch Mirror and additionally holds its own launches while it is itself Mirror-cursed (`if (!BTActive[BT_MIRROR]) launchWeapon(...)`, `BTComputer.C:838-850`) to avoid backfiring; the port bot has no such gate, so a cursed port bot's launches would backfire or fizzle through `deliver_weapon`.

### 29. Twilight Zone (`WeaponToken::Twilight`)

- **Price / duration**: `450` / `0` in both (`btweaponsp.db`; `weapons.rs:260`).
- **Effect / mechanism**: Walks the whole board and hides every present cell by setting a hidden flag, so the cell renders as nothing (`id()` returns -1 while hidden) but keeps its true kind and value. Original `BTBoardManager.C:390-401` (`map_[j][i]->hide()`) ↔ port `board.rs:573-582` (`c.hide()`). The cells stay in the grid, so collision (grid occupancy) and line-clears/funds are unaffected; only rendering is suppressed. (The original's code comment says "replacing each box with an invisible box," but the executed statement is `hide()`, a flag on the existing box, not a `BTInvisiBox`.)
- **Targeting / trigger / relay**: Follows shared mechanics. Opponent-targeted; applied at the victim's next lock; standard relay, backfires under Mirror.
- **Duration & stacking**: One-shot, duration 0; no ongoing active state and no revert (the hide is permanent for the rest of the board's life; `revert_weapon` has no Twilight case).
- **RNG**: None.
- **Edge cases / exact differences**:
  - Representation: both set a hidden flag on the existing cell/box (original `hidden_` on `BTBox`, `BTBox.H:72,74`; port `hidden:bool` on `Cell`, `cell.rs:85,124-140`), gating `id()` to -1. Neither swaps in the separate invisible type (`BTInvisiBox` / `CellKind::Invisible{}` / `Color(-1)`) that Bug and board reconstruction use.
  - Structure cells are hidden too (Twilight has no `is_removable` guard, unlike Gimp). Same on both sides.
  - Collision and line-clear still work (cells keep their value and occupancy); only rendering is suppressed. The hidden flag round-trips through the port's board encode/decode, so it persists across a later board transfer (Swap/spy).
  - No differences found.

### 30. Slick Willy (`WeaponToken::Slick`)

- **Price / duration**: `650` / `3` in both (`btweaponsp.db`; `weapons.rs:261`).
- **Effect / mechanism**: While active, the opponent's falling piece auto-slides sideways on a 20-unit timer. It starts toward `left_x` (`slick_dir` 0); each step attempts to move one cell in the current direction, committing the x-shift on success or flipping direction (without moving that step) when blocked by a wall. Original `slicktime` `BTGame.C:333-343` (cadence 20 at `:133`) ↔ port `slick_step` `game.rs:1283-1291` (cadence 20 in `tick_weapons` at `:1264-1271`). Identical move/flip logic. The auto-slide is suspended during hard-drop and the slide/lock phase and re-armed on a new piece: the original removes/adds `BT_SLICK_TIMEOUT` at `beginDrop`/`startSlide`/spawn (`BTGame.C:726,743,808-809`); the port gates `tick_weapons` on `phase==Falling && !dropping` (`game.rs:1265-1266`), which spawn re-satisfies.
- **Targeting**: The opponent's currently falling piece.
- **Trigger / duration / relay**: Follows shared mechanics. Cross-player; applied at the victim's next lock; duration 3 (boolean active flag plus accumulating `remaining`). Standard relay, backfires under Mirror.
- **RNG**: None (move-or-flip is deterministic).
- **Edge cases / exact differences**:
  - Distinct from NoSlide (entry 25): Slick auto-slides the piece; NoSlide removes the lock-delay slide grace. No shared logic.
  - Representation: the original arms/suspends an explicit `BT_SLICK_TIMEOUT` across seven sites (launch, expiry, `beginDrop`, `startSlide`, Lawyers place, spawn, plus a bazaar pause); the port has no per-Slick timeout and instead drives a per-tick `slick_accum` gated on `Falling && !dropping`, with `tick`'s early return covering pause/bazaar/over.
  - Accumulator phase: the original re-arms a fresh 20-unit countdown on each `addTimeOut` (e.g. at spawn); the port carries `slick_accum` across drop/slide/spawn boundaries (zeroed only at construction and restore), so the first slide step after a re-arm can be timed differently between the two.
  - AI: the port bot launches Slick (in `BUY_PRIORITY`, `bt-ai/weapons.rs:78`).

### 31. Broken Record (`WeaponToken::Broken`)

- **Price / duration**: `325` / `5` in both (`btweaponsp.db`; `weapons.rs:262`).
- **Effect / mechanism**: While Broken is active, the opponent's piece selection (`create`) uses a three-way branch: if `hap_on==0` and (not broken, or broken and the `lrand48()%BT_BROKEN_PROB` gate hits 0), run the normal rejection loop; else if `hap_on==0` and broken, repeat the last piece (`old_piece`); else emit the forced happy piece (NiceDay). `BT_BROKEN_PROB=10` (a 1-in-10 chance to draw a fresh random piece, otherwise repeat). Original `BTPieceManager.C:184-208` ↔ port `piece_manager.rs:123-145`. Draw order while broken: the `lrand48` gate is drawn first, then (on a hit) the `rand()`+`drand48()` rejection loop. NiceDay takes precedence: `hap_on` is checked first, so a forced happy piece preempts a broken repeat. The broken state is a single boolean flag set/cleared by `weapon_on`/`weapon_off` (`piece_manager.rs:181-183,222` ↔ `BTPieceManager.C:101-104,147`), not a counter.
- **Targeting / trigger / duration / relay**: Follows shared mechanics. Opponent-targeted piece-stream flag; applied at the victim's next lock; duration 5 (boolean active flag plus accumulating `remaining`). Standard relay, backfires under Mirror.
- **RNG**: While broken and `hap_on==0`, the `lrand48()%10` gate is the first draw; on a 0 result the `rand()`+`drand48()` rejection loop follows (one or more pairs); otherwise no further draw (reuse `old_piece`). When `old_piece` is valid the draw sequence matches both sides.
- **Edge cases / exact differences**:
  - The only difference: the invalid-`old_piece` path. If the broken-repeat arm runs while `old_piece` is still its initializer 0 (no piece spawned yet), the original asserts (`assert(piece_[i])`, `BTPieceManager.C:211`) while the port falls back to `BT_EL_PIECE` (`piece_manager.rs:137-141`). This path is unreachable in normal play (a Game always spawns a piece first, so `old_piece` is non-zero before Broken can repeat); a port test exercises the degenerate case.
  - `BT_BROKEN_PROB=10`, NiceDay precedence, and the draw order are identical.

### 32. The Force (`WeaponToken::Force`)

- **Price / duration**: `325` / `5` in both (`btweaponsp.db`; `weapons.rs:263`).
- **Effect / mechanism**: While Force is active on the opponent's board, a cleared line is still detected, counted, valued, and credited (funds/lines are tallied before the row is removed), but the rows above the cleared line are not shifted down to fill the gap. In `remove_line`, the Force branch erases only the cleared row's cells and `continue`s, bypassing the shift-down assignment; the post-loop top/bottom row clear is also gated off; and `check_lines` does not re-examine the just-cleared row index (the `j += 1` skip runs only when not Force, since nothing falls into that index). Original `BTBoardManager.C:94-116` (normal) / `:126-148` (upside) ↔ port `board.rs:330-347` (normal) / `:355-372` (upside). Identical, in both gravity orientations.
- **Targeting / trigger / duration / relay**: Follows shared mechanics. Opponent-targeted; applied at the victim's next lock; duration 5 (boolean active flag plus accumulating `remaining`). Standard relay, backfires under Mirror.
- **RNG**: None (the collapse-skip path draws nothing).
- **Edge cases / exact differences**:
  - Funds/lines are still scored for a Force-suppressed clear (tallied before the row removal), on both sides.
  - Handled in both gravity orientations (the Force gate is duplicated in the normal and Upbyside branches).
  - No differences found.

### 33. The Gimp (`WeaponToken::Gimp`)

- **Price / duration**: `25` / `0` in both (`btweaponsp.db`; `weapons.rs:264`). The cheapest weapon (25); a cosmetic/distraction effect.
- **Effect / mechanism**: Walks the board and converts every removable cell into a "gimp" cell that carries the original cell's value but renders under `BT_GIMP_ID=23` (a distracting skin). The cell's value is captured and preserved, so collision, occupancy, and line-clears are unaffected; only the rendered id changes. Original `BTBoardManager.C:373-387` (`value = value(); dispose; createGimp(j,i,value)`) ↔ port `board.rs:562-572` (`Cell::gimp(c.value())`). Structure cells are excluded via the `is_removable` guard (the original comments that gimpifying a non-removable bottleneck box "corrupts the board and can crash on a BT_BOTTLE/BT_GIMP/BT_BLIND combo"); the same guard is in both. The gimp cell is itself removable.
- **Targeting / trigger / relay**: Follows shared mechanics. Opponent-targeted; applied at the victim's next lock; standard relay, backfires under Mirror.
- **Duration & stacking**: One-shot, duration 0; the conversion happens once per application, with no persistent state.
- **RNG**: None.
- **Edge cases / exact differences**:
  - Representation: the original disposes the old box and creates a distinct `BTGimpBox` object (object-type swap); the port replaces the cell with a `CellKind::Gimp(value)` variant (cell-kind replacement). Both carry the value and render under id 23.
  - The `is_removable` guard excludes structure cells, unlike Twilight (entry 29), which hides every cell including structure. Same guard on both sides.
  - Value preserved (funds, occupancy, line-clear unaffected); a cosmetic skin only.
  - AI: the port bot never launches Gimp (`WClass::Skip`, `bt-ai/weapons.rs:53`).
  - No differences found.
