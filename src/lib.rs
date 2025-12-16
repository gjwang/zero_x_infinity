//! 0xInfinity - High-Frequency Trading Engine
//!
//! A production-grade matching engine in Rust, built step by step.
//!
//! # Modules
//!
//! - [`types`] - Core type definitions (AssetId, UserId, etc.)
//! - [`config`] - Trading configuration (assets, symbols)
//! - [`models`] - Order and Trade types
//! - [`orderbook`] - BTreeMap-based order book
//! - [`engine`] - Matching engine logic
//! - [`balance`] - Enforced balance type
//! - [`user_account`] - User account management
//! - [`ledger`] - Settlement audit log
//! - [`perf`] - Performance metrics
//! - [`csv_io`] - CSV loading/saving

// Core types - must be first!
pub mod core_types;

// Configuration
pub mod config;

// Trading components
pub mod balance;
pub mod csv_io;
pub mod engine;
pub mod ledger;
pub mod models;
pub mod orderbook;
pub mod perf;
pub mod user_account;

// Legacy re-exports for backwards compatibility
pub mod symbol_manager;

// Convenient re-exports at crate root
pub use balance::Balance;
pub use config::TradingConfig;
pub use core_types::{AssetId, OrderId, SeqNum, TradeId, UserId};
pub use engine::MatchingEngine;
pub use models::{Order, OrderResult, OrderStatus, OrderType, Side, Trade};
pub use orderbook::OrderBook;
pub use user_account::UserAccount;
