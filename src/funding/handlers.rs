use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;

use crate::funding::chain_adapter::{MockBtcChain, MockEvmChain};
use crate::gateway::{state::AppState, types::ApiResponse, types::error_codes}; // We instantiate ad-hoc or store in state?
// For MVP we instantiate ad-hoc. State wiring is better but requires large refactor.
// We can store generic ChainClient in AppState if needed, but AppState is crowded.
// Let's instantiate locally as they are stateless mocks.

use super::{deposit::DepositService, withdraw::WithdrawService};

// --- Requests ---

#[derive(Debug, Deserialize)]
pub struct MockDepositRequest {
    pub user_id: i64,
    pub asset: String,
    pub amount: String,
    pub tx_hash: String,
}

#[derive(Debug, Deserialize)]
pub struct WithdrawApplyRequest {
    pub asset: String,
    pub amount: String,
    pub address: String,
    pub fee: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GetAddressRequest {
    pub asset: String,
    pub network: Option<String>,
}

// --- Responses ---

#[derive(Debug, Serialize)]
pub struct AddressResponse {
    pub address: String,
    pub network: String,
}

#[derive(Debug, Serialize)]
pub struct WithdrawResponse {
    pub request_id: String,
    pub status: String,
}

// --- Handlers ---

/// Internal Mock Deposit (Debug/Scanner Trigger)
///
/// [SECURITY WARNING] This endpoint is for development/testing ONLY.
/// It allows direct injection of funds into user balances without real blockchain transactions.
/// FIXME: REMOVE THIS ENDPOINT once Phase 0x11-a Real Chain (Sentinel) is fully stable.
///
/// POST /internal/mock/deposit
#[cfg(feature = "mock-api")]
pub async fn mock_deposit(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<MockDepositRequest>,
) -> Result<Json<ApiResponse<String>>, (StatusCode, Json<ApiResponse<()>>)> {
    // QA-03: Internal Auth Check
    let secret = headers
        .get("X-Internal-Secret")
        .and_then(|v| v.to_str().ok());
    if secret != Some("dev-secret") {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ApiResponse::<()>::error(
                error_codes::AUTH_FAILED,
                "Access Denied: Missing or Invalid X-Internal-Secret",
            )),
        ));
    }

    // Check if PG DB is available
    let db = state.pg_db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ApiResponse::<()>::error(
            error_codes::INTERNAL_ERROR,
            "DB unavailable",
        )),
    ))?;

    let service = DepositService::new(db.clone());
    let amount = Decimal::from_str(&req.amount).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error(
                error_codes::INVALID_PARAMETER,
                "Invalid amount",
            )),
        )
    })?;

    match service
        .process_deposit(&req.tx_hash, req.user_id, &req.asset, amount)
        .await
    {
        Ok(msg) => Ok(Json(ApiResponse::success(msg))),
        Err(e) => {
            // Check for idempotency
            let err_str = e.to_string();
            // Log the error for debugging
            println!("Mock Deposit Error: {:?}", err_str);

            if err_str.contains("already processed") || err_str.contains("AlreadyProcessed") {
                Ok(Json(ApiResponse::success(
                    "Ignored: Already Processed".to_string(),
                )))
            } else {
                let (status, code) = match e {
                    super::deposit::DepositError::InvalidAmount
                    | super::deposit::DepositError::AssetNotFound(_) => {
                        (StatusCode::BAD_REQUEST, error_codes::INVALID_PARAMETER)
                    }
                    _ => (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        error_codes::INTERNAL_ERROR,
                    ),
                };
                Err((status, Json(ApiResponse::<()>::error(code, e.to_string()))))
            }
        }
    }
}

/// Get Deposit Address
/// GET /api/v1/capital/deposit/address
pub async fn get_deposit_address(
    State(state): State<Arc<AppState>>,
    axum::Extension(claims): axum::Extension<crate::user_auth::service::Claims>,
    Query(req): Query<GetAddressRequest>,
) -> Result<Json<ApiResponse<AddressResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    let db = state.pg_db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ApiResponse::<()>::error(
            error_codes::INTERNAL_ERROR,
            "DB unavailable",
        )),
    ))?;

    let user_id = claims.sub.parse::<i64>().unwrap_or_default();
    let service = DepositService::new(db.clone());

    // Select Chain Adapter based on Asset/Network
    // MVP: ETH=MockEvm, BTC=MockBtc. Default to ETH for others.
    let network_raw = req.network.as_deref().unwrap_or("ETH");
    let network_upper = network_raw.to_uppercase();
    // Normalize to lowercase for DB (chain_slug uses lowercase: "eth", "btc")
    let chain_slug = network_raw.to_lowercase();

    let address = if network_upper == "BTC" {
        let adapter = MockBtcChain;
        service
            .get_address(&adapter, user_id, &req.asset, &chain_slug)
            .await
    } else {
        let adapter = MockEvmChain;
        service
            .get_address(&adapter, user_id, &req.asset, &chain_slug)
            .await
    };

    match address {
        Ok(addr) => Ok(Json(ApiResponse::success(AddressResponse {
            address: addr,
            network: chain_slug, // Return lowercase chain_slug
        }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error(
                error_codes::INTERNAL_ERROR,
                e.to_string(),
            )),
        )),
    }
}

