//! Order Service - Business logic for order operations
//!
//! This separates business logic from HTTP handlers for better testability
//! and adherence to Single Responsibility Principle.

use std::sync::Arc;

use crossbeam_queue::ArrayQueue;

use crate::pipeline::{OrderAction, SequencedOrder};
use crate::symbol_manager::SymbolManager;

use crate::gateway::handlers::helpers::{now_ms, now_ns};
use crate::gateway::types::ClientOrder;

/// Order service error
#[derive(Debug)]
pub enum OrderError {
    /// Invalid order parameters
    InvalidParameter(String),
    /// Order queue is full
    QueueFull,
    /// Internal error
    Internal(String),
}

impl std::fmt::Display for OrderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderError::InvalidParameter(msg) => write!(f, "{}", msg),
            OrderError::QueueFull => write!(f, "Order queue is full, please try again later"),
            OrderError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for OrderError {}

/// Response data for order operations
pub struct OrderResult {
    pub order_id: u64,
    pub cid: Option<String>,
    pub status: &'static str,
    pub timestamp_ms: u64,
}

/// Order Service - handles all order-related business logic
pub struct OrderService<'a> {
    order_queue: &'a Arc<ArrayQueue<OrderAction>>,
    symbol_mgr: &'a SymbolManager,
    active_symbol_id: u32,
    next_order_id: Box<dyn Fn() -> u64 + 'a>,
}

impl<'a> OrderService<'a> {
    /// Create a new OrderService
    pub fn new(
        order_queue: &'a Arc<ArrayQueue<OrderAction>>,
        symbol_mgr: &'a SymbolManager,
        active_symbol_id: u32,
        next_order_id: impl Fn() -> u64 + 'a,
    ) -> Self {
        Self {
            order_queue,
            symbol_mgr,
            active_symbol_id,
            next_order_id: Box::new(next_order_id),
        }
    }

    /// Create a new order
    pub fn create_order(&self, user_id: u64, req: ClientOrder) -> Result<OrderResult, OrderError> {
        tracing::info!("[TRACE] Create Order: Received from User {}", user_id);
        tracing::info!("[TRACE] Request Details: {:?}", req);

        // 1. Validate and parse ClientOrder
        let validated = crate::gateway::types::validate_client_order(req.clone(), self.symbol_mgr)
            .map_err(|e| OrderError::InvalidParameter(e.to_string()))?;

        // 2. Generate order_id and timestamp
        let order_id = (self.next_order_id)();
        let timestamp = now_ns();

        // 3. Convert to InternalOrder (uses SymbolManager intent-based API)
        let internal_order = validated
            .into_internal_order(order_id, user_id, timestamp, self.symbol_mgr)
            .map_err(|e| OrderError::InvalidParameter(e.to_string()))?;

        // 4. Push to queue
        tracing::info!(
            "[TRACE] Create Order {}: Pushing to Ingestion Queue",
            order_id
        );
        let action = OrderAction::Place(SequencedOrder::new(order_id, internal_order, timestamp));
        self.push_action(action)?;
        tracing::info!(
            "[TRACE] Create Order {}: ✅ Pushed to Ingestion Queue",
            order_id
        );

        Ok(OrderResult {
            order_id,
            cid: req.cid,
            status: "ACCEPTED",
            timestamp_ms: now_ms(),
        })
    }

    /// Cancel an order
    pub fn cancel_order(&self, user_id: u64, order_id: u64) -> Result<OrderResult, OrderError> {
        tracing::info!(
            "[TRACE] Cancel Order {}: Received from User {}",
            order_id,
            user_id
        );

        let action = OrderAction::Cancel {
            order_id,
            user_id,
            ingested_at_ns: now_ns(),
        };

        self.push_action(action)?;
        tracing::info!(
            "[TRACE] Cancel Order {}: ✅ Gateway -> Ingestion Queue (User {})",
            order_id,
            user_id
        );

        Ok(OrderResult {
            order_id,
            cid: None,
            status: "CANCEL_PENDING",
            timestamp_ms: now_ms(),
        })
    }

    /// Reduce order quantity
    pub fn reduce_order(
        &self,
        user_id: u64,
        order_id: u64,
        reduce_qty: rust_decimal::Decimal,
    ) -> Result<OrderResult, OrderError> {
        // Use SymbolManager intent-based API (money-type-safety.md compliance)
        let reduce_qty_u64 = self
            .symbol_mgr
            .decimal_to_qty(reduce_qty, self.active_symbol_id)
            .map_err(|e| OrderError::InvalidParameter(e.to_string()))?;

        tracing::info!(
            "[TRACE] Reduce Order {}: Received from User {}",
            order_id,
            user_id
        );

        let action = OrderAction::Reduce {
            order_id,
            user_id,
            reduce_qty: reduce_qty_u64,
            ingested_at_ns: now_ns(),
        };

        self.push_action(action)?;

        Ok(OrderResult {
            order_id,
            cid: None,
            status: "ACCEPTED",
            timestamp_ms: now_ms(),
        })
    }

    /// Move order to new price
    pub fn move_order(
        &self,
        user_id: u64,
        order_id: u64,
        new_price: rust_decimal::Decimal,
    ) -> Result<OrderResult, OrderError> {
        // Use SymbolManager intent-based API (money-type-safety.md compliance)
        let new_price_u64 = self
            .symbol_mgr
            .decimal_to_price(new_price, self.active_symbol_id)
            .map_err(|e| OrderError::InvalidParameter(e.to_string()))?;

        tracing::info!(
            "[TRACE] Move Order {}: Received from User {}",
            order_id,
            user_id
        );

        let action = OrderAction::Move {
            order_id,
            user_id,
            new_price: new_price_u64,
            ingested_at_ns: now_ns(),
        };

        self.push_action(action)?;

        Ok(OrderResult {
            order_id,
            cid: None,
            status: "ACCEPTED",
            timestamp_ms: now_ms(),
        })
    }

    /// Push action to queue
    fn push_action(&self, action: OrderAction) -> Result<(), OrderError> {
        if self.order_queue.push(action).is_err() {
            tracing::error!("[TRACE] Order queue is full");
            Err(OrderError::QueueFull)
        } else {
            Ok(())
        }
    }
}
