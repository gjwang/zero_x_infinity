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
macro_rules! p_info {
    ($target:expr, $($arg:tt)+) => {
        tracing::info!(target: $target, $($arg)+);
    }
}

macro_rules! p_span {
    ($target:expr, $name:expr, $($arg:tt)*) => {
        tracing::info_span!(target: $target, $name, $($arg)*)
    }
}

use crate::csv_io::{ACTION_CANCEL, ACTION_PLACE, InputOrder};
use crate::engine::MatchingEngine;
use crate::ledger::{LedgerEntry, LedgerWriter, OP_CREDIT, OP_DEBIT};
use crate::messages::TradeEvent;
use crate::models::{InternalOrder, OrderStatus, OrderType, Side};
use crate::orderbook::OrderBook;
use crate::ubscore::UBSCore;
use crate::user_account::UserAccount;
use rustc_hash::FxHashMap;

// ============================================================
// LOGGING & PERFORMANCE CONSTANTS
// ============================================================

const TARGET_ME: &str = "0XINFI::ME";
const TARGET_PERS: &str = "0XINFI::PERS";

const LOG_ORDER: &str = "ORDER";
const LOG_CANCEL: &str = "CANCEL";
const LOG_TRADE: &str = "TRADE";

const IDLE_SPIN_LIMIT: u32 = 1000;
const IDLE_SLEEP_US: Duration = Duration::from_micros(100);

const UBSC_SETTLE_BATCH: usize = 128; // Max settlements per round
const UBSC_ORDER_BATCH: usize = 16; // Max new orders per round

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
    BalanceUpdateRequest, MultiThreadQueues, OrderAction, PipelineConfig, PipelineServices,
    PipelineStats, PriceImprovement, SequencedOrder, ShutdownSignal, ValidAction,
};

/// Type alias for owned multi-threaded services
pub type MultiThreadServices = PipelineServices<UBSCore, OrderBook, LedgerWriter>;

/// Market context derived from symbol configuration
#[derive(Clone, Copy)]
pub struct MarketContext {
    pub qty_unit: u64,
    pub base_id: u32,
    pub quote_id: u32,
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
pub fn run_pipeline_multi_thread(
    orders: Vec<InputOrder>,
    services: MultiThreadServices,
    config: PipelineConfig,
    queues: Arc<MultiThreadQueues>,
    // TDengine persistence support (Gateway mode)
    rt_handle: Option<tokio::runtime::Handle>,
    db_client: Option<Arc<crate::persistence::TDengineClient>>,
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
        qty_unit: symbol_info.qty_unit(),
        base_id: symbol_info.base_asset_id,
        quote_id: symbol_info.quote_asset_id,
    };

    // Create shared state
    // queues passed in

    let stats = Arc::new(PipelineStats::new(sample_rate));
    let shutdown = Arc::new(ShutdownSignal::new());

    let _start_time = Instant::now();

    // The execution setup is now much clearer - spawn each processing stage
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

    let t2_ubscore = {
        let mut service = crate::pipeline_services::UBSCoreService::new(
            services.ubscore,
            queues.clone(),
            stats.clone(),
            _start_time,
        );
        let s = shutdown.clone();
        thread::spawn(move || {
            service.run(&s);
            service.into_inner()
        })
    };

    let t3_me = {
        let mut service = crate::pipeline_services::MatchingService::new(
            services.book,
            queues.clone(),
            stats.clone(),
            market,
            100, // depth_update_interval_ms: 100ms
        );
        let s = shutdown.clone();
        thread::spawn(move || {
            service.run(&s);
            service.into_inner()
        })
    };

    let t4_settlement = {
        let service = crate::pipeline_services::SettlementService::new(
            services.ledger,
            queues.clone(),
            stats.clone(),
            rt_handle.clone(),
            db_client.clone(),
            active_symbol_id,
        );
        let s = shutdown.clone();

        // Always use async mode with dedicated runtime
        thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            rt.block_on(service.run_async(s));
        })
    };
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
    let _final_ledger = t4_settlement.join().expect("Settlement thread panicked");

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
    fn test_placeholder() {
        // Placeholder for future tests
        assert!(true);
    }
}
