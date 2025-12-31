use axum::{Json, extract::State, http::StatusCode};
use std::sync::Arc;

use super::auth_service::{AuthResponse, LoginRequest, RegisterRequest};
use crate::gateway::types::error_codes;
use crate::gateway::{state::AppState, types::ApiResponse};

/// Register a new user
///
/// POST /api/v1/auth/register
#[utoipa::path(
    post,
    path = "/api/v1/auth/register",
    request_body = RegisterRequest,
    responses(
        (status = 201, description = "User registered successfully", body = ApiResponse<i64>),
        (status = 400, description = "Invalid input or user already exists"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Auth"
)]
pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<ApiResponse<i64>>), (StatusCode, Json<ApiResponse<()>>)> {
    // Validate input (basic check)
    if req.email.is_empty() || req.password.len() < 8 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error(
                error_codes::INVALID_PARAMETER,
                "Invalid email or password (min 8 chars)",
            )),
        ));
    }

    let user_auth = state.user_auth.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ApiResponse::<()>::error(
            error_codes::INTERNAL_ERROR,
            "Auth service unavailable",
        )),
    ))?;

    match user_auth.register(req).await {
        Ok(user_id) => Ok((StatusCode::CREATED, Json(ApiResponse::success(user_id)))),
        Err(e) => {
            let err_msg = e.to_string();
            if err_msg.contains("duplicate key") {
                tracing::warn!("Registration attempt for existing user: {}", err_msg);
                Err((
                    StatusCode::CONFLICT,
                    Json(ApiResponse::<()>::error(
                        error_codes::INVALID_PARAMETER,
                        "Username or Email already exists",
                    )),
                ))
            } else {
                tracing::error!("Registration failed: {:?}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse::<()>::error(
                        error_codes::INTERNAL_ERROR,
                        "Registration failed",
                    )),
                ))
            }
        }
    }
}

/// Login user
///
/// POST /api/v1/auth/login
#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Login successful", body = ApiResponse<AuthResponse>),
        (status = 401, description = "Invalid credentials"),
        (status = 500, description = "Internal server error")
    ),
    tag = "Auth"
)]
pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Result<(StatusCode, Json<ApiResponse<AuthResponse>>), (StatusCode, Json<ApiResponse<()>>)> {
    let user_auth = state.user_auth.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ApiResponse::<()>::error(
            error_codes::INTERNAL_ERROR,
            "Auth service unavailable",
        )),
    ))?;

    match user_auth.login(req).await {
        Ok(resp) => Ok((StatusCode::OK, Json(ApiResponse::success(resp)))),
        Err(e) => {
            tracing::warn!("Login failed: {:?}", e);
            Err((
                StatusCode::UNAUTHORIZED,
                Json(ApiResponse::<()>::error(
                    error_codes::AUTH_FAILED,
                    "Invalid email or password",
                )),
            ))
        }
    }
}

use serde::{Deserialize, Serialize};

/// Create API Key Request
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CreateApiKeyRequest {
    #[schema(example = "Trading Bot 1")]
    pub label: String,
}

/// Create API Key Response
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct CreateApiKeyResponse {
    pub api_key: String,
    pub api_secret: String,
}

/// Generate new API Key
///
/// POST /api/v1/user/apikeys
#[utoipa::path(
    post,
    path = "/api/v1/user/apikeys",
    request_body = CreateApiKeyRequest,
    responses(
        (status = 201, description = "API Key generated", body = ApiResponse<CreateApiKeyResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    ),
    tag = "User"
)]
pub async fn create_api_key(
    State(state): State<Arc<AppState>>,
    axum::Extension(claims): axum::Extension<super::auth_service::Claims>,
    Json(req): Json<CreateApiKeyRequest>,
) -> Result<
    (StatusCode, Json<ApiResponse<CreateApiKeyResponse>>),
    (StatusCode, Json<ApiResponse<()>>),
> {
    let user_id = claims.sub.parse::<i64>().unwrap_or_default();

    let user_auth = state.user_auth.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ApiResponse::<()>::error(
            error_codes::INTERNAL_ERROR,
            "Auth service unavailable",
        )),
    ))?;

    match user_auth.generate_api_key(user_id, req.label).await {
        Ok((api_key, api_secret)) => Ok((
            StatusCode::CREATED,
            Json(ApiResponse::success(CreateApiKeyResponse {
                api_key,
                api_secret,
            })),
        )),
        Err(e) => {
            tracing::error!("Failed to generate API Key: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()>::error(
                    error_codes::INTERNAL_ERROR,
                    "Failed to generate API Key",
                )),
            ))
        }
    }
}

/// List API Keys
///
/// GET /api/v1/user/apikeys
#[utoipa::path(
    get,
    path = "/api/v1/user/apikeys",
    responses(
        (status = 200, description = "List of API Keys", body = ApiResponse<Vec<super::auth_service::ApiKeyInfo>>),
        (status = 401, description = "Unauthorized")
    ),
    tag = "User"
)]
pub async fn list_api_keys(
    State(state): State<Arc<AppState>>,
    axum::Extension(claims): axum::Extension<super::auth_service::Claims>,
) -> Result<
    (
        StatusCode,
        Json<ApiResponse<Vec<super::auth_service::ApiKeyInfo>>>,
    ),
    (StatusCode, Json<ApiResponse<()>>),
> {
    let user_id = claims.sub.parse::<i64>().unwrap_or_default();

    let user_auth = state.user_auth.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ApiResponse::<()>::error(
            error_codes::INTERNAL_ERROR,
            "Auth service unavailable",
        )),
    ))?;

    match user_auth.list_api_keys(user_id).await {
        Ok(keys) => Ok((StatusCode::OK, Json(ApiResponse::success(keys)))),
        Err(e) => {
            tracing::error!("Failed to list API Keys: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()>::error(
                    error_codes::INTERNAL_ERROR,
                    "Failed to list API Keys",
                )),
            ))
        }
    }
}

/// Delete API Key
///
/// DELETE /api/v1/user/apikeys/{api_key}
#[utoipa::path(
    delete,
    path = "/api/v1/user/apikeys/{api_key}",
    params(
        ("api_key" = String, Path, description = "API Key to delete")
    ),
    responses(
        (status = 200, description = "API Key deleted"),
        (status = 404, description = "API Key not found"),
        (status = 401, description = "Unauthorized")
    ),
    tag = "User"
)]
pub async fn delete_api_key(
    State(state): State<Arc<AppState>>,
    axum::Extension(claims): axum::Extension<super::auth_service::Claims>,
    axum::extract::Path(api_key): axum::extract::Path<String>,
) -> Result<(StatusCode, Json<ApiResponse<()>>), (StatusCode, Json<ApiResponse<()>>)> {
    let user_id = claims.sub.parse::<i64>().unwrap_or_default();

    let user_auth = state.user_auth.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ApiResponse::<()>::error(
            error_codes::INTERNAL_ERROR,
            "Auth service unavailable",
        )),
    ))?;

    match user_auth.delete_api_key(user_id, api_key).await {
        Ok(_) => Ok((StatusCode::OK, Json(ApiResponse::success(())))),
        Err(e) => {
            // Check if not found
            if e.to_string().contains("not found") {
                Err((
                    StatusCode::NOT_FOUND,
                    Json(ApiResponse::<()>::error(
                        error_codes::INVALID_PARAMETER,
                        "API Key not found",
                    )),
                ))
            } else {
                tracing::error!("Failed to delete API Key: {:?}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse::<()>::error(
                        error_codes::INTERNAL_ERROR,
                        "Failed to delete API Key",
                    )),
                ))
            }
        }
    }
}
