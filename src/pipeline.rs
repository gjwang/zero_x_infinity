//! Pipeline - Ring Buffer based service pipeline
//!
//! This module implements the multi-service pipeline using lock-free ring buffers
//! (crossbeam-queue::ArrayQueue) for inter-service communication.
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────┐     order_queue      ┌──────────────┐     valid_order_queue   ┌──────────────┐
//! │   Ingestion  │ ───────────────────▶ │   UBSCore    │ ────────────────────────▶ │      ME      │
//! │              │                      │ (Pre-Trade)  │                          │  (Matching)  │
//! └──────────────┘                      └──────────────┘                          └──────┬───────┘
//!                                                                                        │
//!                                                                                        │ trade_queue
//!                                                                                        ▼
//!                                       ┌──────────────┐                          ┌──────────────┐
//!                                       │   UBSCore    │ ◀──────────────────────── │  Settlement  │
//!                                       │   (Settle)   │       trade_queue         │   (Persist)  │
//!                                       └──────────────┘                          └──────────────┘
//! ```
//!
//! # Key Design
//!
//! - **Single Producer, Single Consumer (SPSC)**: Each queue has exactly one writer and one reader
//! - **Lock-free**: Uses atomic operations, no mutexes
//! - **Backpressure**: Spin-wait when queues are full (HFT: prefer latency over throughput)
//! - **Deterministic**: Order-level and trade-level events maintain separate version spaces

use crossbeam_queue::ArrayQueue;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::messages::{BalanceEvent, RejectReason, TradeEvent, ValidOrder};
use crate::models::InternalOrder;

// ============================================================
// QUEUE CAPACITY CONFIGURATION
// ============================================================

/// Capacity for order queue (Ingestion → UBSCore)
/// Should handle burst without blocking, but not too large to waste memory
pub const ORDER_QUEUE_CAPACITY: usize = 16384;

/// Capacity for valid order queue (UBSCore → ME)
/// Smaller than order_queue since rejects are filtered out
pub const VALID_ORDER_QUEUE_CAPACITY: usize = 16384;

/// Capacity for trade queue (ME → Settlement + UBSCore)
/// Larger because one order may generate multiple trades
pub const TRADE_QUEUE_CAPACITY: usize = 16384;

/// Capacity for push event queue (Settlement → WsService)
/// Should handle burst of push notifications
pub const PUSH_EVENT_QUEUE_CAPACITY: usize = 65536;

/// Capacity for depth event queue (ME → DepthService)
/// Market data can be dropped if queue is full
pub const DEPTH_EVENT_QUEUE_CAPACITY: usize = 1;

// ============================================================
// PIPELINE INPUT/OUTPUT TYPES
// ============================================================

/// Order with sequence number from ingestion
#[derive(Debug, Clone)]
pub struct SequencedOrder {
    /// Global sequence number assigned at ingestion
    pub seq_id: u64,
    /// The actual order
    pub order: InternalOrder,
    /// Timestamp when order was ingested (nanoseconds)
    pub ingested_at_ns: u64,
}

impl SequencedOrder {
    pub fn new(seq_id: u64, order: InternalOrder, ingested_at_ns: u64) -> Self {
        Self {
            seq_id,
            order,
            ingested_at_ns,
        }
    }
}

/// Order action in the pipeline
///
/// Unified type for the order queue, supporting both new orders and cancel requests.
/// This maintains FIFO ordering between place and cancel operations.
#[derive(Debug, Clone)]
pub enum OrderAction {
    /// Place a new order
    Place(SequencedOrder),
    /// Cancel an existing order
    Cancel {
        /// The order ID to cancel
        order_id: u64,
        /// The user ID (for validation/logging)
        user_id: u64,
        /// Timestamp when order was ingested (nanoseconds)
        ingested_at_ns: u64,
    },
}

/// Result from UBSCore processing
#[derive(Debug)]
pub enum PreTradeResult {
    /// Order accepted, balance locked, ready for matching
    Accepted(ValidOrder),
    /// Order rejected (not enough balance, invalid, etc.)
    Rejected(RejectReason),
}

