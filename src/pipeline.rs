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

use crate::messages::{RejectReason, TradeEvent, ValidOrder};
use crate::models::InternalOrder;

// ============================================================
// QUEUE CAPACITY CONFIGURATION
// ============================================================

/// Capacity for order queue (Ingestion → UBSCore)
/// Should handle burst without blocking, but not too large to waste memory
pub const ORDER_QUEUE_CAPACITY: usize = 4096;

/// Capacity for valid order queue (UBSCore → ME)
/// Smaller than order_queue since rejects are filtered out
pub const VALID_ORDER_QUEUE_CAPACITY: usize = 4096;

/// Capacity for trade queue (ME → Settlement + UBSCore)
/// Larger because one order may generate multiple trades
pub const TRADE_QUEUE_CAPACITY: usize = 16384;

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

/// Result from UBSCore processing
#[derive(Debug)]
pub enum PreTradeResult {
    /// Order accepted, balance locked, ready for matching
    Accepted(ValidOrder),
    /// Order rejected with reason
    Rejected {
        seq_id: u64,
        order_id: u64,
        user_id: u64,
        reason: RejectReason,
    },
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
// PIPELINE STATISTICS
// ============================================================

/// Statistics for pipeline execution
#[derive(Debug, Default)]
pub struct PipelineStats {
    /// Total orders ingested
    pub orders_ingested: AtomicU64,
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
}

impl PipelineStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn incr_ingested(&self) {
        self.orders_ingested.fetch_add(1, Ordering::Relaxed);
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
        self.backpressure_events.fetch_add(1, Ordering::Relaxed);
    }

    /// Get snapshot of current stats
    pub fn snapshot(&self) -> PipelineStatsSnapshot {
        PipelineStatsSnapshot {
            orders_ingested: self.orders_ingested.load(Ordering::Relaxed),
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
            "Pipeline Stats: ingested={}, accepted={}, rejected={}, trades={}, settled={}, backpressure={}",
            self.orders_ingested,
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
        let stats = PipelineStats::new();

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
        let stats = PipelineStats::new();

        // Push should succeed immediately when queue not full
        let result = push_with_backpressure(&queue, 42, &stats);
        assert!(result);
        assert_eq!(stats.snapshot().backpressure_events, 0);
    }
}
