//! Account management module
//!
//! PostgreSQL-based storage for users, assets,//! Account management module

pub mod db;
pub mod models;
pub mod repository;
pub mod validation;

// Re-export commonly used types
pub use db::Database;
pub use models::{Asset, Symbol, User, UserStatus};
pub use repository::{AssetManager, SymbolManager, UserRepository};
pub use validation::{AssetName, SymbolName, ValidationError};
