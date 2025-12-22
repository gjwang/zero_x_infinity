//! API Key repository for database operations.
//!
//! Provides async database access for API Key management.
//! Uses runtime queries to avoid sqlx compile-time database connection.

use super::models::ApiKeyRecord;
use crate::account::Database;
use sqlx::Row;
use std::sync::Arc;

/// API Key repository for database operations.
pub struct ApiKeyRepository {
    db: Arc<Database>,
}

impl ApiKeyRepository {
    /// Create a new repository with database connection.
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    /// Get an active API Key by its string value.
    ///
    /// Returns `None` if the key doesn't exist or is disabled.
    pub async fn get_active_by_key(
        &self,
        api_key: &str,
    ) -> Result<Option<ApiKeyRecord>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT key_id, user_id, api_key, key_type, key_data, label, 
                   permissions, status, last_ts_nonce
            FROM api_keys_tb 
            WHERE api_key = $1 AND status = 1
            "#,
        )
        .bind(api_key)
        .fetch_optional(self.db.pool())
        .await?;

        Ok(row.map(|r| Self::row_to_record(&r)))
    }

    /// Update the last_ts_nonce for an API Key.
    ///
    /// Uses atomic update to prevent race conditions.
    pub async fn update_ts_nonce(&self, api_key: &str, ts_nonce: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query(
            r#"
            UPDATE api_keys_tb 
            SET last_ts_nonce = $2, last_used_at = NOW()
            WHERE api_key = $1 AND last_ts_nonce < $2
            "#,
        )
        .bind(api_key)
        .bind(ts_nonce)
        .execute(self.db.pool())
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Get API Key by key_id.
    pub async fn get_by_id(&self, key_id: i32) -> Result<Option<ApiKeyRecord>, sqlx::Error> {
        let row = sqlx::query(
            r#"
            SELECT key_id, user_id, api_key, key_type, key_data, label, 
                   permissions, status, last_ts_nonce
            FROM api_keys_tb 
            WHERE key_id = $1
            "#,
        )
        .bind(key_id)
        .fetch_optional(self.db.pool())
        .await?;

        Ok(row.map(|r| Self::row_to_record(&r)))
    }

    /// List all API Keys for a user.
    pub async fn list_by_user(&self, user_id: i64) -> Result<Vec<ApiKeyRecord>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT key_id, user_id, api_key, key_type, key_data, label, 
                   permissions, status, last_ts_nonce
            FROM api_keys_tb 
            WHERE user_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(self.db.pool())
        .await?;

        Ok(rows.iter().map(Self::row_to_record).collect())
    }

    /// Convert a database row to ApiKeyRecord.
    fn row_to_record(row: &sqlx::postgres::PgRow) -> ApiKeyRecord {
        ApiKeyRecord {
            key_id: row.get("key_id"),
            user_id: row.get("user_id"),
            api_key: row.get("api_key"),
            key_type: row.get("key_type"),
            key_data: row.get("key_data"),
            label: row.get("label"),
            permissions: row.get("permissions"),
            status: row.get("status"),
            last_ts_nonce: row.get("last_ts_nonce"),
        }
    }
}
