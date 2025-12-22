//! Repository layer for database operations

use super::models::{Asset, Symbol, User, UserStatus};
use sqlx::{PgPool, Row};

/// User repository for CRUD operations
pub struct UserRepository;

impl UserRepository {
    /// Get user by ID
    pub async fn get_by_id(pool: &PgPool, user_id: i64) -> Result<Option<User>, sqlx::Error> {
        let row = sqlx::query(
            r#"SELECT user_id, username, email, status, user_flags, created_at 
               FROM users WHERE user_id = $1"#,
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| User {
            user_id: r.get("user_id"),
            username: r.get("username"),
            email: r.get("email"),
            status: UserStatus::from(r.get::<i16, _>("status")),
            user_flags: r.get("user_flags"),
            created_at: r.get("created_at"),
        }))
    }

    /// Get user by username
    pub async fn get_by_username(
        pool: &PgPool,
        username: &str,
    ) -> Result<Option<User>, sqlx::Error> {
        let row = sqlx::query(
            r#"SELECT user_id, username, email, status, user_flags, created_at 
               FROM users WHERE username = $1"#,
        )
        .bind(username)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(|r| User {
            user_id: r.get("user_id"),
            username: r.get("username"),
            email: r.get("email"),
            status: UserStatus::from(r.get::<i16, _>("status")),
            user_flags: r.get("user_flags"),
            created_at: r.get("created_at"),
        }))
    }

    /// Create a new user
    pub async fn create(
        pool: &PgPool,
        username: &str,
        email: Option<&str>,
    ) -> Result<i64, sqlx::Error> {
        let row =
            sqlx::query(r#"INSERT INTO users (username, email) VALUES ($1, $2) RETURNING user_id"#)
                .bind(username)
                .bind(email)
                .fetch_one(pool)
                .await?;

        Ok(row.get("user_id"))
    }
}

/// Asset manager for loading and caching assets
pub struct AssetManager;

