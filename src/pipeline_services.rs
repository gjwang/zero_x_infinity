//! Pipeline Services Module
//!
//! This module contains service structs that encapsulate pipeline stage logic.
//! Each service provides a `run` method for multi-threaded execution.
//!
//! Migration is done incrementally:
//! - Phase 1: IngestionService
//! - Phase 2: UBSCoreService  
//! - Phase 3: MatchingService
//! - Phase 4: SettlementService

use std::sync::Arc;
use std::time::Instant;
// unused import removed
use crate::ledger::LedgerWriter;

use crate::csv_io::{ACTION_CANCEL, ACTION_PLACE, InputOrder};
use crate::models::{InternalOrder, OrderStatus};
use crate::pipeline::{
    MultiThreadQueues, OrderAction, PipelineStats, SequencedOrder, ShutdownSignal,
};

// ============================================================
// INGESTION SERVICE
// ============================================================

/// Service that handles order ingestion into the pipeline.
///
/// Converts `InputOrder` to `OrderAction` and pushes to the order queue
/// with backpressure handling.
pub struct IngestionService {
    orders: Vec<InputOrder>,
    queues: Arc<MultiThreadQueues>,
    stats: Arc<PipelineStats>,
    active_symbol_id: u32,
    start_time: Instant,
    seq_counter: u64,
}

impl IngestionService {
    pub fn new(
        orders: Vec<InputOrder>,
        queues: Arc<MultiThreadQueues>,
        stats: Arc<PipelineStats>,
        active_symbol_id: u32,
        start_time: Instant,
    ) -> Self {
        Self {
            orders,
            queues,
            stats,
            active_symbol_id,
            start_time,
            seq_counter: 0,
        }
    }

    /// Run the ingestion service (MT blocking mode with backpressure)
    pub fn run(&mut self, shutdown: &ShutdownSignal) {
        for input in self.orders.drain(..) {
            // [TRACE]
            if input.action == crate::csv_io::ACTION_PLACE {
                tracing::info!(
                    "[TRACE] Order {}: Ingestion Service Picked Up",
                    input.order_id
                );
            }

            if shutdown.is_shutdown_requested() {
                break;
            }

            let ingested_at_ns = Instant::now().duration_since(self.start_time).as_nanos() as u64;

            // Create OrderAction based on input action type
            let action = if input.action == ACTION_CANCEL {
                // Cancel order - no seq needed, just pass order_id
                self.stats.incr_cancel();
                self.stats.record_cancel();
                OrderAction::Cancel {
                    order_id: input.order_id,
                    user_id: input.user_id,
                    ingested_at_ns,
                }
            } else {
                // Place order - assign sequence number
                self.stats.incr_place();
                self.stats.record_place();
                let _ = ACTION_PLACE; // Suppress unused warning if it happens
                self.seq_counter += 1;
                let mut order = InternalOrder::new_with_time(
                    input.order_id,
                    input.user_id,
                    self.active_symbol_id,
                    input.price,
                    input.qty,
                    input.side,
                    ingested_at_ns,
                );
                order.ingested_at_ns = ingested_at_ns;
                OrderAction::Place(SequencedOrder::new(self.seq_counter, order, ingested_at_ns))
            };

            // Push with backpressure (same path for both place and cancel)
            loop {
                match self.queues.order_queue.push(action.clone()) {
                    Ok(()) => break,
                    Err(_) => {
                        self.stats.incr_backpressure("mt_order_queue");
                        // std::hint::spin_loop();
                        std::thread::sleep(IDLE_SLEEP_US);
                    }
                }
            }

            self.stats.incr_ingested();
        }
    }
}

// ============================================================
// UBSCORE SERVICE
// ============================================================

use std::time::Duration;

use crate::internal_transfer::channel::{
    TransferReceiver, TransferSender, process_transfer_requests,
};
use crate::internal_transfer::types::InternalTransferId;
use crate::messages::BalanceEvent;
use crate::pipeline::ValidAction;
use crate::ubscore::UBSCore;

const IDLE_SPIN_LIMIT: u32 = 1000;
const IDLE_SLEEP_US: Duration = Duration::from_micros(100);
const UBSC_SETTLE_BATCH: usize = 128;
const UBSC_ORDER_BATCH: usize = 16;
const UBSC_TRANSFER_BATCH: usize = 16;

