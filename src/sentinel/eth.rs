//! ETH Scanner Implementation
//!
//! Scans Ethereum blockchain for deposits to monitored addresses.
//! Supports both real Anvil/Geth RPC and mock mode for testing.

use super::config::EthChainConfig;
use super::error::ScannerError;
use super::scanner::{ChainScanner, NodeHealth, ScannedBlock};
use async_trait::async_trait;
use rust_decimal::Decimal;
use std::collections::HashSet;
use std::str::FromStr;
use tracing::{debug, info};

/// ETH Scanner that connects to Ethereum node via JSON-RPC
pub struct EthScanner {
    #[allow(dead_code)]
    config: EthChainConfig,
    /// Set of monitored addresses (lowercase for comparison)
    watched_addresses: HashSet<String>,
    /// Mock mode for testing without real node
    mock_mode: bool,
    /// Mock blocks for testing
    mock_blocks: Vec<ScannedBlock>,
}

impl EthScanner {
    /// Create a new ETH scanner
    pub fn new(config: EthChainConfig) -> Result<Self, ScannerError> {
        info!(
            "Initializing ETH scanner for {} network at {}",
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
    pub fn new_mock(config: EthChainConfig) -> Self {
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
    /// Addresses are normalized to lowercase for case-insensitive matching
    pub fn reload_addresses(&mut self, addresses: Vec<String>) {
        debug!("Reloading {} ETH addresses", addresses.len());
        self.watched_addresses = addresses.into_iter().map(|a| a.to_lowercase()).collect();
    }

    /// Check if an address is being watched (case-insensitive)
    pub fn is_watched(&self, address: &str) -> bool {
        self.watched_addresses.contains(&address.to_lowercase())
    }

    /// Get the number of watched addresses
    pub fn watched_count(&self) -> usize {
        self.watched_addresses.len()
    }
}

#[async_trait]
impl ChainScanner for EthScanner {
    fn chain_id(&self) -> &str {
        "ETH"
    }

    async fn get_latest_height(&self) -> Result<u64, ScannerError> {
        if self.mock_mode {
            return Ok(self.mock_blocks.len() as u64);
        }

        Err(ScannerError::RpcConnection(
            "Real ETH RPC not yet implemented - use mock mode for testing".to_string(),
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
            "Real ETH RPC not yet implemented".to_string(),
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
            "Real ETH RPC not yet implemented".to_string(),
        ))
    }

    async fn health_check(&self) -> Result<NodeHealth, ScannerError> {
        if self.mock_mode {
            return Ok(NodeHealth {
                is_synced: true,
                block_height: self.mock_blocks.len() as u64,
                block_time: chrono::Utc::now().timestamp(),
                peers: 1, // Anvil is single node
            });
        }

        Err(ScannerError::RpcConnection(
            "Real ETH RPC not yet implemented".to_string(),
        ))
    }
}

/// Convert wei (u128 string) to Decimal with 18 decimals
pub fn wei_to_eth(wei_str: &str) -> Decimal {
    Decimal::from_str(wei_str)
        .map(|d| d / Decimal::from(10u64.pow(18)))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::super::scanner::DetectedDeposit;
    use super::*;

    fn test_config() -> EthChainConfig {
        EthChainConfig {
            chain_id: "ETH".to_string(),
            network: "anvil".to_string(),
            rpc: super::super::config::EthRpcConfig {
                url: "http://127.0.0.1:8545".to_string(),
            },
            scanning: super::super::config::ScanningConfig {
                required_confirmations: 1,
                max_reorg_depth: 10,
                start_height: None,
            },
            health: super::super::config::HealthConfig {
                max_block_lag_seconds: 3600,
            },
        }
    }

    #[test]
    fn test_eth_scanner_creation() {
        let scanner = EthScanner::new_mock(test_config());
        assert_eq!(scanner.chain_id(), "ETH");
        assert!(scanner.mock_mode);
    }

    #[test]
    fn test_address_watching_case_insensitive() {
        let mut scanner = EthScanner::new_mock(test_config());

        scanner.reload_addresses(vec![
            "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045".to_string(), // Vitalik
        ]);

        assert_eq!(scanner.watched_count(), 1);
        // Should match regardless of case
        assert!(scanner.is_watched("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"));
        assert!(scanner.is_watched("0xd8da6bf26964af9d7eed9e03e53415d37aa96045"));
        assert!(scanner.is_watched("0xD8DA6BF26964AF9D7EED9E03E53415D37AA96045"));
    }

    #[tokio::test]
    async fn test_mock_block_scanning() {
        let mut scanner = EthScanner::new_mock(test_config());

        let mock_block = ScannedBlock {
            height: 0,
            hash: "0xabc123".to_string(),
            parent_hash: "0x000000".to_string(),
            timestamp: 1700000000,
            deposits: vec![DetectedDeposit {
                tx_hash: "0xtx123".to_string(),
                tx_index: 0,
                vout_index: 0,
                to_address: "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045".to_string(),
                asset: "ETH".to_string(),
                amount: Decimal::new(1000000000000000000, 18), // 1 ETH
                raw_amount: "1000000000000000000".to_string(),
            }],
        };

        scanner.set_mock_blocks(vec![mock_block]);

        let latest = scanner.get_latest_height().await.unwrap();
        assert_eq!(latest, 1);

        let block = scanner.scan_block(0).await.unwrap();
        assert_eq!(block.height, 0);
        assert_eq!(block.deposits.len(), 1);
        assert_eq!(block.deposits[0].asset, "ETH");
    }

    #[test]
    fn test_wei_to_eth_conversion() {
        // 1 ETH = 10^18 wei
        let one_eth = wei_to_eth("1000000000000000000");
        assert_eq!(one_eth, Decimal::new(1, 0));

        // 0.5 ETH
        let half_eth = wei_to_eth("500000000000000000");
        assert_eq!(half_eth, Decimal::new(5, 1));

        // Invalid input returns zero
        let invalid = wei_to_eth("not_a_number");
        assert_eq!(invalid, Decimal::ZERO);
    }

    #[tokio::test]
    async fn test_mock_health_check() {
        let scanner = EthScanner::new_mock(test_config());

        let health = scanner.health_check().await.unwrap();
        assert!(health.is_synced);
        assert_eq!(health.peers, 1);
    }
}
