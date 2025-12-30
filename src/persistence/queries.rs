use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use taos::*;
use utoipa::ToSchema;

use crate::money;
use crate::symbol_manager::SymbolManager;

/// Format internal u64 to display string with specified decimals
/// Delegates to crate::money for unified implementation
#[inline]
fn format_amount(value: u64, decimals: u32, display_decimals: u32) -> String {
    money::format_amount(value, decimals, display_decimals)
}

/// Order record from TDengine (matches database schema)
#[derive(Debug, Clone, Deserialize)]
struct OrderRow {
    ts: String,
    order_id: i64,
    user_id: i64,
    side: i8,
    order_type: i8,
    price: i64,
    qty: i64,
    filled_qty: i64,
    status: i8,
    cid: String,
}

/// Order API response data (compliant with API conventions)
#[derive(Debug, Serialize)]
pub struct OrderApiData {
    pub order_id: u64,
    pub user_id: u64,
    pub symbol: String, // Symbol name (not ID)
    pub side: String,
    pub order_type: String,
    pub price: String,      // Formatted with price_display_decimal
    pub qty: String,        // Formatted with base_asset.display_decimals
    pub filled_qty: String, // Formatted with base_asset.display_decimals
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cid: Option<String>,
    pub created_at: String,
}

/// Trade record from TDengine
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TradeRow {
    ts: String,
    trade_id: i64,
    order_id: i64,
    user_id: i64,
    side: i8,
    price: i64,
    qty: i64,
    fee: i64,
    role: i8,
}

/// Trade record without fee (fee is queried separately from balance_events)
#[derive(Debug, Deserialize)]
struct TradeRowWithoutFee {
    ts: String,
    trade_id: i64,
    order_id: i64,
    user_id: i64,
    side: i8,
    price: i64,
    qty: i64,
    role: i8,
}

/// Fee record from balance_events table
#[derive(Debug, Deserialize, Default)]
struct FeeRow {
    trade_id: i64,
    fee_amount: i64,
}

/// Trade API response data (compliant with API conventions)
#[derive(Debug, Serialize)]
pub struct TradeApiData {
    pub trade_id: u64,
    pub order_id: u64,
    pub user_id: u64,
    pub symbol: String, // Symbol name (not ID)
    pub side: String,
    pub price: String,     // Formatted with price_display_decimal
    pub qty: String,       // Formatted with base_asset.display_decimals
    pub fee: String,       // Formatted with fee_asset decimals
    pub fee_asset: String, // Asset in which fee was paid (BUY→base, SELL→quote)
    pub role: String,
    pub created_at: String,
}

/// Public Trade API response data (for public market data endpoints)
///
/// This struct is used for public trade history endpoints and does NOT expose
/// sensitive information like user_id or order_id.
#[derive(Debug, Serialize, ToSchema)]
pub struct PublicTradeApiData {
    pub id: i64,              // Trade ID
    pub price: String,        // Formatted with price_display_decimal
    pub qty: String,          // Formatted with base_asset.display_decimals
    pub quote_qty: String,    // price * qty (formatted with quote_asset.display_decimals)
    pub time: i64,            // Unix milliseconds
    pub is_buyer_maker: bool, // true if buyer is maker (sell order matched)
    pub is_best_match: bool,  // Always true for our matching engine
}

/// Balance record from TDengine
#[derive(Debug, Deserialize)]
struct BalanceRow {
    ts: String,
    avail: i64,
    frozen: i64,
    lock_version: i64,
    settle_version: i64,
}

/// Balance API response data (compliant with API conventions)
#[derive(Debug, Serialize)]
pub struct BalanceApiData {
    pub user_id: u64,
    pub asset: String,  // Asset name (not ID)
    pub avail: String,  // Formatted with asset.display_decimals
    pub frozen: String, // Formatted with asset.display_decimals
    pub lock_version: u64,
    pub settle_version: u64,
    pub updated_at: String,
}

/// Query single order by ID (returns latest status)
pub async fn query_order(
    taos: &Taos,
    order_id: u64,
    symbol_id: u32,
    symbol_mgr: &SymbolManager,
) -> Result<Option<OrderApiData>> {
    // Query from Super Table with WHERE clause
    let sql = format!(
        "SELECT ts, order_id, user_id, side, order_type, price, qty, filled_qty, status, cid FROM trading.orders WHERE symbol_id = {} AND order_id = {} ORDER BY ts DESC LIMIT 1",
        symbol_id, order_id
    );

    let mut result = taos
        .query(&sql)
        .await
        .map_err(|e| anyhow::anyhow!("Query failed: {}", e))?;

    // Use deserialize to convert rows to OrderRow structs
    let rows: Vec<OrderRow> = result
        .deserialize()
        .try_collect()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to deserialize: {}", e))?;

    Ok(rows.into_iter().next().map(|row| {
        // Get symbol info for formatting
        let symbol_info = symbol_mgr.get_symbol_info_by_id(symbol_id).unwrap();
        let base_decimals = symbol_mgr
            .get_asset_decimal(symbol_info.base_asset_id)
            .unwrap();
        let base_display_decimals = symbol_mgr
            .get_asset_display_decimals(symbol_info.base_asset_id)
            .unwrap();

        OrderApiData {
            order_id: row.order_id as u64,
            user_id: row.user_id as u64,
            symbol: symbol_info.symbol.clone(),
            side: if row.side == 0 { "BUY" } else { "SELL" }.to_string(),
            order_type: if row.order_type == 0 {
                "LIMIT"
            } else {
                "MARKET"
            }
            .to_string(),
            price: format_amount(
                row.price as u64,
                symbol_info.price_decimal,
                symbol_info.price_display_decimal,
            ),
            qty: format_amount(row.qty as u64, base_decimals, base_display_decimals),
            filled_qty: format_amount(row.filled_qty as u64, base_decimals, base_display_decimals),
            status: match row.status {
                0 => "NEW",
                1 => "PARTIALLY_FILLED",
                2 => "FILLED",
                3 => "CANCELED",
                4 => "REJECTED",
                5 => "EXPIRED",
                _ => "UNKNOWN",
            }
            .to_string(),
            cid: if row.cid.is_empty() {
                None
            } else {
                Some(row.cid)
            },
            created_at: row.ts,
        }
    }))
}