/// Service that handles UBSCore processing (pre-trade + post-trade balance updates).
///
/// Also handles internal transfer requests from TradingAdapter.
/// Processes orders through UBSCore, commits to WAL, and publishes results downstream.
pub struct UBSCoreService {
    ubscore: UBSCore,
    queues: Arc<MultiThreadQueues>,
    stats: Arc<PipelineStats>,
    start_time: Instant,
    batch_actions: Vec<ValidAction>,
    batch_events: Vec<BalanceEvent>,
    /// Optional: Internal transfer receiver (Phase 0x0B-a)
    transfer_receiver: Option<TransferReceiver>,
    /// Processed transfer request IDs for idempotency
    processed_transfers: std::collections::HashSet<InternalTransferId>,
}

impl UBSCoreService {
    pub fn new(
        ubscore: UBSCore,
        queues: Arc<MultiThreadQueues>,
        stats: Arc<PipelineStats>,
        start_time: Instant,
    ) -> Self {
        Self {
            ubscore,
            queues,
            stats,
            start_time,
            batch_actions: Vec::with_capacity(UBSC_ORDER_BATCH),
            batch_events: Vec::with_capacity(UBSC_ORDER_BATCH + UBSC_SETTLE_BATCH * 4),
            transfer_receiver: None,
            processed_transfers: std::collections::HashSet::new(),
        }
    }

    /// Create service with transfer channel for internal transfer support (Phase 0x0B-a)
    pub fn with_transfer_channel(
        ubscore: UBSCore,
        queues: Arc<MultiThreadQueues>,
        stats: Arc<PipelineStats>,
        start_time: Instant,
        transfer_receiver: TransferReceiver,
    ) -> Self {
        Self {
            ubscore,
            queues,
            stats,
            start_time,
            batch_actions: Vec::with_capacity(UBSC_ORDER_BATCH),
            batch_events: Vec::with_capacity(UBSC_ORDER_BATCH + UBSC_SETTLE_BATCH * 4),
            transfer_receiver: Some(transfer_receiver),
            processed_transfers: std::collections::HashSet::new(),
        }
    }

    /// Get the TransferSender for this service (if transfer channel was configured)
    /// This is used by external code to get the sender side of the channel
    pub fn get_transfer_sender(&self) -> Option<TransferSender> {
        // Note: We can't return the sender from here since we only have the receiver
        // The sender should be created at the same time as the service
        None
    }

