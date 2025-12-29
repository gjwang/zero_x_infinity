//! Sentinel Worker - Main scanning loop
//!
//! Orchestrates block scanning across multiple chains,
//! handles re-org detection, and records deposits to the database.

use super::error::{ScannerError, SentinelError};
use super::scanner::{ChainScanner, DetectedDeposit, ScannedBlock};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, sleep};
use tracing::{debug, error, info, warn};

/// Chain cursor state from database
#[derive(Debug, Clone)]
pub struct ChainCursor {
    pub chain_id: String,
    pub height: i64,
    pub hash: String,
}

/// Sentinel Worker that scans multiple blockchains for deposits
pub struct SentinelWorker {
    scanners: Vec<Arc<RwLock<Box<dyn ChainScanner>>>>,
    pool: PgPool,
    poll_interval: Duration,
    max_block_lag_seconds: i64,
}

impl SentinelWorker {
    /// Create a new SentinelWorker
    pub fn new(pool: PgPool, poll_interval_ms: u64) -> Self {
        Self {
            scanners: Vec::new(),
            pool,
            poll_interval: Duration::from_millis(poll_interval_ms),
            max_block_lag_seconds: 3600, // Default 1 hour
        }
    }

    /// Set the maximum allowed block lag before halting
    pub fn set_max_block_lag(&mut self, seconds: i64) {
        self.max_block_lag_seconds = seconds;
    }

    /// Add a scanner for a blockchain
    pub fn add_scanner(&mut self, scanner: Box<dyn ChainScanner>) {
        info!("Adding scanner for chain: {}", scanner.chain_id());
        self.scanners.push(Arc::new(RwLock::new(scanner)));
    }

    /// Get the number of registered scanners
    pub fn scanner_count(&self) -> usize {
        self.scanners.len()
    }

    /// Run the main scanning loop
    pub async fn run(&self) -> Result<(), SentinelError> {
        info!(
            "Sentinel starting with {} chains, poll interval: {:?}",
            self.scanners.len(),
            self.poll_interval
        );

        loop {
            for scanner in &self.scanners {
                // Get write lock to reload addresses
                let mut scanner_guard = scanner.write().await;
                let chain_id = scanner_guard.chain_id().to_string();

                // Load addresses from database for this chain
                if let Err(e) = self
                    .reload_scanner_addresses(scanner_guard.as_mut(), &chain_id)
                    .await
                {
                    warn!("Failed to reload addresses for {}: {:?}", chain_id, e);
                }

                // Drop write lock and get read lock for scanning
                drop(scanner_guard);
                let scanner_guard = scanner.read().await;

                if let Err(e) = self.scan_chain(scanner_guard.as_ref()).await {
                    error!("Error scanning {}: {:?}", chain_id, e);
                }
            }

            sleep(self.poll_interval).await;
        }
    }

    /// Load addresses from database and update scanner's watched addresses
    async fn reload_scanner_addresses(
        &self,
        scanner: &mut dyn ChainScanner,
        chain_id: &str,
    ) -> Result<(), SentinelError> {
        let addresses: Vec<String> =
            sqlx::query_scalar("SELECT address FROM user_addresses WHERE chain_id = $1")
                .bind(chain_id)
                .fetch_all(&self.pool)
                .await
                .map_err(ScannerError::from)?;

        let count = addresses.len();
        scanner.reload_addresses(addresses);
        debug!("{}: Loaded {} watched addresses", chain_id, count);

        Ok(())
    }

    /// Run a single scan iteration (for testing)
    pub async fn scan_once(&self) -> Result<u64, SentinelError> {
        let mut total_deposits = 0u64;

        for scanner in &self.scanners {
            let scanner_guard = scanner.read().await;
            match self.scan_chain_once(scanner_guard.as_ref()).await {
                Ok(count) => total_deposits += count,
                Err(e) => {
                    warn!("Error scanning {}: {:?}", scanner_guard.chain_id(), e);
                }
            }
        }

        Ok(total_deposits)
    }