/// Query orders list for a user
pub async fn query_orders(
    taos: &Taos,
    user_id: u64,
    symbol_id: u32,
    limit: usize,
    symbol_mgr: &SymbolManager,
) -> Result<Vec<OrderApiData>> {
    // Query from Super Table with WHERE clause
    let sql = format!(
        "SELECT ts, order_id, user_id, side, order_type, price, qty, filled_qty, status, cid FROM trading.orders WHERE symbol_id = {} AND user_id = {} ORDER BY ts DESC LIMIT {}",
        symbol_id, user_id, limit
    );

    let mut result = taos
        .query(&sql)
        .await
        .map_err(|e| anyhow::anyhow!("Query failed: {}", e))?;

    let rows: Vec<OrderRow> = result
        .deserialize()
        .try_collect()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to deserialize: {}", e))?;

    // Get symbol info for formatting
    let symbol_info = symbol_mgr.get_symbol_info_by_id(symbol_id).unwrap();
    let base_decimals = symbol_mgr
        .get_asset_decimal(symbol_info.base_asset_id)
        .unwrap();
    let base_display_decimals = symbol_mgr
        .get_asset_display_decimals(symbol_info.base_asset_id)
        .unwrap();

    Ok(rows
        .into_iter()
        .map(|row| OrderApiData {
            order_id: row.order_id as u64,
            user_id: row.user_id as u64,
            symbol: symbol_info.symbol.clone(),
            side: if row.side == 0 { "BUY" } else { "SELL" }.to_string(),
            order_type: if row.order_type == 0 {
                "LIMIT"
            } else {
                "MARKET"
            }
            .to_string(),
            price: format_amount(
                row.price as u64,
                symbol_info.price_decimal,
                symbol_info.price_display_decimal,
            ),
            qty: format_amount(row.qty as u64, base_decimals, base_display_decimals),
            filled_qty: format_amount(row.filled_qty as u64, base_decimals, base_display_decimals),
            status: match row.status {
                0 => "NEW",
                1 => "PARTIALLY_FILLED",
                2 => "FILLED",
                3 => "CANCELED",
                4 => "REJECTED",
                5 => "EXPIRED",
                _ => "UNKNOWN",
            }
            .to_string(),
            cid: if row.cid.is_empty() {
                None
            } else {
                Some(row.cid)
            },
            created_at: row.ts,
        })
        .collect())
}