/// Generic container for pipeline services/resources
///
/// Can hold owned objects (MT) or mutable references (Single-Thread).
pub struct PipelineServices<U, B, L> {
    pub ubscore: U,
    pub book: B,
    pub ledger: L,
}

/// Common configuration for both pipeline modes
pub struct PipelineConfig<'a> {
    pub symbol_mgr: &'a crate::symbol_manager::SymbolManager,
    pub active_symbol_id: u32,
    pub sample_rate: usize,
    pub continuous: bool,
}

/// Action for ME thread (UBSCore → ME)
///
/// Unified type for the action queue between UBSCore and ME.
/// Both place orders and cancel requests go through the same path.
#[derive(Debug, Clone)]
pub enum ValidAction {
    /// Valid order ready for matching (balance already locked)
    Order(ValidOrder),
    /// Cancel request (no balance lock needed, ME will remove from book)
    Cancel {
        /// The order ID to cancel
        order_id: u64,
        /// The user ID (for validation/logging)
        user_id: u64,
        /// Timestamp when order was ingested (nanoseconds)
        ingested_at_ns: u64,
    },
}

// ============================================================
// MULTI-THREAD MESSAGE TYPES
// ============================================================

/// Settlement request (ME → UBSCore)
///
/// After matching, ME sends this to UBSCore thread for balance settlement.
#[derive(Debug, Clone)]
pub struct SettleRequest {
    /// The trade event to settle
    pub trade_event: TradeEvent,
    /// Price improvement refund (for limit buy orders filled at better price)
    pub price_improvement: Option<PriceImprovement>,
}

/// Price improvement refund info
#[derive(Debug, Clone)]
pub struct PriceImprovement {
    pub user_id: u64,
    pub asset_id: u32,
    pub amount: u64,
}

impl SettleRequest {
    pub fn new(trade_event: TradeEvent) -> Self {
        Self {
            trade_event,
            price_improvement: None,
        }
    }

    pub fn with_price_improvement(
        trade_event: TradeEvent,
        user_id: u64,
        asset_id: u32,
        amount: u64,
    ) -> Self {
        Self {
            trade_event,
            price_improvement: Some(PriceImprovement {
                user_id,
                asset_id,
                amount,
            }),
        }
    }
}

/// Pipeline events for Ledger thread
///
/// All threads send events here for centralized logging.
#[derive(Debug, Clone)]
pub enum PipelineEvent {
    /// Order accepted by UBSCore
    OrderAccepted {
        seq_id: u64,
        order_id: u64,
        user_id: u64,
    },
    /// Order rejected by UBSCore
    OrderRejected {
        order_id: u64,
        user_id: u64,
        reason: RejectReason,
    },
    /// Balance locked for order
    BalanceLocked {
        user_id: u64,
        asset_id: u32,
        seq_id: u64,
        amount: u64,
        version: u64,
        avail_after: u64,
        frozen_after: u64,
    },
    /// Balance unlocked (cancel)
    BalanceUnlocked {
        user_id: u64,
        asset_id: u32,
        order_id: u64,
        amount: u64,
        version: u64,
        avail_after: u64,
        frozen_after: u64,
    },
    /// Trade executed
    TradeExecuted {
        trade_id: u64,
        buyer_order_id: u64,
        seller_order_id: u64,
        price: u64,
        qty: u64,
    },
    /// Order filled
    OrderFilled {
        order_id: u64,
        user_id: u64,
        filled_qty: u64,
        avg_price: u64,
    },
    /// Order partially filled
    OrderPartialFilled {
        order_id: u64,
        user_id: u64,
        filled_qty: u64,
        remaining_qty: u64,
    },
    /// Order cancelled
    OrderCancelled {
        order_id: u64,
        user_id: u64,
        unfilled_qty: u64,
    },
    /// Settlement: spend frozen
    SettleSpend {
        user_id: u64,
        asset_id: u32,
        trade_id: u64,
        amount: u64,
        version: u64,
        avail_after: u64,
        frozen_after: u64,
    },
    /// Settlement: receive
    SettleReceive {
        user_id: u64,
        asset_id: u32,
        trade_id: u64,
        amount: u64,
        version: u64,
        avail_after: u64,
        frozen_after: u64,
    },
    /// Settlement: restore (price improvement refund)
    SettleRestore {
        user_id: u64,
        asset_id: u32,
        trade_id: u64,
        amount: u64,
        version: u64,
        avail_after: u64,
        frozen_after: u64,
    },
    /// Ledger entry (for t2_ledger.csv)
    LedgerEntry {
        trade_id: u64,
        user_id: u64,
        asset_id: u32,
        op: &'static str, // "credit" or "debit"
        delta: u64,
        balance_after: u64,
    },
    /// Shutdown signal
    Shutdown,
}

