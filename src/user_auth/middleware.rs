use axum::{
    Json,
    body::Body,
    extract::State,
    http::{Request, StatusCode, header},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

use crate::gateway::{
    state::AppState,
    types::{ApiResponse, error_codes},
};

pub async fn jwt_auth_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request<Body>,
    next: Next,
) -> Result<Response, (StatusCode, Json<ApiResponse<()>>)> {
    // 1. Extract Authorization header
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .ok_or((
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<()>::error(
                error_codes::MISSING_AUTH,
                "Missing Authorization header",
            )),
        ))?;

    if !auth_header.starts_with("Bearer ") {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<()>::error(
                error_codes::AUTH_FAILED,
                "Invalid token format",
            )),
        ));
    }

    let token = &auth_header[7..];

    // 2. Verify Token
    // We need the secret. It's inside UserAuthService but private.
    // Hack: We can expose it or share it.
    // Or, since AppState has UserAuthService, we can add a getter or public field?
    // UserAuthService struct definition in service.rs: `jwt_secret` is private.
    // I will assume I can access it or I need to make it public.
    // Let's modify service.rs to make `jwt_secret` pub(crate) or add a getter.
    // For now, assume a getter `jwt_secret()` exists or add it.

    // Better: Validation logic belongs in Service.
    // state.user_auth.verify_token(token)?

    let user_auth = state.user_auth.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ApiResponse::<()>::error(
            error_codes::INTERNAL_ERROR,
            "Auth service unavailable",
        )),
    ))?;

    match user_auth.verify_token(token) {
        Ok(claims) => {
            // 3. Inject User ID
            request.extensions_mut().insert(claims);
            Ok(next.run(request).await)
        }
        Err(_) => Err((
            StatusCode::UNAUTHORIZED,
            Json(ApiResponse::<()>::error(
                error_codes::AUTH_FAILED,
                "Invalid or expired token",
            )),
        )),
    }
}