/// Query trades for a symbol
pub async fn query_trades(
    taos: &Taos,
    symbol_id: u32,
    limit: usize,
    symbol_mgr: &SymbolManager,
) -> Result<Vec<TradeApiData>> {
    // Query trades (without fee - fee is in balance_events)
    let trades_sql = format!(
        "SELECT ts, trade_id, order_id, user_id, side, price, qty, role FROM trading.trades WHERE symbol_id = {} ORDER BY ts DESC LIMIT {}",
        symbol_id, limit
    );

    let mut result = taos
        .query(&trades_sql)
        .await
        .map_err(|e| anyhow::anyhow!("Query trades failed: {}", e))?;

    let rows: Vec<TradeRowWithoutFee> = result
        .deserialize()
        .try_collect()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to deserialize trades: {}", e))?;

    // Get trade_ids for balance_events lookup
    let trade_ids: Vec<i64> = rows.iter().map(|r| r.trade_id).collect();

    // Query fee from balance_events (per-user fee stored here)
    // event_type = 4 is SettleReceive which contains fee_amount
    let fee_map: std::collections::HashMap<i64, i64> = if !trade_ids.is_empty() {
        let trade_id_list = trade_ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let fee_sql = format!(
            "SELECT trade_id, fee_amount FROM trading.balance_events WHERE trade_id IN ({}) AND event_type = 4",
            trade_id_list
        );

        match taos.query(&fee_sql).await {
            Ok(mut fee_result) => {
                let fee_rows: Vec<FeeRow> = fee_result
                    .deserialize()
                    .try_collect()
                    .await
                    .unwrap_or_default();
                fee_rows
                    .into_iter()
                    .map(|r| (r.trade_id, r.fee_amount))
                    .collect()
            }
            Err(e) => {
                tracing::warn!("Failed to query balance_events for fee: {}", e);
                std::collections::HashMap::new()
            }
        }
    } else {
        std::collections::HashMap::new()
    };

    // Get symbol info for formatting
    let symbol_info = symbol_mgr.get_symbol_info_by_id(symbol_id).unwrap();
    let base_decimals = symbol_mgr
        .get_asset_decimal(symbol_info.base_asset_id)
        .unwrap();
    let base_display_decimals = symbol_mgr
        .get_asset_display_decimals(symbol_info.base_asset_id)
        .unwrap();
    let quote_decimals = symbol_mgr
        .get_asset_decimal(symbol_info.quote_asset_id)
        .unwrap();
    let quote_display_decimals = symbol_mgr
        .get_asset_display_decimals(symbol_info.quote_asset_id)
        .unwrap();

    // Get asset names for fee_asset field
    let base_asset_name = symbol_mgr
        .get_asset_name(symbol_info.base_asset_id)
        .unwrap_or_else(|| "BASE".to_string());
    let quote_asset_name = symbol_mgr
        .get_asset_name(symbol_info.quote_asset_id)
        .unwrap_or_else(|| "QUOTE".to_string());

    Ok(rows
        .into_iter()
        .map(|row| {
            let is_buy = row.side == 0;
            // Fee is paid in received asset: BUY→base, SELL→quote
            let (fee_asset, fee_decimals, fee_display_decimals) = if is_buy {
                (
                    base_asset_name.clone(),
                    base_decimals,
                    base_display_decimals,
                )
            } else {
                (
                    quote_asset_name.clone(),
                    quote_decimals,
                    quote_display_decimals,
                )
            };

            // Get fee from balance_events (keyed by trade_id)
            let fee = fee_map.get(&row.trade_id).copied().unwrap_or(0);

            TradeApiData {
                trade_id: row.trade_id as u64,
                order_id: row.order_id as u64,
                user_id: row.user_id as u64,
                symbol: symbol_info.symbol.clone(),
                side: if is_buy { "BUY" } else { "SELL" }.to_string(),
                price: format_amount(
                    row.price as u64,
                    symbol_info.price_decimal,
                    symbol_info.price_display_decimal,
                ),
                qty: format_amount(row.qty as u64, base_decimals, base_display_decimals),
                fee: format_amount(fee as u64, fee_decimals, fee_display_decimals),
                fee_asset,
                role: if row.role == 1 { "TAKER" } else { "MAKER" }.to_string(),
                created_at: row.ts,
            }
        })
        .collect())
}

/// Query public trades for a symbol (for public API endpoints)
///
/// This function returns trade history WITHOUT exposing user_id or order_id.
/// Supports pagination via from_id parameter.
pub async fn query_public_trades(
    taos: &Taos,
    symbol_id: u32,
    limit: usize,
    from_id: Option<i64>,
    symbol_mgr: &SymbolManager,
) -> Result<Vec<PublicTradeApiData>> {
    // Build WHERE clause with optional from_id filter
    let where_clause = if let Some(from_id) = from_id {
        format!("symbol_id = {} AND trade_id > {}", symbol_id, from_id)
    } else {
        format!("symbol_id = {}", symbol_id)
    };

    // Query trades (only select fields needed for public API)
    let trades_sql = format!(
        "SELECT ts, trade_id, side, price, qty FROM trading.trades WHERE {} ORDER BY trade_id DESC LIMIT {}",
        where_clause, limit
    );

    let mut result = taos
        .query(&trades_sql)
        .await
        .map_err(|e| anyhow::anyhow!("Query public trades failed: {}", e))?;

    // Define a minimal row struct for public trades
    #[derive(Debug, Deserialize)]
    struct PublicTradeRow {
        ts: String,
        trade_id: i64,
        side: i8,
        price: i64,
        qty: i64,
    }

    let rows: Vec<PublicTradeRow> = result
        .deserialize()
        .try_collect()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to deserialize public trades: {}", e))?;

    // Get symbol info for formatting
    let symbol_info = symbol_mgr.get_symbol_info_by_id(symbol_id).unwrap();
    let base_decimals = symbol_mgr
        .get_asset_decimal(symbol_info.base_asset_id)
        .unwrap();
    let base_display_decimals = symbol_mgr
        .get_asset_display_decimals(symbol_info.base_asset_id)
        .unwrap();
    let quote_decimals = symbol_mgr
        .get_asset_decimal(symbol_info.quote_asset_id)
        .unwrap();
    let quote_display_decimals = symbol_mgr
        .get_asset_display_decimals(symbol_info.quote_asset_id)
        .unwrap();

    Ok(rows
        .into_iter()
        .map(|row| {
            let is_buy = row.side == 0;

            // Calculate quote_qty = price * qty
            // price is in price_decimal units, qty is in base_decimals units
            // Result should be in quote_decimals units
            let price_u64 = row.price as u64;
            let qty_u64 = row.qty as u64;

            // quote_qty = (price * qty) / 10^base_decimals
            // This gives us the quote amount in internal units (×10^quote_decimals)
            let quote_qty_internal = (price_u64 * qty_u64) / 10u64.pow(base_decimals);

            // Parse timestamp to milliseconds
            let time_ms = chrono::DateTime::parse_from_rfc3339(&row.ts)
                .map(|dt| dt.timestamp_millis())
                .unwrap_or(0);

            PublicTradeApiData {
                id: row.trade_id,
                price: format_amount(
                    price_u64,
                    symbol_info.price_decimal,
                    symbol_info.price_display_decimal,
                ),
                qty: format_amount(qty_u64, base_decimals, base_display_decimals),
                quote_qty: format_amount(
                    quote_qty_internal,
                    quote_decimals,
                    quote_display_decimals,
                ),
                time: time_ms,
                is_buyer_maker: !is_buy, // If side is SELL (1), then buyer is maker
                is_best_match: true,     // Always true for our matching engine
            }
        })
        .collect())
}

