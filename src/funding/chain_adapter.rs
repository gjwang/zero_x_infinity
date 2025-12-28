use async_trait::async_trait;
use rand::Rng;
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
    async fn generate_address(&self, _user_id: i64) -> Result<String, ChainError> {
        // Real ETH format: 0x + 40 hex chars (20 bytes)
        let mut bytes = [0u8; 20];
        rand::thread_rng().fill(&mut bytes);
        let hex_addr = hex::encode(bytes);
        Ok(format!("0x{}", hex_addr))
    }

    fn validate_address(&self, address: &str) -> bool {
        // Strict Check: starts with 0x, len 42, all hex
        if !address.starts_with("0x") {
            return false;
        }
        if address.len() != 42 {
            return false;
        }
        // Check hex part
        address[2..].chars().all(|c| c.is_ascii_hexdigit())
    }

    async fn broadcast_withdraw(&self, _to: &str, _amount: &str) -> Result<String, ChainError> {
        // Return a fake TxHash (0x + 64 hex chars)
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill(&mut bytes);
        Ok(format!("0x{}", hex::encode(bytes)))
    }
}

/// Mock BTC Chain Client
#[derive(Debug)]
pub struct MockBtcChain;

#[async_trait]
impl ChainClient for MockBtcChain {
    async fn generate_address(&self, _user_id: i64) -> Result<String, ChainError> {
        // Simulate Regtest Bech32 address: bcrt1 + alphanumeric
        // This satisfies DEF-001 (Gateway generating Mainnet addresses)
        let len = rand::thread_rng().gen_range(30..=50);
        let charset = b"023456789acdefghjklmnpqrstuvwxyz"; // Bech32 charset
        let mut rng = rand::thread_rng();

        let suffix: String = (0..len)
            .map(|_| {
                let idx = rng.gen_range(0..charset.len());
                charset[idx] as char
            })
            .collect();

        Ok(format!("bcrt1{}", suffix))
    }

    fn validate_address(&self, address: &str) -> bool {
        // BTC Logic:
        // 1. Legacy: Starts with '1', len 26-35, alphanumeric
        // 2. P2SH: Starts with '3', len 26-35, alphanumeric
        // 3. Bech32: Starts with 'bc1', len <= 90, alphanumeric (approx)

        if address.starts_with('1') || address.starts_with('3') {
            if address.len() < 26 || address.len() > 35 {
                return false;
            }
            return address.chars().all(|c| c.is_ascii_alphanumeric());
        }

        if address.starts_with("bc1") {
            if address.len() > 90 {
                return false;
            }
            return address.chars().all(|c| c.is_ascii_alphanumeric());
        }

        false
    }

    async fn broadcast_withdraw(&self, _to: &str, _amount: &str) -> Result<String, ChainError> {
        // Return a fake TxHash (64 hex chars, no prefix usually for BTC explorers, but internal sys uses strings)
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill(&mut bytes);
        Ok(hex::encode(bytes))
    }
}