// ============================================================
// PIPELINE QUEUES (Shared between threads)
// ============================================================

/// Shared ring buffers connecting pipeline stages
///
/// All queues are wrapped in Arc for thread-safe sharing.
/// Each queue is SPSC (single-producer, single-consumer).
pub struct PipelineQueues {
    /// Orders from ingestion → UBSCore
    ///
    /// Producer: Ingestion thread
    /// Consumer: UBSCore thread
    pub order_queue: Arc<ArrayQueue<SequencedOrder>>,

    /// Valid orders from UBSCore → ME
    ///
    /// Producer: UBSCore thread
    /// Consumer: ME thread
    pub valid_order_queue: Arc<ArrayQueue<ValidOrder>>,

    /// Trade events from ME → Settlement (and back to UBSCore for balance)
    ///
    /// Producer: ME thread
    /// Consumer: Settlement/UBSCore thread
    pub trade_queue: Arc<ArrayQueue<TradeEvent>>,
}

impl PipelineQueues {
    /// Create new pipeline queues with default capacities
    pub fn new() -> Self {
        Self {
            order_queue: Arc::new(ArrayQueue::new(ORDER_QUEUE_CAPACITY)),
            valid_order_queue: Arc::new(ArrayQueue::new(VALID_ORDER_QUEUE_CAPACITY)),
            trade_queue: Arc::new(ArrayQueue::new(TRADE_QUEUE_CAPACITY)),
        }
    }

    /// Create pipeline queues with custom capacities
    pub fn with_capacity(
        order_capacity: usize,
        valid_order_capacity: usize,
        trade_capacity: usize,
    ) -> Self {
        Self {
            order_queue: Arc::new(ArrayQueue::new(order_capacity)),
            valid_order_queue: Arc::new(ArrayQueue::new(valid_order_capacity)),
            trade_queue: Arc::new(ArrayQueue::new(trade_capacity)),
        }
    }
}

impl Default for PipelineQueues {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// MULTI-THREAD QUEUE CAPACITIES
// ============================================================

/// Capacity for balance update request queue (ME → UBSCore)
pub const BALANCE_UPDATE_QUEUE_CAPACITY: usize = 16384;

/// Capacity for balance event queue (UBSCore → Settlement)
/// This handles ALL balance changes: deposit, withdraw, lock, settle, unlock
pub const BALANCE_EVENT_QUEUE_CAPACITY: usize = 65536;

/// Capacity for event queue (All → Ledger) - legacy
pub const EVENT_QUEUE_CAPACITY: usize = 65536;

// ============================================================
// MULTI-THREAD QUEUES
// ============================================================

/// Extended queues for multi-threaded pipeline
///
/// Architecture from 0x08-a Trading Pipeline Design:
/// - ME → trade_queue → Settlement (持久化)
/// - ME → balance_update_queue → UBSCore (余额更新)
/// - UBSCore → balance_event_queue → Settlement (余额事件持久化)
pub struct MultiThreadQueues {
    /// Order actions from Ingestion → UBSCore (Pre-Trade)
    ///
    /// Unified queue for both place and cancel orders.
    /// Single queue ensures deterministic ordering (single source of truth).
    pub order_queue: Arc<ArrayQueue<OrderAction>>,

    /// Actions from UBSCore → ME
    ///
    /// Unified queue for both valid orders and cancel requests.
    /// Same path for all commands, each service does what it needs.
    pub action_queue: Arc<ArrayQueue<ValidAction>>,

