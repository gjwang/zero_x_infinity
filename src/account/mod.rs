//! Account management module
//!
//! PostgreSQL-based storage for users, assets,//! Account management module

pub mod models;
pub mod repository;
pub mod validation;

// Re-export commonly used types
pub use models::{Asset, Symbol, User, UserStatus};
pub use repository::{AssetManager, SymbolManager, UserRepository};
pub use validation::{AssetName, SymbolName, ValidationError};

// Re-export Database from top-level db module
pub use crate::db::Database;
