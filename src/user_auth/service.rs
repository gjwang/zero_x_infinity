use anyhow::{Context, Result};
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use utoipa::ToSchema;

/// JWT Claims structure
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String, // Subject (user_id as string)
    pub exp: usize,  // Expiration time (as UTC timestamp)
    pub iat: usize,  // Issued at
}

/// User Registration Request
#[derive(Debug, Deserialize, ToSchema)]
pub struct RegisterRequest {
    #[schema(example = "user1")]
    pub username: String,
    #[schema(example = "user1@example.com")]
    pub email: String,
    #[schema(example = "password123")]
    pub password: String,
}

/// User Login Request
#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    #[schema(example = "user1@example.com")]
    pub email: String,
    #[schema(example = "password123")]
    pub password: String,
}

/// Auth Response (JWT)
#[derive(Debug, Serialize, ToSchema)]
pub struct AuthResponse {
    pub token: String,
    pub user_id: i64,
    pub username: String,
    pub email: String,
}

pub struct UserAuthService {
    db: Pool<Postgres>,
    jwt_secret: String,
}

impl UserAuthService {
    pub fn new(db: Pool<Postgres>, jwt_secret: String) -> Self {
        Self { db, jwt_secret }
    }

    /// Register a new user
    pub async fn register(&self, req: RegisterRequest) -> Result<i64> {
        // 1. Hash password
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(req.password.as_bytes(), &salt)
            .map_err(|e| anyhow::anyhow!("Hashing failed: {}", e))?
            .to_string();

        // 2. Insert into DB
        let rec = sqlx::query!(
            r#"
            INSERT INTO users_tb (username, email, password_hash, salt)
            VALUES ($1, $2, $3, $4)
            RETURNING user_id
            "#,
            req.username,
            req.email,
            password_hash,
            salt.as_str()
        )
        .fetch_one(&self.db)
        .await
        .context("Failed to insert user")?;

        Ok(rec.user_id)
    }

    /// Login user and issue JWT
    pub async fn login(&self, req: LoginRequest) -> Result<AuthResponse> {
        // 1. Find user by email
        let user = sqlx::query!(
            r#"
            SELECT user_id, username, email, password_hash
            FROM users_tb
            WHERE email = $1
            "#,
            req.email
        )
        .fetch_optional(&self.db)
        .await
        .context("DB query failed")?
        .ok_or_else(|| anyhow::anyhow!("Invalid email or password"))?;

        let password_hash_str = user
            .password_hash
            .ok_or_else(|| anyhow::anyhow!("User has no password set"))?;

        // 2. Verify password
        let parsed_hash = PasswordHash::new(&password_hash_str)
            .map_err(|e| anyhow::anyhow!("Invalid hash format: {}", e))?;

        Argon2::default()
            .verify_password(req.password.as_bytes(), &parsed_hash)
            .map_err(|_| anyhow::anyhow!("Invalid email or password"))?;

        // 3. Generate JWT
        let expiration = Utc::now()
            .checked_add_signed(Duration::hours(24))
            .expect("valid timestamp")
            .timestamp();

        let claims = Claims {
            sub: user.user_id.to_string(),
            exp: expiration as usize,
            iat: Utc::now().timestamp() as usize,
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )
        .context("Failed to generate token")?;

        Ok(AuthResponse {
            token,
            user_id: user.user_id,
            username: user.username,
            email: user.email.unwrap_or_default(),
        })
    }

    /// Verify JWT token
    pub fn verify_token(&self, token: &str) -> Result<Claims> {
        let decoding_key = DecodingKey::from_secret(self.jwt_secret.as_bytes());
        let validation = Validation::new(Algorithm::HS256);
        let token_data = decode::<Claims>(token, &decoding_key, &validation)?;
        Ok(token_data.claims)
    }

    /// Generate a new API Key (Ed25519)
    /// Returns (api_key, secret_key, label)
    pub async fn generate_api_key(&self, user_id: i64, label: String) -> Result<(String, String)> {
        use ed25519_dalek::SigningKey;
        use hex;
        use rand::rngs::OsRng;

        // 1. Generate Keypair
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let verifying_key = signing_key.verifying_key();

        // Secret Key (private) - to show user ONCE
        let secret_hex = hex::encode(signing_key.to_bytes());

        // Public Key (to store)
        let public_bytes = verifying_key.to_bytes(); // 32 bytes

        // 2. Generate API Key ID (AK_ + 16 hex)
        // We use part of public key hash or just random bytes for ID
        let id_bytes: [u8; 8] = rand::random();
        let api_key_id = format!("AK_{}", hex::encode(id_bytes).to_uppercase());

        // 3. Store in DB
        // api_keys_tb: key_id, user_id, api_key, key_type=1 (Ed25519), key_data, label
        sqlx::query!(
            r#"
            INSERT INTO api_keys_tb (user_id, api_key, key_type, key_data, label, permissions)
            VALUES ($1, $2, 1, $3, $4, 15)
            "#,
            user_id,
            api_key_id,
            &public_bytes[..], // Store raw bytes
            label
        )
        .execute(&self.db)
        .await
        .context("Failed to insert API key")?;

        Ok((api_key_id, secret_hex))
    }

    /// List API Keys
    pub async fn list_api_keys(&self, user_id: i64) -> Result<Vec<ApiKeyInfo>> {
        let rows = sqlx::query!(
            r#"
            SELECT api_key, label, created_at, status
            FROM api_keys_tb
            WHERE user_id = $1
            ORDER BY created_at DESC
            "#,
            user_id
        )
        .fetch_all(&self.db)
        .await
        .context("Failed to list keys")?;

        Ok(rows
            .into_iter()
            .map(|r| ApiKeyInfo {
                api_key: r.api_key,
                label: r.label,
                created_at: r.created_at.map(|t| t.to_rfc3339()).unwrap_or_default(),
                status: r.status,
            })
            .collect())
    }

    /// Delete API Key
    pub async fn delete_api_key(&self, user_id: i64, api_key: String) -> Result<()> {
        let res = sqlx::query!(
            r#"
            DELETE FROM api_keys_tb
            WHERE user_id = $1 AND api_key = $2
            "#,
            user_id,
            api_key
        )
        .execute(&self.db)
        .await
        .context("Failed to delete key")?;

        if res.rows_affected() == 0 {
            return Err(anyhow::anyhow!("Key not found"));
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApiKeyInfo {
    pub api_key: String,
    pub label: Option<String>,
    pub created_at: String,
    pub status: i16,
}
