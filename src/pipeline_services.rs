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
use taos::AsyncQueryable;

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

use crate::messages::BalanceEvent;
use crate::pipeline::ValidAction;
use crate::ubscore::UBSCore;

const IDLE_SPIN_LIMIT: u32 = 1000;
const IDLE_SLEEP_US: Duration = Duration::from_micros(100);
const UBSC_SETTLE_BATCH: usize = 128;
const UBSC_ORDER_BATCH: usize = 16;

/// Service that handles UBSCore processing (pre-trade + post-trade balance updates).
///
/// Processes orders through UBSCore, commits to WAL, and publishes results downstream.
pub struct UBSCoreService {
    ubscore: UBSCore,
    queues: Arc<MultiThreadQueues>,
    stats: Arc<PipelineStats>,
    start_time: Instant,
    batch_actions: Vec<ValidAction>,
    batch_events: Vec<BalanceEvent>,
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
        }
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
                    while let Err(_) = self.queues.action_queue.push(action.clone()) {
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
                    while let Err(_) = self.queues.balance_event_queue.push(event.clone()) {
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
use crate::messages::TradeEvent;
use crate::models::{OrderType, Side};
use crate::orderbook::OrderBook;
use crate::pipeline::{BalanceUpdateRequest, PriceImprovement};
use crate::pipeline_mt::MarketContext;

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

/// Service that handles matching engine processing.
///
/// Processes valid orders through the matching engine and generates trades.
pub struct MatchingService {
    book: OrderBook,
    queues: Arc<MultiThreadQueues>,
    stats: Arc<PipelineStats>,
    market: MarketContext,
    // Depth snapshot update tracking
    depth_dirty: bool,
    last_depth_update: std::time::Instant,
    depth_update_interval_ms: u64,
}

impl MatchingService {
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
        }
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

use crate::ledger::{LedgerEntry, LedgerWriter, OP_CREDIT, OP_DEBIT};

const TARGET_PERS: &str = "0XINFI::PERS";
const LOG_TRADE: &str = "TRADE";

/// Service that handles trade settlement and ledger persistence.
///
/// Persists balance events and trade ledger entries to the settlement log.
pub struct SettlementService {
    ledger: LedgerWriter,
    queues: Arc<MultiThreadQueues>,
    stats: Arc<PipelineStats>,
    db_client: Option<Arc<crate::persistence::TDengineClient>>,
    symbol_id: u32,
}

impl SettlementService {
    pub fn new(
        ledger: LedgerWriter,
        queues: Arc<MultiThreadQueues>,
        stats: Arc<PipelineStats>,
        db_client: Option<Arc<crate::persistence::TDengineClient>>,
        symbol_id: u32,
    ) -> Self {
        Self {
            ledger,
            queues,
            stats,
            db_client,
            symbol_id,
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

                        // Call the shared batch insert function (includes auto-create fallback)
                        if let Err(e) = crate::persistence::balances::batch_upsert_balance_events(
                            db.taos(),
                            &batch,
                        )
                        .await
                        {
                            tracing::error!("[PERSIST] async batch balance failed: {}", e);
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
                            Self::push_trade_events(&queues, trade_event, &trade_event.trade);
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
    ) {
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

        // Order update
        let _ = queues
            .push_event_queue
            .push(crate::websocket::PushEvent::OrderUpdate {
                user_id: taker_user_id,
                order_id: trade_event.taker_order_id,
                symbol_id: 0,
                status: taker_status,
                filled_qty: trade_event.taker_filled_qty,
                avg_price: Some(trade.price),
            });

        // Buyer trade
        let _ = queues
            .push_event_queue
            .push(crate::websocket::PushEvent::Trade {
                user_id: trade.buyer_user_id,
                trade_id: trade.trade_id,
                order_id: trade.buyer_order_id,
                symbol_id: 0,
                side: Side::Buy,
                price: trade.price,
                qty: trade.qty,
                role: if trade_event.taker_side == Side::Buy {
                    1
                } else {
                    0
                },
            });

        // Seller trade
        let _ = queues
            .push_event_queue
            .push(crate::websocket::PushEvent::Trade {
                user_id: trade.seller_user_id,
                trade_id: trade.trade_id,
                order_id: trade.seller_order_id,
                symbol_id: 0,
                side: Side::Sell,
                price: trade.price,
                qty: trade.qty,
                role: if trade_event.taker_side == Side::Sell {
                    1
                } else {
                    0
                },
            });
    }

    /// Consume the service and return the inner LedgerWriter
    pub fn into_inner(self) -> LedgerWriter {
        self.ledger
    }
}
