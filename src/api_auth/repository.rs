//! API Key repository for database operations.
//!
//! Provides async database access for API Key management.
//! Uses runtime queries to avoid sqlx compile-time database connection.

use super::models::ApiKeyRecord;
use crate::account::Database;
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

        if let Some(r) = row {
            Ok(Some(Self::row_to_record(&r)?))
        } else {
            Ok(None)
        }
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

        if let Some(r) = row {
            Ok(Some(Self::row_to_record(&r)?))
        } else {
            Ok(None)
        }
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

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(Self::row_to_record(&row)?);
        }
        Ok(out)
    }

    /// Convert a database row to ApiKeyRecord.
    fn row_to_record(row: &sqlx::postgres::PgRow) -> Result<ApiKeyRecord, sqlx::Error> {
        use crate::db::SafeRow;
        Ok(ApiKeyRecord {
            key_id: row
                .try_get_log("key_id")
                .ok_or_else(|| sqlx::Error::ColumnNotFound("key_id".into()))?,
            user_id: row
                .try_get_log("user_id")
                .ok_or_else(|| sqlx::Error::ColumnNotFound("user_id".into()))?,
            api_key: row
                .try_get_log("api_key")
                .ok_or_else(|| sqlx::Error::ColumnNotFound("api_key".into()))?,
            key_type: row
                .try_get_log("key_type")
                .ok_or_else(|| sqlx::Error::ColumnNotFound("key_type".into()))?,
            key_data: row
                .try_get_log("key_data")
                .ok_or_else(|| sqlx::Error::ColumnNotFound("key_data".into()))?,
            label: row.try_get_log("label"),
            permissions: row
                .try_get_log("permissions")
                .ok_or_else(|| sqlx::Error::ColumnNotFound("permissions".into()))?,
            status: row
                .try_get_log("status")
                .ok_or_else(|| sqlx::Error::ColumnNotFound("status".into()))?,
            last_ts_nonce: row
                .try_get_log("last_ts_nonce")
                .ok_or_else(|| sqlx::Error::ColumnNotFound("last_ts_nonce".into()))?,
        })
    }
}
