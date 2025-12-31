//! Multi-Threaded Pipeline Runner
//!
//! This module implements a multi-threaded version of the pipeline,
//! following the architecture defined in 0x08-a Trading Pipeline Design.
//!
//! # Thread Architecture (from 0x08-a)
//!
//! ```text
//! Thread 1: Ingestion      Thread 2: UBSCore           Thread 3: ME          Thread 4: Settlement
//! ┌────────────┐           ┌────────────────────┐      ┌────────────┐        ┌────────────────┐
//! │ Read       │ order_q   │ Pre-Trade:         │ valid│ Match      │ trade_q│ Persist:       │
//! │ Orders     │ ────────▶ │   Write WAL        │ ───▶ │ Generate   │ ──────▶│   Trades       │
//! │            │           │   Lock Balance     │      │ Trades     │        │   Orders       │
//! └────────────┘           └─────────┬──────────┘      └──────┬─────┘        │   Ledger       │
//!                                    │                        │              │   Balances     │
//!                                    │                        │              └───────▲────────┘
//!                                    │ balance_update_queue   │                      │
//!                                    │ ◀──────────────────────┘                      │
//!                                    │                                               │
//!                                    ▼                                               │
//!                          ┌────────────────────┐  balance_event_queue (TODO)        │
//!                          │ Post-Trade:        │ ───────────────────────────────────┘
//!                          │   spend_frozen     │
//!                          │   deposit          │
//!                          │   generate events  │
//!                          └────────────────────┘
//! ```
//!
//! Key design: ME sends Trade Events to BOTH Settlement AND UBSCore in parallel (fan-out).
//!
//! TODO: UBSCore should send ALL Balance Events to Settlement via balance_event_queue:
//!   - Pre-Trade: Lock events
//!   - Post-Trade: SpendFrozen + Credit events  
//!   - Cancel/Reject: Unlock events
//!   - External: Deposit/Withdraw events

use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

// High-frequency lifecycle logs are sent to hierarchical targets under "0XINFI"
// Example: "0XINFI::ME", "0XINFI::UBSC". This allows per-service toggling.
use crate::csv_io::InputOrder;
use crate::ledger::LedgerWriter;
use crate::orderbook::OrderBook;
use crate::ubscore::UBSCore;
use crate::user_account::UserAccount;
use rustc_hash::FxHashMap;

use crate::internal_transfer::channel::TransferReceiver;

// ============================================================
// LOGGING & PERFORMANCE CONSTANTS
// ============================================================

// ============================================================
// MULTI-THREAD PIPELINE RESULT
// ============================================================

/// Result of multi-threaded pipeline execution
pub struct MultiThreadPipelineResult {
    pub accepted: u64,
    pub rejected: u64,
    pub total_trades: u64,
    pub stats: Arc<PipelineStats>,
    /// Final accounts after all processing (for verification)
    pub final_accounts: FxHashMap<u64, UserAccount>,
}

use crate::pipeline::{
    MultiThreadQueues, PipelineConfig, PipelineServices, PipelineStats, ShutdownSignal,
};

/// Type alias for owned multi-threaded services
pub type MultiThreadServices = PipelineServices<UBSCore, OrderBook, LedgerWriter>;

/// Market context derived from symbol configuration
#[derive(Clone, Copy)]
pub struct MarketContext {
    pub qty_unit: u64,
    pub base_id: u32,
    pub quote_id: u32,
    /// Symbol ID for fee lookup
    pub symbol_id: u32,
}

// ============================================================
// MULTI-THREAD PIPELINE RUNNER
// ============================================================

