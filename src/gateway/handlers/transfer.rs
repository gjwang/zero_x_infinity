//! Transfer-related handlers (internal transfers, FSM-based)

use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Path, State},
};

use super::super::state::AppState;
use super::super::types::{ApiError, ApiResult, ok};

/// Create internal transfer endpoint
///
/// POST /api/v1/private/transfer
///
/// Uses FSM-based transfer when coordinator is available, otherwise falls back to legacy.
#[utoipa::path(
    post,
    path = "/api/v1/private/transfer",
    request_body(content = String, description = "Transfer request: from, to, asset, amount", content_type = "application/json"),
    responses(
        (status = 200, description = "Transfer completed", content_type = "application/json"),
        (status = 400, description = "Invalid parameters or insufficient balance"),
        (status = 401, description = "Authentication failed"),
        (status = 503, description = "Service unavailable")
    ),
    security(("ed25519_auth" = [])),
    tag = "Transfer"
)]
pub async fn create_transfer(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::api_auth::AuthenticatedUser>,
    Json(req): Json<crate::funding::transfer::TransferRequest>,
) -> ApiResult<crate::funding::transfer::TransferResponse> {
    // 1. Extract user_id from authenticated user
    let user_id = user.user_id as u64;
    tracing::info!("[TRACE] Transfer Request: From User {}", user_id);

    // 2. Check if FSM coordinator is available
    if let Some(ref coordinator) = state.transfer_coordinator {
        // Use new FSM-based transfer
        return create_transfer_fsm_handler(state.clone(), coordinator, user_id, req).await;
    }

    // 3. Fallback to legacy transfer service
    let db = state
        .pg_db
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("Database not available"))?;

    match crate::funding::transfer_service::TransferService::execute(db, user_id as i64, req).await
    {
        Ok(resp) => ok(resp),
        Err(e) => {
            let err_msg = e.to_string();
            tracing::error!("Transfer failed: {}", err_msg);
            ApiError::bad_request(err_msg).into_err()
        }
    }
}

/// FSM-based transfer handler (internal)
pub(crate) async fn create_transfer_fsm_handler(
    state: Arc<AppState>,
    coordinator: &std::sync::Arc<crate::internal_transfer::TransferCoordinator>,
    user_id: u64,
    req: crate::funding::transfer::TransferRequest,
) -> ApiResult<crate::funding::transfer::TransferResponse> {
    // Convert legacy request to FSM request
    tracing::info!(
        "[DEBUG] FSM Handler Raw Request: from='{}' to='{}' asset='{}' amount='{}' cid={:?}",
        req.from,
        req.to,
        req.asset,
        req.amount,
        req.cid
    );

    let fsm_req = crate::internal_transfer::TransferApiRequest {
        from: req.from.clone(),
        to: req.to.clone(),
        asset: req.asset.clone(),
        amount: req.amount.clone(),
        cid: req.cid.clone(), // Pass through client idempotency key
    };

    // Lookup asset and create validation info
    let asset = state
        .pg_assets
        .iter()
        .find(|a| a.asset.to_lowercase() == req.asset.to_lowercase())
        .ok_or_else(|| ApiError::bad_request(format!("Asset not found: {}", req.asset)))?;

    // Create AssetValidationInfo for security checks
    let asset_info = crate::internal_transfer::AssetValidationInfo::from_asset(asset);

    // Call FSM transfer with full asset validation
    match crate::internal_transfer::create_transfer_fsm(coordinator, user_id, fsm_req, asset_info)
        .await
    {
        Ok(fsm_resp) => {
            // Convert FSM response to legacy response
            let legacy_resp = crate::funding::transfer::TransferResponse {
                transfer_id: fsm_resp.transfer_id,
                status: fsm_resp.status,
                from: fsm_resp.from,
                to: fsm_resp.to,
                asset: fsm_resp.asset,
                amount: fsm_resp.amount,
                timestamp: fsm_resp.timestamp,
            };
            ok(legacy_resp)
        }
        Err((status, err_resp)) => {
            ApiError::new(status, err_resp.code, err_resp.msg.unwrap_or_default()).into_err()
        }
    }
}

/// Get transfer status endpoint
///
/// GET /api/v1/private/transfer/{req_id}
#[utoipa::path(
    get,
    path = "/api/v1/private/transfer/{req_id}",
    params(
        ("req_id" = String, Path, description = "Transfer request ID (ULID format)")
    ),
    responses(
        (status = 200, description = "Transfer status", content_type = "application/json"),
        (status = 400, description = "Invalid request ID format"),
        (status = 404, description = "Transfer not found"),
        (status = 503, description = "Service unavailable")
    ),
    security(("ed25519_auth" = [])),
    tag = "Transfer"
)]
pub async fn get_transfer(
    State(state): State<Arc<AppState>>,
    Path(req_id_str): Path<String>,
) -> ApiResult<crate::internal_transfer::TransferApiResponse> {
    // Parse req_id from string (ULID format)
    let req_id: crate::internal_transfer::InternalTransferId = req_id_str
        .parse()
        .map_err(|_| ApiError::bad_request("Invalid request ID format"))?;

    // Check if FSM coordinator is available
    let coordinator = state.transfer_coordinator.as_ref().ok_or_else(|| {
        ApiError::service_unavailable("Transfer service not available (FSM not enabled)")
    })?;

    // Get decimals from first asset (default 8) - in production, would lookup from transfer record
    let decimals = state
        .pg_assets
        .first()
        .map(|a| a.internal_scale as u32)
        .ok_or_else(|| ApiError::internal("No assets configured for scaling"))?;

    // Query transfer status
    match crate::internal_transfer::get_transfer_status(coordinator, req_id, decimals).await {
        Ok(resp) => ok(resp),
        Err((status, err_resp)) => {
            ApiError::new(status, err_resp.code, err_resp.msg.unwrap_or_default()).into_err()
        }
    }
}
