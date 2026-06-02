# syntax=docker/dockerfile:1
#
# Two stages: build the wasm client + a fully-static (musl) server binary, then
# ship it on Chainguard's distroless `static` image (no shell, nonroot, minimal
# CVE surface). The server has no C deps and makes no outbound TLS, so a static
# binary on `static` (no libc, no certs) is sufficient. Targets fly.io's amd64
# builders (build with `fly deploy --remote-only`).

FROM rust:1-bookworm AS builder
WORKDIR /build

RUN rustup target add wasm32-unknown-unknown x86_64-unknown-linux-musl \
 && apt-get update && apt-get install -y --no-install-recommends musl-tools \
 && rm -rf /var/lib/apt/lists/* \
 && curl -sSf https://rustwasm.github.io/wasm-pack/installer/init.sh | sh

COPY . .

# The commit being built, stamped into recordings (read by bt-wasm at compile
# time via option_env!("BT_GIT_SHA")); defaults to "dev". Scoped to the wasm
# RUN so only that layer's cache depends on it — the server build stays cached.
ARG BT_GIT_SHA=dev

# Browser client (release) -> bt-wasm/pkg, and a static server binary.
RUN BT_GIT_SHA="$BT_GIT_SHA" wasm-pack build bt-wasm --target web --out-dir pkg --release
RUN cargo build -p bt-server --release --target x86_64-unknown-linux-musl

# ---- runtime: Chainguard distroless static (nonroot, no shell) ----
FROM cgr.dev/chainguard/static:latest AS runtime
COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/bt-server /app/bt-server
# Static root holds www/ (page + assets) and pkg/ (wasm); the page imports ../pkg.
COPY --from=builder /build/bt-wasm/www /app/site/www
COPY --from=builder /build/bt-wasm/pkg /app/site/pkg

ENV STATIC_DIR=/app/site
ENV PORT=8080
# Durable ratings: persisted to a fly volume mounted at /data (see fly.toml).
ENV RATINGS_FILE=/data/ratings.json
# Run as root so the process can write the root-owned volume mount. (Still a
# distroless, no-shell, minimal-CVE image; chainguard/static defaults to
# nonroot, which can't write a fly volume.)
USER root
EXPOSE 8080
ENTRYPOINT ["/app/bt-server"]