/// Run orders through multi-threaded pipeline
///
/// Architecture from 0x08-a:
/// - Thread 1: Ingestion
/// - Thread 2: UBSCore (Pre-Trade: WAL + Lock) + (Post-Trade: Balance Update)
/// - Thread 3: ME (Match → Trade Events)
/// - Thread 4: Settlement (Persist Trade/Order/Ledger)
///
/// Fan-out: ME sends Trade Events to both Settlement and UBSCore in parallel.
#[allow(clippy::too_many_arguments)]
pub fn run_pipeline_multi_thread(
    orders: Vec<InputOrder>,
    services: MultiThreadServices,
    config: PipelineConfig,
    queues: Arc<MultiThreadQueues>,
    // TDengine persistence support (Gateway mode)
    _rt_handle: Option<tokio::runtime::Handle>,
    db_client: Option<Arc<crate::persistence::TDengineClient>>,
    // Optional: Internal transfer channel (Phase 0x0B-a)
    transfer_receiver: Option<TransferReceiver>,
    // Optional: Matching Service Persistence Config (Phase 0x0D)
    matching_persistence_config: Option<crate::config::MatchingPersistenceConfig>,
    // Optional: Settlement Service Persistence Config (Phase 0x0D)
    settlement_persistence_config: Option<crate::config::SettlementPersistenceConfig>,
) -> MultiThreadPipelineResult {
    tracing::info!(
        "[TRACE] Starting Multi-Thread Pipeline with {} orders...",
        orders.len()
    );
    let active_symbol_id = config.active_symbol_id;
    let sample_rate = config.sample_rate;

    let symbol_info = config
        .symbol_mgr
        .get_symbol_info_by_id(active_symbol_id)
        .expect("Active symbol not found");

    let market = MarketContext {
        qty_unit: *symbol_info.qty_unit(),
        base_id: symbol_info.base_asset_id,
        quote_id: symbol_info.quote_asset_id,
        symbol_id: active_symbol_id,
    };

    // Create shared state
    // queues passed in

    let stats = Arc::new(PipelineStats::new(sample_rate));
    let shutdown = Arc::new(ShutdownSignal::new());

    let _start_time = Instant::now();

    // --- PHASE 0x0D: Synchronized Recovery Sequence ---

    // 1. Initialize Ingestion
    let t1_ingestion = {
        let mut service = crate::pipeline_services::IngestionService::new(
            orders,
            queues.clone(),
            stats.clone(),
            active_symbol_id,
            _start_time,
        );
        let s = shutdown.clone();
        thread::spawn(move || service.run(&s))
    };

    // 2. Initialize ME Service (Blocking Recovery + UBSCore Replay)
    let mut me_service = if let Some(ref mp_config) = matching_persistence_config {
        if mp_config.enabled {
            tracing::info!(
                "[ME] Persistence enabled: dir={}, snapshot_interval={}",
                mp_config.data_dir,
                mp_config.snapshot_interval_trades
            );

            crate::pipeline_services::MatchingService::new_with_persistence(
                &mp_config.data_dir,
                queues.clone(),
                stats.clone(),
                market,
                100, // depth_update_interval_ms: 100ms
                mp_config.snapshot_interval_trades,
                Some(&services.ubscore), // Pass UBSCore for replay (ISSUE-002)
            )
            .expect("[ME] Failed to initialize persistence")
        } else {
            tracing::info!("[ME] Persistence disabled by config");
            crate::pipeline_services::MatchingService::new(
                services.book,
                queues.clone(),
                stats.clone(),
                market,
                100,
            )
        }
    } else {
        tracing::info!("[ME] No persistence config provided");
        crate::pipeline_services::MatchingService::new(
            services.book,
            queues.clone(),
            stats.clone(),
            market,
            100,
        )
    };

    // 3. Initialize Settlement Service (Blocking Recovery + Matching Replay)
    let settlement_service = if let Some(ref sp_config) = settlement_persistence_config {
        if sp_config.enabled {
            tracing::info!(
                "[Settlement] Persistence enabled: dir={}, checkpoint_interval={}, snapshot_interval={}",
                sp_config.data_dir,
                sp_config.checkpoint_interval,
                sp_config.snapshot_interval
            );

            crate::pipeline_services::SettlementService::new_with_persistence(
                services.ledger,
                queues.clone(),
                stats.clone(),
                db_client.clone(),
                active_symbol_id,
                Arc::new(config.symbol_mgr.clone()),
                &sp_config.data_dir,
                sp_config.checkpoint_interval,
                sp_config.snapshot_interval,
                Some(&me_service), // Pass MatchingService for replay (ISSUE-003c)
            )
            .expect("[Settlement] Failed to initialize persistence")
        } else {
            tracing::info!("[Settlement] Persistence disabled by config");
            crate::pipeline_services::SettlementService::new(
                services.ledger,
                queues.clone(),
                stats.clone(),
                db_client.clone(),
                active_symbol_id,
                Arc::new(config.symbol_mgr.clone()),
            )
        }
    } else {
        tracing::info!("[Settlement] No persistence config provided");
        crate::pipeline_services::SettlementService::new(
            services.ledger,
            queues.clone(),
            stats.clone(),
            db_client.clone(),
            active_symbol_id,
            Arc::new(config.symbol_mgr.clone()),
        )
    };

    // 4. Initialize UBSCore Service (takes ownership of services.ubscore)
    let mut ubscore_service = if let Some(transfer_rx) = transfer_receiver {
        crate::pipeline_services::UBSCoreService::with_transfer_channel(
            services.ubscore,
            queues.clone(),
            stats.clone(),
            _start_time,
            transfer_rx,
        )
    } else {
        crate::pipeline_services::UBSCoreService::new(
            services.ubscore,
            queues.clone(),
            stats.clone(),
            _start_time,
        )
    };

    // 5. Spawn threads for MT execution
    let s2 = shutdown.clone();
    let t2_ubscore = thread::spawn(move || {
        ubscore_service.run(&s2);
        ubscore_service.into_inner()
    });

    let s3 = shutdown.clone();
    let t3_me = thread::spawn(move || {
        me_service.run(&s3);
        me_service.into_inner()
    });

    let s4 = shutdown.clone();
    let t4_settlement = thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(settlement_service.run_async(s4));
    });
    // Wait for completion
    // ================================================================

    // Wait for ingestion to complete
    t1_ingestion.join().expect("Ingestion thread panicked");

    if config.continuous {
        tracing::info!("[TRACE] Pipeline executing in CONTINUOUS mode (ctrl+c to stop)");
        loop {
            // Keep main thread alive
            std::thread::sleep(Duration::from_secs(1));
        }
    } else {
        // Wait for all processing queues to drain before signaling shutdown
        // This ensures all orders are fully processed through the pipeline
        loop {
            if queues.all_empty() {
                break;
            }
            std::hint::spin_loop();
        }

        // Now signal shutdown (all threads should notice queues empty and exit)
        shutdown.request_shutdown();
    }

    // Wait for all threads
    let final_ubscore = t2_ubscore.join().expect("UBSCore thread panicked");
    let _final_book = t3_me.join().expect("ME thread panicked");
    t4_settlement.join().expect("Settlement thread panicked");

    MultiThreadPipelineResult {
        accepted: stats
            .orders_accepted
            .load(std::sync::atomic::Ordering::Relaxed),
        rejected: stats
            .orders_rejected
            .load(std::sync::atomic::Ordering::Relaxed),
        total_trades: stats
            .trades_generated
            .load(std::sync::atomic::Ordering::Relaxed),
        stats,
        final_accounts: final_ubscore.accounts().clone(),
    }
}

// ============================================================
// TESTS
// ============================================================

#[cfg(test)]
mod tests {
    #[test]
    fn test_multi_thread_pipeline_placeholder() {
        // TODO: Add multi-thread pipeline tests
    }
}
