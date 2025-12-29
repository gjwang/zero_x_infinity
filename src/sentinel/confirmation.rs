//! Confirmation Monitor
//!
//! Tracks confirmation counts for detected deposits and
//! transitions them through the state machine:
//! DETECTED -> CONFIRMING -> FINALIZED
//!
//! When a deposit reaches FINALIZED state, it triggers
//! the balance credit through the pipeline.

use super::error::{ScannerError, SentinelError};
use super::scanner::ChainScanner;
use sqlx::{PgPool, Row};
use tracing::{debug, info, warn};

/// Deposit status values
pub mod status {
    pub const DETECTED: &str = "DETECTED";
    pub const CONFIRMING: &str = "CONFIRMING";
    pub const FINALIZED: &str = "FINALIZED";
    pub const ORPHANED: &str = "ORPHANED";
    pub const SUCCESS: &str = "SUCCESS"; // After balance credited
}

/// Pending deposit record from database
#[derive(Debug, Clone)]
pub struct PendingDeposit {
    pub tx_hash: String,
    pub user_id: i64,
    pub asset: String,
    pub amount: i64,
    pub chain_slug: String,
    pub block_height: i64,
    pub block_hash: String,
    pub status: String,
    pub confirmations: i32,
}

/// Confirmation monitor that advances deposit states
pub struct ConfirmationMonitor {
    pool: PgPool,
}

impl ConfirmationMonitor {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Update confirmation counts for all pending deposits on a chain
    /// and transition states as needed
    pub async fn update_confirmations(
        &self,
        chain_id: &str,
        current_height: u64,
        required_confirmations: u32,
        scanner: &dyn ChainScanner,
    ) -> Result<Vec<PendingDeposit>, SentinelError> {
        // 1. Fetch all pending deposits (DETECTED or CONFIRMING)
        let pending = self.get_pending_deposits(chain_id).await?;

        if pending.is_empty() {
            return Ok(vec![]);
        }

        debug!("{}: Checking {} pending deposits", chain_id, pending.len());

        let mut finalized = Vec::new();

        for deposit in pending {
            // 2. Calculate current confirmations
            let current_confs = if current_height >= deposit.block_height as u64 {
                (current_height - deposit.block_height as u64 + 1) as i32
            } else {
                0 // Block height is ahead of current (shouldn't happen normally)
            };

            // 3. Verify block hash still matches (re-org check)
            let hash_valid = scanner
                .verify_block_hash(deposit.block_height as u64, &deposit.block_hash)
                .await
                .unwrap_or(false);

            if !hash_valid {
                // Re-org detected - mark as orphaned
                warn!(
                    "Deposit {} orphaned due to re-org at height {}",
                    deposit.tx_hash, deposit.block_height
                );
                self.update_status(&deposit.tx_hash, status::ORPHANED, current_confs)
                    .await?;
                continue;
            }

            // 4. Update confirmation count and status
            let new_status = if current_confs >= required_confirmations as i32 {
                status::FINALIZED
            } else if current_confs > 0 {
                status::CONFIRMING
            } else {
                status::DETECTED
            };

            // Only update if status changed or confirmations changed
            if deposit.status != new_status || deposit.confirmations != current_confs {
                self.update_status(&deposit.tx_hash, new_status, current_confs)
                    .await?;

                info!(
                    "Deposit {} updated: {} -> {} ({}/{} confirmations)",
                    deposit.tx_hash,
                    deposit.status,
                    new_status,
                    current_confs,
                    required_confirmations
                );
            }

            // 5. Collect finalized deposits for balance crediting
            if new_status == status::FINALIZED && deposit.status != status::FINALIZED {
                let mut finalized_deposit = deposit.clone();
                finalized_deposit.status = status::FINALIZED.to_string();
                finalized_deposit.confirmations = current_confs;
                finalized.push(finalized_deposit);
            }
        }

        Ok(finalized)
    }

    /// Get all pending deposits for a chain
    async fn get_pending_deposits(
        &self,
        chain_id: &str,
    ) -> Result<Vec<PendingDeposit>, SentinelError> {
        let chain_slug = chain_id.to_lowercase();
        let rows = sqlx::query(
            r#"SELECT tx_hash, user_id, asset, amount, chain_slug, block_height, block_hash, status, confirmations
               FROM deposit_history
               WHERE chain_slug = $1 AND status IN ('DETECTED', 'CONFIRMING')
               ORDER BY block_height ASC"#,
        )
        .bind(&chain_slug)
        .fetch_all(&self.pool)
        .await
        .map_err(ScannerError::from)?;

        let deposits = rows
            .into_iter()
            .map(|r| PendingDeposit {
                tx_hash: r.get("tx_hash"),
                user_id: r.get("user_id"),
                asset: r.get("asset"),
                amount: r.get("amount"),
                chain_slug: r.get("chain_slug"),
                block_height: r.get("block_height"),
                block_hash: r.get("block_hash"),
                status: r.get("status"),
                confirmations: r.get("confirmations"),
            })
            .collect();

        Ok(deposits)
    }

    /// Update deposit status and confirmation count
    async fn update_status(
        &self,
        tx_hash: &str,
        status: &str,
        confirmations: i32,
    ) -> Result<(), SentinelError> {
        sqlx::query(
            r#"UPDATE deposit_history
               SET status = $1, confirmations = $2
               WHERE tx_hash = $3"#,
        )
        .bind(status)
        .bind(confirmations)
        .bind(tx_hash)
        .execute(&self.pool)
        .await
        .map_err(ScannerError::from)?;

        Ok(())
    }

    /// Mark a deposit as successfully processed (balance credited)
    pub async fn mark_success(&self, tx_hash: &str) -> Result<(), SentinelError> {
        sqlx::query(
            r#"UPDATE deposit_history
               SET status = $1
               WHERE tx_hash = $2"#,
        )
        .bind(status::SUCCESS)
        .bind(tx_hash)
        .execute(&self.pool)
        .await
        .map_err(ScannerError::from)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_constants() {
        assert_eq!(status::DETECTED, "DETECTED");
        assert_eq!(status::CONFIRMING, "CONFIRMING");
        assert_eq!(status::FINALIZED, "FINALIZED");
        assert_eq!(status::ORPHANED, "ORPHANED");
        assert_eq!(status::SUCCESS, "SUCCESS");
    }

    #[test]
    fn test_pending_deposit_struct() {
        let deposit = PendingDeposit {
            tx_hash: "0xabc".to_string(),
            user_id: 1,
            asset: "BTC".to_string(),
            amount: 100_000_000,
            chain_slug: "btc".to_string(),
            block_height: 100,
            block_hash: "hash123".to_string(),
            status: status::DETECTED.to_string(),
            confirmations: 0,
        };

        assert_eq!(deposit.tx_hash, "0xabc");
        assert_eq!(deposit.chain_slug, "btc");
        assert_eq!(deposit.status, "DETECTED");
    }
}
