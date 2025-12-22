//! Repository layer for database operations

use sqlx::PgPool;
use super::models::{Asset, Symbol, User, UserStatus};

/// User repository for CRUD operations
pub struct UserRepository;

impl UserRepository {
    /// Get user by ID
    pub async fn get_by_id(pool: &PgPool, user_id: i64) -> Result<Option<User>, sqlx::Error> {
        let row = sqlx::query_as!(
            UserRow,
            r#"SELECT user_id, username, email, status, created_at FROM users WHERE user_id = $1"#,
            user_id
        )
        .fetch_optional(pool)
        .await?;
        
        Ok(row.map(|r| r.into()))
    }
    
    /// Get user by username
    pub async fn get_by_username(pool: &PgPool, username: &str) -> Result<Option<User>, sqlx::Error> {
        let row = sqlx::query_as!(
            UserRow,
            r#"SELECT user_id, username, email, status, created_at FROM users WHERE username = $1"#,
            username
        )
        .fetch_optional(pool)
        .await?;
        
        Ok(row.map(|r| r.into()))
    }
    
    /// Create a new user
    pub async fn create(pool: &PgPool, username: &str, email: Option<&str>) -> Result<i64, sqlx::Error> {
        let row = sqlx::query!(
            r#"INSERT INTO users (username, email) VALUES ($1, $2) RETURNING user_id"#,
            username,
            email
        )
        .fetch_one(pool)
        .await?;
        
        Ok(row.user_id)
    }
}

/// Internal row type for sqlx
#[derive(Debug)]
struct UserRow {
    user_id: i64,
    username: String,
    email: Option<String>,
    status: i16,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        User {
            user_id: row.user_id,
            username: row.username,
            email: row.email,
            status: UserStatus::from(row.status),
            created_at: row.created_at,
        }
    }
}

/// Asset manager for loading and caching assets
pub struct AssetManager;

impl AssetManager {
    /// Load all active assets
    pub async fn load_all(pool: &PgPool) -> Result<Vec<Asset>, sqlx::Error> {
        let rows = sqlx::query_as!(
            Asset,
            r#"SELECT asset_id, symbol, name, decimals, status FROM assets WHERE status = 1"#
        )
        .fetch_all(pool)
        .await?;
        
        Ok(rows)
    }
    
    /// Get asset by ID
    pub async fn get_by_id(pool: &PgPool, asset_id: i32) -> Result<Option<Asset>, sqlx::Error> {
        let row = sqlx::query_as!(
            Asset,
            r#"SELECT asset_id, symbol, name, decimals, status FROM assets WHERE asset_id = $1"#,
            asset_id
        )
        .fetch_optional(pool)
        .await?;
        
        Ok(row)
    }
}

/// Symbol manager for loading and caching trading pairs
pub struct SymbolManager;

impl SymbolManager {
    /// Load all active symbols
    pub async fn load_all(pool: &PgPool) -> Result<Vec<Symbol>, sqlx::Error> {
        let rows = sqlx::query_as!(
            Symbol,
            r#"SELECT symbol_id, name, base_asset_id, quote_asset_id, 
                      price_decimals, qty_decimals, min_qty, status 
               FROM symbols WHERE status = 1"#
        )
        .fetch_all(pool)
        .await?;
        
        Ok(rows)
    }
    
    /// Get symbol by ID
    pub async fn get_by_id(pool: &PgPool, symbol_id: i32) -> Result<Option<Symbol>, sqlx::Error> {
        let row = sqlx::query_as!(
            Symbol,
            r#"SELECT symbol_id, name, base_asset_id, quote_asset_id,
                      price_decimals, qty_decimals, min_qty, status
               FROM symbols WHERE symbol_id = $1"#,
            symbol_id
        )
        .fetch_optional(pool)
        .await?;
        
        Ok(row)
    }
    
    /// Get symbol by name
    pub async fn get_by_name(pool: &PgPool, name: &str) -> Result<Option<Symbol>, sqlx::Error> {
        let row = sqlx::query_as!(
            Symbol,
            r#"SELECT symbol_id, name, base_asset_id, quote_asset_id,
                      price_decimals, qty_decimals, min_qty, status
               FROM symbols WHERE name = $1"#,
            name
        )
        .fetch_optional(pool)
        .await?;
        
        Ok(row)
    }
}