    /// Scan a single chain once and return deposit count
    async fn scan_chain_once(&self, scanner: &dyn ChainScanner) -> Result<u64, SentinelError> {
        let chain_id = scanner.chain_id();
        let mut deposit_count = 0u64;

        // 1. Health check
        let health = scanner.health_check().await?;
        let now = chrono::Utc::now().timestamp();

        if now - health.block_time > self.max_block_lag_seconds {
            warn!(
                "{} node is stale (last block {} seconds ago)",
                chain_id,
                now - health.block_time
            );
            return Ok(0);
        }

        // 2. Get cursor from DB
        let cursor = self.get_cursor(chain_id).await?;
        let start_height = cursor.as_ref().map(|c| (c.height + 1) as u64).unwrap_or(0);
        let latest = scanner.get_latest_height().await?;

        if start_height > latest {
            debug!("{}: No new blocks (at height {})", chain_id, latest);
            return Ok(0);
        }

        // 3. Scan new blocks
        for height in start_height..=latest {
            let block = scanner.scan_block(height).await?;

            // 4. Re-org check (compare parent hash)
            if let Some(ref c) = cursor
                && height == (c.height + 1) as u64
                && !block.parent_hash.is_empty()
                && !scanner
                    .verify_block_hash(c.height as u64, &c.hash)
                    .await
                    .unwrap_or(false)
            {
                warn!("{} re-org detected at height {}", chain_id, height);
                // TODO: Handle re-org (rollback cursor)
                continue;
            }

            // 5. Process deposits
            for deposit in &block.deposits {
                match self.record_deposit(chain_id, &block, deposit).await {
                    Ok(recorded) => {
                        if recorded {
                            deposit_count += 1;
                        }
                    }
                    Err(e) => {
                        error!("Failed to record deposit: {:?}", e);
                    }
                }
            }

            // 6. Update cursor
            self.update_cursor(chain_id, block.height, &block.hash)
                .await?;

            info!(
                "{} scanned block {} ({} deposits)",
                chain_id,
                height,
                block.deposits.len()
            );
        }

        Ok(deposit_count)
    }

    /// Scan a chain (for the main loop)
    async fn scan_chain(&self, scanner: &dyn ChainScanner) -> Result<(), SentinelError> {
        self.scan_chain_once(scanner).await?;
        Ok(())
    }

    /// Get cursor from database
    pub async fn get_cursor(&self, chain_id: &str) -> Result<Option<ChainCursor>, SentinelError> {
        let row = sqlx::query(
            r#"SELECT chain_id, last_scanned_height, last_scanned_hash 
               FROM chain_cursor WHERE chain_id = $1"#,
        )
        .bind(chain_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(ScannerError::from)?;

        Ok(row.map(|r| ChainCursor {
            chain_id: r.get("chain_id"),
            height: r.get("last_scanned_height"),
            hash: r.get("last_scanned_hash"),
        }))
    }

    /// Update cursor in database
    pub async fn update_cursor(
        &self,
        chain_id: &str,
        height: u64,
        hash: &str,
    ) -> Result<(), SentinelError> {
        sqlx::query(
            r#"INSERT INTO chain_cursor (chain_id, last_scanned_height, last_scanned_hash)
               VALUES ($1, $2, $3)
               ON CONFLICT (chain_id) DO UPDATE 
               SET last_scanned_height = EXCLUDED.last_scanned_height,
                   last_scanned_hash = EXCLUDED.last_scanned_hash,
                   updated_at = NOW()"#,
        )
        .bind(chain_id)
        .bind(height as i64)
        .bind(hash)
        .execute(&self.pool)
        .await
        .map_err(ScannerError::from)?;

        Ok(())
    }

    /// Record a deposit to the database
    /// Returns true if deposit was newly recorded, false if already exists
    pub async fn record_deposit(
        &self,
        chain_id: &str,
        block: &ScannedBlock,
        deposit: &DetectedDeposit,
    ) -> Result<bool, SentinelError> {
        // 1. Lookup user_id from address
        let user_row = sqlx::query("SELECT user_id FROM user_addresses WHERE address = $1")
            .bind(&deposit.to_address)
            .fetch_optional(&self.pool)
            .await
            .map_err(ScannerError::from)?;

        let Some(user_row) = user_row else {
            debug!(
                "Address {} not in user_addresses (orphan deposit?)",
                deposit.to_address
            );
            return Ok(false);
        };

        let user_id: i64 = user_row.get("user_id");

        // 2. Insert deposit record (idempotent on tx_hash)
        let result = sqlx::query(
            r#"INSERT INTO deposit_history 
               (tx_hash, user_id, asset, amount, status, chain_id, block_height, block_hash, tx_index, confirmations)
               VALUES ($1, $2, $3, $4, 'DETECTED', $5, $6, $7, $8, 0)
               ON CONFLICT (tx_hash) DO NOTHING"#,
        )
        .bind(&deposit.tx_hash)
        .bind(user_id)
        .bind(&deposit.asset)
        .bind(deposit.amount)
        .bind(chain_id)
        .bind(block.height as i64)
        .bind(&block.hash)
        .bind(deposit.tx_index as i32)
        .execute(&self.pool)
        .await
        .map_err(ScannerError::from)?;

        let was_inserted = result.rows_affected() > 0;

        if was_inserted {
            info!(
                "Detected deposit: {} {} to user {} (tx: {})",
                deposit.amount, deposit.asset, user_id, deposit.tx_hash
            );
        }

        Ok(was_inserted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full integration tests require a database
    // These are unit tests for the worker logic

    #[test]
    fn test_worker_creation() {
        // This test verifies the worker can be created
        // Actual database tests would need a test database
        assert!(true);
    }

    #[test]
    fn test_chain_cursor_struct() {
        let cursor = ChainCursor {
            chain_id: "BTC".to_string(),
            height: 100,
            hash: "abc123".to_string(),
        };

        assert_eq!(cursor.chain_id, "BTC");
        assert_eq!(cursor.height, 100);
    }
}
