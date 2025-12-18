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

use crate::csv_io::{ACTION_CANCEL, ACTION_PLACE, InputOrder};
use crate::models::InternalOrder;
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
                        self.stats.incr_backpressure();
                        std::hint::spin_loop();
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
                    did_work = true;
                    match self.ubscore.apply_order_action(action) {
                        Ok((actions, events)) => {
                            self.batch_actions.extend(actions);
                            self.batch_events.extend(events);
                            self.stats.incr_accepted();
                        }
                        Err(_) => self.stats.incr_rejected(),
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
                    while let Err(_) = self.queues.action_queue.push(action.clone()) {
                        std::hint::spin_loop();
                        if shutdown.is_shutdown_requested() {
                            break;
                        }
                    }
                }

                for event in self.batch_events.drain(..) {
                    let lat_ingested = event.ingested_at_ns;
                    while let Err(_) = self.queues.balance_event_queue.push(event.clone()) {
                        std::hint::spin_loop();
                        self.stats.incr_backpressure();
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
