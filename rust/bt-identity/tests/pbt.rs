//! Property-based tests for bt-identity.
//!
//! Covers round-trip signing, wrong-key rejection, tampered-token rejection,
//! and panic-safety for arbitrary garbage inputs.

use bt_identity::{issue_token_with, sanitize_name, verify_token_with};
use proptest::prelude::*;

// Fixed IAT so we don't introduce any time-dependency.
const IAT: i64 = 1_700_000_000;

// A short but representative byte secret.
fn secret_a() -> Vec<u8> {
    b"secret-alpha".to_vec()
}
fn secret_b() -> Vec<u8> {
    b"secret-beta".to_vec()
}

/// Strategy for arbitrary raw strings to test sanitize + round-trip.
/// We deliberately include unicode, mixed whitespace, long strings, and empty.
fn arb_name() -> impl Strategy<Value = String> {
    prop_oneof![
        // Pure ASCII, short
        "[a-zA-Z0-9 _-]{0,40}",
        // Unicode (may include emoji, CJK, etc.)
        ".*",
        // Very long
        prop::string::string_regex("[a-z]{0,128}").unwrap(),
        // All-whitespace (should sanitize to None)
        prop::string::string_regex("[ \t\n\r]{0,20}").unwrap(),
        // Empty
        Just(String::new()),
    ]
}

/// Names GUARANTEED to survive sanitization unchanged: non-empty, no whitespace
/// to trim, within the length cap. The non-vacuous tests below build on this —
/// every `let Some(clean) = sanitize_name(..) else { return }` test passes
/// trivially if `sanitize_name` ever degenerates to `-> None`, so we need at
/// least one property that *demands* `Some` with a concrete expected value.
fn valid_name() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-zA-Z0-9_-]{1,32}").unwrap()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    // -----------------------------------------------------------------------
    // (a0) NON-VACUOUS anchor: a guaranteed-valid name must sanitize to ITSELF
    //      and survive issue→verify unchanged. This is the property that fails
    //      the instant `sanitize_name` returns `None` (or mangles the name),
    //      which the `let Some(clean) = .. else { return }` guards everywhere
    //      else would silently tolerate.
    // -----------------------------------------------------------------------
    #[test]
    fn valid_names_sanitize_to_self_and_round_trip(name in valid_name()) {
        prop_assert_eq!(
            sanitize_name(&name),
            Some(name.clone()),
            "a no-whitespace, in-cap name must sanitize to itself, not {:?}",
            sanitize_name(&name)
        );
        let secret = secret_a();
        let token = issue_token_with(&secret, &name, IAT);
        prop_assert_eq!(
            verify_token_with(&secret, &token),
            Some(name.clone()),
            "issue→verify must recover the exact name for {:?}",
            name
        );
    }

    // -----------------------------------------------------------------------
    // (a) Round-trip: sanitize_name(name) == Some(clean) implies
    //     verify_token_with(secret, issue_token_with(secret, clean, iat))
    //     == Some(clean).
    // -----------------------------------------------------------------------
    #[test]
    fn round_trip_valid_names(raw in arb_name()) {
        // If the name doesn't survive sanitization, skip — nothing to round-trip.
        let Some(clean) = sanitize_name(&raw) else {
            return Ok(());
        };

        let secret = secret_a();
        let token = issue_token_with(&secret, &clean, IAT);
        let verified = verify_token_with(&secret, &token);

        // verify_token_with re-sanitizes internally, so the result should equal
        // sanitize_name(clean) — which is clean itself (idempotent for trimmed,
        // length-bounded strings).
        let expected = sanitize_name(&clean);
        prop_assert_eq!(
            verified,
            expected,
            "round-trip failed for name {:?}: token={:?}",
            clean,
            token
        );
    }

    // -----------------------------------------------------------------------
    // (b) Wrong key: a token signed with secret A always fails verification
    //     under secret B (where A != B).
    // -----------------------------------------------------------------------
    #[test]
    fn wrong_key_always_rejected(raw in arb_name()) {
        let Some(clean) = sanitize_name(&raw) else {
            return Ok(());
        };

        let token = issue_token_with(&secret_a(), &clean, IAT);
        let result = verify_token_with(&secret_b(), &token);
        prop_assert!(
            result.is_none(),
            "wrong key should reject token; got {:?} for name {:?}",
            result,
            clean
        );
    }

    // -----------------------------------------------------------------------
    // (c1) Tampered token fails verification.
    //      Strategy: flip one byte in the signature segment.
    // -----------------------------------------------------------------------
    #[test]
    fn tampered_signature_rejected(
        raw in "[a-zA-Z0-9]{1,30}",  // always valid names
        // which byte in the sig segment to flip (mod seg length)
        flip_idx in any::<usize>(),
    ) {
        let Some(clean) = sanitize_name(&raw) else {
            return Ok(());
        };
        let secret = secret_a();
        let token = issue_token_with(&secret, &clean, IAT);

        // Split off the signature segment (last dot-separated part).
        let last_dot = token.rfind('.').unwrap();
        let (prefix, sig) = token.split_at(last_dot + 1);
        let mut sig_bytes = sig.as_bytes().to_vec();
        if sig_bytes.is_empty() {
            return Ok(());
        }
        let idx = flip_idx % sig_bytes.len();
        // Flip to any different byte value.
        sig_bytes[idx] ^= 0xFF;
        let tampered = format!("{}{}", prefix, String::from_utf8_lossy(&sig_bytes));

        let result = verify_token_with(&secret, &tampered);
        prop_assert!(
            result.is_none(),
            "tampered signature should be rejected; got {:?} for name {:?}",
            result,
            clean
        );
    }

    // -----------------------------------------------------------------------
    // (c2) Tampered payload (swap to a different name) fails verification.
    // -----------------------------------------------------------------------
    #[test]
    fn tampered_payload_rejected(
        raw in "[a-zA-Z]{1,20}",
    ) {
        let Some(clean) = sanitize_name(&raw) else {
            return Ok(());
        };
        let secret = secret_a();
        let token = issue_token_with(&secret, &clean, IAT);

        // Replace the middle segment with a different name's payload.
        let mut parts: Vec<&str> = token.splitn(3, '.').collect();
        if parts.len() != 3 {
            return Ok(());
        }
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine;
        let evil_payload = URL_SAFE_NO_PAD.encode(
            serde_json::json!({ "name": "mallory", "iat": IAT }).to_string()
        );
        parts[1] = &evil_payload;
        let forged = parts.join(".");

        let result = verify_token_with(&secret, &forged);
        prop_assert!(
            result.is_none(),
            "payload swap should be rejected; got {:?}",
            result
        );
    }

    // -----------------------------------------------------------------------
    // (c3) No panics on arbitrary garbage strings.
    // -----------------------------------------------------------------------
    #[test]
    fn verify_never_panics_on_garbage(garbage in ".*") {
        let _ = verify_token_with(&secret_a(), &garbage);
    }

    // -----------------------------------------------------------------------
    // (c4) No panics on arbitrary byte-string secrets (any length, any bytes).
    // -----------------------------------------------------------------------
    #[test]
    fn verify_with_arbitrary_secret_never_panics(
        raw in "[a-zA-Z]{1,20}",
        secret in prop::collection::vec(any::<u8>(), 0..64),
    ) {
        let Some(clean) = sanitize_name(&raw) else {
            return Ok(());
        };
        let token = issue_token_with(&secret, &clean, IAT);
        let _ = verify_token_with(&secret, &token);
    }

    // -----------------------------------------------------------------------
    // Extra: sanitize_name is idempotent — applying it twice == once.
    // -----------------------------------------------------------------------
    // Was failing (sanitize trimmed THEN capped, so a 32nd-char space survived
    // one pass but not the next); fixed by re-trimming after the cap.
    #[test]
    fn sanitize_is_idempotent(raw in arb_name()) {
        let once = sanitize_name(&raw);
        let twice = once.as_deref().and_then(sanitize_name);
        prop_assert_eq!(
            once,
            twice,
            "sanitize_name was not idempotent for {:?}",
            raw
        );
    }

    // -----------------------------------------------------------------------
    // Extra: verified name is always <= MAX_NAME_LEN chars.
    // -----------------------------------------------------------------------
    #[test]
    fn verified_name_respects_max_len(raw in arb_name()) {
        let Some(clean) = sanitize_name(&raw) else {
            return Ok(());
        };
        let secret = secret_a();
        let token = issue_token_with(&secret, &clean, IAT);
        if let Some(verified) = verify_token_with(&secret, &token) {
            prop_assert!(
                verified.chars().count() <= bt_identity::MAX_NAME_LEN,
                "verified name too long ({} chars): {:?}",
                verified.chars().count(),
                verified
            );
        }
    }
}

