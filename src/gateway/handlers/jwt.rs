//! JWT-authenticated handlers (Phase 0x11-b compatible)

use std::sync::Arc;

use axum::{
    Extension, Json,
    extract::{Query, State},
    http::StatusCode,
};

use crate::pipeline::OrderAction;

use super::super::state::AppState;
use super::super::types::{
    AccountResponseData, ApiError, ApiResult, CancelOrderRequest, ClientOrder, OrderResponseData,
    ok,
};
use super::helpers::{now_ms, now_ns};
use super::transfer::create_transfer_fsm_handler;

/// Account info for JWT auth
pub async fn get_account_jwt(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<crate::user_auth::auth_service::Claims>,
) -> ApiResult<AccountResponseData> {
    let user_id = claims
        .sub
        .parse::<i64>()
        .map_err(|_| ApiError::unauthorized("Invalid user ID in token"))?;
    tracing::info!("DEBUG: get_account_jwt called for user_id: {}", user_id);

    // 1. Get Funding balances from Postgres
    let pg_db = state
        .pg_db
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("Account database not available"))?;

    let mut balances = match crate::funding::transfer_service::TransferService::get_all_balances(
        pg_db.pool(),
        user_id,
    )
    .await
    {
        Ok(b) => b,
        Err(e) => {
            return ApiError::db_error(format!("Postgres query failed: {}", e)).into_err();
        }
    };

    // Filter out any "spot" records from Postgres
    balances.retain(|b| b.account_type != "spot");

    // 2. Get Spot balances from TDengine
    if let Some(ref t_client) = state.db_client {
        match crate::persistence::queries::query_all_balances(
            t_client.taos(),
            user_id as u64,
            &state.symbol_mgr,
        )
        .await
        {
            Ok(spot_balances) => {
                for sb in spot_balances {
                    balances.push(crate::funding::transfer_service::BalanceInfo {
                        asset_id: 0,
                        asset: sb.asset,
                        account_type: "spot".to_string(),
                        available: sb.avail,
                        frozen: sb.frozen,
                    });
                }
            }
            Err(e) => {
                tracing::error!("TDengine Spot balance query failed: {}", e);
            }
        }
    }

    let data = AccountResponseData { balances };
    ok(data)
}

/// Create transfer (JWT)
pub async fn create_transfer_jwt(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<crate::user_auth::auth_service::Claims>,
    Json(req): Json<crate::funding::transfer::TransferRequest>,
) -> ApiResult<crate::funding::transfer::TransferResponse> {
    let user_id = claims
        .sub
        .parse::<i64>()
        .map_err(|_| ApiError::unauthorized("Invalid user ID in token"))?;
    tracing::info!("[TRACE] Transfer Request (JWT): User {}", user_id);

    if let Some(ref coordinator) = state.transfer_coordinator {
        // FSM path returns legacy Result type, so we need to convert
        return match create_transfer_fsm_handler(state.clone(), coordinator, user_id as u64, req)
            .await
        {
            Ok((_, json)) => Ok((StatusCode::OK, json)),
            Err((status, json)) => Err((status, json)),
        };
    }

    let db = state
        .pg_db
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("Database not available"))?;

    match crate::funding::transfer_service::TransferService::execute(db, user_id, req).await {
        Ok(resp) => ok(resp),
        Err(e) => ApiError::bad_request(e.to_string()).into_err(),
    }
}

/// Create order (JWT)
pub async fn create_order_jwt(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<crate::user_auth::auth_service::Claims>,
    Json(req): Json<ClientOrder>,
) -> ApiResult<OrderResponseData> {
    let user_id = claims
        .sub
        .parse::<u64>() // safe: user_id from JWT claims
        .map_err(|_| ApiError::unauthorized("Invalid user ID in token"))?;
    tracing::info!("[TRACE] Create Order (JWT): User {}", user_id);

    let validated = super::super::types::validate_client_order(req.clone(), &state.symbol_mgr)
        .map_err(ApiError::bad_request)?;

    let order_id = state.next_order_id();
    let timestamp = now_ns();

    // Convert to InternalOrder (uses SymbolManager intent-based API)
    let internal_order = validated
        .into_internal_order(order_id, user_id, timestamp, &state.symbol_mgr)
        .map_err(ApiError::bad_request)?;

    let action = OrderAction::Place(crate::pipeline::SequencedOrder::new(
        order_id,
        internal_order,
        timestamp,
    ));

    if state.order_queue.push(action).is_err() {
        return ApiError::service_unavailable("Order queue is full").into_err();
    }

    ok(OrderResponseData {
        order_id,
        cid: req.cid,
        order_status: "ACCEPTED".to_string(),
        accepted_at: now_ms(),
    })
}

/// Cancel order (JWT)
pub async fn cancel_order_jwt(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<crate::user_auth::auth_service::Claims>,
    Json(req): Json<CancelOrderRequest>,
) -> ApiResult<OrderResponseData> {
    let user_id = claims
        .sub
        .parse::<u64>() // safe: user_id from JWT claims
        .map_err(|_| ApiError::unauthorized("Invalid user ID in token"))?;
    tracing::info!(
        "[TRACE] Cancel Order (JWT) {}: User {}",
        req.order_id,
        user_id
    );

    let action = OrderAction::Cancel {
        order_id: req.order_id,
        user_id,
        ingested_at_ns: now_ns(),
    };

    if state.order_queue.push(action).is_err() {
        return ApiError::service_unavailable("Order queue is full").into_err();
    }

    ok(OrderResponseData {
        order_id: req.order_id,
        cid: None,
        order_status: "CANCEL_PENDING".to_string(),
        accepted_at: now_ms(),
    })
}

/// Get orders (JWT)
pub async fn get_orders_jwt(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<crate::user_auth::auth_service::Claims>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> ApiResult<Vec<crate::persistence::queries::OrderApiData>> {
    let user_id = claims
        .sub
        .parse::<u64>() // safe: user_id from JWT claims
        .map_err(|_| ApiError::unauthorized("Invalid user ID in token"))?;
    let db_client = state
        .db_client
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("Persistence not enabled"))?;

    let limit: usize = params
        .get("limit")
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    match crate::persistence::queries::query_orders(
        db_client.taos(),
        user_id,
        state.active_symbol_id,
        limit,
        &state.symbol_mgr,
    )
    .await
    {
        Ok(orders) => ok(orders),
        Err(e) => ApiError::db_error(format!("Query failed: {}", e)).into_err(),
    }
}

/// Get single balance (JWT)
pub async fn get_balance_jwt(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<crate::user_auth::auth_service::Claims>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> ApiResult<crate::persistence::queries::BalanceApiData> {
    let user_id = claims
        .sub
        .parse::<u64>() // safe: user_id from JWT claims
        .map_err(|_| ApiError::unauthorized("Invalid user ID in token"))?;
    let db_client = state
        .db_client
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("Persistence not enabled"))?;

    let asset_id: u32 = params
        .get("asset_id")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| ApiError::bad_request("Missing or invalid asset_id parameter"))?;

    match crate::persistence::queries::query_balance(
        db_client.taos(),
        user_id,
        asset_id,
        &state.symbol_mgr,
    )
    .await
    {
        Ok(Some(balance)) => ok(balance),
        Ok(None) => ApiError::not_found("Balance not found").into_err(),
        Err(e) => ApiError::db_error(format!("Query failed: {}", e)).into_err(),
    }
}