    /// Run the UBSCore service (MT blocking mode)
    pub fn run(&mut self, shutdown: &ShutdownSignal) {
        tracing::info!("[TRACE] UBSCoreService thread started");
        let mut spin_count = 0u32;

        loop {
            let mut did_work = false;

            // PHASE 1: COLLECT (Batch accumulation of intentions)

            // 1.1 Process Settlements
            for _ in 0..UBSC_SETTLE_BATCH {
                if let Some(req) = self.queues.balance_update_queue.pop() {
                    did_work = true;
                    if let Ok(events) = self.ubscore.apply_balance_update(req) {
                        self.batch_events.extend(events);
                    }
                } else {
                    break;
                }
            }

            // 1.2 Process Orders
            for _ in 0..UBSC_ORDER_BATCH {
                if let Some(action) = self.queues.order_queue.pop() {
                    tracing::info!("[TRACE] UBSC: Popped action from order_queue");
                    did_work = true;
                    match self.ubscore.apply_order_action(action) {
                        Ok((actions, events)) => {
                            // [TRACE]
                            if let crate::pipeline::ValidAction::Order(ref o) = actions[0] {
                                tracing::info!(
                                    "[TRACE] Order {}: UBSCore Processed (Locked & Validated)",
                                    o.order.order_id
                                );
                            }
                            self.batch_actions.extend(actions);
                            self.batch_events.extend(events);
                            self.stats.incr_accepted();
                        }
                        Err(e) => {
                            tracing::error!("[TRACE] UBSCore apply_order_action failed: {:?}", e);
                            self.stats.incr_rejected();
                        }
                    }
                } else {
                    break;
                }
            }

            // 1.3 Process Internal Transfers (Phase 0x0B-a)
            if let Some(ref mut receiver) = self.transfer_receiver {
                let transfer_count = process_transfer_requests(
                    &mut self.ubscore,
                    receiver,
                    &mut self.processed_transfers,
                    UBSC_TRANSFER_BATCH,
                );
                if transfer_count > 0 {
                    did_work = true;
                    tracing::debug!(
                        "[TRANSFER] Processed {} internal transfer requests",
                        transfer_count
                    );
                }
            }

            // PHASE 2 & 3: COMMIT & RELEASE (The actual atomic/visibility phase)
            if !self.batch_actions.is_empty() || !self.batch_events.is_empty() {
                // Batch Timing: Focus on the IO bottleneck (WAL Flush)
                let commit_start = Instant::now();
                if let Err(e) = self.ubscore.commit() {
                    tracing::error!("CRITICAL: WAL commit failed: {}", e);
                }
                self.stats
                    .add_settlement_time(commit_start.elapsed().as_nanos() as u64);

                // Publish results to downstream
                for action in self.batch_actions.drain(..) {
                    // [1] Push to Matching Engine
                    while self.queues.action_queue.push(action.clone()).is_err() {
                        std::hint::spin_loop();
                        if shutdown.is_shutdown_requested() {
                            break;
                        }
                    }

                    // [2] Push OrderUpdate(NEW) for new orders (Settlement-First principle: Order is now durable in WAL)
                    if let crate::pipeline::ValidAction::Order(ref seq_order) = action {
                        let _ = self.queues.push_event_queue.push(
                            crate::websocket::PushEvent::OrderUpdate {
                                user_id: seq_order.order.user_id,
                                order_id: seq_order.order.order_id,
                                symbol_id: seq_order.order.symbol_id,
                                status: OrderStatus::NEW,
                                filled_qty: 0,
                                avg_price: None,
                            },
                        );
                        tracing::info!(
                            "[TRACE] Order {}: UBSCore -> Push Event Queue (NEW)",
                            seq_order.order.order_id
                        );
                    }
                }

                for event in self.batch_events.drain(..) {
                    let lat_ingested = event.ingested_at_ns;
                    while self.queues.balance_event_queue.push(event.clone()).is_err() {
                        self.stats.incr_backpressure("balance_event_queue");
                        std::thread::sleep(IDLE_SLEEP_US);
                        // std::hint::spin_loop();
                        if shutdown.is_shutdown_requested() {
                            break;
                        }
                    }
                    if lat_ingested > 0 {
                        let now_ns =
                            Instant::now().duration_since(self.start_time).as_nanos() as u64;
                        self.stats
                            .record_latency(now_ns.saturating_sub(lat_ingested));
                    }
                    self.stats.incr_settled();
                }
            }

            if shutdown.is_shutdown_requested()
                && self.queues.order_queue.is_empty()
                && self.queues.balance_update_queue.is_empty()
            {
                break;
            }

            if !did_work {
                spin_count += 1;
                if spin_count > IDLE_SPIN_LIMIT {
                    std::thread::sleep(IDLE_SLEEP_US);
                    spin_count = 0;
                } else {
                    std::hint::spin_loop();
                }
            } else {
                spin_count = 0;
            }
        }
        let _ = self.ubscore.commit();
    }

    /// Consume the service and return the inner UBSCore
    pub fn into_inner(self) -> UBSCore {
        self.ubscore
    }
}

// ============================================================
// MATCHING SERVICE
// ============================================================

use crate::engine::MatchingEngine;
use crate::matching_wal::{
    MatchingRecovery, MatchingSnapshotter, MatchingWalWriter, RecoveryState,
};
use crate::messages::TradeEvent;
use crate::models::{OrderType, Side};
use crate::orderbook::OrderBook;
use crate::pipeline::{BalanceUpdateRequest, PriceImprovement};
use crate::pipeline_mt::MarketContext;
use std::path::{Path, PathBuf};

// Logging macros (matching pipeline_mt.rs)
const TARGET_ME: &str = "0XINFI::ME";
const LOG_ORDER: &str = "ORDER";
const LOG_CANCEL: &str = "CANCEL";

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

// ============================================================
// Snapshot Trigger Helper
// ============================================================

struct SnapshotTrigger {
    snapshotter: MatchingSnapshotter,
    trade_counter: u64,
    snapshot_interval: u64, // Snapshot every N trades
}

// ============================================================
// Matching Service
// ============================================================

/// Service that handles matching engine processing.
///
/// Processes valid orders through the matching engine and generates trades.
///
/// Optionally supports persistence (WAL + snapshots) via `new_with_persistence()`.
pub struct MatchingService {
    book: OrderBook,
    queues: Arc<MultiThreadQueues>,
    stats: Arc<PipelineStats>,
    market: MarketContext,
    // Depth snapshot update tracking
    depth_dirty: bool,
    last_depth_update: std::time::Instant,
    depth_update_interval_ms: u64,
    // Optional persistence (Phase 2.4)
    trade_wal_writer: Option<MatchingWalWriter>,
    snapshot_trigger: Option<SnapshotTrigger>,
    #[allow(dead_code)] // Reserved for future use
    data_dir: Option<PathBuf>,
}

