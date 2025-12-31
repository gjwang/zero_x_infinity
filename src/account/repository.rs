//! Repository layer for user account operations

use super::models::{User, UserStatus};
use sqlx::PgPool;

/// User repository for CRUD operations
pub struct UserRepository;

impl UserRepository {
    /// Get user by ID
    pub async fn get_by_id(pool: &PgPool, user_id: i64) -> Result<Option<User>, sqlx::Error> {
        let row = sqlx::query(
            r#"SELECT user_id, username, email, status, user_flags, created_at 
               FROM users_tb WHERE user_id = $1"#,
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;

        if let Some(r) = row {
            use crate::db::SafeRow;
            Ok(Some(User {
                user_id: r
                    .try_get_log("user_id")
                    .ok_or_else(|| sqlx::Error::ColumnNotFound("user_id".into()))?,
                username: r
                    .try_get_log("username")
                    .ok_or_else(|| sqlx::Error::ColumnNotFound("username".into()))?,
                email: r.try_get_log("email"),
                status: UserStatus::from(
                    r.try_get_log::<i16>("status")
                        .ok_or_else(|| sqlx::Error::ColumnNotFound("status".into()))?,
                ),
                user_flags: r
                    .try_get_log("user_flags")
                    .ok_or_else(|| sqlx::Error::ColumnNotFound("user_flags".into()))?,
                created_at: r
                    .try_get_log::<chrono::DateTime<chrono::Utc>>("created_at")
                    .ok_or_else(|| sqlx::Error::ColumnNotFound("created_at".into()))?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get user by username
    pub async fn get_by_username(
        pool: &PgPool,
        username: &str,
    ) -> Result<Option<User>, sqlx::Error> {
        let row = sqlx::query(
            r#"SELECT user_id, username, email, status, user_flags, created_at 
               FROM users_tb WHERE username = $1"#,
        )
        .bind(username)
        .fetch_optional(pool)
        .await?;

        if let Some(r) = row {
            use crate::db::SafeRow;
            Ok(Some(User {
                user_id: r
                    .try_get_log("user_id")
                    .ok_or_else(|| sqlx::Error::ColumnNotFound("user_id".into()))?,
                username: r
                    .try_get_log("username")
                    .ok_or_else(|| sqlx::Error::ColumnNotFound("username".into()))?,
                email: r.try_get_log("email"),
                status: UserStatus::from(
                    r.try_get_log::<i16>("status")
                        .ok_or_else(|| sqlx::Error::ColumnNotFound("status".into()))?,
                ),
                user_flags: r
                    .try_get_log("user_flags")
                    .ok_or_else(|| sqlx::Error::ColumnNotFound("user_flags".into()))?,
                created_at: r
                    .try_get_log::<chrono::DateTime<chrono::Utc>>("created_at")
                    .ok_or_else(|| sqlx::Error::ColumnNotFound("created_at".into()))?,
            }))
        } else {
            Ok(None)
        }
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

        use crate::db::SafeRow;
        row.try_get_log("user_id")
            .ok_or_else(|| sqlx::Error::ColumnNotFound("user_id".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_DATABASE_URL: &str =
        "postgresql://trading:trading123@localhost:5432/exchange_info_db";

    #[tokio::test]
    #[ignore] // Requires PostgreSQL running
    async fn test_user_repository_get_by_username_not_found() {
        let db = crate::db::Database::connect(TEST_DATABASE_URL)
            .await
            .expect("Failed to connect");

        let result = UserRepository::get_by_username(db.pool(), "nonexistent_user").await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}
