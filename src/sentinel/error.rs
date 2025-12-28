use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScannerError {
    #[error("RPC connection failed: {0}")]
    RpcConnection(String),

    #[error("Block not found at height {0}")]
    BlockNotFound(u64),

    #[error("Node is unhealthy or stale")]
    NodeUnhealthy,

    #[error("Re-org detected at height {0}")]
    ReorgDetected(u64),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Parse error: {0}")]
    Parse(String),
}

#[derive(Debug, Error)]
pub enum SentinelError {
    #[error("Scanner error: {0}")]
    Scanner(#[from] ScannerError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    Config(String),
}
