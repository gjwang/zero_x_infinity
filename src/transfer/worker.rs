//! Recovery Worker
//!
//! Background worker that scans for and resumes stuck transfers.

use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

use super::coordinator::TransferCoordinator;
use super::state::TransferState;

/// Configuration for the recovery worker
#[derive(Debug, Clone)]
pub struct WorkerConfig {
    /// How often to scan for stale transfers
    pub scan_interval: Duration,
    /// How long a transfer must be stuck to be considered stale
    pub stale_threshold: Duration,
    /// Maximum transfers to process per scan
    pub batch_size: usize,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            scan_interval: Duration::from_secs(30),
            stale_threshold: Duration::from_secs(60), // 1 minute as per design doc
            batch_size: 100,
        }
    }
}

/// Recovery Worker
///
/// Periodically scans for transfers stuck in non-terminal states
/// and resumes their processing.
///
/// # Design Doc Reference
/// From §6.2: "Query: SELECT * FROM transfers_tb WHERE state IN (0, 10, 20, 30, -20)
/// AND updated_at < NOW() - INTERVAL '1 minute';"
pub struct RecoveryWorker {
    coordinator: Arc<TransferCoordinator>,
    config: WorkerConfig,
}

impl RecoveryWorker {
    /// Create a new RecoveryWorker
    pub fn new(coordinator: Arc<TransferCoordinator>, config: WorkerConfig) -> Self {
        Self {
            coordinator,
            config,
        }
    }

    /// Create with default configuration
    pub fn with_defaults(coordinator: Arc<TransferCoordinator>) -> Self {
        Self::new(coordinator, WorkerConfig::default())
    }

    /// Run the recovery worker loop
    ///
    /// This method runs forever, periodically scanning for and resuming stale transfers.
    pub async fn run(&self) -> ! {
        info!(
            scan_interval_secs = self.config.scan_interval.as_secs(),
            stale_threshold_secs = self.config.stale_threshold.as_secs(),
            "Starting recovery worker"
        );

        loop {
            if let Err(e) = self.scan_and_recover().await {
                error!(error = %e, "Recovery scan failed");
            }

            tokio::time::sleep(self.config.scan_interval).await;
        }
    }

    /// Run a single scan and recovery cycle
    pub async fn scan_and_recover(&self) -> Result<usize, super::error::TransferError> {
        let stale_transfers = self
            .coordinator
            .db()
            .find_stale(self.config.stale_threshold)
            .await?;

        if stale_transfers.is_empty() {
            debug!("No stale transfers found");
            return Ok(0);
        }

        info!(
            count = stale_transfers.len(),
            "Found stale transfers to recover"
        );

        let mut recovered = 0;

        for transfer in stale_transfers.iter().take(self.config.batch_size) {
            debug!(
                req_id = transfer.req_id,
                state = %transfer.state,
                retry_count = transfer.retry_count,
                "Recovering transfer"
            );

            // Check for critical stuck states that need alerting
            if transfer.state == TransferState::TargetPending && transfer.retry_count > 10 {
                warn!(
                    req_id = transfer.req_id,
                    retry_count = transfer.retry_count,
                    "CRITICAL: Transfer stuck in TARGET_PENDING with many retries!"
                );
                // TODO: Send alert to ops
            }

            // Attempt to step the transfer forward
            match self.coordinator.step(transfer.req_id).await {
                Ok(new_state) => {
                    if new_state != transfer.state {
                        info!(
                            req_id = transfer.req_id,
                            old_state = %transfer.state,
                            new_state = %new_state,
                            "Transfer state advanced"
                        );
                        recovered += 1;
                    }
                }
                Err(e) => {
                    error!(
                        req_id = transfer.req_id,
                        error = %e,
                        "Failed to recover transfer"
                    );
                }
            }
        }

        if recovered > 0 {
            info!(count = recovered, "Recovered transfers this scan");
        }

        Ok(recovered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_config_default() {
        let config = WorkerConfig::default();
        assert_eq!(config.scan_interval, Duration::from_secs(30));
        assert_eq!(config.stale_threshold, Duration::from_secs(60));
        assert_eq!(config.batch_size, 100);
    }
}
