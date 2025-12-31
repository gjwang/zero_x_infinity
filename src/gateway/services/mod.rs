//! Gateway Services Layer
//!
//! This module contains business logic extracted from handlers.
//! Handlers become thin HTTP adapters that delegate to services.

pub mod order;

pub use order::{OrderError, OrderResult, OrderService};