/// Query latest balance for a user
pub async fn query_balance(
    taos: &Taos,
    user_id: u64,
    asset_id: u32,
    symbol_mgr: &SymbolManager,
) -> Result<Option<BalanceApiData>> {
    let table_name = format!("balances_{}_{}", user_id, asset_id);

    let sql = format!(
        "SELECT ts, avail, frozen, lock_version, settle_version FROM trading.{} ORDER BY ts DESC LIMIT 1",
        table_name
    );

    let mut result = taos
        .query(&sql)
        .await
        .map_err(|e| anyhow::anyhow!("Query failed: {}", e))?;

    let rows: Vec<BalanceRow> = result
        .deserialize()
        .try_collect()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to deserialize: {}", e))?;

    // Get asset info for formatting
    let asset_name = symbol_mgr.get_asset_name(asset_id).unwrap();
    let asset_decimals = symbol_mgr.get_asset_decimal(asset_id).unwrap();
    let asset_display_decimals = symbol_mgr.get_asset_display_decimals(asset_id).unwrap();

    Ok(rows.into_iter().next().map(|row| BalanceApiData {
        user_id,
        asset: asset_name,
        avail: format_amount(row.avail as u64, asset_decimals, asset_display_decimals),
        frozen: format_amount(row.frozen as u64, asset_decimals, asset_display_decimals),
        lock_version: row.lock_version as u64,
        settle_version: row.settle_version as u64,
        updated_at: row.ts,
    }))
}

/// Query all latest balances for a user (Spot only) from TDengine
pub async fn query_all_balances(
    taos: &Taos,
    user_id: u64,
    symbol_mgr: &SymbolManager,
) -> Result<Vec<BalanceApiData>> {
    // Query last values per asset for this user from the supertable
    // Use explicit last() with aliases to ensure proper deserialization
    let sql = format!(
        "SELECT last(ts) as ts, last(avail) as avail, last(frozen) as frozen, last(lock_version) as lock_version, last(settle_version) as settle_version, asset_id FROM trading.balances WHERE user_id = {} GROUP BY asset_id",
        user_id
    );

    let mut result = taos
        .query(&sql)
        .await
        .map_err(|e| anyhow::anyhow!("Query all balances failed: {}", e))?;

    let rows: Vec<BalanceRowWithAsset> = result
        .deserialize()
        .try_collect()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to deserialize all balances: {}", e))?;

    let mut balances = Vec::with_capacity(rows.len());

    for row in rows {
        // Get asset info for formatting (raw -> decimal conversion)
        // CRITICAL: Never use default values for unknown assets - this is financial data!
        let asset_name = match symbol_mgr.get_asset_name(row.asset_id) {
            Some(name) => name,
            None => {
                tracing::error!(
                    "CRITICAL: Unknown asset_id {} in TDengine balance query for user {}. This indicates data integrity issue! Skipping.",
                    row.asset_id,
                    user_id
                );
                continue; // Skip this invalid record
            }
        };

        let asset_decimals = match symbol_mgr.get_asset_decimal(row.asset_id) {
            Some(d) => d,
            None => {
                tracing::error!(
                    "CRITICAL: Missing decimals for asset_id {} in balance query. Skipping.",
                    row.asset_id
                );
                continue;
            }
        };

        let asset_display_decimals = symbol_mgr
            .get_asset_display_decimals(row.asset_id)
            .unwrap_or(asset_decimals);

        tracing::debug!(
            "TDengine balance: asset_id={}, asset={}, avail_raw={}, decimals={}",
            row.asset_id,
            asset_name,
            row.avail,
            asset_decimals
        );

        // format_amount converts raw atomic units (e.g., 1100000000) to decimal string (e.g., "11.00000000")
        balances.push(BalanceApiData {
            user_id,
            asset: asset_name,
            avail: format_amount(row.avail as u64, asset_decimals, asset_display_decimals),
            frozen: format_amount(row.frozen as u64, asset_decimals, asset_display_decimals),
            lock_version: row.lock_version as u64,
            settle_version: row.settle_version as u64,
            updated_at: row.ts.to_rfc3339(),
        });
    }

    Ok(balances)
}

