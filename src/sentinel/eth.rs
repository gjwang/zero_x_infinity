//! ETH Scanner Implementation
//!
//! Scans Ethereum blockchain for deposits to monitored addresses.
//! Supports both real Anvil/Geth RPC and mock mode for testing.
//!
//! Phase 0x11-b: Real ETH RPC implementation using JSON-RPC.

use super::config::EthChainConfig;
use super::error::ScannerError;
use super::scanner::{ChainScanner, DetectedDeposit, NodeHealth, ScannedBlock};
use async_trait::async_trait;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

/// ETH Scanner that connects to Ethereum node via JSON-RPC
pub struct EthScanner {
    config: EthChainConfig,
    /// HTTP client for RPC calls
    client: Option<Arc<Mutex<reqwest::Client>>>,
    /// Set of monitored addresses (lowercase for comparison)
    watched_addresses: HashSet<String>,
    /// Mock mode for testing without real node
    mock_mode: bool,
    /// Mock blocks for testing
    mock_blocks: Vec<ScannedBlock>,
}

/// JSON-RPC request structure
#[derive(Serialize)]
struct JsonRpcRequest<T> {
    jsonrpc: &'static str,
    method: &'static str,
    params: T,
    id: u64,
}

/// JSON-RPC response structure
#[derive(Deserialize)]
struct JsonRpcResponse<T> {
    #[allow(dead_code)]
    jsonrpc: String,
    result: Option<T>,
    error: Option<JsonRpcError>,
    #[allow(dead_code)]
    id: u64,
}

#[derive(Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

/// ETH block structure from RPC
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct EthBlock {
    number: String,
    hash: String,
    parent_hash: String,
    timestamp: String,
    transactions: Vec<EthTransaction>,
}

/// ETH transaction structure from RPC
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct EthTransaction {
    hash: String,
    #[serde(default)]
    transaction_index: String,
    #[allow(dead_code)]
    from: Option<String>,
    to: Option<String>,
    value: String,
}

/// Syncing status from RPC
#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum SyncingStatus {
    NotSyncing(bool),
    #[allow(dead_code)]
    Syncing {
        starting_block: String,
        current_block: String,
        highest_block: String,
    },
}

impl EthScanner {
    /// Create a new ETH scanner with real RPC connection
    pub fn new(config: EthChainConfig) -> Result<Self, ScannerError> {
        info!(
            "Initializing ETH scanner for {} network at {}",
            config.network, config.rpc.url
        );

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| {
                ScannerError::RpcConnection(format!("Failed to create HTTP client: {}", e))
            })?;

