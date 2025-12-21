use anyhow::Result;
use taos::*;

/// Initialize TDengine schema for trading database
pub async fn init_schema(taos: &Taos) -> Result<()> {
    tracing::info!("Initializing TDengine schema...");

    // Create database
    taos.exec(CREATE_DATABASE)
        .await
        .map_err(|e| anyhow::anyhow!("{}: {}", "Failed to create database", e))?;

    // Use database
    taos.exec("USE trading")
        .await
        .map_err(|e| anyhow::anyhow!("{}: {}", "Failed to use database", e))?;

    // Create super tables
    taos.exec(CREATE_ORDERS_TABLE)
        .await
        .map_err(|e| anyhow::anyhow!("{}: {}", "Failed to create orders table", e))?;

    taos.exec(CREATE_TRADES_TABLE)
        .await
        .map_err(|e| anyhow::anyhow!("{}: {}", "Failed to create trades table", e))?;

    taos.exec(CREATE_BALANCES_TABLE)
        .await
        .map_err(|e| anyhow::anyhow!("{}: {}", "Failed to create balances table", e))?;

    taos.exec(CREATE_ORDER_EVENTS_TABLE)
        .await
        .map_err(|e| anyhow::anyhow!("{}: {}", "Failed to create order_events table", e))?;

    taos.exec(CREATE_KLINES_TABLE)
        .await
        .map_err(|e| anyhow::anyhow!("{}: {}", "Failed to create klines table", e))?;

    tracing::info!("TDengine schema initialized successfully");
    Ok(())
}

const CREATE_DATABASE: &str = r#"
CREATE DATABASE IF NOT EXISTS trading 
    KEEP 365d 
    DURATION 10d 
    BUFFER 256 
    WAL_LEVEL 2 
    PRECISION 'us'
"#;

const CREATE_ORDERS_TABLE: &str = r#"
CREATE STABLE IF NOT EXISTS orders (
    ts TIMESTAMP,
    order_id BIGINT UNSIGNED,
    user_id BIGINT UNSIGNED,
    side TINYINT UNSIGNED,
    order_type TINYINT UNSIGNED,
    price BIGINT UNSIGNED,
    qty BIGINT UNSIGNED,
    filled_qty BIGINT UNSIGNED,
    status TINYINT UNSIGNED,
    cid NCHAR(64)
) TAGS (
    symbol_id INT UNSIGNED
)
"#;

const CREATE_TRADES_TABLE: &str = r#"
CREATE STABLE IF NOT EXISTS trades (
    ts TIMESTAMP,
    trade_id BIGINT UNSIGNED,
    order_id BIGINT UNSIGNED,
    user_id BIGINT UNSIGNED,
    side TINYINT UNSIGNED,
    price BIGINT UNSIGNED,
    qty BIGINT UNSIGNED,
    fee BIGINT UNSIGNED,
    role TINYINT UNSIGNED
) TAGS (
    symbol_id INT UNSIGNED
)
"#;

const CREATE_BALANCES_TABLE: &str = r#"
CREATE STABLE IF NOT EXISTS balances (
    ts TIMESTAMP,
    avail BIGINT UNSIGNED,
    frozen BIGINT UNSIGNED,
    lock_version BIGINT UNSIGNED,
    settle_version BIGINT UNSIGNED
) TAGS (
    user_id BIGINT UNSIGNED,
    asset_id INT UNSIGNED
)
"#;

const CREATE_ORDER_EVENTS_TABLE: &str = r#"
CREATE STABLE IF NOT EXISTS order_events (
    ts TIMESTAMP,
    order_id BIGINT UNSIGNED,
    event_type TINYINT UNSIGNED,
    prev_status TINYINT UNSIGNED,
    new_status TINYINT UNSIGNED,
    filled_qty BIGINT UNSIGNED,
    remaining_qty BIGINT UNSIGNED
) TAGS (
    symbol_id INT UNSIGNED
)
"#;

/// K-Line (candlestick) super table for storing aggregated OHLCV data
const CREATE_KLINES_TABLE: &str = r#"
CREATE STABLE IF NOT EXISTS klines (
    ts TIMESTAMP,
    open BIGINT UNSIGNED,
    high BIGINT UNSIGNED,
    low BIGINT UNSIGNED,
    close BIGINT UNSIGNED,
    volume BIGINT UNSIGNED,
    quote_volume DOUBLE,
    trade_count INT UNSIGNED
) TAGS (
    symbol_id INT UNSIGNED,
    intv NCHAR(8)
)
"#;

/// Pre-create subtables for a symbol (orders_X, trades_X, klines_X_1m, etc.)
///
/// Call this during symbol configuration to avoid runtime table creation overhead.
/// First-time table creation can take 400ms+, so we do it during startup.
pub async fn ensure_symbol_tables(taos: &Taos, symbol_id: u32) -> Result<()> {
    // Orders subtable
    taos.exec(format!(
        "CREATE TABLE IF NOT EXISTS orders_{} USING orders TAGS ({})",
        symbol_id, symbol_id
    ))
    .await
    .map_err(|e| anyhow::anyhow!("Failed to create orders_{}: {}", symbol_id, e))?;

    // Trades subtable
    taos.exec(format!(
        "CREATE TABLE IF NOT EXISTS trades_{} USING trades TAGS ({})",
        symbol_id, symbol_id
    ))
    .await
    .map_err(|e| anyhow::anyhow!("Failed to create trades_{}: {}", symbol_id, e))?;

    // K-line subtables for common intervals
    for interval in &["1m", "5m", "15m", "1h", "4h", "1d"] {
        taos.exec(format!(
            "CREATE TABLE IF NOT EXISTS klines_{}_{} USING klines TAGS ({}, '{}')",
            symbol_id, interval, symbol_id, interval
        ))
        .await
        .map_err(|e| {
            anyhow::anyhow!("Failed to create klines_{}_{}: {}", symbol_id, interval, e)
        })?;
    }

    tracing::debug!("Pre-created subtables for symbol_id={}", symbol_id);
    Ok(())
}

/// Pre-create balance subtable for a user+asset pair
///
/// Call this during user onboarding to avoid runtime table creation overhead.
pub async fn ensure_balance_table(taos: &Taos, user_id: u64, asset_id: u32) -> Result<()> {
    taos.exec(format!(
        "CREATE TABLE IF NOT EXISTS balances_{}_{} USING balances TAGS ({}, {})",
        user_id, asset_id, user_id, asset_id
    ))
    .await
    .map_err(|e| anyhow::anyhow!("Failed to create balances_{}_{}: {}", user_id, asset_id, e))?;

    Ok(())
}

/// Pre-create all symbol subtables from SymbolManager
///
/// Call this after schema init and symbol loading to ensure all tables exist.
pub async fn ensure_all_symbol_tables(
    taos: &Taos,
    symbol_mgr: &crate::symbol_manager::SymbolManager,
) -> Result<()> {
    let symbol_ids: Vec<u32> = symbol_mgr.symbol_info.keys().copied().collect();
    tracing::info!("Pre-creating subtables for {} symbols...", symbol_ids.len());

    for symbol_id in symbol_ids {
        ensure_symbol_tables(taos, symbol_id).await?;
    }

    tracing::info!("All symbol subtables pre-created successfully");
    Ok(())
}
