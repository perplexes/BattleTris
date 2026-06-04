//! Player identity: a tiny, self-contained HS256 JWT.
//!
//! `POST /api/identity` with `{"name": "<str>"}` mints a token whose payload is
//! `{"name": <name>, "iat": <unix>}`, signed HMAC-SHA256 with a server secret
//! (`BT_JWT_SECRET`, else a per-process-random secret generated once at
//! startup). The websocket then trusts the *signed* name on `queue`,
//! `available`, and `challenge`, so a client can't impersonate another player by
//! sending a bare `name`.
//!
//! Deliberately hand-rolled rather than pulling in `jsonwebtoken`: HS256 is just
//! `base64url(header).base64url(payload)` HMAC'd, and we only ever issue + verify
//! our own tokens, so the surface is small and fully unit-tested here. Keeps the
//! server's crypto deps to the RustCrypto primitives (`hmac` + `sha2`).

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, Mac};
use serde_json::{json, Value};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Max accepted player-name length (matches the client's `BT_NICKNAMELEN`).
pub const MAX_NAME_LEN: usize = 32;

/// The signing secret. `BT_JWT_SECRET` if set; otherwise 32 random bytes drawn
/// once at process start (so tokens stay valid for the life of the process but
/// don't survive a restart — fine for a lobby session token).
pub fn secret() -> Vec<u8> {
    static SECRET: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    SECRET
        .get_or_init(|| match std::env::var("BT_JWT_SECRET") {
            Ok(s) if !s.is_empty() => s.into_bytes(),
            _ => {
                let mut buf = [0u8; 32];
                // getrandom can only fail if the OS RNG is unavailable; fall back to
                // a time-seeded value so the server still boots (never panics here).
                if getrandom::getrandom(&mut buf).is_err() {
                    let t = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos())
                        .unwrap_or(0);
                    buf[..16].copy_from_slice(&t.to_le_bytes());
                }
                buf.to_vec()
            }
        })
        .clone()
}

/// Trim + bound a requested name. Returns `None` for an empty (post-trim) name;
/// otherwise the name capped at [`MAX_NAME_LEN`] chars. Trims AGAIN after the
/// cap so truncation can't strand trailing whitespace — this keeps `sanitize_name`
/// idempotent (verify re-sanitizes the signed name, so a non-idempotent cap could
/// verify a boundary-length name to a different string than was issued).
pub fn sanitize_name(raw: &str) -> Option<String> {
    let capped: String = raw.trim().chars().take(MAX_NAME_LEN).collect();
    let capped = capped.trim();
    if capped.is_empty() {
        return None;
    }
    Some(capped.to_string())
}

fn sign(secret: &[u8], signing_input: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(signing_input.as_bytes());
    URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
}

/// Mint a token for an already-sanitized `name` using the process secret.
pub fn issue_token(name: &str) -> String {
    issue_token_with(&secret(), name, now_secs())
}

/// Mint a token with an explicit secret + issued-at (the testable core).
pub fn issue_token_with(secret: &[u8], name: &str, iat: i64) -> String {
    let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"HS256","typ":"JWT"}"#);
    let payload = URL_SAFE_NO_PAD.encode(json!({ "name": name, "iat": iat }).to_string());
    let signing_input = format!("{header}.{payload}");
    let sig = sign(secret, &signing_input);
    format!("{signing_input}.{sig}")
}

/// Verify a token against the process secret; returns the signed name if valid.
pub fn verify_token(token: &str) -> Option<String> {
    verify_token_with(&secret(), token)
}

/// Verify a token against an explicit secret (the testable core). Constant-time
/// signature comparison via `Mac::verify_slice`. Returns the signed `name`.
pub fn verify_token_with(secret: &[u8], token: &str) -> Option<String> {
    let mut parts = token.splitn(3, '.');
    let header = parts.next()?;
    let payload = parts.next()?;
    let sig_b64 = parts.next()?;
    if parts.next().is_some() {
        return None; // more than three segments — malformed
    }

    // Recompute the MAC over header.payload and compare in constant time.
    let signing_input = format!("{header}.{payload}");
    let sig = URL_SAFE_NO_PAD.decode(sig_b64).ok()?;
    let mut mac = HmacSha256::new_from_slice(secret).expect("HMAC accepts any key length");
    mac.update(signing_input.as_bytes());
    mac.verify_slice(&sig).ok()?;

    // Signature checks out — decode the claims and pull the name.
    let claims: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).ok()?).ok()?;
    let name = claims.get("name")?.as_str()?;
    sanitize_name(name)
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_a_signed_name() {
        let secret = b"test-secret";
        let token = issue_token_with(secret, "alice", 1700000000);
        assert_eq!(verify_token_with(secret, &token).as_deref(), Some("alice"));
    }

    #[test]
    fn rejects_a_token_signed_with_another_secret() {
        let token = issue_token_with(b"real", "alice", 1700000000);
        assert!(verify_token_with(b"forged", &token).is_none(), "wrong key -> reject");
    }

    #[test]
    fn rejects_a_tampered_payload() {
        // Forge a new payload but keep the original signature -> must fail.
        let token = issue_token_with(b"k", "alice", 1700000000);
        let sig = token.rsplit('.').next().unwrap();
        let evil_payload = URL_SAFE_NO_PAD.encode(r#"{"name":"mallory","iat":1700000000}"#);
        let forged = format!(
            "{}.{}.{}",
            URL_SAFE_NO_PAD.encode(br#"{"alg":"HS256","typ":"JWT"}"#),
            evil_payload,
            sig
        );
        assert!(verify_token_with(b"k", &forged).is_none(), "payload swap breaks the MAC");
    }

    #[test]
    fn rejects_garbage_and_wrong_segment_counts() {
        assert!(verify_token_with(b"k", "").is_none());
        assert!(verify_token_with(b"k", "not.a.jwt").is_none());
        assert!(verify_token_with(b"k", "a.b").is_none(), "two segments");
        assert!(verify_token_with(b"k", "a.b.c.d").is_none(), "four segments");
    }

    #[test]
    fn sanitize_trims_bounds_and_rejects_empty() {
        assert_eq!(sanitize_name("  bob  ").as_deref(), Some("bob"));
        assert!(sanitize_name("   ").is_none(), "empty after trim -> rejected");
        assert!(sanitize_name("").is_none());
        let long = "x".repeat(100);
        assert_eq!(sanitize_name(&long).unwrap().chars().count(), MAX_NAME_LEN);
    }

    #[test]
    fn issued_name_is_sanitized_on_verify() {
        // A token minted around a too-long name still verifies to the capped name.
        let secret = b"k";
        let long = "y".repeat(40);
        let token = issue_token_with(secret, &long, 1);
        let got = verify_token_with(secret, &token).unwrap();
        assert_eq!(got.chars().count(), MAX_NAME_LEN);
    }

    #[test]
    fn secret_is_stable_within_a_process() {
        // Two reads of the (random) secret agree — the OnceLock holds it fixed.
        assert_eq!(secret(), secret());
    }
}
