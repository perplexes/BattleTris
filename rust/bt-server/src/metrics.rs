//! Prometheus metrics for the server. Exposed at `/metrics` in text format for
//! fly.io's managed Grafana to scrape (see `[[metrics]]` in fly.toml). Counters are
//! cheap atomics, so instrumentation is sprinkled on the hot paths directly.
//!
//! What's tracked (the operational picture for release): HTTP request rate
//! ("hit rate"), WebSocket message throughput (msgs/sec, by direction), ping
//! round-trip latency (a histogram → p50/p95), live connections, and matches.

use prometheus::{
    Counter, CounterVec, Encoder, Gauge, Histogram, HistogramOpts, Opts, Registry, TextEncoder,
};
use std::sync::LazyLock;

/// The process-wide metric handles, registered against one [`Registry`] that
/// `/metrics` serializes. Each field is a cheap atomic, so hot-path call sites
/// can bump them inline.
pub struct Metrics {
    /// The collector every metric registers into; [`render`] gathers it.
    registry: Registry,
    /// WebSocket frames, labelled `direction` = "in" | "out". `rate()` → msgs/sec.
    pub ws_messages: CounterVec,
    /// Ping round-trip latency (ms) — the same RTT shown in the lobby.
    pub ws_ping_ms: Histogram,
    /// HTTP requests served (static files + API + ws upgrades). `rate()` → hit rate.
    pub http_requests: Counter,
    /// Currently-open WebSocket connections.
    pub ws_connections: Gauge,
    /// Authoritative matches started.
    pub matches: Counter,
}

/// The one shared metrics instance, built and registered on first access. Call
/// sites reach the counters through `METRICS.…`; [`render`] reads the registry.
pub static METRICS: LazyLock<Metrics> = LazyLock::new(|| {
    let registry = Registry::new();
    let ws_messages = CounterVec::new(
        Opts::new("bt_ws_messages_total", "WebSocket frames processed, by direction"),
        &["direction"],
    )
    .unwrap();
    let ws_ping_ms = Histogram::with_opts(
        HistogramOpts::new("bt_ws_ping_ms", "WebSocket ping round-trip latency (ms)")
            .buckets(vec![5.0, 10.0, 20.0, 40.0, 80.0, 160.0, 320.0, 640.0]),
    )
    .unwrap();
    let http_requests =
        Counter::new("bt_http_requests_total", "HTTP requests served (hit rate)").unwrap();
    let ws_connections =
        Gauge::new("bt_ws_connections", "Currently-open WebSocket connections").unwrap();
    let matches = Counter::new("bt_matches_total", "Authoritative matches started").unwrap();

    registry.register(Box::new(ws_messages.clone())).unwrap();
    registry.register(Box::new(ws_ping_ms.clone())).unwrap();
    registry.register(Box::new(http_requests.clone())).unwrap();
    registry.register(Box::new(ws_connections.clone())).unwrap();
    registry.register(Box::new(matches.clone())).unwrap();

    Metrics { registry, ws_messages, ws_ping_ms, http_requests, ws_connections, matches }
});

/// Count one inbound WebSocket frame — wraps the `direction="in"` label so the
/// read loop stays a one-liner.
pub fn ws_in() {
    METRICS.ws_messages.with_label_values(&["in"]).inc();
}
/// Count one outbound WebSocket frame (`direction="out"`).
pub fn ws_out() {
    METRICS.ws_messages.with_label_values(&["out"]).inc();
}

/// Render the registry as Prometheus text (the `/metrics` body).
pub fn render() -> String {
    let mut buf = Vec::new();
    let encoder = TextEncoder::new();
    let _ = encoder.encode(&METRICS.registry.gather(), &mut buf);
    String::from_utf8(buf).unwrap_or_default()
}