impl MatchingService {
    /// Create a new matching service without persistence (backward compatible)
    pub fn new(
        book: OrderBook,
        queues: Arc<MultiThreadQueues>,
        stats: Arc<PipelineStats>,
        market: MarketContext,
        depth_update_interval_ms: u64,
    ) -> Self {
        Self {
            book,
            queues,
            stats,
            market,
            depth_dirty: false,
            last_depth_update: std::time::Instant::now(),
            depth_update_interval_ms,
            // Persistence disabled
            trade_wal_writer: None,
            snapshot_trigger: None,
            data_dir: None,
        }
    }

    /// Create a new matching service with persistence (Phase 2.4)
    ///
    /// Performs recovery on startup and enables Trade WAL + periodic snapshots.
    pub fn new_with_persistence(
        data_dir: impl AsRef<Path>,
        queues: Arc<MultiThreadQueues>,
        stats: Arc<PipelineStats>,
        market: MarketContext,
        depth_update_interval_ms: u64,
        snapshot_interval_trades: u64,
    ) -> std::io::Result<Self> {
        use std::fs;

        // 1. Recovery (cold or hot start)
        let recovery = MatchingRecovery::new(&data_dir);
        let RecoveryState {
            orderbook,
            next_seq_id,
        } = recovery.recover()?;

        tracing::info!(
            "MatchingService recovery complete: next_seq_id={}",
            next_seq_id
        );

        // 2. Initialize WAL writer
        let wal_dir = data_dir.as_ref().join("wal");
        fs::create_dir_all(&wal_dir)?;
        let wal_path = wal_dir.join("trades.wal");
        let wal_writer = MatchingWalWriter::new(&wal_path, 1, next_seq_id)?;

        // 3. Initialize snapshot trigger
        let snapshot_dir = data_dir.as_ref().join("snapshots");
        let snapshotter = MatchingSnapshotter::new(&snapshot_dir);
        let snapshot_trigger = SnapshotTrigger {
            snapshotter,
            trade_counter: 0,
            snapshot_interval: snapshot_interval_trades,
        };

        Ok(Self {
            book: orderbook,
            queues,
            stats,
            market,
            depth_dirty: false,
            last_depth_update: std::time::Instant::now(),
            depth_update_interval_ms,
            // Persistence enabled
            trade_wal_writer: Some(wal_writer),
            snapshot_trigger: Some(snapshot_trigger),
            data_dir: Some(data_dir.as_ref().to_path_buf()),
        })
    }

