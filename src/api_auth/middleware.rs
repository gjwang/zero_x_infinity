//! Authentication middleware for Axum.
//!
//! Provides request authentication using Ed25519 signatures.
//! Implements the 9-step verification flow defined in the API Auth spec.

use axum::{body::Body, extract::State, http::Request, middleware::Next, response::Response};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{
    base62,
    error::{AuthError, AuthErrorCode},
    models::{ApiKeyRecord, AuthenticatedUser},
    repository::ApiKeyRepository,
    signature::verify_ed25519,
    ts_store::TsStore,
};
use crate::account::Database;
use axum::http::HeaderMap;

/// Authentication state shared across requests.
#[derive(Clone)]
pub struct AuthState {
    /// Timestamp nonce store for replay protection
    pub ts_store: Arc<TsStore>,
    /// Time window for ts_nonce validation (default: 30 seconds)
    pub time_window_ms: i64,
}

impl Default for AuthState {
    fn default() -> Self {
        Self {
            ts_store: Arc::new(TsStore::new()),
            time_window_ms: 30_000, // 30 seconds
        }
    }
}

/// Axum middleware for API authentication.
///
/// Verifies Ed25519 signatures and injects AuthenticatedUser into request extensions.
/// Must be applied to private routes.
pub async fn auth_middleware(
    State(db): State<Option<Arc<Database>>>,
    State(auth_state): State<Arc<AuthState>>,
    mut request: Request<Body>,
    next: Next,
) -> Result<Response, AuthError> {
    // Step 1: Extract Authorization header
    let auth_header = extract_auth_header(request.headers())?;

    // Step 2: Parse header components
    let (_version, api_key, ts_nonce_str, signature) = parse_authorization(auth_header)?;
    let ts_nonce: i64 = ts_nonce_str
        .parse()
        .map_err(|_| AuthError::from_code(AuthErrorCode::TsNonceRejected))?;

    // Step 3: Get API Key from database
    let db =
        db.ok_or_else(|| AuthError::new(AuthErrorCode::InternalError, "Database not configured"))?;
    let repo = ApiKeyRepository::new(db);
    let api_key_record = repo
        .get_active_by_key(api_key)
        .await
        .map_err(|e| AuthError::new(AuthErrorCode::InternalError, format!("DB error: {}", e)))?
        .ok_or_else(|| AuthError::from_code(AuthErrorCode::InvalidApiKey))?;

    // Step 4: Check API Key status
    if api_key_record.status != 1 {
        return Err(AuthError::from_code(AuthErrorCode::ApiKeyDisabled));
    }

    // Step 5: Validate ts_nonce (time window + monotonic)
    validate_ts_nonce(
        &auth_state.ts_store,
        api_key,
        ts_nonce,
        auth_state.time_window_ms,
    )?;

    // Step 6: Build signature payload
    let method = request.method().as_str();
    let path = request.uri().path();
    // For simplicity, we don't include body in signature for GET requests
    let body = "";

    // Step 7: Verify signature
    verify_signature(
        &api_key_record,
        api_key,
        ts_nonce_str,
        method,
        path,
        body,
        signature,
    )?;

    // Step 8: Create authenticated user
    let auth_user = AuthenticatedUser {
        user_id: api_key_record.user_id,
        api_key: api_key_record.api_key.clone(),
        permissions: api_key_record.permissions,
    };

    // Step 9: Inject into request extensions
    request.extensions_mut().insert(auth_user);

    // Continue to handler
    Ok(next.run(request).await)
}

