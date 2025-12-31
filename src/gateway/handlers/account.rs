//! Account handlers (balances, account info)

use std::sync::Arc;

use axum::{
    Extension,
    extract::{Query, State},
};

use super::super::state::AppState;
use super::super::types::{AccountResponseData, ApiError, ApiResult, ok};

/// Get user balance
///
/// GET /api/v1/private/balances?asset_id=1
#[utoipa::path(
    get,
    path = "/api/v1/private/balances",
    params(
        ("asset_id" = u32, Query, description = "Asset ID")
    ),
    responses(
        (status = 200, description = "Balance details", content_type = "application/json"),
        (status = 400, description = "Missing asset_id"),
        (status = 404, description = "Balance not found"),
        (status = 503, description = "Service unavailable")
    ),
    security(("ed25519_auth" = [])),
    tag = "Account"
)]
pub async fn get_balances(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::api_auth::AuthenticatedUser>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> ApiResult<crate::persistence::queries::BalanceApiData> {
    // Check if persistence is enabled
    let db_client = state
        .db_client
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("Persistence not enabled"))?;

    // 1. Use user_id from authenticated user
    let user_id = user.user_id as u64;

    let asset_id: u32 = params
        .get("asset_id")
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| ApiError::bad_request("Missing or invalid asset_id parameter"))?;

    // Query balance from TDengine
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

/// Get all user balances (all account types: Spot, Funding)
///
/// GET /api/v1/private/balances/all
/// Returns balances from PostgreSQL (real source of truth)
#[utoipa::path(
    get,
    path = "/api/v1/private/balances/all",
    responses(
        (status = 200, description = "All user balances", content_type = "application/json"),
        (status = 503, description = "Service unavailable")
    ),
    security(("ed25519_auth" = [])),
    tag = "Account"
)]
pub async fn get_all_balances(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::api_auth::AuthenticatedUser>,
) -> ApiResult<Vec<crate::funding::service::BalanceInfo>> {
    // Extract user_id from authenticated user
    let user_id = user.user_id;

    // Check if PostgreSQL is available
    let pg_db = state
        .pg_db
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("Account database not available"))?;

    // Query all balances
    match crate::funding::service::TransferService::get_all_balances(pg_db.pool(), user_id).await {
        Ok(balances) => ok(balances),
        Err(e) => ApiError::db_error(format!("Query failed: {}", e)).into_err(),
    }
}

/// Get account information (balances) for legacy test script compatibility
///
/// GET /api/v1/private/account
/// Returns balances wrapped in a 'balances' field.
#[utoipa::path(
    get,
    path = "/api/v1/private/account",
    responses(
        (status = 200, description = "User account info", body = AccountResponseData, content_type = "application/json"),
        (status = 503, description = "Service unavailable")
    ),
    security(("ed25519_auth" = [])),
    tag = "Account"
)]
pub async fn get_account(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<crate::api_auth::AuthenticatedUser>,
) -> ApiResult<AccountResponseData> {
    let user_id = user.user_id;
    tracing::info!("DEBUG: get_account called for user_id: {}", user_id);

    // 1. Get Funding balances from Postgres (Source of truth for funding)
    let pg_db = state
        .pg_db
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("Account database not available"))?;

    let mut balances =
        match crate::funding::service::TransferService::get_all_balances(pg_db.pool(), user_id)
            .await
        {
            Ok(b) => b,
            Err(e) => {
                return ApiError::db_error(format!("Postgres query failed: {}", e)).into_err();
            }
        };

    // Filter out any "spot" records from Postgres (if they exist due to legacy/residue)
    // Design intent: Spot balances ONLY come from TDengine/Settlement.
    balances.retain(|b| b.account_type != "spot");

    // 2. Get Spot balances from TDengine (Source of truth for trading)
    // Use PostgreSQL assets_tb as ONLY source of truth for asset configuration
    if let Some(ref t_client) = state.db_client {
        match crate::persistence::queries::query_all_balances_with_pg(
            t_client.taos(),
            pg_db.pool(),
            user_id as u64,
        )
        .await
        {
            Ok(spot_balances) => {
                for sb in spot_balances {
                    // Map BalanceApiData to BalanceInfo
                    balances.push(crate::funding::service::BalanceInfo {
                        asset_id: 0, // Not strictly required for the response type
                        asset: sb.asset,
                        account_type: "spot".to_string(),
                        available: sb.avail,
                        frozen: sb.frozen,
                    });
                }
            }
            Err(e) => {
                tracing::error!("TDengine Spot balance query failed: {}", e);
                // We don't fail the whole request if TDengine is down, just return Funding
            }
        }
    }

    let data = AccountResponseData { balances };
    ok(data)
}
