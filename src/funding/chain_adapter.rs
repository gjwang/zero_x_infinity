use async_trait::async_trait;
use std::fmt::Debug;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChainError {
    #[error("Network error: {0}")]
    Network(String),
    #[error("Invalid address format")]
    InvalidAddress,
    #[error("Unsupported asset")]
    UnsupportedAsset,
}

#[async_trait]
pub trait ChainClient: Send + Sync + Debug {
    /// Generate a new deposit address for a user
    async fn generate_address(&self, user_id: i64) -> Result<String, ChainError>;

    /// Validate an address format
    fn validate_address(&self, address: &str) -> bool;

    /// Broadcast a withdrawal transaction (Simulation)
    /// Returns the simulated TxHash
    async fn broadcast_withdraw(&self, to: &str, amount: &str) -> Result<String, ChainError>;
}

/// Mock EVM Chain Client (ETH, ERC20)
#[derive(Debug)]
pub struct MockEvmChain;

#[async_trait]
impl ChainClient for MockEvmChain {
    async fn generate_address(&self, user_id: i64) -> Result<String, ChainError> {
        // Simulate deterministic address generation based on user_id
        // Format: 0x...
        // We use a simple hash of user_id to make it look real but deterministic
        let hash = md5::compute(format!("eth_{}", user_id));
        Ok(format!("0x{:x}", hash))
    }

    fn validate_address(&self, address: &str) -> bool {
        address.starts_with("0x") && (address.len() == 34 || address.len() == 42)
    }

    async fn broadcast_withdraw(&self, _to: &str, _amount: &str) -> Result<String, ChainError> {
        // Return a fake TxHash
        let tx_id = uuid::Uuid::new_v4();
        Ok(format!("0x{:x}", tx_id.simple()))
    }
}

/// Mock BTC Chain Client
#[derive(Debug)]
pub struct MockBtcChain;

#[async_trait]
impl ChainClient for MockBtcChain {
    async fn generate_address(&self, user_id: i64) -> Result<String, ChainError> {
        // Simulate deterministic BTC address
        // Format: 1... (Legacy) or bc1... (Segwit). Let's use 1... for simplicity of mock
        let hash = md5::compute(format!("btc_{}", user_id));
        Ok(format!("1{:x}", hash))
    }

    fn validate_address(&self, address: &str) -> bool {
        address.starts_with('1') || address.starts_with("bc1")
    }

    async fn broadcast_withdraw(&self, _to: &str, _amount: &str) -> Result<String, ChainError> {
        // Return a fake TxHash
        let tx_id = uuid::Uuid::new_v4();
        Ok(format!("{:x}", tx_id.simple()))
    }
}
