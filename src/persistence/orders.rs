use crate::models::{InternalOrder, OrderStatus};
use anyhow::Result;
use taos::*;

/// Insert a new order into TDengine
pub async fn insert_order(taos: &Taos, order: &InternalOrder, symbol_id: u32) -> Result<()> {
    let table_name = format!("orders_{}", symbol_id);

    // Create subtable if not exists
    let create_subtable = format!(
        "CREATE TABLE IF NOT EXISTS {} USING orders TAGS ({})",
        table_name, symbol_id
    );
    taos.exec(&create_subtable)
        .await
        .map_err(|e| anyhow::anyhow!("{}: {}", "Failed to create orders subtable", e))?;

    // Get cid or use empty string
    let cid = order.cid.as_deref().unwrap_or("");

    // Insert order
    let sql = format!(
        "INSERT INTO {} VALUES (NOW, {}, {}, {}, {}, {}, {}, {}, {}, '{}')",
        table_name,
        order.order_id,
        order.user_id,
        order.side as u8,
        order.order_type as u8,
        order.price,
        order.qty,
        order.filled_qty,
        order.status as u8,
        cid
    );

    taos.exec(&sql)
        .await
        .map_err(|e| anyhow::anyhow!("{}: {}", "Failed to insert order", e))?;

    Ok(())
}

/// Batch insert MEResults (orders + trades) efficiently
///
/// Combines all orders and trades into single SQL statements for each table.
pub async fn batch_insert_me_results(
    taos: &Taos,
    results: &[crate::messages::MEResult],
) -> Result<()> {
    use crate::models::Side;

    if results.is_empty() {
        return Ok(());
    }

    // Group by symbol_id and ensure subtables exist
    let mut symbol_ids: std::collections::HashSet<u32> = std::collections::HashSet::new();
    for r in results {
        symbol_ids.insert(r.symbol_id);
    }

    // Create subtables for orders and trades
    for symbol_id in &symbol_ids {
        let orders_table = format!("orders_{}", symbol_id);
        let trades_table = format!("trades_{}", symbol_id);

        taos.exec(&format!(
            "CREATE TABLE IF NOT EXISTS {} USING orders TAGS ({})",
            orders_table, symbol_id
        ))
        .await
        .ok();

        taos.exec(&format!(
            "CREATE TABLE IF NOT EXISTS {} USING trades TAGS ({})",
            trades_table, symbol_id
        ))
        .await
        .ok();
    }

    // Batch INSERT orders
    let mut orders_sql = String::from("INSERT INTO ");
    for r in results {
        let table_name = format!("orders_{}", r.symbol_id);
        let cid = r.order.cid.as_deref().unwrap_or("");
        orders_sql.push_str(&format!(
            "{} VALUES (NOW, {}, {}, {}, {}, {}, {}, {}, {}, '{}') ",
            table_name,
            r.order.order_id,
            r.order.user_id,
            r.order.side as u8,
            r.order.order_type as u8,
            r.order.price,
            r.order.qty,
            r.order.filled_qty,
            r.order.status as u8,
            cid
        ));
    }
    taos.exec(&orders_sql)
        .await
        .map_err(|e| anyhow::anyhow!("Batch order insert failed: {}", e))?;

    // Batch INSERT trades (if any)
    let has_trades = results.iter().any(|r| !r.trades.is_empty());
    if has_trades {
        let mut trades_sql = String::from("INSERT INTO ");
        for r in results {
            let table_name = format!("trades_{}", r.symbol_id);
            for te in &r.trades {
                let trade = &te.trade;
                // Insert buyer record
                trades_sql.push_str(&format!(
                    "{} VALUES (NOW, {}, {}, {}, {}, {}, {}, {}, {}) ",
                    table_name,
                    trade.trade_id,
                    trade.buyer_order_id,
                    trade.buyer_user_id,
                    Side::Buy as u8,
                    trade.price,
                    trade.qty,
                    trade.fee,
                    trade.role
                ));
                // Insert seller record
                trades_sql.push_str(&format!(
                    "{} VALUES (NOW, {}, {}, {}, {}, {}, {}, {}, {}) ",
                    table_name,
                    trade.trade_id,
                    trade.seller_order_id,
                    trade.seller_user_id,
                    Side::Sell as u8,
                    trade.price,
                    trade.qty,
                    trade.fee,
                    trade.role
                ));
            }
        }
        taos.exec(&trades_sql)
            .await
            .map_err(|e| anyhow::anyhow!("Batch trade insert failed: {}", e))?;
    }

    Ok(())
}

/// Update order status (insert new record with updated status)
pub async fn update_order_status(
    taos: &Taos,
    order_id: u64,
    user_id: u64,
    symbol_id: u32,
    filled_qty: u64,
    status: OrderStatus,
    cid: Option<&str>,
) -> Result<()> {
    let table_name = format!("orders_{}", symbol_id);

    // Create subtable if not exists
    let create_subtable = format!(
        "CREATE TABLE IF NOT EXISTS {} USING orders TAGS ({})",
        table_name, symbol_id
    );
    taos.exec(&create_subtable)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create orders subtable: {}", e))?;

    let cid_str = cid.unwrap_or("");
    let sql = format!(
        "INSERT INTO {} (ts, order_id, user_id, filled_qty, status, cid) VALUES (NOW, {}, {}, {}, {}, '{}')",
        table_name, order_id, user_id, filled_qty, status as u8, cid_str
    );

    taos.exec(&sql)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to update order status: {}", e))?;

    Ok(())
}

/// Insert order event for audit trail
pub async fn insert_order_event(
    taos: &Taos,
    symbol_id: u32,
    order_id: u64,
    event_type: u8,
    prev_status: OrderStatus,
    new_status: OrderStatus,
    filled_qty: u64,
    remaining_qty: u64,
) -> Result<()> {
    let table_name = format!("order_events_{}", symbol_id);

    // Create subtable if not exists
    let create_subtable = format!(
        "CREATE TABLE IF NOT EXISTS {} USING order_events TAGS ({})",
        table_name, symbol_id
    );
    taos.exec(&create_subtable)
        .await
        .map_err(|e| anyhow::anyhow!("{}: {}", "Failed to create order_events subtable", e))?;

    // Insert event
    let sql = format!(
        "INSERT INTO {} VALUES (NOW, {}, {}, {}, {}, {}, {})",
        table_name,
        order_id,
        event_type,
        prev_status as u8,
        new_status as u8,
        filled_qty,
        remaining_qty
    );

    taos.exec(&sql)
        .await
        .map_err(|e| anyhow::anyhow!("{}: {}", "Failed to insert order event", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{OrderType, Side};

    #[tokio::test]
    #[ignore] // Requires TDengine running
    async fn test_insert_order() {
        let client =
            crate::persistence::TDengineClient::connect("taos+ws://root:taosdata@localhost:6041")
                .await
                .expect("Failed to connect");

        client.init_schema().await.expect("Failed to init schema");

        let mut order = InternalOrder::new(
            100,            // order_id
            1001,           // user_id
            1,              // symbol_id
            85000_00000000, // price
            1_000000,       // qty
            Side::Buy,
        );
        order.cid = Some("test-order-001".to_string());

        insert_order(client.taos(), &order, 1)
            .await
            .expect("Failed to insert order");
    }
}