    /// Trade events from ME → Settlement (for persistence)
    ///
    /// Settlement persists Trade Events, Order Events, Ledger
    pub trade_queue: Arc<ArrayQueue<TradeEvent>>,

    /// Balance update requests from ME → UBSCore
    ///
    /// After matching, ME sends balance updates to UBSCore:
    /// - Buyer: spend_frozen(quote), deposit(base)
    /// - Seller: spend_frozen(base), deposit(quote)
    pub balance_update_queue: Arc<ArrayQueue<BalanceUpdateRequest>>,

    /// Balance events from UBSCore → Settlement (for persistence)
    ///
    /// ALL balance changes are sent here for audit logging:
    /// - External: Deposit, Withdraw
    /// - Pre-Trade: Lock
    /// - Post-Trade: SpendFrozen, Credit
    /// - Cancel/Reject: Unlock
    /// - Price Improvement: RefundFrozen
    pub balance_event_queue: Arc<ArrayQueue<BalanceEvent>>,

    /// Push events from Settlement → WsService (for WebSocket push)
    ///
    /// Settlement generates push events after successful persistence:
    /// - OrderUpdate (FILLED/PARTIALLY_FILLED/CANCELED)
    /// - Trade (buyer + seller notifications)
    /// - BalanceUpdate (balance changes)
    pub push_event_queue: Arc<ArrayQueue<crate::websocket::PushEvent>>,

    /// Depth events from ME → DepthService (for market depth)
    ///
    /// ME sends depth snapshots after processing orders:
    /// - Complete bids/asks state
    /// - Sent after each order action (place/cancel)
    /// - Optionally sent periodically
    ///
    /// Non-blocking: ME drops snapshots if queue is full (market data characteristic)
    pub depth_event_queue: Arc<ArrayQueue<crate::messages::DepthSnapshot>>,
}

/// Balance update request from ME to UBSCore
///
/// Unified type for balance updates after ME processing.
/// Same queue for both trade settlements and cancel unlocks.
#[derive(Debug, Clone)]
pub enum BalanceUpdateRequest {
    /// Trade settlement: spend frozen, credit counterparty
    Trade {
        /// The trade event that triggered this update
        trade_event: TradeEvent,
        /// Price improvement refund (for limit buy orders filled at better price)
        price_improvement: Option<PriceImprovement>,
    },
    /// Cancel result: unlock frozen balance
    Cancel {
        /// The cancelled order (contains all info needed for unlock)
        order_id: u64,
        user_id: u64,
        /// Asset to unlock
        asset_id: u32,
        /// Amount to unlock
        unlock_amount: u64,
        /// Timestamp when order was ingested
        ingested_at_ns: u64,
    },
}

impl BalanceUpdateRequest {
    /// Create a trade settlement request
    pub fn trade(trade_event: TradeEvent) -> Self {
        Self::Trade {
            trade_event,
            price_improvement: None,
        }
    }

    /// Create a trade settlement request with price improvement
    pub fn trade_with_price_improvement(
        trade_event: TradeEvent,
        user_id: u64,
        asset_id: u32,
        amount: u64,
    ) -> Self {
        Self::Trade {
            trade_event,
            price_improvement: Some(PriceImprovement {
                user_id,
                asset_id,
                amount,
            }),
        }
    }

    /// Create a cancel unlock request
    pub fn cancel(
        order_id: u64,
        user_id: u64,
        asset_id: u32,
        unlock_amount: u64,
        ingested_at_ns: u64,
    ) -> Self {
        Self::Cancel {
            order_id,
            user_id,
            asset_id,
            unlock_amount,
            ingested_at_ns,
        }
    }
}

// Note: BalanceEvent is now imported from messages module for unified type
// Use messages::BalanceEvent::lock(), unlock(), settle_spend(), settle_receive() etc.

impl MultiThreadQueues {
    /// Create new multi-thread queues with default capacities
    pub fn new() -> Self {
        Self {
            order_queue: Arc::new(ArrayQueue::new(ORDER_QUEUE_CAPACITY)),
            action_queue: Arc::new(ArrayQueue::new(VALID_ORDER_QUEUE_CAPACITY)),
            trade_queue: Arc::new(ArrayQueue::new(TRADE_QUEUE_CAPACITY)),
            balance_update_queue: Arc::new(ArrayQueue::new(BALANCE_UPDATE_QUEUE_CAPACITY)),
            balance_event_queue: Arc::new(ArrayQueue::new(BALANCE_EVENT_QUEUE_CAPACITY)),
            push_event_queue: Arc::new(ArrayQueue::new(PUSH_EVENT_QUEUE_CAPACITY)),
            depth_event_queue: Arc::new(ArrayQueue::new(DEPTH_EVENT_QUEUE_CAPACITY)),
        }
    }