    /// Run the matching service (MT blocking mode)
    pub fn run(&mut self, shutdown: &ShutdownSignal) {
        let mut spin_count = 0u32;

        loop {
            let mut did_work = false;

            if let Some(action) = self.queues.action_queue.pop() {
                did_work = true;
                let task_start = Instant::now();

                match action {
                    ValidAction::Order(valid_order) => {
                        let span =
                            p_span!(TARGET_ME, LOG_ORDER, order_id = valid_order.order.order_id);
                        let _enter = span.enter();

                        // Match order
                        let result = MatchingEngine::process_order(
                            &mut self.book,
                            valid_order.order.clone(),
                        );

                        tracing::info!(
                            "[TRACE] Order {}: Matching Engine Processed (Trades: {})",
                            valid_order.order.order_id,
                            result.trades.len()
                        );

                        p_info!(
                            TARGET_ME,
                            trades = result.trades.len(),
                            "Matching completed"
                        );

                        // Collect all TradeEvents for this order
                        let mut trade_events = Vec::with_capacity(result.trades.len());

                        // Fan-out: Send Trade Events to BOTH Settlement AND UBSCore
                        for trade in &result.trades {
                            // Determine taker and maker order IDs based on taker side
                            let (taker_id, maker_id) = if valid_order.order.side == Side::Buy {
                                (trade.buyer_order_id, trade.seller_order_id)
                            } else {
                                (trade.seller_order_id, trade.buyer_order_id)
                            };

                            let trade_event = TradeEvent::new(
                                trade.clone(),
                                taker_id,
                                maker_id,
                                valid_order.order.side,
                                // Taker order state (from result.order)
                                result.order.qty,
                                result.order.filled_qty,
                                // Maker order state (TODO: get from book or track separately)
                                0, // maker_order_qty - placeholder
                                0, // maker_filled_qty - placeholder
                                self.market.base_id,
                                self.market.quote_id,
                                self.market.qty_unit,
                                valid_order.ingested_at_ns,
                                self.market.symbol_id, // symbol_id for fee lookup
                            );

                            // Collect for MEResult
                            trade_events.push(trade_event.clone());

                            // Calculate price improvement for buy orders
                            let price_improvement = if valid_order.order.side == Side::Buy
                                && valid_order.order.order_type == OrderType::Limit
                                && valid_order.order.price > trade.price
                            {
                                let diff = valid_order.order.price - trade.price;
                                let refund = diff * trade.qty / self.market.qty_unit;
                                if refund > 0 {
                                    Some(PriceImprovement {
                                        user_id: valid_order.order.user_id,
                                        asset_id: self.market.quote_id,
                                        amount: refund,
                                    })
                                } else {
                                    None
                                }
                            } else {
                                None
                            };

                            // Send to UBSCore (for balance update) - balance_update_queue
                            let balance_update = BalanceUpdateRequest::Trade {
                                trade_event,
                                price_improvement,
                            };
                            loop {
                                match self
                                    .queues
                                    .balance_update_queue
                                    .push(balance_update.clone())
                                {
                                    Ok(()) => break,
                                    Err(_) => {
                                        self.stats.incr_backpressure("balance_update_queue");
                                        std::thread::sleep(IDLE_SLEEP_US);
                                    }
                                }
                            }

                            self.stats.add_trades(1);
                            self.stats.record_trades(1);
                        }

                        // [ATOMIC] Push MEResult to Settlement (order + all trades bundled)
                        let me_result = crate::messages::MEResult {
                            order: result.order.clone(),
                            trades: trade_events,
                            maker_updates: result.maker_orders,
                            final_status: result.order.status,
                            symbol_id: valid_order.order.symbol_id,
                        };
                        loop {
                            match self.queues.me_result_queue.push(me_result.clone()) {
                                Ok(()) => break,
                                Err(_) => {
                                    self.stats.incr_backpressure("me_result_queue");
                                    std::thread::sleep(IDLE_SLEEP_US);
                                }
                            }
                        }

                        self.depth_dirty = true; // Mark depth as changed

                        // [PERSISTENCE] Phase 2.4: Write trades to WAL and trigger snapshots
                        if let Some(ref mut wal_writer) = self.trade_wal_writer {
                            // Write all trades to WAL
                            for trade in &result.trades {
                                if let Err(e) =
                                    wal_writer.append_trade(trade, valid_order.order.symbol_id)
                                {
                                    tracing::error!("Failed to write trade to WAL: {}", e);
                                }
                            }

                            // Flush WAL to disk
                            if let Err(e) = wal_writer.flush() {
                                tracing::error!("Failed to flush WAL: {}", e);
                            }

                            // Check snapshot trigger
                            if let Some(ref mut trigger) = self.snapshot_trigger {
                                trigger.trade_counter += result.trades.len() as u64;

                                if trigger.trade_counter >= trigger.snapshot_interval {
                                    let wal_seq = wal_writer.current_seq() - 1; // Last written seq

                                    if let Err(e) =
                                        trigger.snapshotter.create_snapshot(&self.book, wal_seq)
                                    {
                                        tracing::error!("Failed to create snapshot: {}", e);
                                    } else {
                                        tracing::info!(
                                            "Created snapshot at seq {}, resetting counter",
                                            wal_seq
                                        );
                                        trigger.trade_counter = 0; // Reset counter
                                    }
                                }
                            }
                        }
                    }
                    ValidAction::Cancel {
                        order_id,
                        user_id,
                        ingested_at_ns,
                    } => {
                        let span = p_span!(
                            TARGET_ME,
                            LOG_CANCEL,
                            order_id = order_id,
                            user_id = user_id
                        );
                        let _enter = span.enter();
                        // Cancel order: remove from book
                        if let Some(mut cancelled_order) = self.book.remove_order_by_id(order_id) {
                            cancelled_order.status = OrderStatus::CANCELED;
                            let remaining_qty = cancelled_order.remaining_qty();

                            if remaining_qty > 0 {
                                // Calculate unlock amount
                                let mut temp_order = cancelled_order.clone();
                                temp_order.qty = remaining_qty;
                                let unlock_amount =
                                    temp_order.calculate_cost(self.market.qty_unit).unwrap_or(0);
                                let lock_asset_id = match cancelled_order.side {
                                    Side::Buy => self.market.quote_id,
                                    Side::Sell => self.market.base_id,
                                };

                                // Send cancel result to UBSCore for unlock
                                let cancel_update = BalanceUpdateRequest::Cancel {
                                    order_id,
                                    user_id,
                                    asset_id: lock_asset_id,
                                    unlock_amount,
                                    ingested_at_ns,
                                };
                                loop {
                                    match self
                                        .queues
                                        .balance_update_queue
                                        .push(cancel_update.clone())
                                    {
                                        Ok(()) => break,
                                        Err(_) => {
                                            self.stats.incr_backpressure("depth_event_queue");
                                            // std::hint::spin_loop();
                                            std::thread::sleep(IDLE_SLEEP_US);
                                        }
                                    }
                                }

                                // [PUSH] Notify client of cancellation
                                let _ = self.queues.push_event_queue.push(
                                    crate::websocket::PushEvent::OrderUpdate {
                                        user_id,
                                        order_id,
                                        symbol_id: cancelled_order.symbol_id,
                                        status: OrderStatus::CANCELED,
                                        filled_qty: cancelled_order.filled_qty,
                                        avg_price: None, // Could calculate if partially filled, but omitting for now
                                    },
                                );
                            }
                        }
                        self.depth_dirty = true; // Mark depth as changed
                    }
                }
                self.stats
                    .add_matching_time(task_start.elapsed().as_nanos() as u64);
            }

            // [DEPTH] Periodic snapshot update (if dirty and interval elapsed)
            if self.depth_dirty
                && self.last_depth_update.elapsed().as_millis()
                    >= self.depth_update_interval_ms as u128
            {
                let depth = self.book.get_depth(100);
                let snapshot = crate::messages::DepthSnapshot::new(
                    depth.bids,
                    depth.asks,
                    depth.last_update_id,
                );
                let _ = self.queues.depth_event_queue.push(snapshot);
                self.last_depth_update = std::time::Instant::now();
                self.depth_dirty = false;
            }

            // Check for shutdown
            if shutdown.is_shutdown_requested() && self.queues.action_queue.is_empty() {
                break;
            }

            if !did_work {
                spin_count += 1;
                if spin_count > IDLE_SPIN_LIMIT {
                    std::thread::sleep(IDLE_SLEEP_US);
                    spin_count = 0;
                } else {
                    std::hint::spin_loop();
                }
            } else {
                spin_count = 0;
            }
        }
    }

