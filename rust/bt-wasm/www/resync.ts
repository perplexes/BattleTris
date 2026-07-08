// Self-throttle for the client's resync request (asking the server for a fresh
// keyframe once the divergence detector fires, `WasmClient.on_lock_hash`).
//
// The server keeps sending the same authoritative lock pair on every snapshot until
// a fresh keyframe lands, so the detector keeps reporting a divergence for the whole
// window it takes the request to round-trip; without a client-side gate that turns
// into a resync frame sent on every snapshot tick. The server also rate-limits grants
// to 1/s, so an unthrottled client would just spam frames that get dropped on the
// floor. This throttle is the one place in the resync path where dropping a request
// is intentional and not a bug: `shouldSendResync` is the gate that makes the drop.

/** Minimum time between resync requests, in ms. Twice the server's 1/s grant window,
 *  so a request that lands mid-window still leaves room for the next one to be granted. */
export const RESYNC_MIN_INTERVAL_MS = 2000;

/**
 * Whether a resync request is due: true on the very first request (`lastSentMs` is
 * null, nothing sent yet this match) or once at least `RESYNC_MIN_INTERVAL_MS` has
 * elapsed since the last one.
 */
export function shouldSendResync(nowMs: number, lastSentMs: number | null): boolean {
    return lastSentMs === null || nowMs - lastSentMs >= RESYNC_MIN_INTERVAL_MS;
}