    /// Check if all processing queues are empty (for shutdown)
    pub fn all_empty(&self) -> bool {
        self.order_queue.is_empty()
            && self.action_queue.is_empty()
            && self.trade_queue.is_empty()
            && self.balance_update_queue.is_empty()
            && self.balance_event_queue.is_empty()
    }
}

impl Default for MultiThreadQueues {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// PIPELINE STATISTICS
// ============================================================

/// Statistics for pipeline execution
#[derive(Debug, Default)]
pub struct PipelineStats {
    /// Total orders ingested
    pub orders_ingested: AtomicU64,
    /// Place orders count
    pub places_count: AtomicU64,
    /// Cancel orders count
    pub cancels_count: AtomicU64,
    /// Orders accepted by UBSCore
    pub orders_accepted: AtomicU64,
    /// Orders rejected by UBSCore
    pub orders_rejected: AtomicU64,
    /// Trades generated
    pub trades_generated: AtomicU64,
    /// Trades settled
    pub trades_settled: AtomicU64,
    /// Queue full events (backpressure)
    pub backpressure_events: AtomicU64,

    // Stage timings (atomic cumulative nanoseconds)
    pub total_pretrade_ns: AtomicU64,
    pub total_matching_ns: AtomicU64,
    pub total_settlement_ns: AtomicU64,
    pub total_event_log_ns: AtomicU64,

    /// Thread-safe performance samples
    pub perf_samples: std::sync::Mutex<crate::perf::PerfMetrics>,
}

impl PipelineStats {
    pub fn new(sample_rate: usize) -> Self {
        Self {
            perf_samples: std::sync::Mutex::new(crate::perf::PerfMetrics::new(sample_rate)),
            ..Default::default()
        }
    }

    pub fn record_latency(&self, latency_ns: u64) {
        if let Ok(mut perf) = self.perf_samples.lock() {
            perf.add_order_latency(latency_ns);
        }
    }

    pub fn record_place(&self) {
        if let Ok(mut perf) = self.perf_samples.lock() {
            perf.inc_place();
        }
    }

    pub fn record_cancel(&self) {
        if let Ok(mut perf) = self.perf_samples.lock() {
            perf.inc_cancel();
        }
    }

    pub fn record_trades(&self, count: u64) {
        if let Ok(mut perf) = self.perf_samples.lock() {
            perf.add_trades(count);
        }
    }

    pub fn add_pretrade_time(&self, ns: u64) {
        self.total_pretrade_ns.fetch_add(ns, Ordering::Relaxed);
        if let Ok(mut perf) = self.perf_samples.lock() {
            perf.add_pretrade_time(ns);
        }
    }

    pub fn add_matching_time(&self, ns: u64) {
        self.total_matching_ns.fetch_add(ns, Ordering::Relaxed);
        if let Ok(mut perf) = self.perf_samples.lock() {
            perf.add_matching_time(ns);
        }
    }

    pub fn add_settlement_time(&self, ns: u64) {
        self.total_settlement_ns.fetch_add(ns, Ordering::Relaxed);
        if let Ok(mut perf) = self.perf_samples.lock() {
            perf.add_settlement_time(ns);
        }
    }

    pub fn add_event_log_time(&self, ns: u64) {
        self.total_event_log_ns.fetch_add(ns, Ordering::Relaxed);
        if let Ok(mut perf) = self.perf_samples.lock() {
            perf.add_event_log_time(ns);
        }
    }