    /// Consume the service and return the inner OrderBook
    pub fn into_inner(self) -> OrderBook {
        self.book
    }
}

// ============================================================
// SETTLEMENT SERVICE
// ============================================================

// unused imports and constants removed

/// Service that handles trade settlement and ledger persistence.
///
/// Persists balance events and trade ledger entries to the settlement log.
pub struct SettlementService {
    ledger: LedgerWriter,
    queues: Arc<MultiThreadQueues>,
    stats: Arc<PipelineStats>,
    db_client: Option<Arc<crate::persistence::TDengineClient>>,
    symbol_id: u32,
    symbol_mgr: Arc<crate::symbol_manager::SymbolManager>,
}

impl SettlementService {
    pub fn new(
        ledger: LedgerWriter,
        queues: Arc<MultiThreadQueues>,
        stats: Arc<PipelineStats>,
        db_client: Option<Arc<crate::persistence::TDengineClient>>,
        symbol_id: u32,
        symbol_mgr: Arc<crate::symbol_manager::SymbolManager>,
    ) -> Self {
        Self {
            ledger,
            queues,
            stats,
            db_client,
            symbol_id,
            symbol_mgr,
        }
    }

    /// Run the settlement service in async mode (zero block_on overhead)
    ///
    /// This version runs entirely within the tokio runtime, eliminating
    /// the overhead of block_on() calls per batch.
    pub async fn run_async(self, shutdown: Arc<ShutdownSignal>) {
        let db_client = self.db_client.clone();
        let queues = self.queues.clone();
        let stats = self.stats.clone();
        let symbol_id = self.symbol_id;
        let symbol_mgr = self.symbol_mgr.clone();

        // Spawn async balance processor
        let balance_task = Self::spawn_balance_processor_async(
            queues.clone(),
            db_client.clone(),
            shutdown.clone(),
        );

        // Spawn async trade processor
        let trade_task = Self::spawn_trade_processor_async(
            queues.clone(),
            stats.clone(),
            db_client.clone(),
            symbol_id,
            symbol_mgr,
            shutdown.clone(),
        );

        // Wait for both to complete
        let _ = tokio::join!(balance_task, trade_task);
    }

