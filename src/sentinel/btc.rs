//! BTC Scanner Implementation
//!
//! Scans Bitcoin blockchain for deposits to monitored addresses.
//! Supports both real bitcoind RPC and mock mode for testing.

use super::config::BtcChainConfig;
use super::error::ScannerError;
use super::scanner::{ChainScanner, DetectedDeposit, NodeHealth, ScannedBlock};
use async_trait::async_trait;
use bitcoincore_rpc::{Auth, Client, RpcApi};
use rust_decimal::Decimal;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

/// BTC Scanner that connects to bitcoind via JSON-RPC
pub struct BtcScanner {
    config: BtcChainConfig,
    /// RPC client (None in mock mode)
    rpc_client: Option<Arc<Mutex<Client>>>,
    /// Set of monitored addresses (loaded from DB)
    watched_addresses: HashSet<String>,
    /// Mock mode for testing without real node
    mock_mode: bool,
    /// Mock blocks for testing
    mock_blocks: Vec<ScannedBlock>,
}

impl BtcScanner {
    /// Create a new BTC scanner with real RPC connection
    pub fn new(config: BtcChainConfig) -> Result<Self, ScannerError> {
        info!(
            "Initializing BTC scanner for {} network at {}",
            config.network, config.rpc.url
        );

        // Create RPC client
        let auth = Auth::UserPass(config.rpc.user.clone(), config.rpc.password.clone());
        let client = Client::new(&config.rpc.url, auth).map_err(|e| {
            ScannerError::RpcConnection(format!("Failed to create BTC RPC client: {}", e))
        })?;

        // Test connection
        match client.get_blockchain_info() {
            Ok(info) => {
                info!(
                    "Connected to Bitcoin node: chain={}, blocks={}, headers={}",
                    info.chain, info.blocks, info.headers
                );
            }
            Err(e) => {
                warn!("BTC RPC connection test failed (will retry): {}", e);
            }
        }

        Ok(Self {
            config,
            rpc_client: Some(Arc::new(Mutex::new(client))),
            watched_addresses: HashSet::new(),
            mock_mode: false,
            mock_blocks: Vec::new(),
        })
    }

