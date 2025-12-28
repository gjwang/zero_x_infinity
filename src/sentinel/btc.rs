//! BTC Scanner Implementation
//!
//! Scans Bitcoin blockchain for deposits to monitored addresses.
//! Supports both real bitcoind RPC and mock mode for testing.

use super::config::BtcChainConfig;
use super::error::ScannerError;
use super::scanner::{ChainScanner, NodeHealth, ScannedBlock};
use async_trait::async_trait;
use std::collections::HashSet;
use tracing::{debug, info};

/// BTC Scanner that connects to bitcoind via JSON-RPC
pub struct BtcScanner {
    #[allow(dead_code)]
    config: BtcChainConfig,
    /// Set of monitored addresses (loaded from DB)
    watched_addresses: HashSet<String>,
    /// Mock mode for testing without real node
    mock_mode: bool,
    /// Mock blocks for testing
    mock_blocks: Vec<ScannedBlock>,
}

impl BtcScanner {
    /// Create a new BTC scanner
    pub fn new(config: BtcChainConfig) -> Result<Self, ScannerError> {
        info!(
            "Initializing BTC scanner for {} network at {}",
            config.network, config.rpc.url
        );

        Ok(Self {
            config,
            watched_addresses: HashSet::new(),
            mock_mode: false,
            mock_blocks: Vec::new(),
        })
    }

    /// Create a mock scanner for testing
    pub fn new_mock(config: BtcChainConfig) -> Self {
        Self {
            config,
            watched_addresses: HashSet::new(),
            mock_mode: true,
            mock_blocks: Vec::new(),
        }
    }

    /// Set mock blocks for testing
    pub fn set_mock_blocks(&mut self, blocks: Vec<ScannedBlock>) {
        self.mock_blocks = blocks;
    }

    /// Reload watched addresses from database
    pub fn reload_addresses(&mut self, addresses: Vec<String>) {
        debug!("Reloading {} BTC addresses", addresses.len());
        self.watched_addresses = addresses.into_iter().collect();
    }

    /// Check if an address is being watched
    pub fn is_watched(&self, address: &str) -> bool {
        self.watched_addresses.contains(address)
    }

    /// Get the number of watched addresses
    pub fn watched_count(&self) -> usize {
        self.watched_addresses.len()
    }
}

#[async_trait]
impl ChainScanner for BtcScanner {
    fn chain_id(&self) -> &str {
        "BTC"
    }

    async fn get_latest_height(&self) -> Result<u64, ScannerError> {
        if self.mock_mode {
            return Ok(self.mock_blocks.len() as u64);
        }

        // Real RPC call would go here
        // For now, return error indicating real RPC not implemented
        Err(ScannerError::RpcConnection(
            "Real BTC RPC not yet implemented - use mock mode for testing".to_string(),
        ))
    }

    async fn scan_block(&self, height: u64) -> Result<ScannedBlock, ScannerError> {
        if self.mock_mode {
            return self
                .mock_blocks
                .get(height as usize)
                .cloned()
                .ok_or(ScannerError::BlockNotFound(height));
        }

        Err(ScannerError::RpcConnection(
            "Real BTC RPC not yet implemented".to_string(),
        ))
    }

    async fn verify_block_hash(
        &self,
        height: u64,
        expected_hash: &str,
    ) -> Result<bool, ScannerError> {
        if self.mock_mode {
            if let Some(block) = self.mock_blocks.get(height as usize) {
                return Ok(block.hash == expected_hash);
            }
            return Err(ScannerError::BlockNotFound(height));
        }

        Err(ScannerError::RpcConnection(
            "Real BTC RPC not yet implemented".to_string(),
        ))
    }

    async fn health_check(&self) -> Result<NodeHealth, ScannerError> {
        if self.mock_mode {
            return Ok(NodeHealth {
                is_synced: true,
                block_height: self.mock_blocks.len() as u64,
                block_time: chrono::Utc::now().timestamp(),
                peers: 8,
            });
        }

        Err(ScannerError::RpcConnection(
            "Real BTC RPC not yet implemented".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::super::scanner::DetectedDeposit;
    use super::*;
    use rust_decimal::Decimal;

    fn test_config() -> BtcChainConfig {
        BtcChainConfig {
            chain_id: "BTC".to_string(),
            network: "regtest".to_string(),
            rpc: super::super::config::BtcRpcConfig {
                url: "http://127.0.0.1:18443".to_string(),
                user: "user".to_string(),
                password: "pass".to_string(),
            },
            scanning: super::super::config::ScanningConfig {
                required_confirmations: 3,
                max_reorg_depth: 10,
                start_height: Some(0),
            },
            health: super::super::config::HealthConfig {
                max_block_lag_seconds: 3600,
            },
        }
    }

    #[test]
    fn test_btc_scanner_creation() {
        let scanner = BtcScanner::new_mock(test_config());
        assert_eq!(scanner.chain_id(), "BTC");
        assert!(scanner.mock_mode);
    }

    #[test]
    fn test_address_watching() {
        let mut scanner = BtcScanner::new_mock(test_config());

        scanner.reload_addresses(vec![
            "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa".to_string(),
            "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq".to_string(),
        ]);

        assert_eq!(scanner.watched_count(), 2);
        assert!(scanner.is_watched("1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa"));
        assert!(scanner.is_watched("bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq"));
        assert!(!scanner.is_watched("unknown_address"));
    }

    #[tokio::test]
    async fn test_mock_block_scanning() {
        let mut scanner = BtcScanner::new_mock(test_config());

        let mock_block = ScannedBlock {
            height: 0,
            hash: "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f".to_string(),
            parent_hash: "0000000000000000000000000000000000000000000000000000000000000000"
                .to_string(),
            timestamp: 1231006505,
            deposits: vec![DetectedDeposit {
                tx_hash: "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b"
                    .to_string(),
                tx_index: 0,
                vout_index: 0,
                to_address: "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa".to_string(),
                asset: "BTC".to_string(),
                amount: Decimal::new(5000000000, 8), // 50 BTC
                raw_amount: "5000000000".to_string(),
            }],
        };

        scanner.set_mock_blocks(vec![mock_block]);

        let latest = scanner.get_latest_height().await.unwrap();
        assert_eq!(latest, 1);

        let block = scanner.scan_block(0).await.unwrap();
        assert_eq!(block.height, 0);
        assert_eq!(block.deposits.len(), 1);
        assert_eq!(block.deposits[0].asset, "BTC");
    }

    #[tokio::test]
    async fn test_mock_health_check() {
        let scanner = BtcScanner::new_mock(test_config());

        let health = scanner.health_check().await.unwrap();
        assert!(health.is_synced);
        assert_eq!(health.peers, 8);
    }

    #[tokio::test]
    async fn test_verify_block_hash() {
        let mut scanner = BtcScanner::new_mock(test_config());

        let mock_block = ScannedBlock {
            height: 0,
            hash: "blockhash123".to_string(),
            parent_hash: "parent000".to_string(),
            timestamp: 1000,
            deposits: vec![],
        };

        scanner.set_mock_blocks(vec![mock_block]);

        assert!(scanner.verify_block_hash(0, "blockhash123").await.unwrap());
        assert!(!scanner.verify_block_hash(0, "wronghash").await.unwrap());
    }
}