/// Query all latest balances for a user (Spot only) from TDengine
/// Uses PostgreSQL `assets_tb` as the ONLY source of truth for asset configuration
pub async fn query_all_balances_with_pg(
    taos: &Taos,
    pg_pool: &sqlx::PgPool,
    user_id: u64,
) -> Result<Vec<BalanceApiData>> {
    // Query last values per asset for this user from TDengine
    let sql = format!(
        "SELECT last(ts) as ts, last(avail) as avail, last(frozen) as frozen, last(lock_version) as lock_version, last(settle_version) as settle_version, asset_id FROM trading.balances WHERE user_id = {} GROUP BY asset_id",
        user_id
    );

    let mut result = taos
        .query(&sql)
        .await
        .map_err(|e| anyhow::anyhow!("Query all balances failed: {}", e))?;

    let rows: Vec<BalanceRowWithAsset> = result
        .deserialize()
        .try_collect()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to deserialize all balances: {}", e))?;

    let mut balances = Vec::with_capacity(rows.len());

    for row in rows {
        // Query PostgreSQL for asset configuration (ONLY source of truth!)
        let asset_info: Option<(String, i16)> =
            sqlx::query_as("SELECT asset, decimals FROM assets_tb WHERE asset_id = $1")
                .bind(row.asset_id as i32)
                .fetch_optional(pg_pool)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to query asset info: {}", e))?;

        let (asset_name, decimals) = match asset_info {
            Some((name, dec)) => (name, dec as u32),
            None => {
                tracing::error!(
                    "CRITICAL: Unknown asset_id {} in TDengine balance query for user {}. Not found in PostgreSQL assets_tb! Skipping.",
                    row.asset_id,
                    user_id
                );
                continue;
            }
        };

        tracing::debug!(
            "TDengine balance (PG lookup): asset_id={}, asset={}, avail_raw={}, decimals={}",
            row.asset_id,
            asset_name,
            row.avail,
            decimals
        );

        // format_amount converts raw atomic units to decimal string
        balances.push(BalanceApiData {
            user_id,
            asset: asset_name,
            avail: format_amount(row.avail as u64, decimals, decimals),
            frozen: format_amount(row.frozen as u64, decimals, decimals),
            lock_version: row.lock_version as u64,
            settle_version: row.settle_version as u64,
            updated_at: row.ts.to_rfc3339(),
        });
    }

    Ok(balances)
}

/// Helper struct for deserializing multi-record balance queries with asset_id tag
#[derive(Debug, Deserialize)]
struct BalanceRowWithAsset {
    pub ts: DateTime<Utc>,
    pub avail: i64,
    pub frozen: i64,
    pub lock_version: i64,
    pub settle_version: i64,
    pub asset_id: u32,
}

/// K-Line record from TDengine
#[derive(Debug, Deserialize)]
struct KLineRow {
    ts: DateTime<Utc>,
    open: i64,
    high: i64,
    low: i64,
    close: i64,
    volume: i64,
    quote_volume: f64, // DOUBLE in TDengine
    trade_count: i32,
}

/// K-Line API response data (compliant with Binance API conventions)
#[derive(Debug, Serialize)]
pub struct KLineApiData {
    pub symbol: String,
    pub interval: String,
    pub open_time: i64,  // Unix milliseconds
    pub close_time: i64, // Unix milliseconds (open_time + interval - 1)
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: String,
    pub quote_volume: String,
    pub trade_count: u32,
}

/// Convert interval string to milliseconds
fn interval_to_ms(interval: &str) -> i64 {
    match interval {
        "1m" => 60 * 1000,
        "5m" => 5 * 60 * 1000,
        "15m" => 15 * 60 * 1000,
        "30m" => 30 * 60 * 1000,
        "1h" => 60 * 60 * 1000,
        "1d" => 24 * 60 * 60 * 1000,
        _ => 60 * 1000, // default to 1m
    }
}

