//! Exchange information module
//!
//! This module contains the core configuration for the exchange:
//! - Assets (tradable currencies)
//! - Symbols (trading pairs)
//! - Validation logic for asset and symbol names

pub mod asset;
pub mod symbol;
pub mod validation;

// Re-export commonly used types
pub use asset::{Asset, AssetManager};
pub use symbol::{Symbol, SymbolManager};
pub use validation::{AssetName, SymbolName, ValidationError};
