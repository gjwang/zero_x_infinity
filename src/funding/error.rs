use thiserror::Error;

#[derive(Error, Debug)]
pub enum TransferError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Insufficient balance")]
    InsufficientBalance,

    #[error("Invalid account type")]
    InvalidAccountType,

    #[error("Source and destination accounts are the same")]
    SameAccount,

    #[error("Invalid asset: {0}")]
    InvalidAsset(String),

    #[error("Invalid amount: must be positive")]
    InvalidAmount,

    #[error("Invalid amount format")]
    InvalidAmountFormat,
}