/// Query K-Line data for a symbol
pub async fn query_klines(
    taos: &Taos,
    symbol_id: u32,
    interval: &str,
    limit: usize,
    symbol_mgr: &SymbolManager,
) -> Result<Vec<KLineApiData>> {
    // Query from the Stream-generated super table (klines_1m, klines_5m, etc.)
    // Note: Stream creates auto-structured tables with group_id TAG
    let table_name = format!("klines_{}", interval);
    let sql = format!(
        "SELECT ts, open, high, low, close, volume, quote_volume, trade_count FROM {} ORDER BY ts DESC LIMIT {}",
        table_name, limit
    );

    let mut result = taos
        .query(&sql)
        .await
        .map_err(|e| anyhow::anyhow!("Query failed: {}", e))?;

    let rows: Vec<KLineRow> = result
        .deserialize()
        .try_collect()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to deserialize: {}", e))?;

    // Get symbol info for formatting
    let symbol_info = symbol_mgr.get_symbol_info_by_id(symbol_id).unwrap();
    let base_decimals = symbol_mgr
        .get_asset_decimal(symbol_info.base_asset_id)
        .unwrap();
    let base_display_decimals = symbol_mgr
        .get_asset_display_decimals(symbol_info.base_asset_id)
        .unwrap();
    let quote_decimals = symbol_mgr
        .get_asset_decimal(symbol_info.quote_asset_id)
        .unwrap();
    let quote_display_decimals = symbol_mgr
        .get_asset_display_decimals(symbol_info.quote_asset_id)
        .unwrap();

    Ok(rows
        .into_iter()
        .map(|row| KLineApiData {
            symbol: symbol_info.symbol.clone(),
            interval: interval.to_string(),
            open_time: row.ts.timestamp_millis(),
            close_time: row.ts.timestamp_millis() + interval_to_ms(interval) - 1,
            open: format_amount(
                row.open as u64,
                symbol_info.price_decimal,
                symbol_info.price_display_decimal,
            ),
            high: format_amount(
                row.high as u64,
                symbol_info.price_decimal,
                symbol_info.price_display_decimal,
            ),
            low: format_amount(
                row.low as u64,
                symbol_info.price_decimal,
                symbol_info.price_display_decimal,
            ),
            close: format_amount(
                row.close as u64,
                symbol_info.price_decimal,
                symbol_info.price_display_decimal,
            ),
            volume: format_amount(row.volume as u64, base_decimals, base_display_decimals),
            // quote_volume = SUM(price * qty) where:
            // - price is in internal units (×10^price_decimal)
            // - qty is in internal units (×10^base_decimals)
            // To get quote_amount: price * qty / 10^base_decimals
            // Result is already in quote asset internal units (×10^quote_decimals)
            // So divide by 10^(base_decimals + quote_decimals) to get display value
            quote_volume: format!(
                "{:.prec$}",
                row.quote_volume / 10f64.powi(base_decimals as i32 + quote_decimals as i32),
                prec = quote_display_decimals as usize
            ),
            trade_count: row.trade_count as u32,
        })
        .collect())
}

// ============================================================
// TRADE FEE QUERY (from balance_events)
// ============================================================

// NOTE: FeeRow struct is defined at top of file (L76)

/// Query trade fees from balance_events for a user
///
/// Returns a HashMap: trade_id -> fee_amount
pub async fn query_trade_fees(
    taos: &Taos,
    user_id: u64,
    trade_ids: &[u64],
) -> Result<std::collections::HashMap<u64, u64>> {
    use std::collections::HashMap;

    if trade_ids.is_empty() {
        return Ok(HashMap::new());
    }

    // Build IN clause for trade_ids
    let ids: Vec<String> = trade_ids.iter().map(|id| id.to_string()).collect();
    let ids_str = ids.join(", ");

    // Query balance_events for SettleReceive events (event_type=2) with fee
    // account_type=1 for Spot
    let table_name = format!("balance_events_{}_{}", user_id, 1);
    let sql = format!(
        "SELECT trade_id, user_id, fee FROM trading.{} WHERE trade_id IN ({}) AND fee > 0",
        table_name, ids_str
    );

    let result = taos.query(&sql).await;

    match result {
        Ok(mut rs) => {
            let rows: Vec<FeeRow> = rs.deserialize().try_collect().await.unwrap_or_default();

            Ok(rows
                .into_iter()
                .map(|r| (r.trade_id as u64, r.fee_amount as u64))
                .collect())
        }
        Err(_) => {
            // Table may not exist yet, return empty
            Ok(HashMap::new())
        }
    }
}

/// Query trades for a specific user (with real fee from balance_events)
pub async fn query_user_trades(
    taos: &Taos,
    user_id: u64,
    symbol_id: u32,
    limit: usize,
    symbol_mgr: &SymbolManager,
) -> Result<Vec<TradeApiData>> {
    // Query trades for this user
    let sql = format!(
        "SELECT ts, trade_id, order_id, user_id, side, price, qty, fee, role \
         FROM trading.trades \
         WHERE symbol_id = {} AND user_id = {} \
         ORDER BY ts DESC LIMIT {}",
        symbol_id, user_id, limit
    );

    let mut result = taos
        .query(&sql)
        .await
        .map_err(|e| anyhow::anyhow!("Query failed: {}", e))?;

    let rows: Vec<TradeRow> = result
        .deserialize()
        .try_collect()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to deserialize: {}", e))?;

    if rows.is_empty() {
        return Ok(vec![]);
    }

    // Get trade IDs for fee lookup
    let trade_ids: Vec<u64> = rows.iter().map(|r| r.trade_id as u64).collect();

    // Query real fees from balance_events
    let fee_map = query_trade_fees(taos, user_id, &trade_ids).await?;

    // Get symbol info for formatting
    let symbol_info = symbol_mgr.get_symbol_info_by_id(symbol_id).unwrap();
    let base_decimals = symbol_mgr
        .get_asset_decimal(symbol_info.base_asset_id)
        .unwrap();
    let base_display_decimals = symbol_mgr
        .get_asset_display_decimals(symbol_info.base_asset_id)
        .unwrap();
    let quote_decimals = symbol_mgr
        .get_asset_decimal(symbol_info.quote_asset_id)
        .unwrap();
    let quote_display_decimals = symbol_mgr
        .get_asset_display_decimals(symbol_info.quote_asset_id)
        .unwrap();

    // Get asset names for fee_asset field
    let base_asset_name = symbol_mgr
        .get_asset_name(symbol_info.base_asset_id)
        .unwrap_or_else(|| "BASE".to_string());
    let quote_asset_name = symbol_mgr
        .get_asset_name(symbol_info.quote_asset_id)
        .unwrap_or_else(|| "QUOTE".to_string());

    Ok(rows
        .into_iter()
        .map(|row| {
            let is_buy = row.side == 0;
            // Fee is paid in received asset: BUY→base, SELL→quote
            let (fee_asset, fee_decimals, fee_display_decimals) = if is_buy {
                (
                    base_asset_name.clone(),
                    base_decimals,
                    base_display_decimals,
                )
            } else {
                (
                    quote_asset_name.clone(),
                    quote_decimals,
                    quote_display_decimals,
                )
            };

            // Use real fee from balance_events if available, otherwise 0
            let real_fee = fee_map.get(&(row.trade_id as u64)).copied().unwrap_or(0);

            TradeApiData {
                trade_id: row.trade_id as u64,
                order_id: row.order_id as u64,
                user_id: row.user_id as u64,
                symbol: symbol_info.symbol.clone(),
                side: if is_buy { "BUY" } else { "SELL" }.to_string(),
                price: format_amount(
                    row.price as u64,
                    symbol_info.price_decimal,
                    symbol_info.price_display_decimal,
                ),
                qty: format_amount(row.qty as u64, base_decimals, base_display_decimals),
                fee: format_amount(real_fee, fee_decimals, fee_display_decimals),
                fee_asset,
                role: if row.role == 1 { "TAKER" } else { "MAKER" }.to_string(),
                created_at: row.ts,
            }
        })
        .collect())
}

