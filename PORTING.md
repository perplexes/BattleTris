# BattleTris — macOS Porting Notes

## What This Is

BattleTris is a two-player networked Tetris game written in the mid-1990s in pre-standard
C++, targeting Solaris/HP-UX with Motif 1.2 + X11 for graphics and Sun audio for sound.
Authors include Bryan Cantrill (bmc) and Michael Shapiro (mws).

The goal is to get it building and running on modern macOS using XQuartz + OpenMotif,
keeping the original Motif UI intact rather than rewriting to SDL2 or similar.

## Codebase Layout

All source lives under `usr/src/`:

```
game/        Main game client (~24K lines): game loop, piece engine, AI, weapons, board
daemons/     Master/slave server daemons + database server
db/          Hash-based persistent player stats DB with R/W locking
widget/      Motif/X11 widget wrappers (BTDisplay, BTDrawingAreaWidget, etc.)
sockets/     TCP socket abstraction with Xt event loop integration
stdlib/      Custom template containers (List<T>, Block<T>, BTStack, BTRingNode)
audio/       Sun audio device interface (Solaris-specific — needs stubbing)
signals/     Signal handling infrastructure
share/       Game assets: BattleTris.ad (X resources), btweapons.db, images
art/         PPM/XPM artwork
man/         Unix man page
```

Each subdirectory has its own `Makefile`. The root `Makefile` drives everything.
There is an autoconf `configure` / `configure.in` at `usr/src/`.

## Build Prerequisites (macOS)

1. **XQuartz** — already installed at `/opt/X11`
2. **OpenMotif** — install with `brew install openmotif`
   - Headers land in `/opt/homebrew/include`
   - Libraries land in `/opt/homebrew/lib`
3. **Xcode Command Line Tools** — already installed (`clang++`)

## Plan of Attack

### Step 1 — Install OpenMotif
```
brew install openmotif
```

### Step 2 — Run configure
From `usr/src/`, run `./configure`. It was written for Solaris so it will likely
need hints for Motif and X11 paths:
```
./configure --with-motif=/opt/homebrew --x-includes=/opt/X11/include --x-libraries=/opt/X11/lib
```
Inspect the generated `Makeinclude` and `BTConfig.H` to make sure paths are correct.

### Step 3 — Attempt a build, collect errors
```
make 2>&1 | tee build.log
grep -c error: build.log
```

### Step 4 — Fix pre-standard C++ errors
This is the bulk of the work. Expected issues with modern clang++:

- `#include <iostream.h>` → `#include <iostream>` + `using namespace std;`
- `#include <fstream.h>` → `#include <fstream>`
- `#include <strstream.h>` → `#include <sstream>` (also: `ostrstream` → `ostringstream`)
- `#include <string.h>` may need `<cstring>`
- Missing `std::` prefix on `cout`, `cerr`, `endl`, `string`, etc.
- `NULL` vs `nullptr` — leave as `NULL`, it's fine
- Old-style cast syntax — may generate warnings, not errors
- `for` loop variable scoping — old compilers allowed `for(int i=...)` to leak scope
- Template syntax issues — old compilers were lenient; clang++ is not

### Step 5 — Stub out Sun audio
`usr/src/audio/` talks directly to `/dev/audio` (Solaris only). The simplest fix
is to make `BTSoundManager` a no-op:
- In `BTSoundManager.C`, gut the implementation so all methods return immediately
- Audio is entirely optional — the game is fully playable without it

### Step 6 — Fix any macOS-specific issues
- `#include <sys/filio.h>` (Solaris) → `#include <sys/ioctl.h>`
- `#include <sys/select.h>` may be needed
- `bzero()` / `bcopy()` — available on macOS but may need `<strings.h>`
- `SIGPOLL` — not available on macOS; replace with `SIGIO`
- Solaris socket options that don't exist on macOS

## Known Architecture Details

### Game Loop
`BTGame` drives everything via `BTTimeOut` callbacks registered with the Xt event loop:
- `BT_DROP_TIMEOUT` — piece falling
- `BT_SLIDE_TIMEOUT` — horizontal movement
- `BT_SLICK_TIMEOUT` — flip/upside-down weapon effect
- `BT_HATTER_TIMEOUT` — piece-removal weapon
- `BT_JEOPARDY_TIMEOUT` — blind mode weapon

### Networking
Master daemon (`btserverd`) spawns per-connection slave daemons (`btslaved`).
Clients connect via TCP (default: `poptart.eng.sun.com:4404`). The socket layer
in `sockets/` integrates cleanly with the Xt event loop via `XtSocketCB`.
macOS BSD sockets are compatible — this should require minimal changes.

### Computer AI
`BTComputer` evaluates all possible piece placements/rotations, scores board states
by penalizing holes and height variance, and plans weapon purchases via `BTCOrders`.
Pure game logic — no platform dependencies.

### Database
`BTDB` in `db/` is a hash-based persistent database (flat files). Pure POSIX I/O —
should work on macOS without changes.

### Weapons (20+)
Applied via `BTWeaponManager`. Key weapons:
`FLIP_OUT` (mirror board), `BLIND` (black screen), `SWAP` (exchange boards),
`LAWYERS` (drain funds), `FALL_OUT` (extended boundaries), `FEARED_WEIRD`/`FOUR_BY_FOUR`
(weird pieces), `HATTER` (remove pieces), and various named political weapons.

### Piece Types
7 standard Tetris pieces + `BT_DIE_PIECE` (1-6 pip values that score funds when cleared),
`BT_HAPPY_PIECE` (150 pts if cleared same turn), and `BT_WEIRD_PIECE` variants.

## What to Ignore / Defer

- **Audio**: Stub it out, fix later if desired
- **Multiplayer server**: Get single-player vs. AI working first
- **ELO / database server**: Defer until base game works
- **`configure.in` correctness**: Edit `Makeinclude` by hand if autoconf fights you

## Quick Orientation — Key Files

| File | What it does |
|------|-------------|
| `game/BattleTris.C` | `main()`, X11/Motif app init, resource loading |
| `game/BTGame.C` | Central game state machine |
| `game/BTBoardManager.C` | Board grid, collision detection, line clearing |
| `game/BTPiece.C` | Piece types and rotation logic |
| `game/BTComputer.C` | AI opponent |
| `game/BTCommManager.C` | Network comms, opponent state sync |
| `game/BTWeaponManager.C` | Weapon effect application |
| `game/BTBazaar.C` | Weapons purchasing dialog |
| `widget/BTDisplay.C` | X11 display setup |
| `audio/BTSoundManager.C` | **Stub this out** |
| `sockets/BTStreamSocket.C` | TCP socket layer |
| `db/BTDB.C` | Persistent hash database |