impl AssetManager {
    /// Load all active assets
    pub async fn load_all(pool: &PgPool) -> Result<Vec<Asset>, sqlx::Error> {
        let rows: Vec<Asset> = sqlx::query_as(
            r#"SELECT asset_id, asset, name, decimals, status, asset_flags 
               FROM assets WHERE status = 1"#,
        )
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// Get asset by ID
    pub async fn get_by_id(pool: &PgPool, asset_id: i32) -> Result<Option<Asset>, sqlx::Error> {
        let row: Option<Asset> = sqlx::query_as(
            r#"SELECT asset_id, asset, name, decimals, status, asset_flags 
               FROM assets WHERE asset_id = $1"#,
        )
        .bind(asset_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// Get asset by asset code (e.g., "BTC", "USDT")
    pub async fn get_by_asset(pool: &PgPool, asset: &str) -> Result<Option<Asset>, sqlx::Error> {
        let row: Option<Asset> = sqlx::query_as(
            r#"SELECT asset_id, asset, name, decimals, status, asset_flags 
               FROM assets WHERE asset = $1"#,
        )
        .bind(asset)
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
        let rows: Vec<Symbol> = sqlx::query_as(
            r#"SELECT symbol_id, symbol, base_asset_id, quote_asset_id, 
                      price_decimals, qty_decimals, min_qty, status, symbol_flags 
               FROM symbols WHERE status = 1"#,
        )
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// Get symbol by ID
    pub async fn get_by_id(pool: &PgPool, symbol_id: i32) -> Result<Option<Symbol>, sqlx::Error> {
        let row: Option<Symbol> = sqlx::query_as(
            r#"SELECT symbol_id, symbol, base_asset_id, quote_asset_id,
                      price_decimals, qty_decimals, min_qty, status, symbol_flags
               FROM symbols WHERE symbol_id = $1"#,
        )
        .bind(symbol_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// Get symbol by symbol name
    pub async fn get_by_symbol(pool: &PgPool, symbol: &str) -> Result<Option<Symbol>, sqlx::Error> {
        let row: Option<Symbol> = sqlx::query_as(
            r#"SELECT symbol_id, symbol, base_asset_id, quote_asset_id,
                      price_decimals, qty_decimals, min_qty, status, symbol_flags
               FROM symbols WHERE symbol = $1"#,
        )
        .bind(symbol)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::Database;

    const TEST_DATABASE_URL: &str = "postgresql://trading:trading123@localhost:5432/trading";

    #[tokio::test]
    #[ignore] // Requires PostgreSQL with seed data
    async fn test_asset_manager_load_all() {
        let db = Database::connect(TEST_DATABASE_URL)
            .await
            .expect("Failed to connect");

        let assets = AssetManager::load_all(db.pool()).await;
        assert!(assets.is_ok(), "Should load assets successfully");

        let assets = assets.unwrap();
        assert!(!assets.is_empty(), "Should have at least one asset");
        assert!(assets.iter().any(|a| a.asset == "BTC"), "Should have BTC");
    }

    #[tokio::test]
    #[ignore]
    async fn test_asset_manager_get_by_asset() {
        let db = Database::connect(TEST_DATABASE_URL)
            .await
            .expect("Failed to connect");

        let asset = AssetManager::get_by_asset(db.pool(), "BTC").await;
        assert!(asset.is_ok(), "Should query asset successfully");

        let asset = asset.unwrap();
        assert!(asset.is_some(), "BTC should exist");
        assert_eq!(asset.unwrap().asset, "BTC");
    }

    #[tokio::test]
    #[ignore]
    async fn test_symbol_manager_load_all() {
        let db = Database::connect(TEST_DATABASE_URL)
            .await
            .expect("Failed to connect");

        let symbols = SymbolManager::load_all(db.pool()).await;
        assert!(symbols.is_ok(), "Should load symbols successfully");

        let symbols = symbols.unwrap();
        assert!(!symbols.is_empty(), "Should have at least one symbol");
        assert!(
            symbols.iter().any(|s| s.symbol == "BTC_USDT"),
            "Should have BTC_USDT"
        );
    }

    #[tokio::test]
    #[ignore]
    async fn test_symbol_manager_get_by_symbol() {
        let db = Database::connect(TEST_DATABASE_URL)
            .await
            .expect("Failed to connect");

        let symbol = SymbolManager::get_by_symbol(db.pool(), "BTC_USDT").await;
        assert!(symbol.is_ok(), "Should query symbol successfully");

        let symbol = symbol.unwrap();
        assert!(symbol.is_some(), "BTC_USDT should exist");
        assert_eq!(symbol.unwrap().symbol, "BTC_USDT");
    }

    #[tokio::test]
    #[ignore]
    async fn test_user_repository_create_and_get() {
        let db = Database::connect(TEST_DATABASE_URL)
            .await
            .expect("Failed to connect");

        // Create a test user
        let username = format!("test_user_{}", chrono::Utc::now().timestamp());
        let user_id = UserRepository::create(db.pool(), &username, Some("test@example.com"))
            .await
            .expect("Should create user");

        assert!(user_id > 0, "User ID should be positive");

        // Get user by ID
        let user = UserRepository::get_by_id(db.pool(), user_id)
            .await
            .expect("Should query user");

        assert!(user.is_some(), "User should exist");
        let user = user.unwrap();
        assert_eq!(user.username, username);
        assert_eq!(user.email, Some("test@example.com".to_string()));

        // Get user by username
        let user2 = UserRepository::get_by_username(db.pool(), &username)
            .await
            .expect("Should query user");

        assert!(user2.is_some(), "User should exist");
        assert_eq!(user2.unwrap().user_id, user_id);
    }

    #[tokio::test]
    #[ignore]
    async fn test_asset_manager_get_by_id_not_found() {
        let db = Database::connect(TEST_DATABASE_URL)
            .await
            .expect("Failed to connect");

        let result = AssetManager::get_by_id(db.pool(), 99999).await;
        assert!(result.is_ok());
        assert!(
            result.unwrap().is_none(),
            "Should return None for non-existent asset"
        );
    }

    #[tokio::test]
    #[ignore]
    async fn test_symbol_manager_get_by_id_not_found() {
        let db = Database::connect(TEST_DATABASE_URL)
            .await
            .expect("Failed to connect");

        let result = SymbolManager::get_by_id(db.pool(), 99999).await;
        assert!(result.is_ok());
        assert!(
            result.unwrap().is_none(),
            "Should return None for non-existent symbol"
        );
    }

    #[tokio::test]
    #[ignore]
    async fn test_user_repository_get_by_username_not_found() {
        let db = Database::connect(TEST_DATABASE_URL)
            .await
            .expect("Failed to connect");

        let result = UserRepository::get_by_username(db.pool(), "nonexistent_user_12345").await;
        assert!(result.is_ok());
        assert!(
            result.unwrap().is_none(),
            "Should return None for non-existent user"
        );
    }
}