#[cfg(test)]
mod kline_tests {
    use super::*;

    #[test]
    fn test_interval_to_ms() {
        assert_eq!(interval_to_ms("1m"), 60 * 1000);
        assert_eq!(interval_to_ms("5m"), 5 * 60 * 1000);
        assert_eq!(interval_to_ms("15m"), 15 * 60 * 1000);
        assert_eq!(interval_to_ms("30m"), 30 * 60 * 1000);
        assert_eq!(interval_to_ms("1h"), 60 * 60 * 1000);
        assert_eq!(interval_to_ms("1d"), 24 * 60 * 60 * 1000);
        // Unknown interval defaults to 1m
        assert_eq!(interval_to_ms("unknown"), 60 * 1000);
    }

    #[test]
    fn test_close_time_calculation() {
        // For 1m interval: close_time = open_time + 60000 - 1
        let open_time: i64 = 1766144460000;
        let close_time = open_time + interval_to_ms("1m") - 1;
        assert_eq!(close_time, 1766144519999);

        // For 1h interval: close_time = open_time + 3600000 - 1
        let close_time_1h = open_time + interval_to_ms("1h") - 1;
        assert_eq!(close_time_1h, 1766148059999);
    }

    #[test]
    fn test_quote_volume_calculation() {
        // BTC_USDT example:
        // - price_decimal = 2 (price = 37000.00 -> internal 3700000)
        // - base_decimals = 8 (qty = 0.4 -> internal 40000000)
        // - quote_decimals = 2
        // - quote_volume = price * qty = 3700000 * 40000000 = 1.48e14
        // - divide by 10^(8+2) = 10^10
        // - result = 14800.00 USDT

        let raw_quote_volume: f64 = 3700000.0 * 40000000.0; // 1.48e14
        let base_decimals: u32 = 8;
        let quote_decimals: u32 = 2;
        let quote_display_decimals: u32 = 2;

        let result = format!(
            "{:.prec$}",
            raw_quote_volume / 10f64.powi(base_decimals as i32 + quote_decimals as i32),
            prec = quote_display_decimals as usize
        );

        assert_eq!(result, "14800.00");
    }

    #[test]
    fn test_quote_volume_small_trade() {
        // Small trade: 0.01 BTC @ 35000 USDT = 350 USDT
        // - price = 35000.00 -> internal 3500000
        // - qty = 0.01 -> internal 1000000
        // - quote_volume = 3500000 * 1000000 = 3.5e12
        // - divide by 10^10 = 350.00

        let raw_quote_volume: f64 = 3500000.0 * 1000000.0;
        let base_decimals: u32 = 8;
        let quote_decimals: u32 = 2;

        let result = format!(
            "{:.2}",
            raw_quote_volume / 10f64.powi(base_decimals as i32 + quote_decimals as i32)
        );

        assert_eq!(result, "350.00");
    }

    #[test]
    fn test_kline_api_data_serialization() {
        let kline = KLineApiData {
            symbol: "BTC_USDT".to_string(),
            interval: "1m".to_string(),
            open_time: 1766144460000,
            close_time: 1766144519999,
            open: "37000.00".to_string(),
            high: "37500.00".to_string(),
            low: "36800.00".to_string(),
            close: "37200.00".to_string(),
            volume: "0.400000".to_string(),
            quote_volume: "14800.00".to_string(),
            trade_count: 8,
        };

        let json = serde_json::to_string(&kline).unwrap();

        // Verify key fields in JSON
        assert!(json.contains("\"open_time\":1766144460000"));
        assert!(json.contains("\"close_time\":1766144519999"));
        assert!(json.contains("\"symbol\":\"BTC_USDT\""));
        assert!(json.contains("\"volume\":\"0.400000\""));
        assert!(json.contains("\"quote_volume\":\"14800.00\""));
    }
}

