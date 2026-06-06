# syntax=docker/dockerfile:1
#
# Two stages: build the wasm client + a fully-static (musl) server binary, then
# ship it on Chainguard's distroless `static` image (no shell, nonroot, minimal
# CVE surface). The server has no C deps and makes no outbound TLS, so a static
# binary on `static` (no libc, no certs) is sufficient. Targets fly.io's amd64
# builders (build with `fly deploy --remote-only`).

FROM rust:1-bookworm AS builder
WORKDIR /build

# musl-tools: static server build. nodejs/npm: compile the browser TypeScript
# (www/*.ts -> www/*.js) — the page is served as plain ES modules, tsc is the only
# build step (no bundler). Bookworm ships node 18 + npm 9, ample for tsc.
RUN rustup target add wasm32-unknown-unknown x86_64-unknown-linux-musl \
 && apt-get update && apt-get install -y --no-install-recommends musl-tools nodejs npm \
 && rm -rf /var/lib/apt/lists/* \
 && curl -sSf https://rustwasm.github.io/wasm-pack/installer/init.sh | sh

COPY . .

# The commit being built, stamped into recordings (read by bt-wasm at compile
# time via option_env!("BT_GIT_SHA")); defaults to "dev". Scoped to the wasm
# RUN so only that layer's cache depends on it — the server build stays cached.
ARG BT_GIT_SHA=dev

# Browser client (release) -> bt-wasm/pkg, and a static server binary.
RUN BT_GIT_SHA="$BT_GIT_SHA" wasm-pack build bt-wasm --target web --out-dir pkg --release
# Compile the browser TypeScript -> www/*.js. Runs AFTER wasm-pack so the generated
# pkg/bt_wasm.d.ts exists to type the `../pkg/bt_wasm.js` import. noEmitOnError is set
# in tsconfig, so a type error fails the build here. Skip Playwright's browser
# download (only `typescript` is needed to compile; the e2e isn't run in the image).
RUN cd bt-wasm && PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1 npm ci && npm run build:ts
# bt-server now embeds SQLite (rusqlite "bundled" compiles it from C). Point the
# cc crate at the musl cross-compiler so SQLite links statically into the musl
# binary — keeps the runtime image libc-free.
ENV CC_x86_64_unknown_linux_musl=musl-gcc
RUN cargo build -p bt-server --release --target x86_64-unknown-linux-musl

# ---- runtime: Chainguard distroless static (nonroot, no shell) ----
FROM cgr.dev/chainguard/static:latest AS runtime
COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/bt-server /app/bt-server
# Static root holds www/ (page + assets) and pkg/ (wasm); the page imports ../pkg.
COPY --from=builder /build/bt-wasm/www /app/site/www
COPY --from=builder /build/bt-wasm/pkg /app/site/pkg

ENV STATIC_DIR=/app/site
ENV PORT=8080
# Durable ratings + uploaded replays: persisted to a fly volume at /data (see
# fly.toml). Replays (the "report a bug" link and the replay library) live in a
# SQLite DB; REPLAYS_DIR is the legacy JSON dir, imported into the DB once at
# startup if any files from an older deploy are still on the volume.
ENV RATINGS_FILE=/data/ratings.json
ENV REPLAY_DB=/data/replays.db
ENV REPLAYS_DIR=/data/replays
# Run as root so the process can write the root-owned volume mount. (Still a
# distroless, no-shell, minimal-CVE image; chainguard/static defaults to
# nonroot, which can't write a fly volume.)
USER root
EXPOSE 8080
ENTRYPOINT ["/app/bt-server"]
