//! Exchange info handlers (assets, symbols, exchange_info)

use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode};
use utoipa::ToSchema;

use super::super::state::AppState;
use super::super::types::ApiResponse;
use super::helpers::now_ms;

/// Asset API response data
#[derive(serde::Serialize, ToSchema)]
pub struct AssetApiData {
    /// Asset ID
    #[schema(example = 1)]
    pub asset_id: i32,
    /// Asset symbol (e.g., BTC)
    #[schema(example = "BTC")]
    pub asset: String,
    /// Asset full name
    #[schema(example = "Bitcoin")]
    pub name: String,
    /// Decimal precision
    #[schema(example = 8)]
    pub decimals: i16,
    /// Can deposit
    pub can_deposit: bool,
    /// Can withdraw
    pub can_withdraw: bool,
    /// Can trade
    pub can_trade: bool,
}

/// Symbol API response data
#[derive(serde::Serialize, ToSchema)]
pub struct SymbolApiData {
    /// Symbol ID
    #[schema(example = 1)]
    pub symbol_id: i32,
    /// Symbol name (e.g., BTC_USDT)
    #[schema(example = "BTC_USDT")]
    pub symbol: String,
    /// Base asset symbol
    #[schema(example = "BTC")]
    pub base_asset: String,
    /// Quote asset symbol
    #[schema(example = "USDT")]
    pub quote_asset: String,
    /// Price decimal precision
    pub price_decimals: i16,
    /// Quantity decimal precision
    pub qty_decimals: i16,
    /// Is trading enabled
    pub is_tradable: bool,
    /// Is visible in UI
    pub is_visible: bool,
    /// Base maker fee in basis points
    #[schema(example = 10)]
    pub base_maker_fee: i32,
    /// Base taker fee in basis points
    #[schema(example = 20)]
    pub base_taker_fee: i32,
}

/// Exchange info response data
#[derive(serde::Serialize, ToSchema)]
pub struct ExchangeInfoData {
    /// All available assets
    pub assets: Vec<AssetApiData>,
    /// All trading pairs
    pub symbols: Vec<SymbolApiData>,
    /// Server timestamp in milliseconds
    #[schema(example = 1703494800000_u64)]
    pub server_time: u64,
}

/// Get all assets
///
/// GET /api/v1/assets
#[utoipa::path(
    get,
    path = "/api/v1/public/assets",
    responses(
        (status = 200, description = "List of all assets", body = ApiResponse<Vec<AssetApiData>>)
    ),
    tag = "Market Data"
)]
pub async fn get_assets(
    State(state): State<Arc<AppState>>,
) -> Result<(StatusCode, Json<ApiResponse<Vec<AssetApiData>>>), (StatusCode, Json<ApiResponse<()>>)>
{
    // Use TTL-cached loader (30 second cache, refreshes on expiry)
    if let Some(ref pg_db) = state.pg_db {
        match super::super::cache::load_assets_cached(pg_db.pool().clone().into()).await {
            Ok(assets) => {
                let data: Vec<AssetApiData> = assets
                    .iter()
                    .map(|a| AssetApiData {
                        asset_id: a.asset_id,
                        asset: a.asset.clone(),
                        name: a.name.clone(),
                        decimals: a.decimals,
                        can_deposit: a.can_deposit(),
                        can_withdraw: a.can_withdraw(),
                        can_trade: a.can_trade(),
                    })
                    .collect();
                return Ok((StatusCode::OK, Json(ApiResponse::success(data))));
            }
            Err(e) => {
                tracing::warn!("[get_assets] Cached loader failed, falling back: {}", e);
            }
        }
    }

    // Fallback to startup cache if DB unavailable
    let assets: Vec<AssetApiData> = state
        .pg_assets
        .iter()
        .map(|a| AssetApiData {
            asset_id: a.asset_id,
            asset: a.asset.clone(),
            name: a.name.clone(),
            decimals: a.decimals,
            can_deposit: a.can_deposit(),
            can_withdraw: a.can_withdraw(),
            can_trade: a.can_trade(),
        })
        .collect();

    Ok((StatusCode::OK, Json(ApiResponse::success(assets))))
}

