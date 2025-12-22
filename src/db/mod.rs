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
            .max_connections(10)
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
