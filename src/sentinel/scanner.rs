use async_trait::async_trait;
use rust_decimal::Decimal;

use super::error::ScannerError;

/// Unified interface for scanning different blockchains
#[async_trait]
pub trait ChainScanner: Send + Sync {
    /// Chain identifier (e.g., "BTC", "ETH")
    fn chain_id(&self) -> &str;

    /// Get the latest block height from the node
    async fn get_latest_height(&self) -> Result<u64, ScannerError>;

    /// Fetch a specific block and extract deposits
    async fn scan_block(&self, height: u64) -> Result<ScannedBlock, ScannerError>;

    /// Verify if a block at given height still has the expected hash
    /// Used for re-org detection
    async fn verify_block_hash(
        &self,
        height: u64,
        expected_hash: &str,
    ) -> Result<bool, ScannerError>;

    /// Health check: is the node synced and responsive?
    async fn health_check(&self) -> Result<NodeHealth, ScannerError>;

    /// Reload watched addresses from database
    /// Called periodically by Worker to sync with user_addresses table
    fn reload_addresses(&mut self, addresses: Vec<String>);

    /// Get the number of watched addresses
    fn watched_count(&self) -> usize;
}

/// Result of scanning a single block
#[derive(Debug, Clone)]
pub struct ScannedBlock {
    pub height: u64,
    pub hash: String,
    pub parent_hash: String,
    pub timestamp: i64,
    pub deposits: Vec<DetectedDeposit>,
}

/// A detected deposit in a block
#[derive(Debug, Clone)]
pub struct DetectedDeposit {
    pub tx_hash: String,
    pub tx_index: u32,
    pub vout_index: u32,
    pub to_address: String,
    pub asset: String,
    pub amount: Decimal,
    pub raw_amount: String,
}

/// Node health status
#[derive(Debug, Clone)]
pub struct NodeHealth {
    pub is_synced: bool,
    pub block_height: u64,
    pub block_time: i64,
    pub peers: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    // TDD Step 1 (RED): Test that ScannedBlock can be created with deposits
    #[test]
    fn test_scanned_block_with_deposits() {
        let deposit = DetectedDeposit {
            tx_hash: "abc123".to_string(),
            tx_index: 0,
            vout_index: 0,
            to_address: "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa".to_string(),
            asset: "BTC".to_string(),
            amount: Decimal::new(100_000_000, 8), // 1 BTC
            raw_amount: "100000000".to_string(),
        };

        let block = ScannedBlock {
            height: 100,
            hash: "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f".to_string(),
            parent_hash: "0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            timestamp: 1231006505,
            deposits: vec![deposit.clone()],
        };

        assert_eq!(block.height, 100);
        assert_eq!(block.deposits.len(), 1);
        assert_eq!(block.deposits[0].asset, "BTC");
    }

    // TDD Step 1 (RED): Test NodeHealth creation
    #[test]
    fn test_node_health_synced() {
        let health = NodeHealth {
            is_synced: true,
            block_height: 800_000,
            block_time: 1703721600,
            peers: 8,
        };

        assert!(health.is_synced);
        assert_eq!(health.block_height, 800_000);
    }
}
