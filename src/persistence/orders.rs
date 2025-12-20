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

/// Batch insert MEResults (orders + trades) using Stmt parameter binding
///
/// Uses prepared statement with parameter binding for better performance.
/// Skips SQL parsing by sending binary data directly.
/// Uses static cache to avoid repeated CREATE TABLE calls.
pub async fn batch_insert_me_results(
    taos: &Taos,
    results: &[crate::messages::MEResult],
) -> Result<()> {
    use crate::models::Side;
    use once_cell::sync::Lazy;
    use std::sync::Mutex;
    use taos_query::prelude::ColumnView;

    // Cache of already-created tables (survives across calls)
    static CREATED_SYMBOLS: Lazy<Mutex<std::collections::HashSet<u32>>> =
        Lazy::new(|| Mutex::new(std::collections::HashSet::new()));

    if results.is_empty() {
        return Ok(());
    }

    // Find symbol_ids that need table creation
    let mut new_symbols: Vec<u32> = Vec::new();
    {
        let created = CREATED_SYMBOLS.lock().unwrap();
        for r in results {
            if !created.contains(&r.symbol_id) && !new_symbols.contains(&r.symbol_id) {
                new_symbols.push(r.symbol_id);
            }
        }
    }

    // Create only NEW subtables for orders and trades
    for symbol_id in &new_symbols {
        let orders_table = format!("orders_{}", symbol_id);
        let trades_table = format!("trades_{}", symbol_id);

        let orders_ok = taos
            .exec(&format!(
                "CREATE TABLE IF NOT EXISTS {} USING orders TAGS ({})",
                orders_table, symbol_id
            ))
            .await
            .is_ok();

        let trades_ok = taos
            .exec(&format!(
                "CREATE TABLE IF NOT EXISTS {} USING trades TAGS ({})",
                trades_table, symbol_id
            ))
            .await
            .is_ok();

        if orders_ok && trades_ok {
            CREATED_SYMBOLS.lock().unwrap().insert(*symbol_id);
        }
    }

    // Group results by symbol_id for orders
    let mut orders_by_symbol: std::collections::HashMap<u32, Vec<&crate::messages::MEResult>> =
        std::collections::HashMap::new();
    for r in results {
        orders_by_symbol.entry(r.symbol_id).or_default().push(r);
    }

    // Batch INSERT orders using Stmt
    // orders schema: ts, order_id, user_id, side, order_type, price, qty, filled_qty, status, cid
    let mut orders_stmt: taos::Stmt = <taos::Stmt as taos::AsyncBindable<Taos>>::init(taos)
        .await
        .map_err(|e| anyhow::anyhow!("Orders Stmt init failed: {}", e))?;
    orders_stmt
        .prepare("INSERT INTO ? VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?, ?)")
        .await
        .map_err(|e| anyhow::anyhow!("Orders Stmt prepare failed: {}", e))?;

    for (symbol_id, symbol_results) in &orders_by_symbol {
        let table_name = format!("orders_{}", symbol_id);
        orders_stmt
            .set_tbname(&table_name)
            .await
            .map_err(|e| anyhow::anyhow!("Orders Stmt set_tbname failed: {}", e))?;

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let timestamps: Vec<i64> = (0..symbol_results.len())
            .map(|i| now_ms + i as i64)
            .collect();
        let order_ids: Vec<u64> = symbol_results.iter().map(|r| r.order.order_id).collect();
        let user_ids: Vec<u64> = symbol_results.iter().map(|r| r.order.user_id).collect();
        let sides: Vec<u8> = symbol_results.iter().map(|r| r.order.side as u8).collect();
        let order_types: Vec<u8> = symbol_results
            .iter()
            .map(|r| r.order.order_type as u8)
            .collect();
        let prices: Vec<u64> = symbol_results.iter().map(|r| r.order.price).collect();
        let qtys: Vec<u64> = symbol_results.iter().map(|r| r.order.qty).collect();
        let filled_qtys: Vec<u64> = symbol_results.iter().map(|r| r.order.filled_qty).collect();
        let statuses: Vec<u8> = symbol_results
            .iter()
            .map(|r| r.order.status as u8)
            .collect();
        let cids: Vec<&str> = symbol_results
            .iter()
            .map(|r| r.order.cid.as_deref().unwrap_or(""))
            .collect();

        let params = vec![
            ColumnView::from_millis_timestamp(timestamps),
            ColumnView::from_unsigned_big_ints(order_ids),
            ColumnView::from_unsigned_big_ints(user_ids),
            ColumnView::from_unsigned_tiny_ints(sides),
            ColumnView::from_unsigned_tiny_ints(order_types),
            ColumnView::from_unsigned_big_ints(prices),
            ColumnView::from_unsigned_big_ints(qtys),
            ColumnView::from_unsigned_big_ints(filled_qtys),
            ColumnView::from_unsigned_tiny_ints(statuses),
            ColumnView::from_varchar(cids),
        ];

        orders_stmt
            .bind(&params)
            .await
            .map_err(|e| anyhow::anyhow!("Orders Stmt bind failed: {}", e))?;
        orders_stmt
            .add_batch()
            .await
            .map_err(|e| anyhow::anyhow!("Orders Stmt add_batch failed: {}", e))?;
    }

    orders_stmt
        .execute()
        .await
        .map_err(|e| anyhow::anyhow!("Orders Stmt execute failed: {}", e))?;

    // Batch INSERT trades (if any)
    let has_trades = results.iter().any(|r| !r.trades.is_empty());
    if has_trades {
        // Group trades by symbol_id
        let mut trades_by_symbol: std::collections::HashMap<
            u32,
            Vec<(&crate::models::Trade, Side, Side)>,
        > = std::collections::HashMap::new();

        for r in results {
            for te in &r.trades {
                let trade = &te.trade;
                // Buyer record
                trades_by_symbol.entry(r.symbol_id).or_default().push((
                    trade,
                    Side::Buy,
                    te.taker_side,
                ));
                // Seller record
                trades_by_symbol.entry(r.symbol_id).or_default().push((
                    trade,
                    Side::Sell,
                    te.taker_side,
                ));
            }
        }

        // trades schema: ts, trade_id, order_id, user_id, side, price, qty, fee, role
        let mut trades_stmt: taos::Stmt = <taos::Stmt as taos::AsyncBindable<Taos>>::init(taos)
            .await
            .map_err(|e| anyhow::anyhow!("Trades Stmt init failed: {}", e))?;
        trades_stmt
            .prepare("INSERT INTO ? VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .await
            .map_err(|e| anyhow::anyhow!("Trades Stmt prepare failed: {}", e))?;

        for (symbol_id, trade_records) in &trades_by_symbol {
            let table_name = format!("trades_{}", symbol_id);
            trades_stmt
                .set_tbname(&table_name)
                .await
                .map_err(|e| anyhow::anyhow!("Trades Stmt set_tbname failed: {}", e))?;

            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64;

            let timestamps: Vec<i64> = (0..trade_records.len())
                .map(|i| now_ms + i as i64)
                .collect();
            let trade_ids: Vec<u64> = trade_records.iter().map(|(t, _, _)| t.trade_id).collect();
            let order_ids: Vec<u64> = trade_records
                .iter()
                .map(|(t, side, _)| {
                    if *side == Side::Buy {
                        t.buyer_order_id
                    } else {
                        t.seller_order_id
                    }
                })
                .collect();
            let user_ids: Vec<u64> = trade_records
                .iter()
                .map(|(t, side, _)| {
                    if *side == Side::Buy {
                        t.buyer_user_id
                    } else {
                        t.seller_user_id
                    }
                })
                .collect();
            let sides: Vec<u8> = trade_records
                .iter()
                .map(|(_, side, _)| *side as u8)
                .collect();
            let prices: Vec<u64> = trade_records.iter().map(|(t, _, _)| t.price).collect();
            let qtys: Vec<u64> = trade_records.iter().map(|(t, _, _)| t.qty).collect();
            let fees: Vec<u64> = trade_records.iter().map(|(t, _, _)| t.fee).collect();
            let roles: Vec<u8> = trade_records.iter().map(|(t, _, _)| t.role).collect();

            let params = vec![
                ColumnView::from_millis_timestamp(timestamps),
                ColumnView::from_unsigned_big_ints(trade_ids),
                ColumnView::from_unsigned_big_ints(order_ids),
                ColumnView::from_unsigned_big_ints(user_ids),
                ColumnView::from_unsigned_tiny_ints(sides),
                ColumnView::from_unsigned_big_ints(prices),
                ColumnView::from_unsigned_big_ints(qtys),
                ColumnView::from_unsigned_big_ints(fees),
                ColumnView::from_unsigned_tiny_ints(roles),
            ];

            trades_stmt
                .bind(&params)
                .await
                .map_err(|e| anyhow::anyhow!("Trades Stmt bind failed: {}", e))?;
            trades_stmt
                .add_batch()
                .await
                .map_err(|e| anyhow::anyhow!("Trades Stmt add_batch failed: {}", e))?;
        }

        trades_stmt
            .execute()
            .await
            .map_err(|e| anyhow::anyhow!("Trades Stmt execute failed: {}", e))?;
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
