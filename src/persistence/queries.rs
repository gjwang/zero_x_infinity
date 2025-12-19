use anyhow::Result;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use taos::*;

use crate::symbol_manager::SymbolManager;

/// Format internal u64 to display string with specified decimals
fn format_amount(value: u64, decimals: u32, display_decimals: u32) -> String {
    let decimal_value = Decimal::from(value) / Decimal::from(10u64.pow(decimals));
    format!("{:.prec$}", decimal_value, prec = display_decimals as usize)
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

/// Trade API response data (compliant with API conventions)
#[derive(Debug, Serialize)]
pub struct TradeApiData {
    pub trade_id: u64,
    pub order_id: u64,
    pub user_id: u64,
    pub symbol: String, // Symbol name (not ID)
    pub side: String,
    pub price: String, // Formatted with price_display_decimal
    pub qty: String,   // Formatted with base_asset.display_decimals
    pub fee: String,   // Formatted with quote_asset.display_decimals
    pub role: String,
    pub created_at: String,
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
        "SELECT ts, order_id, user_id, side, order_type, price, qty, filled_qty, status, cid FROM orders WHERE symbol_id = {} AND order_id = {} ORDER BY ts DESC LIMIT 1",
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
                3 => "CANCELLED",
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
        "SELECT ts, order_id, user_id, side, order_type, price, qty, filled_qty, status, cid FROM orders WHERE symbol_id = {} AND user_id = {} ORDER BY ts DESC LIMIT {}",
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
                3 => "CANCELLED",
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
    // Query from Super Table
    let sql = format!(
        "SELECT ts, trade_id, order_id, user_id, side, price, qty, fee, role FROM trades WHERE symbol_id = {} ORDER BY ts DESC LIMIT {}",
        symbol_id, limit
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
        .map(|row| TradeApiData {
            trade_id: row.trade_id as u64,
            order_id: row.order_id as u64,
            user_id: row.user_id as u64,
            symbol: symbol_info.symbol.clone(),
            side: if row.side == 0 { "BUY" } else { "SELL" }.to_string(),
            price: format_amount(
                row.price as u64,
                symbol_info.price_decimal,
                symbol_info.price_display_decimal,
            ),
            qty: format_amount(row.qty as u64, base_decimals, base_display_decimals),
            fee: format_amount(row.fee as u64, quote_decimals, quote_display_decimals),
            role: if row.role == 0 { "MAKER" } else { "TAKER" }.to_string(),
            created_at: row.ts,
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
        "SELECT ts, avail, frozen, lock_version, settle_version FROM {} ORDER BY ts DESC LIMIT 1",
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

/// K-Line record from TDengine
#[derive(Debug, Deserialize)]
struct KLineRow {
    ts: String,
    open: i64,
    high: i64,
    low: i64,
    close: i64,
    volume: i64,
    quote_volume: f64, // DOUBLE in TDengine
    trade_count: i32,
}

/// K-Line API response data (compliant with API conventions)
#[derive(Debug, Serialize)]
pub struct KLineApiData {
    pub symbol: String,
    pub interval: String,
    pub open_time: String,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    pub volume: String,
    pub quote_volume: String,
    pub trade_count: u32,
}

/// Query K-Line data for a symbol
pub async fn query_klines(
    taos: &Taos,
    symbol_id: u32,
    interval: &str,
    limit: usize,
    symbol_mgr: &SymbolManager,
) -> Result<Vec<KLineApiData>> {
    // Query from the specific subtable created by the stream
    // Stream creates subtables with format: kl_{interval}_{symbol_id}
    let subtable_name = format!("kl_{}_{}", interval, symbol_id);
    let sql = format!(
        "SELECT ts, open, high, low, close, volume, quote_volume, trade_count FROM {} ORDER BY ts DESC LIMIT {}",
        subtable_name, limit
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
            open_time: row.ts,
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
            quote_volume: format!(
                "{:.prec$}",
                row.quote_volume / 10f64.powi(quote_decimals as i32),
                prec = quote_display_decimals as usize
            ),
            trade_count: row.trade_count as u32,
        })
        .collect())
}
