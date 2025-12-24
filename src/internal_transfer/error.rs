//! Transfer Error Types
//!
//! Defines all error types for the transfer module following the design doc.

use thiserror::Error;

/// Transfer error types
///
/// Error codes match the design document for consistent API responses.
#[derive(Error, Debug, Clone)]
pub enum TransferError {
    // === Validation Errors ===
    #[error("User not authenticated")]
    Unauthorized,

    #[error("User ID mismatch - forbidden")]
    Forbidden,

    #[error("Source and target account cannot be the same")]
    SameAccount,

    #[error("Invalid account type: {0}")]
    InvalidAccountType(String),

    #[error("Unsupported account type: {0}")]
    UnsupportedAccountType(String),

    #[error("Amount must be greater than zero")]
    InvalidAmount,

    #[error("Amount precision exceeds asset limit")]
    PrecisionOverflow,

    #[error("Amount is too small (below minimum)")]
    AmountTooSmall,

    #[error("Amount is too large (exceeds maximum)")]
    AmountTooLarge,

    #[error("Amount would cause overflow")]
    Overflow,

    // === Asset Errors ===
    #[error("Asset not found: {0}")]
    InvalidAsset(String),

    #[error("Asset is suspended")]
    AssetSuspended,

    #[error("Internal transfer not allowed for this asset")]
    TransferNotAllowed,

    // === Account Errors ===
    #[error("Source account not found")]
    SourceAccountNotFound,

    #[error("Target account not found (FUNDING must exist)")]
    TargetAccountNotFound,

    #[error("Account is frozen")]
    AccountFrozen,

    #[error("Account is disabled")]
    AccountDisabled,

    #[error("Insufficient balance")]
    InsufficientBalance,

    // === Idempotency Errors ===
    #[error("Duplicate request (cid already exists)")]
    DuplicateRequest,

    // === System Errors ===
    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Internal system error: {0}")]
    SystemError(String),

    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),

    #[error("Transfer not found: {0}")]
    TransferNotFound(String),

    #[error("Invalid state transition: {0}")]
    InvalidStateTransition(String),
}

impl TransferError {
    /// Get the error code for API responses
    pub fn code(&self) -> &'static str {
        match self {
            TransferError::Unauthorized => "UNAUTHORIZED",
            TransferError::Forbidden => "FORBIDDEN",
            TransferError::SameAccount => "SAME_ACCOUNT",
            TransferError::InvalidAccountType(_) => "INVALID_ACCOUNT_TYPE",
            TransferError::UnsupportedAccountType(_) => "UNSUPPORTED_ACCOUNT_TYPE",
            TransferError::InvalidAmount => "INVALID_AMOUNT",
            TransferError::PrecisionOverflow => "PRECISION_OVERFLOW",
            TransferError::AmountTooSmall => "AMOUNT_TOO_SMALL",
            TransferError::AmountTooLarge => "AMOUNT_TOO_LARGE",
            TransferError::Overflow => "OVERFLOW",
            TransferError::InvalidAsset(_) => "INVALID_ASSET",
            TransferError::AssetSuspended => "ASSET_SUSPENDED",
            TransferError::TransferNotAllowed => "TRANSFER_NOT_ALLOWED",
            TransferError::SourceAccountNotFound => "SOURCE_ACCOUNT_NOT_FOUND",
            TransferError::TargetAccountNotFound => "TARGET_ACCOUNT_NOT_FOUND",
            TransferError::AccountFrozen => "ACCOUNT_FROZEN",
            TransferError::AccountDisabled => "ACCOUNT_DISABLED",
            TransferError::InsufficientBalance => "INSUFFICIENT_BALANCE",
            TransferError::DuplicateRequest => "DUPLICATE_REQUEST",
            TransferError::DatabaseError(_) => "DATABASE_ERROR",
            TransferError::SystemError(_) => "SYSTEM_ERROR",
            TransferError::ServiceUnavailable(_) => "SERVICE_UNAVAILABLE",
            TransferError::TransferNotFound(_) => "TRANSFER_NOT_FOUND",
            TransferError::InvalidStateTransition(_) => "INVALID_STATE_TRANSITION",
        }
    }

    /// Get HTTP status code suggestion
    pub fn http_status(&self) -> u16 {
        match self {
            TransferError::Unauthorized => 401,
            TransferError::Forbidden => 403,
            TransferError::SameAccount
            | TransferError::InvalidAccountType(_)
            | TransferError::UnsupportedAccountType(_)
            | TransferError::InvalidAmount
            | TransferError::PrecisionOverflow
            | TransferError::AmountTooSmall
            | TransferError::AmountTooLarge
            | TransferError::Overflow
            | TransferError::InvalidAsset(_)
            | TransferError::DuplicateRequest => 400,
            TransferError::AssetSuspended
            | TransferError::TransferNotAllowed
            | TransferError::SourceAccountNotFound
            | TransferError::TargetAccountNotFound
            | TransferError::AccountFrozen
            | TransferError::AccountDisabled
            | TransferError::InsufficientBalance => 422,
            TransferError::TransferNotFound(_) => 404,
            TransferError::DatabaseError(_)
            | TransferError::SystemError(_)
            | TransferError::InvalidStateTransition(_) => 500,
            TransferError::ServiceUnavailable(_) => 503,
        }
    }
}

impl From<sqlx::Error> for TransferError {
    fn from(e: sqlx::Error) -> Self {
        TransferError::DatabaseError(e.to_string())
    }
}

impl From<anyhow::Error> for TransferError {
    fn from(e: anyhow::Error) -> Self {
        TransferError::SystemError(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        assert_eq!(TransferError::SameAccount.code(), "SAME_ACCOUNT");
        assert_eq!(
            TransferError::InsufficientBalance.code(),
            "INSUFFICIENT_BALANCE"
        );
        assert_eq!(TransferError::Unauthorized.code(), "UNAUTHORIZED");
    }

    #[test]
    fn test_http_status() {
        assert_eq!(TransferError::Unauthorized.http_status(), 401);
        assert_eq!(TransferError::Forbidden.http_status(), 403);
        assert_eq!(TransferError::InvalidAmount.http_status(), 400);
        assert_eq!(TransferError::InsufficientBalance.http_status(), 422);
        assert_eq!(TransferError::SystemError("test".into()).http_status(), 500);
    }

    #[test]
    fn test_display() {
        let err = TransferError::InsufficientBalance;
        assert_eq!(err.to_string(), "Insufficient balance");
    }
}