/// Get all symbols (trading pairs)
///
/// GET /api/v1/symbols
#[utoipa::path(
    get,
    path = "/api/v1/public/symbols",
    responses(
        (status = 200, description = "List of all trading pairs", body = ApiResponse<Vec<SymbolApiData>>)
    ),
    tag = "Market Data"
)]
pub async fn get_symbols(
    State(state): State<Arc<AppState>>,
) -> Result<(StatusCode, Json<ApiResponse<Vec<SymbolApiData>>>), (StatusCode, Json<ApiResponse<()>>)>
{
    // Use TTL-cached loaders (30 second cache, refreshes on expiry)
    if let Some(ref pg_db) = state.pg_db {
        let pool = pg_db.pool().clone().into();
        let assets_result = super::super::cache::load_assets_cached(Arc::clone(&pool)).await;
        let symbols_result = super::super::cache::load_symbols_cached(pool).await;

        if let (Ok(assets), Ok(symbols)) = (assets_result, symbols_result) {
            let asset_map: std::collections::HashMap<i32, &crate::account::Asset> =
                assets.iter().map(|a| (a.asset_id, a)).collect();

            let data: Vec<SymbolApiData> = symbols
                .iter()
                .map(|s| {
                    let base_asset = asset_map
                        .get(&s.base_asset_id)
                        .map(|a| a.asset.clone())
                        .unwrap_or_else(|| format!("UNKNOWN_{}", s.base_asset_id));
                    let quote_asset = asset_map
                        .get(&s.quote_asset_id)
                        .map(|a| a.asset.clone())
                        .unwrap_or_else(|| format!("UNKNOWN_{}", s.quote_asset_id));

                    SymbolApiData {
                        symbol_id: s.symbol_id,
                        symbol: s.symbol.clone(),
                        base_asset,
                        quote_asset,
                        price_decimals: s.price_decimals,
                        qty_decimals: s.qty_decimals,
                        is_tradable: s.is_tradable(),
                        is_visible: s.is_visible(),
                        base_maker_fee: s.base_maker_fee,
                        base_taker_fee: s.base_taker_fee,
                    }
                })
                .collect();
            return Ok((StatusCode::OK, Json(ApiResponse::success(data))));
        } else {
            tracing::warn!("[get_symbols] Cached loader failed, falling back to startup cache");
        }
    }

    // Fallback to startup cache if DB unavailable
    let asset_map: std::collections::HashMap<i32, &crate::account::Asset> =
        state.pg_assets.iter().map(|a| (a.asset_id, a)).collect();

    let symbols: Vec<SymbolApiData> = state
        .pg_symbols
        .iter()
        .map(|s| {
            let base_asset = asset_map
                .get(&s.base_asset_id)
                .map(|a| a.asset.clone())
                .unwrap_or_else(|| format!("UNKNOWN_{}", s.base_asset_id));
            let quote_asset = asset_map
                .get(&s.quote_asset_id)
                .map(|a| a.asset.clone())
                .unwrap_or_else(|| format!("UNKNOWN_{}", s.quote_asset_id));

            SymbolApiData {
                symbol_id: s.symbol_id,
                symbol: s.symbol.clone(),
                base_asset,
                quote_asset,
                price_decimals: s.price_decimals,
                qty_decimals: s.qty_decimals,
                is_tradable: s.is_tradable(),
                is_visible: s.is_visible(),
                base_maker_fee: s.base_maker_fee,
                base_taker_fee: s.base_taker_fee,
            }
        })
        .collect();

    Ok((StatusCode::OK, Json(ApiResponse::success(symbols))))
}

/// Get exchange info (combined assets and symbols)
///
/// GET /api/v1/exchange_info
/// Returns all assets and symbols in a single response.
#[utoipa::path(
    get,
    path = "/api/v1/public/exchange_info",
    responses(
        (status = 200, description = "Exchange metadata", body = ApiResponse<ExchangeInfoData>)
    ),
    tag = "Market Data"
)]
pub async fn get_exchange_info(
    State(state): State<Arc<AppState>>,
) -> Result<(StatusCode, Json<ApiResponse<ExchangeInfoData>>), (StatusCode, Json<ApiResponse<()>>)>
{
    // Build asset list
    let assets: Vec<AssetApiData> = state
        .pg_assets
        .iter()
        .map(|a| AssetApiData {
            asset_id: a.asset_id,
            asset: a.asset.clone(),
            name: a.name.clone(),
            decimals: a.decimals,
            can_deposit: a.can_deposit(),
            can_withdraw: a.can_withdraw(),
            can_trade: a.can_trade(),
        })
        .collect();

    // Build asset lookup map for symbols
    let asset_map: std::collections::HashMap<i32, &crate::account::Asset> =
        state.pg_assets.iter().map(|a| (a.asset_id, a)).collect();

    // Build symbol list
    let symbols: Vec<SymbolApiData> = state
        .pg_symbols
        .iter()
        .map(|s| {
            let base_asset = asset_map
                .get(&s.base_asset_id)
                .map(|a| a.asset.clone())
                .unwrap_or_else(|| format!("UNKNOWN_{}", s.base_asset_id));
            let quote_asset = asset_map
                .get(&s.quote_asset_id)
                .map(|a| a.asset.clone())
                .unwrap_or_else(|| format!("UNKNOWN_{}", s.quote_asset_id));

            SymbolApiData {
                symbol_id: s.symbol_id,
                symbol: s.symbol.clone(),
                base_asset,
                quote_asset,
                price_decimals: s.price_decimals,
                qty_decimals: s.qty_decimals,
                is_tradable: s.is_tradable(),
                is_visible: s.is_visible(),
                base_maker_fee: s.base_maker_fee,
                base_taker_fee: s.base_taker_fee,
            }
        })
        .collect();

    let exchange_info = ExchangeInfoData {
        assets,
        symbols,
        server_time: now_ms(),
    };

    Ok((StatusCode::OK, Json(ApiResponse::success(exchange_info))))
}
