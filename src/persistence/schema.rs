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

    // IMPORTANT: Check database precision at startup
    // Wrong precision causes "Timestamp data out of range" errors
    match check_database_precision(taos).await {
        Ok(precision) => {
            if precision == "us" {
                tracing::info!("✅ TDengine database precision: {} (correct)", precision);
            } else {
                tracing::error!(
                    "❌ TDengine database precision is '{}', expected 'us'! \
                     This will cause 'Timestamp data out of range' errors. \
                     Solution: DROP DATABASE trading; then restart.",
                    precision
                );
                return Err(anyhow::anyhow!(
                    "Wrong database precision: '{}', expected 'us'",
                    precision
                ));
            }
        }
        Err(e) => {
            tracing::warn!("Could not check database precision: {}", e);
        }
    }

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

    taos.exec(CREATE_BALANCE_EVENTS_TABLE)
        .await
        .map_err(|e| anyhow::anyhow!("{}: {}", "Failed to create balance_events table", e))?;

    tracing::info!("TDengine schema initialized successfully");
    Ok(())
}

async fn check_database_precision(taos: &Taos) -> Result<String> {
    #[derive(serde::Deserialize)]
    struct ShowCreate {
        #[serde(rename = "Create Database")]
        create: String,
    }

    // Use SHOW CREATE DATABASE trading
    let mut result = taos
        .query("SHOW CREATE DATABASE trading")
        .await
        .map_err(|e| anyhow::anyhow!("Failed to query database info: {}", e))?;

    let rows: Vec<ShowCreate> = result
        .deserialize()
        .try_collect()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to deserialize: {}", e))?;

    if let Some(row) = rows.first() {
        let create_stmt = row.create.to_uppercase();

        if create_stmt.contains("PRECISION 'US'") || create_stmt.contains("PRECISION 'NS'") {
            if create_stmt.contains("PRECISION 'US'") {
                return Ok("us".to_string());
            } else {
                return Ok("ns".to_string());
            }
        } else if create_stmt.contains("PRECISION 'MS'") {
            return Ok("ms".to_string());
        }
    }

    // Default precision is milliseconds if not specified
    Ok("ms (default)".to_string())
}

// =============================================================================
// TDengine Database Configuration
// =============================================================================
//
// CRITICAL: PRECISION MUST BE 'us' (microseconds)
//
// Our code uses `SystemTime::now().as_micros()` to generate timestamps.
// If the database is created with wrong precision (e.g., 'ms' or 'ns'),
// you will get: "Timestamp data out of range" errors.
//
// Common scenarios that cause this:
// 1. A stale database created with different precision still exists
// 2. CI environment reuses TDengine container with old database
//
// Solution: ci_clean.py drops the database before each test run to ensure
// fresh creation with correct precision.
//
// Timestamp range for 'us' precision:
// - Min: 1970-01-01 00:00:00.000000
// - Max: 2106-02-07 06:28:15.999999
//
// =============================================================================
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

/// Balance Events super table for fee record and event sourcing
/// Dual TAGs: (user_id, account_type) for efficient user queries
const CREATE_BALANCE_EVENTS_TABLE: &str = r#"
CREATE STABLE IF NOT EXISTS balance_events (
    ts          TIMESTAMP,
    event_type  TINYINT UNSIGNED,   -- 1=TradeSettled, 2=FeeReceived, 3=Deposit, 4=Withdraw
    trade_id    BIGINT UNSIGNED,    -- Links to trades table (0 for non-trade events)
    source_id   BIGINT UNSIGNED,    -- Order ID or external ref
    asset_id    INT UNSIGNED,       -- Asset for this event
    delta       BIGINT,             -- Change amount (positive=credit, negative=debit)
    avail_after BIGINT UNSIGNED,    -- Balance after change
    frozen_after BIGINT UNSIGNED,   -- Frozen after change
    from_user   BIGINT UNSIGNED     -- FeeReceived: source user (0 if N/A)
) TAGS (
    user_id       BIGINT UNSIGNED,  -- User identifier (0=REVENUE)
    account_type  TINYINT UNSIGNED  -- 1=Spot, 2=Funding, 3=Futures...
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

pub async fn ensure_balance_table(taos: &Taos, user_id: u64, asset_id: u32) -> Result<()> {
    let sql = format!(
        "CREATE TABLE IF NOT EXISTS balances_{}_{} USING balances TAGS ({}, {})",
        user_id, asset_id, user_id, asset_id
    );

    // Retry logic to handle "Sync leader is restoring" errors during CI startup
    let mut last_err = None;
    for attempt in 1..=5 {
        match taos.exec(&sql).await {
            Ok(_) => return Ok(()),
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("restoring") || err_str.contains("not ready") {
                    tracing::warn!(
                        "TDengine not ready (attempt {}/5): {}. Retrying in 1s...",
                        attempt,
                        err_str
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    last_err = Some(e);
                    continue;
                }
                return Err(anyhow::anyhow!(
                    "Failed to create balances_{}_{}: {}",
                    user_id,
                    asset_id,
                    e
                ));
            }
        }
    }

    Err(anyhow::anyhow!(
        "Failed to create balance table after retries: {}",
        last_err.unwrap()
    ))
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

    let mut tasks = Vec::new();
    for symbol_id in symbol_ids {
        tasks.push(ensure_symbol_tables(taos, symbol_id));
    }

    // Run all symbol table creations in parallel
    let results = futures::future::join_all(tasks).await;
    for (idx, res) in results.into_iter().enumerate() {
        if let Err(e) = res {
            tracing::error!("Failed to pre-create tables for symbol {}: {}", idx, e);
        }
    }

    tracing::info!("All symbol subtables pre-creation finished");
    Ok(())
}
