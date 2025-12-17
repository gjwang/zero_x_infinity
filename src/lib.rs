//! 0xInfinity - High-Frequency Trading Engine
//!
//! A production-grade matching engine in Rust, built step by step.
//!
//! # Modules
//!
//! - [`types`] - Core type definitions (AssetId, UserId, etc.)
//! - [`symbol_manager`] - Symbol and asset configuration
//! - [`models`] - InternalOrder and Trade types
//! - [`messages`] - Inter-service communication types
//! - [`ubscore`] - User Balance Core Service (single-threaded balance ops)
//! - [`orderbook`] - BTreeMap-based order book
//! - [`engine`] - Matching engine logic
//! - [`balance`] - Enforced balance type
//! - [`user_account`] - User account management
//! - [`wal`] - Write-Ahead Log for order persistence
//! - [`ledger`] - Settlement audit log
//! - [`perf`] - Performance metrics
//! - [`csv_io`] - CSV loading/saving
//! - [`pipeline`] - Ring Buffer based service pipeline

// Core types - must be first!
pub mod core_types;

// Symbol/Asset configuration
pub mod symbol_manager;

// Trading components
pub mod balance;
pub mod csv_io;
pub mod engine;
pub mod ledger;
pub mod messages;
pub mod models;
pub mod orderbook;
pub mod perf;
pub mod pipeline;
pub mod ubscore;
pub mod user_account;
pub mod wal;

// Convenient re-exports at crate root
pub use balance::Balance;
pub use core_types::{AssetId, OrderId, SeqNum, TradeId, UserId};
pub use engine::MatchingEngine;
pub use messages::{
    BalanceOp, BalanceUpdate, OrderEvent, OrderMessage, RejectReason, TradeEvent, ValidOrder,
};
pub use models::{CostError, InternalOrder, OrderResult, OrderStatus, OrderType, Side, Trade};
pub use orderbook::OrderBook;
pub use symbol_manager::{SymbolInfo, SymbolManager};
pub use ubscore::UBSCore;
pub use user_account::UserAccount;
pub use wal::{WalConfig, WalWriter};

// Pipeline re-exports
pub use pipeline::{
    PipelineQueues, PipelineStats, PipelineStatsSnapshot, SequencedOrder, ShutdownSignal,
    SingleThreadPipeline,
};
