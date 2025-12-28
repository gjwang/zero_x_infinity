//! Sentinel Service Module
//!
//! Phase 0x11-a: Real Chain Integration
//!
//! The Sentinel service monitors blockchain nodes and detects deposits
//! to user addresses. It supports:
//! - Multiple chains (BTC, ETH)
//! - Confirmation tracking
//! - Re-org detection
//! - Configurable per-chain settings

pub mod btc;
pub mod config;
pub mod error;
pub mod eth;
pub mod scanner;
pub mod worker;

// Re-exports for convenience
pub use config::{BtcChainConfig, EthChainConfig, SentinelConfig};
pub use error::{ScannerError, SentinelError};
pub use scanner::{ChainScanner, DetectedDeposit, NodeHealth, ScannedBlock};
pub use worker::{ChainCursor, SentinelWorker};

// Scanner implementations
pub use btc::BtcScanner;
pub use eth::EthScanner;
