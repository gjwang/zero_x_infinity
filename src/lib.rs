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
//! - [`pipeline_runner`] - Pipeline execution logic

// Core types - must be first!
pub mod core_types;

// Symbol/Asset configuration
pub mod symbol_manager;

// Trading components
pub mod balance;
pub mod config;
pub mod csv_io;
pub mod engine;
pub mod gateway;
pub mod ledger;
pub mod logging;
pub mod market; // Market data services (depth, etc.)
pub mod messages;
pub mod models;
pub mod orderbook;
pub mod perf;
pub mod persistence;
pub mod pipeline;
pub mod pipeline_mt;
pub mod pipeline_runner;
pub mod pipeline_services;
pub mod ubscore;
pub mod user_account;
pub mod wal;
pub mod websocket;

// Account management (Phase 0x0A)
pub mod account;

// Convenient re-exports at crate root
pub use balance::Balance;
pub use core_types::{AssetId, OrderId, SeqNum, TradeId, UserId};
pub use engine::MatchingEngine;
pub use messages::{
    BalanceEvent, BalanceEventType, BalanceOp, BalanceUpdate, OrderEvent, OrderMessage,
    RejectReason, SourceType, TradeEvent, ValidOrder,
};
pub use models::{CostError, InternalOrder, OrderResult, OrderStatus, OrderType, Side, Trade};
pub use orderbook::OrderBook;
pub use symbol_manager::{SymbolInfo, SymbolManager};
pub use ubscore::UBSCore;
pub use user_account::UserAccount;
pub use wal::{WalConfig, WalWriter};

// Account management re-exports (Phase 0x0A)
pub use account::{
    Asset, AssetManager, Database, Symbol, SymbolManager as AccountSymbolManager, User,
    UserRepository,
};

// Pipeline re-exports
pub use pipeline::{
    BalanceUpdateRequest, MultiThreadQueues, OrderAction, PipelineQueues, PipelineStats,
    PipelineStatsSnapshot, PriceImprovement, SequencedOrder, ShutdownSignal, SingleThreadPipeline,
    ValidAction,
};
pub use pipeline_mt::{MultiThreadPipelineResult, run_pipeline_multi_thread};
pub use pipeline_runner::{PipelineResult, run_pipeline_single_thread};
