//! Database connection management

use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;

/// PostgreSQL database connection pool
pub struct Database {
    pool: PgPool,
}

impl Database {
    /// Create a new database connection pool
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(50)
            .acquire_timeout(Duration::from_secs(5))
            .connect(database_url)
            .await?;

        tracing::info!("PostgreSQL connection pool established");
        Ok(Self { pool })
    }

    /// Get a reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Check database health
    pub async fn health_check(&self) -> Result<(), sqlx::Error> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    /// Load VIP levels for all users from users_tb
    /// Returns Vec<(user_id, vip_level)>
    pub async fn load_user_vip_levels(&self) -> Result<Vec<(u64, u8)>, sqlx::Error> {
        let rows = sqlx::query("SELECT user_id, vip_level FROM users_tb WHERE vip_level > 0")
            .fetch_all(&self.pool)
            .await?;

        let result: Vec<(u64, u8)> = rows
            .iter()
            .filter_map(|row| {
                let user_id: i64 = row.try_get_log("user_id")?;
                let vip_level: i16 = row.try_get_log("vip_level").unwrap_or(0); // SAFE_DEFAULT: non-VIP users have level 0
                Some((user_id as u64, vip_level as u8))
            })
            .collect();

        tracing::info!("Loaded {} users with VIP levels from DB", result.len());
        Ok(result)
    }
}

/// Extension trait for safe row access with logging
pub trait SafeRow {
    /// Try to get a value from a column, log error and return None if it fails
    fn try_get_log<'r, T>(&'r self, column: &str) -> Option<T>
    where
        T: sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres>;
}

impl SafeRow for sqlx::postgres::PgRow {
    fn try_get_log<'r, T>(&'r self, column: &str) -> Option<T>
    where
        T: sqlx::Decode<'r, sqlx::Postgres> + sqlx::Type<sqlx::Postgres>,
    {
        use sqlx::Row;
        match self.try_get(column) {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::error!("Failed to read column '{}': {}", column, e);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require a running PostgreSQL instance
    // Run with: docker-compose up -d postgres

    const TEST_DATABASE_URL: &str =
        "postgresql://trading:trading123@localhost:5432/exchange_info_db";

    #[tokio::test]
    #[ignore] // Requires PostgreSQL running
    async fn test_database_connect_success() {
        let db = Database::connect(TEST_DATABASE_URL).await;
        assert!(db.is_ok(), "Should connect to PostgreSQL successfully");
    }

    #[tokio::test]
    #[ignore]
    async fn test_database_connect_invalid_url() {
        let db = Database::connect("postgresql://invalid:invalid@localhost:9999/invalid").await;
        assert!(db.is_err(), "Should fail with invalid connection string");
    }

    #[tokio::test]
    #[ignore]
    async fn test_database_health_check() {
        let db = Database::connect(TEST_DATABASE_URL)
            .await
            .expect("Failed to connect");

        let health = db.health_check().await;
        assert!(health.is_ok(), "Health check should pass");
    }
}