/// Names that deliberately land whitespace and multibyte characters AT / around
/// the MAX_NAME_LEN truncation boundary — where the `sanitize_name`
/// non-idempotency bug lived (trim-then-truncate could strand a trailing space
/// at char 32). Targets the exact failure mode, not just broad unicode.
fn arb_cap_boundary_name() -> impl Strategy<Value = String> {
    (
        // up to 40 chars (incl. multibyte) so the boundary at 32 is well inside
        prop::string::string_regex("[a-zA-Zé漢🙂0-9]{0,40}").unwrap(),
        prop::string::string_regex("[ \t]{1,3}").unwrap(), // whitespace that may land on the cap
        prop::string::string_regex("[a-z]{0,10}").unwrap(),
    )
        .prop_map(|(a, ws, b)| format!("{a}{ws}{b}"))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// `sanitize_name` is idempotent AND returns a trimmed, cap-bounded string,
    /// even when whitespace/multibyte chars straddle the MAX_NAME_LEN boundary.
    #[test]
    fn sanitize_idempotent_at_cap_boundary(raw in arb_cap_boundary_name()) {
        let once = sanitize_name(&raw);
        let twice = once.as_deref().and_then(sanitize_name);
        prop_assert_eq!(&once, &twice, "sanitize_name not idempotent for {:?}", raw);
        if let Some(s) = &once {
            prop_assert_eq!(s.trim(), s.as_str(), "result has untrimmed whitespace: {:?}", s);
            prop_assert!(s.chars().count() <= bt_identity::MAX_NAME_LEN, "result exceeds cap: {:?}", s);
        }
    }
}