/// Apply Withdraw
/// POST /api/v1/capital/withdraw/apply
pub async fn apply_withdraw(
    State(state): State<Arc<AppState>>,
    axum::Extension(claims): axum::Extension<crate::user_auth::service::Claims>,
    Json(req): Json<WithdrawApplyRequest>,
) -> Result<Json<ApiResponse<WithdrawResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    let db = state.pg_db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ApiResponse::<()>::error(
            error_codes::INTERNAL_ERROR,
            "DB unavailable",
        )),
    ))?;

    let user_id = claims.sub.parse::<i64>().unwrap_or_default();
    let service = WithdrawService::new(db.clone());

    let amount = Decimal::from_str(&req.amount).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error(
                error_codes::INVALID_PARAMETER,
                "Invalid amount",
            )),
        )
    })?;
    let fee = Decimal::from_str(req.fee.as_deref().unwrap_or("0")).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()>::error(
                error_codes::INVALID_PARAMETER,
                "Invalid fee",
            )),
        )
    })?;

    // Adapter selection (Simple logic for MVP)
    let res = if req.asset == "BTC" {
        let adapter = MockBtcChain;
        service
            .apply_withdraw(&adapter, user_id, &req.asset, &req.address, amount, fee)
            .await
    } else {
        let adapter = MockEvmChain;
        service
            .apply_withdraw(&adapter, user_id, &req.asset, &req.address, amount, fee)
            .await
    };

    match res {
        Ok(req_id) => Ok(Json(ApiResponse::success(WithdrawResponse {
            request_id: req_id,
            status: "PROCESSING".to_string(), // or from DB result
        }))),
        Err(e) => {
            let (status, code) = match e {
                super::withdraw::WithdrawError::InsufficientFunds => {
                    (StatusCode::BAD_REQUEST, error_codes::INSUFFICIENT_BALANCE)
                }
                super::withdraw::WithdrawError::InvalidAddress
                | super::withdraw::WithdrawError::InvalidAmount
                | super::withdraw::WithdrawError::AssetNotFound(_) => {
                    (StatusCode::BAD_REQUEST, error_codes::INVALID_PARAMETER)
                }
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    error_codes::INTERNAL_ERROR,
                ),
            };
            Err((status, Json(ApiResponse::<()>::error(code, e.to_string()))))
        }
    }
}

/// Get Deposit History
/// GET /api/v1/capital/deposit/history
pub async fn get_deposit_history(
    State(state): State<Arc<AppState>>,
    axum::Extension(claims): axum::Extension<crate::user_auth::service::Claims>,
) -> Result<
    Json<ApiResponse<Vec<super::deposit::DepositRecord>>>,
    (StatusCode, Json<ApiResponse<()>>),
> {
    let db = state.pg_db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ApiResponse::<()>::error(
            error_codes::INTERNAL_ERROR,
            "DB unavailable",
        )),
    ))?;

    let user_id = claims.sub.parse::<i64>().unwrap_or_default();
    let service = DepositService::new(db.clone());

    match service.get_history(user_id).await {
        Ok(records) => Ok(Json(ApiResponse::success(records))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error(
                error_codes::INTERNAL_ERROR,
                e.to_string(),
            )),
        )),
    }
}

/// Get Withdraw History
/// GET /api/v1/capital/withdraw/history
pub async fn get_withdraw_history(
    State(state): State<Arc<AppState>>,
    axum::Extension(claims): axum::Extension<crate::user_auth::service::Claims>,
) -> Result<
    Json<ApiResponse<Vec<super::withdraw::WithdrawRecord>>>,
    (StatusCode, Json<ApiResponse<()>>),
> {
    let db = state.pg_db.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ApiResponse::<()>::error(
            error_codes::INTERNAL_ERROR,
            "DB unavailable",
        )),
    ))?;

    let user_id = claims.sub.parse::<i64>().unwrap_or_default();
    let service = WithdrawService::new(db.clone());

    match service.get_history(user_id).await {
        Ok(records) => Ok(Json(ApiResponse::success(records))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()>::error(
                error_codes::INTERNAL_ERROR,
                e.to_string(),
            )),
        )),
    }
}
