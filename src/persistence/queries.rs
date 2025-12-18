use anyhow::Result;
use serde::{Deserialize, Serialize};
use taos::*;

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

/// Order API response data
#[derive(Debug, Serialize)]
pub struct OrderApiData {
    pub order_id: u64,
    pub user_id: u64,
    pub side: String,
    pub order_type: String,
    pub price: u64,
    pub qty: u64,
    pub filled_qty: u64,
    pub status: String,
    pub cid: Option<String>,
    pub created_at: String,
}

impl From<OrderRow> for OrderApiData {
    fn from(row: OrderRow) -> Self {
        OrderApiData {
            order_id: row.order_id as u64,
            user_id: row.user_id as u64,
            side: if row.side == 0 {
                "BUY".to_string()
            } else {
                "SELL".to_string()
            },
            order_type: if row.order_type == 0 {
                "LIMIT".to_string()
            } else {
                "MARKET".to_string()
            },
            price: row.price as u64,
            qty: row.qty as u64,
            filled_qty: row.filled_qty as u64,
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
    }
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

/// Trade API response data
#[derive(Debug, Serialize)]
pub struct TradeApiData {
    pub trade_id: u64,
    pub order_id: u64,
    pub user_id: u64,
    pub side: String,
    pub price: u64,
    pub qty: u64,
    pub fee: u64,
    pub role: String,
    pub created_at: String,
}

impl From<TradeRow> for TradeApiData {
    fn from(row: TradeRow) -> Self {
        TradeApiData {
            trade_id: row.trade_id as u64,
            order_id: row.order_id as u64,
            user_id: row.user_id as u64,
            side: if row.side == 0 {
                "BUY".to_string()
            } else {
                "SELL".to_string()
            },
            price: row.price as u64,
            qty: row.qty as u64,
            fee: row.fee as u64,
            role: if row.role == 0 {
                "MAKER".to_string()
            } else {
                "TAKER".to_string()
            },
            created_at: row.ts,
        }
    }
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

/// Balance API response data
#[derive(Debug, Serialize)]
pub struct BalanceApiData {
    pub user_id: u64,
    pub asset_id: u32,
    pub avail: u64,
    pub frozen: u64,
    pub lock_version: u64,
    pub settle_version: u64,
    pub updated_at: String,
}

/// Query single order by ID (returns latest status)
pub async fn query_order(
    taos: &Taos,
    order_id: u64,
    symbol_id: u32,
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

    Ok(rows.into_iter().next().map(|row| row.into()))
}

/// Query orders list for a user
pub async fn query_orders(
    taos: &Taos,
    user_id: u64,
    symbol_id: u32,
    limit: usize,
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

    Ok(rows.into_iter().map(|row| row.into()).collect())
}

/// Query trades for a symbol
pub async fn query_trades(taos: &Taos, symbol_id: u32, limit: usize) -> Result<Vec<TradeApiData>> {
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

    Ok(rows.into_iter().map(|row| row.into()).collect())
}

/// Query latest balance for a user
pub async fn query_balance(
    taos: &Taos,
    user_id: u64,
    asset_id: u32,
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

    Ok(rows.into_iter().next().map(|row| BalanceApiData {
        user_id,
        asset_id,
        avail: row.avail as u64,
        frozen: row.frozen as u64,
        lock_version: row.lock_version as u64,
        settle_version: row.settle_version as u64,
        updated_at: row.ts,
    }))
}