    /// Create a mock scanner for testing
    pub fn new_mock(config: BtcChainConfig) -> Self {
        Self {
            config,
            rpc_client: None,
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

    /// Get the config (for required_confirmations etc)
    pub fn config(&self) -> &BtcChainConfig {
        &self.config
    }

    /// Scan a block for deposits to watched addresses (real RPC)
    fn scan_block_for_deposits(
        &self,
        client: &Client,
        height: u64,
    ) -> Result<ScannedBlock, ScannerError> {
        // Get block hash at height
        let block_hash = client.get_block_hash(height).map_err(|e| {
            ScannerError::RpcConnection(format!("Failed to get block hash at {}: {}", height, e))
        })?;

        // Get full block with transactions
        let block = client.get_block(&block_hash).map_err(|e| {
            ScannerError::RpcConnection(format!("Failed to get block {}: {}", height, e))
        })?;

        let parent_hash = block.header.prev_blockhash.to_string();
        let timestamp = block.header.time as i64;
        let hash = block_hash.to_string();

        let mut deposits = Vec::new();

        // Scan each transaction
        for (tx_index, tx) in block.txdata.iter().enumerate() {
            let tx_hash = tx.compute_txid().to_string();

            // Scan each output
            for (vout_index, output) in tx.output.iter().enumerate() {
                // Try to extract address from script_pubkey
                if let Some(address) = self.extract_address(&output.script_pubkey) {
                    // Check if this address is being watched
                    if self.is_watched(&address) {
                        // Use Decimal arithmetic instead of f64 (money-type-safety.md)
                        let sat_amount = Decimal::from(output.value.to_sat());
                        let btc_divisor = Decimal::from(100_000_000u64); // 10^8 for BTC
                        let amount_btc = sat_amount / btc_divisor;

                        deposits.push(DetectedDeposit {
                            tx_hash: tx_hash.clone(),
                            tx_index: tx_index as u32,
                            vout_index: vout_index as u32,
                            to_address: address,
                            asset: "BTC".to_string(),
                            amount: amount_btc,
                            raw_amount: output.value.to_sat().to_string(),
                        });
                    }
                }
            }
        }

        Ok(ScannedBlock {
            height,
            hash,
            parent_hash,
            timestamp,
            deposits,
        })
    }

    /// Extract address from script_pubkey (simplified - handles common types)
    fn extract_address(&self, script: &bitcoincore_rpc::bitcoin::ScriptBuf) -> Option<String> {
        use bitcoincore_rpc::bitcoin::Network;

        // Determine network from config
        let network = match self.config.network.as_str() {
            "mainnet" | "main" => Network::Bitcoin,
            "testnet" | "test" => Network::Testnet,
            "regtest" => Network::Regtest,
            "signet" => Network::Signet,
            _ => Network::Regtest,
        };

        // Try to extract address
        bitcoincore_rpc::bitcoin::Address::from_script(script, network)
            .ok()
            .map(|addr| addr.to_string())
    }
}

#[async_trait]
impl ChainScanner for BtcScanner {
    fn chain_id(&self) -> &str {
        "BTC"
    }

    fn required_confirmations(&self) -> u32 {
        self.config.scanning.required_confirmations
    }

    async fn get_latest_height(&self) -> Result<u64, ScannerError> {
        if self.mock_mode {
            return Ok(self.mock_blocks.len() as u64);
        }

        let client = self
            .rpc_client
            .as_ref()
            .ok_or_else(|| ScannerError::RpcConnection("No RPC client".to_string()))?;

        let client_guard = client.lock().await;
        let count = client_guard.get_block_count().map_err(|e| {
            ScannerError::RpcConnection(format!("Failed to get block count: {}", e))
        })?;

        Ok(count)
    }

    async fn scan_block(&self, height: u64) -> Result<ScannedBlock, ScannerError> {
        if self.mock_mode {
            return self
                .mock_blocks
                .get(height as usize)
                .cloned()
                .ok_or(ScannerError::BlockNotFound(height));
        }

        let client = self
            .rpc_client
            .as_ref()
            .ok_or_else(|| ScannerError::RpcConnection("No RPC client".to_string()))?;

        let client_guard = client.lock().await;
        self.scan_block_for_deposits(&client_guard, height)
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

        let client = self
            .rpc_client
            .as_ref()
            .ok_or_else(|| ScannerError::RpcConnection("No RPC client".to_string()))?;

        let client_guard = client.lock().await;
        let block_hash = client_guard
            .get_block_hash(height)
            .map_err(|e| ScannerError::RpcConnection(format!("Failed to get block hash: {}", e)))?;

        Ok(block_hash.to_string() == expected_hash)
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

        let client = self
            .rpc_client
            .as_ref()
            .ok_or_else(|| ScannerError::RpcConnection("No RPC client".to_string()))?;

        let client_guard = client.lock().await;

        let info = client_guard.get_blockchain_info().map_err(|e| {
            ScannerError::RpcConnection(format!("Failed to get blockchain info: {}", e))
        })?;

        let network_info = client_guard.get_network_info().map_err(|e| {
            ScannerError::RpcConnection(format!("Failed to get network info: {}", e))
        })?;

        // Get best block time
        let best_block_hash = client_guard.get_best_block_hash().map_err(|e| {
            ScannerError::RpcConnection(format!("Failed to get best block hash: {}", e))
        })?;

        let best_block = client_guard
            .get_block_header(&best_block_hash)
            .map_err(|e| {
                ScannerError::RpcConnection(format!("Failed to get block header: {}", e))
            })?;

        let is_synced = info.blocks == info.headers && !info.initial_block_download;

        Ok(NodeHealth {
            is_synced,
            block_height: info.blocks,
            block_time: best_block.time as i64,
            peers: network_info.connections as u32,
        })
    }

    fn reload_addresses(&mut self, addresses: Vec<String>) {
        debug!("Reloading {} BTC addresses", addresses.len());
        self.watched_addresses = addresses.into_iter().collect();
    }

    fn watched_count(&self) -> usize {
        self.watched_addresses.len()
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

    /// DEF-002: Test P2WPKH (SegWit) address extraction
    /// This test reproduces the bug where Sentinel fails to detect SegWit deposits.
    #[test]
    fn test_segwit_p2wpkh_extraction_def_002() {
        use bitcoincore_rpc::bitcoin::script::Builder;

        let scanner = BtcScanner::new_mock(test_config());

        // Construct a P2WPKH script: OP_0 <20-byte-pubkey-hash>
        // This is what a SegWit address (bcrt1...) looks like in script form
        // Format: 0x00 0x14 <20 bytes>
        let pubkey_hash: [u8; 20] = [
            0xe8, 0xdf, 0x01, 0x8c, 0x7e, 0x32, 0x6c, 0xc2, 0x97, 0x41, 0x25, 0x89, 0xa7, 0x26,
            0x87, 0xc1, 0xf9, 0x22, 0xc9, 0x53,
        ];

        // Build P2WPKH script manually: OP_0 PUSH20 <hash>
        let script = Builder::new()
            .push_int(0) // OP_0 (witness version 0)
            .push_slice(pubkey_hash)
            .into_script();

        // Verify it's a valid P2WPKH script
        assert!(script.is_p2wpkh(), "Script should be identified as P2WPKH");

        // DEF-002: This is where the bug manifests - extract_address returns None
        let extracted = scanner.extract_address(&script);

        // Expected: bcrt1qarls3s8lxjmxzja2y5c6ung0s0ujjef48egz5t (for regtest)
        assert!(
            extracted.is_some(),
            "DEF-002: extract_address MUST return Some for P2WPKH scripts"
        );

        let address = extracted.unwrap();
        // SegWit regtest addresses start with bcrt1
        assert!(
            address.starts_with("bcrt1"),
            "Regtest P2WPKH address should start with bcrt1, got: {}",
            address
        );
    }
}