    /// Spawn async balance processor (zero block_on overhead)
    ///
    /// Uses tokio::spawn with smart batching:
    /// - Wait for first item (with timeout)
    /// - Greedy drain remaining items
    /// - Single async exec per batch
    fn spawn_balance_processor_async(
        queues: Arc<MultiThreadQueues>,
        db_client: Option<Arc<crate::persistence::TDengineClient>>,
        shutdown: Arc<ShutdownSignal>,
    ) -> tokio::task::JoinHandle<()> {
        const BATCH_SIZE: usize = 128;
        const POLL_INTERVAL_MS: u64 = 1; // Fast polling for low latency

        tokio::spawn(async move {
            let mut batch: Vec<crate::messages::BalanceEvent> = Vec::with_capacity(BATCH_SIZE);

            loop {
                batch.clear();

                // Greedy drain: pop all available events
                while batch.len() < BATCH_SIZE {
                    if let Some(event) = queues.balance_event_queue.pop() {
                        batch.push(event);
                    } else {
                        break;
                    }
                }

                if !batch.is_empty() {
                    // TDengine: batch persist using shared function
                    if let Some(ref db) = db_client {
                        let start = std::time::Instant::now();

                        // 1. Balance snapshots (for latest balance query)
                        if let Err(e) = crate::persistence::balances::batch_upsert_balance_events(
                            db.taos(),
                            &batch,
                        )
                        .await
                        {
                            tracing::error!("[PERSIST] async batch balance snapshot failed: {}", e);
                        }

                        // 2. Balance events (for event sourcing/fee audit)
                        // account_type = 1 for Spot, matches design doc 4.2
                        if let Err(e) = crate::persistence::balances::batch_insert_balance_events(
                            db.taos(),
                            &batch,
                            1, // account_type = Spot
                        )
                        .await
                        {
                            tracing::error!("[PERSIST] async batch balance_events failed: {}", e);
                        }

                        if start.elapsed().as_millis() > 5 {
                            tracing::warn!(
                                "[PROFILE] async_balance_persist={}ms count={}",
                                start.elapsed().as_millis(),
                                batch.len()
                            );
                        }
                    }

                    // WebSocket: push balance updates
                    for event in &batch {
                        let _ = queues.push_event_queue.push(
                            crate::websocket::PushEvent::BalanceUpdate {
                                user_id: event.user_id,
                                asset_id: event.asset_id,
                                avail: event.avail_after,
                                frozen: event.frozen_after,
                            },
                        );
                    }
                } else {
                    // Queue empty - check shutdown or yield
                    if shutdown.is_shutdown_requested() {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)).await;
                }
            }
            tracing::info!("[SETTLEMENT] Async balance processor exiting");
        })
    }

    /// Spawn async trade processor (zero block_on overhead)
    fn spawn_trade_processor_async(
        queues: Arc<MultiThreadQueues>,
        stats: Arc<PipelineStats>,
        db_client: Option<Arc<crate::persistence::TDengineClient>>,
        _symbol_id: u32,
        symbol_mgr: Arc<crate::symbol_manager::SymbolManager>,
        shutdown: Arc<ShutdownSignal>,
    ) -> tokio::task::JoinHandle<()> {
        const BATCH_SIZE: usize = 128;
        const POLL_INTERVAL_MS: u64 = 1;

        tokio::spawn(async move {
            let mut batch: Vec<crate::messages::MEResult> = Vec::with_capacity(BATCH_SIZE);

            loop {
                batch.clear();

                // Greedy drain
                while batch.len() < BATCH_SIZE {
                    if let Some(result) = queues.me_result_queue.pop() {
                        batch.push(result);
                    } else {
                        break;
                    }
                }

                if !batch.is_empty() {
                    // TDengine: batch persist (direct await, no block_on!)
                    if let Some(ref db) = db_client {
                        let start = std::time::Instant::now();
                        let result =
                            crate::persistence::orders::batch_insert_me_results(db.taos(), &batch)
                                .await;

                        if let Err(e) = result {
                            tracing::error!("[PERSIST] async batch me_result failed: {}", e);
                        }

                        if start.elapsed().as_millis() > 5 {
                            tracing::warn!(
                                "[PROFILE] async_me_persist={}ms orders={} trades={}",
                                start.elapsed().as_millis(),
                                batch.len(),
                                batch.iter().map(|r| r.trades.len()).sum::<usize>()
                            );
                        }
                    }

                    // WebSocket: push order update + trade events
                    for me_result in &batch {
                        let symbol_id = me_result.symbol_id;

                        for trade_event in &me_result.trades {
                            Self::push_trade_events(
                                &queues,
                                trade_event,
                                &trade_event.trade,
                                symbol_id,
                                &symbol_mgr,
                            );
                        }

                        if me_result.trades.is_empty() {
                            let _ = queues.push_event_queue.push(
                                crate::websocket::PushEvent::OrderUpdate {
                                    user_id: me_result.order.user_id,
                                    order_id: me_result.order.order_id,
                                    symbol_id,
                                    status: me_result.final_status,
                                    filled_qty: me_result.order.filled_qty,
                                    avg_price: None,
                                },
                            );
                        }
                    }

                    stats.add_event_log_time(0);
                } else {
                    if shutdown.is_shutdown_requested() {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS)).await;
                }
            }
            tracing::info!("[SETTLEMENT] Async trade processor exiting");
        })
    }

    /// Push trade events to WebSocket
    fn push_trade_events(
        queues: &Arc<MultiThreadQueues>,
        trade_event: &TradeEvent,
        trade: &crate::models::Trade,
        symbol_id: u32,
        symbol_mgr: &crate::symbol_manager::SymbolManager,
    ) {
        // Get fee rates from SymbolManager
        let symbol_info = symbol_mgr.get_symbol_info_by_id(symbol_id);
        let (maker_fee_rate, taker_fee_rate) = symbol_info
            .map(|s| (s.base_maker_fee, s.base_taker_fee))
            .unwrap_or((1000, 2000)); // Default: 0.10%/0.20%

        // --- Taker Side ---
        let taker_status = if trade_event.taker_filled_qty >= trade_event.taker_order_qty {
            OrderStatus::FILLED
        } else {
            OrderStatus::PARTIALLY_FILLED
        };
        let taker_user_id = if trade_event.taker_side == Side::Buy {
            trade.buyer_user_id
        } else {
            trade.seller_user_id
        };

        // Taker Order update
        let _ = queues
            .push_event_queue
            .push(crate::websocket::PushEvent::OrderUpdate {
                user_id: taker_user_id,
                order_id: trade_event.taker_order_id,
                symbol_id,
                status: taker_status,
                filled_qty: trade_event.taker_filled_qty,
                avg_price: Some(trade.price),
            });

        // --- Maker Side ---
        let maker_status = if trade_event.maker_filled_qty >= trade_event.maker_order_qty {
            OrderStatus::FILLED
        } else {
            OrderStatus::PARTIALLY_FILLED
        };
        let maker_user_id = if trade_event.taker_side == Side::Buy {
            trade.seller_user_id
        } else {
            trade.buyer_user_id
        };

        // Maker Order update
        let _ = queues
            .push_event_queue
            .push(crate::websocket::PushEvent::OrderUpdate {
                user_id: maker_user_id,
                order_id: trade_event.maker_order_id,
                symbol_id,
                status: maker_status,
                filled_qty: trade_event.maker_filled_qty,
                avg_price: Some(trade.price),
            });

        // Calculate fees
        // Buyer receives base_asset, fee deducted from base
        // Seller receives quote_asset, fee deducted from quote
        let buyer_is_maker = trade_event.taker_side != Side::Buy;
        let seller_is_maker = trade_event.taker_side != Side::Sell;

        let buyer_fee_rate = if buyer_is_maker {
            maker_fee_rate
        } else {
            taker_fee_rate
        };
        let seller_fee_rate = if seller_is_maker {
            maker_fee_rate
        } else {
            taker_fee_rate
        };

        // Buyer's gain: trade.qty (base units), fee calculated on this
        let buyer_fee = crate::fee::calculate_fee(trade.qty, buyer_fee_rate);

        // Seller's gain: trade.price * trade.qty / qty_unit, fee calculated on this
        let quote_amount =
            (trade.price as u128 * trade.qty as u128 / trade_event.qty_unit as u128) as u64;
        let seller_fee = crate::fee::calculate_fee(quote_amount, seller_fee_rate);

        // Buyer trade
        let _ = queues
            .push_event_queue
            .push(crate::websocket::PushEvent::Trade {
                user_id: trade.buyer_user_id,
                trade_id: trade.trade_id,
                order_id: trade.buyer_order_id,
                symbol_id,
                side: Side::Buy,
                price: trade.price,
                qty: trade.qty,
                fee: buyer_fee,
                fee_asset_id: trade_event.base_asset_id,
                is_maker: buyer_is_maker,
            });

        // Seller trade
        let _ = queues
            .push_event_queue
            .push(crate::websocket::PushEvent::Trade {
                user_id: trade.seller_user_id,
                trade_id: trade.trade_id,
                order_id: trade.seller_order_id,
                symbol_id,
                side: Side::Sell,
                price: trade.price,
                qty: trade.qty,
                fee: seller_fee,
                fee_asset_id: trade_event.quote_asset_id,
                is_maker: seller_is_maker,
            });
    }

    /// Consume the service and return the inner LedgerWriter
    pub fn into_inner(self) -> LedgerWriter {
        self.ledger
    }
}