#[cfg(test)]
mod public_trades_tests {
    use super::*;

    #[test]
    fn test_public_trade_api_data_no_sensitive_fields() {
        // Verify that PublicTradeApiData does NOT have user_id or order_id fields
        let trade = PublicTradeApiData {
            id: 12345,
            price: "43000.00".to_string(),
            qty: "0.1000".to_string(),
            quote_qty: "4300.00".to_string(),
            time: 1703660555000,
            is_buyer_maker: true,
            is_best_match: true,
        };

        // Serialize to JSON
        let json = serde_json::to_string(&trade).unwrap();

        // Verify sensitive fields are NOT present
        assert!(
            !json.contains("user_id"),
            "PublicTradeApiData should NOT contain user_id"
        );
        assert!(
            !json.contains("order_id"),
            "PublicTradeApiData should NOT contain order_id"
        );

        // Verify expected fields ARE present
        assert!(json.contains("\"id\":12345"));
        assert!(json.contains("\"price\":\"43000.00\""));
        assert!(json.contains("\"qty\":\"0.1000\""));
        assert!(json.contains("\"quote_qty\":\"4300.00\""));
        assert!(json.contains("\"time\":1703660555000"));
        assert!(json.contains("\"is_buyer_maker\":true"));
        assert!(json.contains("\"is_best_match\":true"));
    }

    #[test]
    fn test_quote_qty_calculation_btc_usdt() {
        // BTC_USDT example:
        // - price_decimal = 2 (price = 43000.00 -> internal 4300000)
        // - base_decimals = 8 (qty = 0.1 -> internal 10000000)
        // - quote_decimals = 2
        // - quote_qty = (price * qty) / 10^base_decimals
        //             = (4300000 * 10000000) / 10^8
        //             = 43000000000000 / 100000000
        //             = 430000 (internal units, quote_decimals=2)
        // - display: 430000 / 10^2 = 4300.00 USDT

        let price: u64 = 4300000; // 43000.00 (price_decimal=2)
        let qty: u64 = 10000000; // 0.1 BTC (base_decimals=8)
        let base_decimals: u32 = 8;
        let quote_decimals: u32 = 2;
        let quote_display_decimals: u32 = 2;

        let quote_qty_internal = (price * qty) / 10u64.pow(base_decimals);
        assert_eq!(quote_qty_internal, 430000);

        let quote_qty_str =
            format_amount(quote_qty_internal, quote_decimals, quote_display_decimals);
        assert_eq!(quote_qty_str, "4300.00");
    }

    #[test]
    fn test_quote_qty_calculation_small_trade() {
        // Small trade: 0.01 BTC @ 35000 USDT = 350 USDT
        // - price = 35000.00 -> internal 3500000
        // - qty = 0.01 -> internal 1000000
        // - quote_qty = (3500000 * 1000000) / 10^8 = 35000

        let price: u64 = 3500000;
        let qty: u64 = 1000000;
        let base_decimals: u32 = 8;
        let quote_decimals: u32 = 2;
        let quote_display_decimals: u32 = 2;

        let quote_qty_internal = (price * qty) / 10u64.pow(base_decimals);
        assert_eq!(quote_qty_internal, 35000);

        let quote_qty_str =
            format_amount(quote_qty_internal, quote_decimals, quote_display_decimals);
        assert_eq!(quote_qty_str, "350.00");
    }

    #[test]
    fn test_is_buyer_maker_logic() {
        // is_buyer_maker should be true when side is SELL (1)
        // Because if a SELL order is in the book (maker), the buyer is the taker

        // Case 1: side = 0 (BUY) -> is_buyer_maker = false
        let side_buy: i8 = 0;
        let is_buy = side_buy == 0;
        let is_buyer_maker = !is_buy;
        assert!(!is_buyer_maker, "BUY order: buyer is taker, not maker");

        // Case 2: side = 1 (SELL) -> is_buyer_maker = true
        let side_sell: i8 = 1;
        let is_buy = side_sell == 0;
        let is_buyer_maker = !is_buy;
        assert!(is_buyer_maker, "SELL order: buyer is maker");
    }

    #[test]
    fn test_public_trade_string_formatting() {
        // Verify all numeric fields are Strings, not numbers
        let trade = PublicTradeApiData {
            id: 99999,
            price: "50000.50".to_string(),
            qty: "1.234567".to_string(),
            quote_qty: "61728.62".to_string(),
            time: 1700000000000,
            is_buyer_maker: false,
            is_best_match: true,
        };

        let json = serde_json::to_string(&trade).unwrap();

        // Prices and quantities should be quoted strings in JSON
        assert!(
            json.contains("\"price\":\"50000.50\""),
            "price should be a string"
        );
        assert!(
            json.contains("\"qty\":\"1.234567\""),
            "qty should be a string"
        );
        assert!(
            json.contains("\"quote_qty\":\"61728.62\""),
            "quote_qty should be a string"
        );

        // time should be a number
        assert!(
            json.contains("\"time\":1700000000000"),
            "time should be a number"
        );
    }
}