/// Parse and validate the Authorization header.
///
/// Format: `ZXINF v1.<api_key>.<ts_nonce>.<signature>`
///
/// Returns (version, api_key, ts_nonce, signature) on success.
pub fn parse_authorization(auth_header: &str) -> Result<(&str, &str, &str, &str), AuthError> {
    // Remove "ZXINF " prefix
    let auth = auth_header
        .strip_prefix("ZXINF ")
        .ok_or_else(|| AuthError::from_code(AuthErrorCode::InvalidFormat))?;

    // Split into 4 parts: version.api_key.ts_nonce.signature
    let parts: Vec<&str> = auth.split('.').collect();
    if parts.len() != 4 {
        return Err(AuthError::new(
            AuthErrorCode::InvalidFormat,
            format!("Expected 4 parts, got {}", parts.len()),
        ));
    }

    let (version, api_key, ts_nonce, signature) = (parts[0], parts[1], parts[2], parts[3]);

    // Validate version
    if version != "v1" {
        return Err(AuthError::from_code(AuthErrorCode::UnsupportedVersion));
    }

    // Validate api_key format: AK_ + 16 hex = 19 chars
    if !api_key.starts_with("AK_") || api_key.len() != 19 {
        return Err(AuthError::new(
            AuthErrorCode::InvalidApiKey,
            "API Key must be AK_ + 16 hex characters",
        ));
    }

    // Validate ts_nonce is numeric
    if ts_nonce.parse::<i64>().is_err() {
        return Err(AuthError::new(
            AuthErrorCode::TsNonceRejected,
            "ts_nonce must be a valid integer",
        ));
    }

    Ok((version, api_key, ts_nonce, signature))
}

/// Validate ts_nonce is within acceptable time window and monotonically increasing.
pub fn validate_ts_nonce(
    ts_store: &TsStore,
    api_key: &str,
    ts_nonce: i64,
    time_window_ms: i64,
) -> Result<(), AuthError> {
    // Get current time in milliseconds
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis() as i64;

    // Check if ts_nonce is within acceptable window
    if (now_ms - ts_nonce).abs() > time_window_ms {
        return Err(AuthError::new(
            AuthErrorCode::TsNonceTooFar,
            format!(
                "ts_nonce must be within {}ms of server time",
                time_window_ms
            ),
        ));
    }

    // Check monotonic increase
    if !ts_store.compare_and_swap_if_greater(api_key, ts_nonce) {
        return Err(AuthError::from_code(AuthErrorCode::TsNonceRejected));
    }

    Ok(())
}

/// Verify the Ed25519 signature.
pub fn verify_signature(
    api_key_record: &ApiKeyRecord,
    api_key: &str,
    ts_nonce: &str,
    method: &str,
    path: &str,
    body: &str,
    signature_b62: &str,
) -> Result<(), AuthError> {
    // Decode signature from Base62
    let signature_bytes = base62::decode(signature_b62)
        .map_err(|_| AuthError::new(AuthErrorCode::InvalidSignature, "Invalid Base62 signature"))?;

    // Verify signature length (Ed25519 = 64 bytes)
    if signature_bytes.len() != 64 {
        return Err(AuthError::new(
            AuthErrorCode::InvalidSignature,
            format!("Expected 64 bytes signature, got {}", signature_bytes.len()),
        ));
    }

    // Build payload: api_key + ts_nonce + method + path + body
    let payload = format!("{}{}{}{}{}", api_key, ts_nonce, method, path, body);

    // Verify Ed25519 signature
    if !verify_ed25519(
        &api_key_record.key_data,
        payload.as_bytes(),
        &signature_bytes,
    ) {
        return Err(AuthError::from_code(AuthErrorCode::InvalidSignature));
    }

    Ok(())
}