    pub fn incr_ingested(&self) {
        self.orders_ingested.fetch_add(1, Ordering::Relaxed);
    }

    pub fn incr_place(&self) {
        self.places_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn incr_cancel(&self) {
        self.cancels_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn incr_accepted(&self) {
        self.orders_accepted.fetch_add(1, Ordering::Relaxed);
    }

    pub fn incr_rejected(&self) {
        self.orders_rejected.fetch_add(1, Ordering::Relaxed);
    }

    pub fn add_trades(&self, count: u64) {
        self.trades_generated.fetch_add(count, Ordering::Relaxed);
    }

    pub fn incr_settled(&self) {
        self.trades_settled.fetch_add(1, Ordering::Relaxed);
    }

    pub fn incr_backpressure(&self) {
        let count = self.backpressure_events.fetch_add(1, Ordering::Relaxed);
        if count % 10000 == 0 {
            tracing::warn!(target: "0XINFI", total_backpressure = count + 1, "Backpressure detected (1/10000)");
        }
    }

    /// Get snapshot of current stats
    pub fn snapshot(&self) -> PipelineStatsSnapshot {
        PipelineStatsSnapshot {
            orders_ingested: self.orders_ingested.load(Ordering::Relaxed),
            places_count: self.places_count.load(Ordering::Relaxed),
            cancels_count: self.cancels_count.load(Ordering::Relaxed),
            orders_accepted: self.orders_accepted.load(Ordering::Relaxed),
            orders_rejected: self.orders_rejected.load(Ordering::Relaxed),
            trades_generated: self.trades_generated.load(Ordering::Relaxed),
            trades_settled: self.trades_settled.load(Ordering::Relaxed),
            backpressure_events: self.backpressure_events.load(Ordering::Relaxed),
        }
    }
}

/// Immutable snapshot of stats (for reporting)
#[derive(Debug, Clone)]
pub struct PipelineStatsSnapshot {
    pub orders_ingested: u64,
    pub places_count: u64,
    pub cancels_count: u64,
    pub orders_accepted: u64,
    pub orders_rejected: u64,
    pub trades_generated: u64,
    pub trades_settled: u64,
    pub backpressure_events: u64,
}

impl std::fmt::Display for PipelineStatsSnapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Pipeline Stats: ingested={} (place={}, cancel={}), accepted={}, rejected={}, trades={}, settled={}, backpressure={}",
            self.orders_ingested,
            self.places_count,
            self.cancels_count,
            self.orders_accepted,
            self.orders_rejected,
            self.trades_generated,
            self.trades_settled,
            self.backpressure_events
        )
    }
}

// ============================================================
// SHUTDOWN SIGNALING
// ============================================================

/// Shutdown signal for graceful pipeline termination
#[derive(Debug)]
pub struct ShutdownSignal {
    /// Flag to indicate shutdown requested
    pub shutdown: AtomicBool,
}

impl ShutdownSignal {
    pub fn new() -> Self {
        Self {
            shutdown: AtomicBool::new(false),
        }
    }

    /// Request shutdown
    pub fn request_shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }

    /// Check if shutdown was requested
    pub fn is_shutdown_requested(&self) -> bool {
        self.shutdown.load(Ordering::SeqCst)
    }
}

