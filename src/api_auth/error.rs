//! Authentication error types.
//!
//! Provides structured error codes for API authentication failures.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// Authentication error codes (4001-4008).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum AuthErrorCode {
    /// 4001: Authorization header format error
    InvalidFormat = 4001,
    /// 4002: Unsupported protocol version
    UnsupportedVersion = 4002,
    /// 4003: API Key format error or not found
    InvalidApiKey = 4003,
    /// 4004: ts_nonce must be monotonically increasing
    TsNonceRejected = 4004,
    /// 4005: ts_nonce too far from server time
    TsNonceTooFar = 4005,
    /// 4006: Signature verification failed
    InvalidSignature = 4006,
    /// 4007: Insufficient permissions
    PermissionDenied = 4007,
    /// 4008: API Key is disabled
    ApiKeyDisabled = 4008,
    /// 4009: Internal server error
    InternalError = 4009,
}

impl AuthErrorCode {
    /// Get error code as i32.
    pub fn code(self) -> i32 {
        self as i32
    }

    /// Get error name string.
    pub fn name(self) -> &'static str {
        match self {
            Self::InvalidFormat => "INVALID_FORMAT",
            Self::UnsupportedVersion => "UNSUPPORTED_VERSION",
            Self::InvalidApiKey => "INVALID_API_KEY",
            Self::TsNonceRejected => "TS_NONCE_REJECTED",
            Self::TsNonceTooFar => "TS_NONCE_TOO_FAR",
            Self::InvalidSignature => "INVALID_SIGNATURE",
            Self::PermissionDenied => "PERMISSION_DENIED",
            Self::ApiKeyDisabled => "API_KEY_DISABLED",
            Self::InternalError => "INTERNAL_ERROR",
        }
    }

    /// Get HTTP status code.
    pub fn http_status(self) -> StatusCode {
        match self {
            Self::PermissionDenied => StatusCode::FORBIDDEN,
            Self::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
            _ => StatusCode::UNAUTHORIZED,
        }
    }
}

/// Authentication error with message.
#[derive(Debug, Clone)]
pub struct AuthError {
    pub code: AuthErrorCode,
    pub message: String,
}

impl AuthError {
    /// Create a new auth error.
    pub fn new(code: AuthErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// Create error with default message.
    pub fn from_code(code: AuthErrorCode) -> Self {
        let message = match code {
            AuthErrorCode::InvalidFormat => "Invalid Authorization header format",
            AuthErrorCode::UnsupportedVersion => "Unsupported auth protocol version",
            AuthErrorCode::InvalidApiKey => "Invalid or unknown API Key",
            AuthErrorCode::TsNonceRejected => "ts_nonce must be greater than previous value",
            AuthErrorCode::TsNonceTooFar => "ts_nonce too far from server time",
            AuthErrorCode::InvalidSignature => "Signature verification failed",
            AuthErrorCode::PermissionDenied => "Insufficient permissions for this operation",
            AuthErrorCode::ApiKeyDisabled => "API Key is disabled",
            AuthErrorCode::InternalError => "Internal server error",
        };
        Self::new(code, message)
    }
}

/// JSON response body for auth errors.
#[derive(Debug, Serialize)]
pub struct AuthErrorResponse {
    pub code: i32,
    pub error: &'static str,
    pub message: String,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let body = AuthErrorResponse {
            code: self.code.code(),
            error: self.code.name(),
            message: self.message,
        };
        (self.code.http_status(), Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        assert_eq!(AuthErrorCode::InvalidFormat.code(), 4001);
        assert_eq!(AuthErrorCode::ApiKeyDisabled.code(), 4008);
    }

    #[test]
    fn test_error_names() {
        assert_eq!(AuthErrorCode::InvalidFormat.name(), "INVALID_FORMAT");
        assert_eq!(AuthErrorCode::TsNonceRejected.name(), "TS_NONCE_REJECTED");
    }

    #[test]
    fn test_http_status() {
        assert_eq!(
            AuthErrorCode::InvalidSignature.http_status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            AuthErrorCode::PermissionDenied.http_status(),
            StatusCode::FORBIDDEN
        );
    }

    #[test]
    fn test_error_from_code() {
        let err = AuthError::from_code(AuthErrorCode::InvalidApiKey);
        assert_eq!(err.code, AuthErrorCode::InvalidApiKey);
        assert!(err.message.contains("Invalid"));
    }
}