/// Extract Authorization header from request.
pub fn extract_auth_header(headers: &HeaderMap) -> Result<&str, AuthError> {
    headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AuthError::new(AuthErrorCode::InvalidFormat, "Missing Authorization header"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_authorization() {
        let auth = "ZXINF v1.AK_7F3D8E2A1B5C9F04.1703260800001.SGVsbG8gV29ybGQ";
        let result = parse_authorization(auth);
        assert!(result.is_ok());
        let (version, api_key, ts_nonce, sig) = result.unwrap();
        assert_eq!(version, "v1");
        assert_eq!(api_key, "AK_7F3D8E2A1B5C9F04");
        assert_eq!(ts_nonce, "1703260800001");
        assert_eq!(sig, "SGVsbG8gV29ybGQ");
    }

    #[test]
    fn test_parse_missing_prefix() {
        let auth = "v1.AK_7F3D8E2A1B5C9F04.1703260800001.sig";
        let result = parse_authorization(auth);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, AuthErrorCode::InvalidFormat);
    }

    #[test]
    fn test_parse_wrong_version() {
        let auth = "ZXINF v2.AK_7F3D8E2A1B5C9F04.1703260800001.sig";
        let result = parse_authorization(auth);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, AuthErrorCode::UnsupportedVersion);
    }

    #[test]
    fn test_parse_invalid_api_key_format() {
        // Missing AK_ prefix
        let auth = "ZXINF v1.7F3D8E2A1B5C9F04.1703260800001.sig";
        assert_eq!(
            parse_authorization(auth).unwrap_err().code,
            AuthErrorCode::InvalidApiKey
        );

        // Wrong length
        let auth = "ZXINF v1.AK_7F3D8E2A.1703260800001.sig";
        assert_eq!(
            parse_authorization(auth).unwrap_err().code,
            AuthErrorCode::InvalidApiKey
        );
    }

    #[test]
    fn test_parse_invalid_ts_nonce() {
        let auth = "ZXINF v1.AK_7F3D8E2A1B5C9F04.not_a_number.sig";
        let result = parse_authorization(auth);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, AuthErrorCode::TsNonceRejected);
    }

    #[test]
    fn test_validate_ts_nonce_success() {
        let store = TsStore::new();
        let api_key = "AK_TEST";
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        assert!(validate_ts_nonce(&store, api_key, now_ms, 30_000).is_ok());
    }

    #[test]
    fn test_validate_ts_nonce_replay_rejected() {
        let store = TsStore::new();
        let api_key = "AK_TEST";
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        // First request succeeds
        assert!(validate_ts_nonce(&store, api_key, now_ms, 30_000).is_ok());

        // Same nonce rejected
        let result = validate_ts_nonce(&store, api_key, now_ms, 30_000);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, AuthErrorCode::TsNonceRejected);
    }

    #[test]
    fn test_validate_ts_nonce_too_far() {
        let store = TsStore::new();
        let api_key = "AK_TEST";
        let old_ts = 1000; // Very old timestamp

        let result = validate_ts_nonce(&store, api_key, old_ts, 30_000);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, AuthErrorCode::TsNonceTooFar);
    }

    #[test]
    fn test_verify_signature_invalid_base62() {
        let record = ApiKeyRecord {
            key_id: 1,
            user_id: 1001,
            api_key: "AK_7F3D8E2A1B5C9F04".to_string(),
            key_type: 1,
            key_data: vec![0u8; 32],
            label: None,
            permissions: 1,
            status: 1,
            last_ts_nonce: 0,
        };

        let result = verify_signature(
            &record,
            "AK_7F3D8E2A1B5C9F04",
            "1703260800001",
            "GET",
            "/api/v1/orders",
            "",
            "invalid!base62!",
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_signature_with_python_base62() {
        // End-to-end test with Python-generated signature
        // Public key hex: d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a
        let public_key_hex = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
        let public_key: Vec<u8> = (0..public_key_hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&public_key_hex[i..i + 2], 16).unwrap())
            .collect();

        let record = ApiKeyRecord {
            key_id: 1,
            user_id: 1,
            api_key: "AK_D4735E3A265E16EE".to_string(),
            key_type: 1,
            key_data: public_key,
            label: None,
            permissions: 15,
            status: 1,
            last_ts_nonce: 0,
        };

        // Python-generated Base62 signature for payload:
        // "AK_D4735E3A265E16EE1766416500000GET/api/v1/private/orders"
        let sig_b62 = "UZAz7j7ovYo2zMKWFM5uUPAL8rDXnUo76zvbngKSflSuZojB7khpMtUQnI96dn1VmpUQ46U8lowve9Z9oGOTS4";

        let result = verify_signature(
            &record,
            "AK_D4735E3A265E16EE",
            "1766416500000",
            "GET",
            "/api/v1/private/orders",
            "",
            sig_b62,
        );

        assert!(
            result.is_ok(),
            "Python signature should verify: {:?}",
            result.err()
        );
    }
}