impl Default for ShutdownSignal {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// QUEUE OPERATIONS WITH BACKPRESSURE
// ============================================================

/// Push to queue with spin-wait backpressure
///
/// For HFT, we prefer busy-waiting to minimize latency variance.
/// The CPU cost is acceptable for the latency benefit.
#[inline]
pub fn push_with_backpressure<T>(queue: &ArrayQueue<T>, item: T, stats: &PipelineStats) -> bool {
    let mut item = item;
    loop {
        match queue.push(item) {
            Ok(()) => return true,
            Err(returned) => {
                item = returned;
                stats.incr_backpressure();
                std::hint::spin_loop();
            }
        }
    }
}

/// Try to pop from queue, non-blocking
#[inline]
pub fn try_pop<T>(queue: &ArrayQueue<T>) -> Option<T> {
    queue.pop()
}

// ============================================================
// SINGLE-THREAD PIPELINE RUNNER
// ============================================================

/// Single-thread pipeline runner for validation
///
/// This runs all pipeline stages in a single thread, polling each queue
/// in round-robin fashion. Useful for validating correctness before
/// implementing multi-threaded version.
///
/// # Flow
/// 1. Pop from order_queue → UBSCore → push to valid_order_queue (or reject)
/// 2. Pop from valid_order_queue → ME → push trades to trade_queue
/// 3. Pop from trade_queue → Settlement (persist) + UBSCore (settle balance)
pub struct SingleThreadPipeline {
    pub queues: PipelineQueues,
    pub stats: Arc<PipelineStats>,
    pub shutdown: Arc<ShutdownSignal>,
}

impl SingleThreadPipeline {
    /// Create a new single-thread pipeline
    pub fn new(sample_rate: usize) -> Self {
        Self {
            queues: PipelineQueues::new(),
            stats: Arc::new(PipelineStats::new(sample_rate)),
            shutdown: Arc::new(ShutdownSignal::new()),
        }
    }

    /// Create with custom queue capacities
    pub fn with_capacity(
        order_capacity: usize,
        valid_order_capacity: usize,
        trade_capacity: usize,
        sample_rate: usize,
    ) -> Self {
        Self {
            queues: PipelineQueues::with_capacity(
                order_capacity,
                valid_order_capacity,
                trade_capacity,
            ),
            stats: Arc::new(PipelineStats::new(sample_rate)),
            shutdown: Arc::new(ShutdownSignal::new()),
        }
    }

    /// Ingest an order into the pipeline
    pub fn ingest(&self, seq_id: u64, order: InternalOrder, timestamp_ns: u64) -> bool {
        let seq_order = SequencedOrder::new(seq_id, order, timestamp_ns);
        match self.queues.order_queue.push(seq_order) {
            Ok(()) => {
                self.stats.incr_ingested();
                true
            }
            Err(_) => {
                self.stats.incr_backpressure();
                false
            }
        }
    }

    /// Check if pipeline has pending work
    pub fn has_pending_work(&self) -> bool {
        !self.queues.order_queue.is_empty()
            || !self.queues.valid_order_queue.is_empty()
            || !self.queues.trade_queue.is_empty()
    }

    /// Request graceful shutdown
    pub fn request_shutdown(&self) {
        self.shutdown.request_shutdown();
    }

    /// Check if shutdown was requested
    pub fn is_shutdown_requested(&self) -> bool {
        self.shutdown.is_shutdown_requested()
    }

    /// Get current stats snapshot
    pub fn get_stats(&self) -> PipelineStatsSnapshot {
        self.stats.snapshot()
    }

    /// Get reference to order queue (for external processing)
    pub fn order_queue(&self) -> &Arc<ArrayQueue<SequencedOrder>> {
        &self.queues.order_queue
    }

    /// Get reference to valid order queue (for external processing)
    pub fn valid_order_queue(&self) -> &Arc<ArrayQueue<ValidOrder>> {
        &self.queues.valid_order_queue
    }

    /// Get reference to trade queue (for external processing)
    pub fn trade_queue(&self) -> &Arc<ArrayQueue<TradeEvent>> {
        &self.queues.trade_queue
    }

    /// Get reference to stats (for external update)
    pub fn stats(&self) -> &Arc<PipelineStats> {
        &self.stats
    }
}

// ============================================================
// TESTS
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Side;

    fn make_test_order(id: u64, user_id: u64) -> InternalOrder {
        InternalOrder::new(id, user_id, 1, 100_000_000, 1_000_000, Side::Buy)
    }

    #[test]
    fn test_pipeline_queues_creation() {
        let queues = PipelineQueues::new();

        assert!(queues.order_queue.is_empty());
        assert!(queues.valid_order_queue.is_empty());
        assert!(queues.trade_queue.is_empty());

        assert_eq!(queues.order_queue.capacity(), ORDER_QUEUE_CAPACITY);
        assert_eq!(
            queues.valid_order_queue.capacity(),
            VALID_ORDER_QUEUE_CAPACITY
        );
        assert_eq!(queues.trade_queue.capacity(), TRADE_QUEUE_CAPACITY);
    }

