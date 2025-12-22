//! Account management module
//!
//! This module handles user account management.

pub mod models;
pub mod repository;

// Re-export commonly used types
pub use models::{User, UserStatus};
pub use repository::UserRepository;

// Re-export Database from top-level db module
pub use crate::db::Database;

// Re-export exchange_info types for backward compatibility
pub use crate::exchange_info::{
    Asset, AssetManager, AssetName, Symbol, SymbolManager, SymbolName, ValidationError,
};
