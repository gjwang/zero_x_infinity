//! Repository layer for user account operations

use super::models::{User, UserStatus};
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

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_DATABASE_URL: &str = "postgresql://trading:trading123@localhost:5432/trading";

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
