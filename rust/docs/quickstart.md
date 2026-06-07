# Quickstart

See BattleTris running in under a minute. Two paths: play the live deployment
(zero setup), or build the WebAssembly client and play vs-Computer locally
(100% client-side — no server needed).

## Play online (no setup)

Open the live deployment:

<https://battletris.fly.dev>

Click **Find Match** to be paired by rating. If nobody's around, region bots
(Bert / Ernie / The Count) keep the lobby populated, so there's almost always
an opponent. Or click **Practice** / **Play Ernie** to play solo against the
computer right in the browser.

## Run vs-Computer locally (no server)

The vs-Computer mode is the whole game running in your browser via WebAssembly —
it does **not** talk to `bt-server` at all. So the only thing you need locally is
the compiled wasm plus any static file server. (This is exactly how the e2e
suite runs it: a plain `python3 -m http.server` over the `bt-wasm/` directory,
no `bt-server` — see [`bt-wasm/playwright.config.js`](../bt-wasm/playwright.config.js).)

Prerequisites: [Rust](https://rustup.rs) (stable), `wasm-pack`
(`cargo install wasm-pack`), and Python 3 (for the static server). From `rust/`:

```sh
# 1. Build the wasm client into bt-wasm/pkg/ (the page imports ../pkg).
wasm-pack build bt-wasm --target web --out-dir pkg --dev

# 2. Compile the browser TypeScript (www/*.ts -> www/*.js). Needs Node + tsc.
cd bt-wasm && npm install && npm run build:ts

# 3. Serve the static files (page in /www/, wasm in /pkg/).
python3 -m http.server 4173
```

Open <http://localhost:4173/www/>, then click **Practice** to play solo or
**Play Ernie** (the vs-Computer button — its label tracks the difficulty
slider) to battle the `bt-ai` opponent on the side board. No server process is
involved.

> Already have wasm-pack and Node set up? `cd bt-wasm && npm run build` does
> steps 1 and 2 in one shot (`build:wasm` + `build:ts`).

## Now go deeper

- Every build/run mode, plus the online server and a local region bot, with a
  full environment-variable table: [building-and-running.md](building-and-running.md)
- The crate map and the three data-flow paths (vs-Computer / online / bot):
  [../ARCHITECTURE.md](../ARCHITECTURE.md)
- Toolchain and the project's house rules:
  [../CONTRIBUTING.md](../CONTRIBUTING.md)
- The long-form project dossier (architecture, netcode, weapons, TLA+
  write-ups) lives in `screenshots/*.html` — start at `screenshots/index.html`.
