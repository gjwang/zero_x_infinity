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