    #[test]
    fn test_sequenced_order() {
        let order = make_test_order(1, 100);
        let seq_order = SequencedOrder::new(42, order, 1234567890);

        assert_eq!(seq_order.seq_id, 42);
        assert_eq!(seq_order.order.order_id, 1);
        assert_eq!(seq_order.ingested_at_ns, 1234567890);
    }

    #[test]
    fn test_order_queue_push_pop() {
        let queues = PipelineQueues::new();
        let order = make_test_order(1, 100);
        let seq_order = SequencedOrder::new(1, order, 0);

        // Push
        assert!(queues.order_queue.push(seq_order).is_ok());
        assert!(!queues.order_queue.is_empty());

        // Pop
        let popped = queues.order_queue.pop();
        assert!(popped.is_some());
        assert_eq!(popped.unwrap().seq_id, 1);
        assert!(queues.order_queue.is_empty());
    }

    #[test]
    fn test_pipeline_stats() {
        let stats = PipelineStats::new(1);

        stats.incr_ingested();
        stats.incr_ingested();
        stats.incr_accepted();
        stats.incr_rejected();
        stats.add_trades(3);

        let snap = stats.snapshot();
        assert_eq!(snap.orders_ingested, 2);
        assert_eq!(snap.orders_accepted, 1);
        assert_eq!(snap.orders_rejected, 1);
        assert_eq!(snap.trades_generated, 3);
    }

    #[test]
    fn test_shutdown_signal() {
        let signal = ShutdownSignal::new();

        assert!(!signal.is_shutdown_requested());
        signal.request_shutdown();
        assert!(signal.is_shutdown_requested());
    }

    #[test]
    fn test_push_with_backpressure_success() {
        let queue = ArrayQueue::new(10);
        let stats = PipelineStats::new(1);

        // Push should succeed immediately when queue not full
        let result = push_with_backpressure(&queue, 42, &stats);
        assert!(result);
        assert_eq!(stats.snapshot().backpressure_events, 0);
    }

    #[test]
    fn test_single_thread_pipeline_creation() {
        let pipeline = SingleThreadPipeline::new(1);

        assert!(!pipeline.has_pending_work());
        assert!(!pipeline.is_shutdown_requested());

        let stats = pipeline.get_stats();
        assert_eq!(stats.orders_ingested, 0);
    }

    #[test]
    fn test_single_thread_pipeline_ingest() {
        let pipeline = SingleThreadPipeline::new(1);
        let order = make_test_order(1, 100);

        // Ingest order
        let success = pipeline.ingest(1, order, 0);
        assert!(success);
        assert!(pipeline.has_pending_work());

        let stats = pipeline.get_stats();
        assert_eq!(stats.orders_ingested, 1);

        // Pop from order queue
        let seq_order = pipeline.order_queue().pop();
        assert!(seq_order.is_some());
        assert_eq!(seq_order.unwrap().seq_id, 1);

        // Now no pending work
        assert!(!pipeline.has_pending_work());
    }

    #[test]
    fn test_single_thread_pipeline_shutdown() {
        let pipeline = SingleThreadPipeline::new(1);

        assert!(!pipeline.is_shutdown_requested());
        pipeline.request_shutdown();
        assert!(pipeline.is_shutdown_requested());
    }

    #[test]
    fn test_single_thread_pipeline_multiple_orders() {
        let pipeline = SingleThreadPipeline::new(1);

        // Ingest multiple orders
        for i in 1..=5 {
            let order = make_test_order(i, 100 + i);
            assert!(pipeline.ingest(i, order, i * 1000));
        }

        let stats = pipeline.get_stats();
        assert_eq!(stats.orders_ingested, 5);

        // Pop all and verify order
        for i in 1..=5 {
            let seq_order = pipeline.order_queue().pop().unwrap();
            assert_eq!(seq_order.seq_id, i);
            assert_eq!(seq_order.order.order_id, i);
        }

        assert!(!pipeline.has_pending_work());
    }
}