        Ok(Self {
            config,
            client: Some(Arc::new(Mutex::new(client))),
            watched_addresses: HashSet::new(),
            mock_mode: false,
            mock_blocks: Vec::new(),
        })
    }

    /// Create a mock scanner for testing
    pub fn new_mock(config: EthChainConfig) -> Self {
        Self {
            config,
            client: None,
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

    /// Get config reference
    pub fn config(&self) -> &EthChainConfig {
        &self.config
    }

    /// Make a JSON-RPC call
    async fn rpc_call<T, R>(&self, method: &'static str, params: T) -> Result<R, ScannerError>
    where
        T: Serialize,
        R: for<'de> Deserialize<'de>,
    {
        let client = self.client.as_ref().ok_or_else(|| {
            ScannerError::RpcConnection("No HTTP client (mock mode?)".to_string())
        })?;

        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            method,
            params,
            id: 1,
        };

        let client_guard = client.lock().await;
        let response = client_guard
            .post(&self.config.rpc.url)
            .json(&request)
            .send()
            .await
            .map_err(|e| ScannerError::RpcConnection(format!("HTTP request failed: {}", e)))?;

        let rpc_response: JsonRpcResponse<R> = response
            .json()
            .await
            .map_err(|e| ScannerError::RpcConnection(format!("Failed to parse response: {}", e)))?;

        if let Some(error) = rpc_response.error {
            return Err(ScannerError::RpcConnection(format!(
                "RPC error {}: {}",
                error.code, error.message
            )));
        }

        rpc_response
            .result
            .ok_or_else(|| ScannerError::RpcConnection("No result in RPC response".to_string()))
    }

    /// Scan a block for ETH deposits to watched addresses
    async fn scan_block_for_deposits(&self, height: u64) -> Result<ScannedBlock, ScannerError> {
        let height_hex = format!("0x{:x}", height);

        // Get block with transactions
        let block: EthBlock = self
            .rpc_call("eth_getBlockByNumber", (height_hex, true))
            .await?;

        let mut deposits = Vec::new();

        // Scan each transaction for ETH transfers to watched addresses
        for tx in &block.transactions {
            if let Some(ref to_addr) = tx.to
                && self.is_watched(to_addr)
            {
                // Parse value from hex (wei)
                let value_wei =
                    u128::from_str_radix(tx.value.trim_start_matches("0x"), 16).unwrap_or(0);

                if value_wei > 0 {
                    let tx_index =
                        u32::from_str_radix(tx.transaction_index.trim_start_matches("0x"), 16)
                            .unwrap_or(0);

                    deposits.push(DetectedDeposit {
                        tx_hash: tx.hash.clone(),
                        tx_index,
                        vout_index: 0, // ETH doesn't have vout, use 0
                        to_address: to_addr.clone(),
                        asset: "ETH".to_string(),
                        amount: wei_to_eth(&value_wei.to_string()),
                        raw_amount: value_wei.to_string(),
                    });

                    info!(
                        "Detected ETH deposit: {} wei to {} in tx {}",
                        value_wei, to_addr, tx.hash
                    );
                }
            }
        }

        // Parse block metadata
        let block_number =
            u64::from_str_radix(block.number.trim_start_matches("0x"), 16).unwrap_or(height);
        let timestamp =
            i64::from_str_radix(block.timestamp.trim_start_matches("0x"), 16).unwrap_or(0);

        Ok(ScannedBlock {
            height: block_number,
            hash: block.hash,
            parent_hash: block.parent_hash,
            timestamp,
            deposits,
        })
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

        let result: String = self.rpc_call("eth_blockNumber", ()).await?;
        let height = u64::from_str_radix(result.trim_start_matches("0x"), 16)
            .map_err(|e| ScannerError::RpcConnection(format!("Invalid block number: {}", e)))?;

        Ok(height)
    }

    async fn scan_block(&self, height: u64) -> Result<ScannedBlock, ScannerError> {
        if self.mock_mode {
            return self
                .mock_blocks
                .get(height as usize)
                .cloned()
                .ok_or(ScannerError::BlockNotFound(height));
        }

        self.scan_block_for_deposits(height).await
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

        let height_hex = format!("0x{:x}", height);
        let block: EthBlock = self
            .rpc_call("eth_getBlockByNumber", (height_hex, false))
            .await?;

        Ok(block.hash.to_lowercase() == expected_hash.to_lowercase())
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

        // Check syncing status
        let syncing: SyncingStatus = self.rpc_call("eth_syncing", ()).await?;
        let is_synced = matches!(syncing, SyncingStatus::NotSyncing(false));

        // Get latest block
        let block_number: String = self.rpc_call("eth_blockNumber", ()).await?;
        let height = u64::from_str_radix(block_number.trim_start_matches("0x"), 16).unwrap_or(0);

        // Get block timestamp
        let height_hex = format!("0x{:x}", height);
        let block: EthBlock = self
            .rpc_call("eth_getBlockByNumber", (height_hex, false))
            .await?;
        let block_time =
            i64::from_str_radix(block.timestamp.trim_start_matches("0x"), 16).unwrap_or(0);

        // Get peer count (may not be available on all nodes)
        let peer_count: Result<String, _> = self.rpc_call("net_peerCount", ()).await;
        let peers = peer_count
            .ok()
            .and_then(|p| u32::from_str_radix(p.trim_start_matches("0x"), 16).ok())
            .unwrap_or(1);

        // Check if node is stale
        let now = chrono::Utc::now().timestamp();
        let lag_seconds = now - block_time;
        if lag_seconds > self.config.health.max_block_lag_seconds {
            warn!(
                "ETH node is stale: block time is {} seconds behind current time",
                lag_seconds
            );
        }

        Ok(NodeHealth {
            is_synced,
            block_height: height,
            block_time,
            peers,
        })
    }

    fn reload_addresses(&mut self, addresses: Vec<String>) {
        debug!("Reloading {} ETH addresses", addresses.len());
        self.watched_addresses = addresses.into_iter().map(|a| a.to_lowercase()).collect();
    }

    fn watched_count(&self) -> usize {
        self.watched_addresses.len()
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

    /// Test real RPC scanner creation (doesn't require running node)
    #[test]
    fn test_real_scanner_creation() {
        let result = EthScanner::new(test_config());
        assert!(result.is_ok());
        let scanner = result.unwrap();
        assert!(!scanner.mock_mode);
        assert!(scanner.client.is_some());
    }
}
