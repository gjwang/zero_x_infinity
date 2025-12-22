//! Account management module
//!
//! PostgreSQL-based storage for users, assets, and trading pairs.

mod db;
mod models;
mod repository;

pub use db::Database;
pub use models::{Asset, Symbol, User, UserStatus};
pub use repository::{AssetManager, SymbolManager, UserRepository};
