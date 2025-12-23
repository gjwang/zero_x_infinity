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

/// Fast batch insert MEResults (orders + trades) using batch SQL
///
/// Optimized for high-frequency writes:
/// - One network round-trip per type (orders + trades)
/// - No Mutex locks
/// - Pre-allocated string buffers
/// - No Stmt set_tbname overhead
///
/// Note: Tables must exist (created during symbol configuration, not here)
pub async fn batch_insert_me_results(
    taos: &Taos,
    results: &[crate::messages::MEResult],
) -> Result<()> {
    use crate::models::Side;
    use std::fmt::Write;

    if results.is_empty() {
        return Ok(());
    }

    let now_us = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as i64;

    // === BATCH INSERT ORDERS ===
    // orders schema: ts, order_id, user_id, side, order_type, price, qty, filled_qty, status, cid
    let mut orders_sql = String::with_capacity(results.len() * 200 + 20);
    orders_sql.push_str("INSERT INTO ");

    let mut ts_offset = 0i64;
    for r in results {
        // Taker order update
        let o = &r.order;
        let cid = o.cid.as_deref().unwrap_or("");
        write!(
            orders_sql,
            "orders_{} VALUES({}, {}, {}, {}, {}, {}, {}, {}, {}, '{}') ",
            r.symbol_id,
            now_us + ts_offset,
            o.order_id,
            o.user_id,
            o.side as u8,
            o.order_type as u8,
            o.price,
            o.qty,
            o.filled_qty,
            o.status as u8,
            cid
        )
        .unwrap();
        ts_offset += 1;

        // Maker order updates
        for m in r.maker_updates.iter() {
            let m_cid = m.cid.as_deref().unwrap_or("");
            write!(
                orders_sql,
                "orders_{} VALUES({}, {}, {}, {}, {}, {}, {}, {}, {}, '{}') ",
                r.symbol_id,
                now_us + ts_offset,
                m.order_id,
                m.user_id,
                m.side as u8,
                m.order_type as u8,
                m.price,
                m.qty,
                m.filled_qty,
                m.status as u8,
                m_cid
            )
            .unwrap();
            ts_offset += 1;
        }
    }

    // First attempt for orders
    let orders_insert_result = taos.exec(&orders_sql).await;
    if let Err(e) = &orders_insert_result {
        let err_str = e.to_string();
        if err_str.contains("Table does not exist") || err_str.contains("0x2662") {
            tracing::debug!("Orders table not found, auto-creating...");
            // Auto-create missing orders tables
            let mut created_symbols = std::collections::HashSet::new();
            for r in results {
                if !created_symbols.contains(&r.symbol_id) {
                    let create_sql = format!(
                        "CREATE TABLE IF NOT EXISTS orders_{} USING orders TAGS ({})",
                        r.symbol_id, r.symbol_id
                    );
                    let _ = taos.exec(&create_sql).await;
                    created_symbols.insert(r.symbol_id);
                }
            }
            // Retry
            taos.exec(&orders_sql).await.map_err(|e| {
                anyhow::anyhow!("Batch orders insert failed after auto-create: {}", e)
            })?;
        } else {
            return Err(anyhow::anyhow!("Batch orders insert failed: {}", e));
        }
    }

    // === BATCH INSERT TRADES ===
    let has_trades = results.iter().any(|r| !r.trades.is_empty());
    if has_trades {
        // trades schema: ts, trade_id, order_id, user_id, side, price, qty, fee, role
        // Each trade generates 2 records (buyer + seller)
        let trade_count: usize = results.iter().map(|r| r.trades.len() * 2).sum();
        let mut trades_sql = String::with_capacity(trade_count * 150 + 20);
        trades_sql.push_str("INSERT INTO ");

        let mut ts_offset = 0i64;
        for r in results {
            for te in &r.trades {
                let t = &te.trade;

                // Buyer record
                write!(
                    trades_sql,
                    "trades_{} VALUES({}, {}, {}, {}, {}, {}, {}, {}, {}) ",
                    r.symbol_id,
                    now_us + ts_offset,
                    t.trade_id,
                    t.buyer_order_id,
                    t.buyer_user_id,
                    Side::Buy as u8,
                    t.price,
                    t.qty,
                    t.fee,
                    if te.taker_side == Side::Buy { 0u8 } else { 1u8 } // 0=taker, 1=maker
                )
                .unwrap();
                ts_offset += 1;

                // Seller record
                write!(
                    trades_sql,
                    "trades_{} VALUES({}, {}, {}, {}, {}, {}, {}, {}, {}) ",
                    r.symbol_id,
                    now_us + ts_offset,
                    t.trade_id,
                    t.seller_order_id,
                    t.seller_user_id,
                    Side::Sell as u8,
                    t.price,
                    t.qty,
                    t.fee,
                    if te.taker_side == Side::Sell {
                        0u8
                    } else {
                        1u8
                    } // 0=taker, 1=maker
                )
                .unwrap();
                ts_offset += 1;
            }
        }

        // First attempt for trades
        let trades_insert_result = taos.exec(&trades_sql).await;
        if let Err(e) = &trades_insert_result {
            let err_str = e.to_string();
            if err_str.contains("Table does not exist") || err_str.contains("0x2662") {
                tracing::debug!("Trades table not found, auto-creating...");
                // Auto-create missing trades tables
                let mut created_symbols = std::collections::HashSet::new();
                for r in results {
                    if !created_symbols.contains(&r.symbol_id) {
                        let create_sql = format!(
                            "CREATE TABLE IF NOT EXISTS trades_{} USING trades TAGS ({})",
                            r.symbol_id, r.symbol_id
                        );
                        let _ = taos.exec(&create_sql).await;
                        created_symbols.insert(r.symbol_id);
                    }
                }
                // Retry
                taos.exec(&trades_sql).await.map_err(|e| {
                    anyhow::anyhow!("Batch trades insert failed after auto-create: {}", e)
                })?;
            } else {
                return Err(anyhow::anyhow!("Batch trades insert failed: {}", e));
            }
        }
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
#[allow(clippy::too_many_arguments)]
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

    #[tokio::test]
    #[ignore] // Requires TDengine running
    async fn test_batch_insert_me_results() {
        use crate::messages::{MEResult, TradeEvent};
        use crate::models::{OrderStatus, Trade};

        let client =
            crate::persistence::TDengineClient::connect("taos+ws://root:taosdata@localhost:6041")
                .await
                .expect("Failed to connect");

        client.init_schema().await.expect("Failed to init schema");

        // Create test orders
        let mut order1 = InternalOrder::new(100, 1001, 1, 85000_00000000, 1_000000, Side::Buy);
        order1.order_type = OrderType::Limit;
        order1.status = OrderStatus::PARTIALLY_FILLED;
        order1.filled_qty = 500000;
        order1.cid = Some("test-batch-001".to_string());

        let mut order2 = InternalOrder::new(101, 1002, 1, 85100_00000000, 2_000000, Side::Sell);
        order2.order_type = OrderType::Limit;
        order2.status = OrderStatus::NEW;
        order2.cid = Some("test-batch-002".to_string());

        // Create test trade
        let trade = Trade {
            trade_id: 1000,
            buyer_order_id: 100,
            seller_order_id: 101,
            buyer_user_id: 1001,
            seller_user_id: 1002,
            price: 85050_00000000,
            qty: 500000,
            fee: 100,
            role: 1,
        };

        let trade_event = TradeEvent::new(
            trade,
            100,       // taker_order_id
            101,       // maker_order_id
            Side::Buy, // taker_side
            1_000000,  // taker_order_qty
            500000,    // taker_filled_qty
            2_000000,  // maker_order_qty
            500000,    // maker_filled_qty
            1,         // base_asset_id
            2,         // quote_asset_id
            1_000000,  // qty_unit
            0,         // taker_ingested_at_ns
        );

        // Create MEResults
        let results = vec![
            MEResult {
                order: order1,
                trades: vec![trade_event.clone()],
                maker_updates: vec![],
                symbol_id: 1,
                final_status: OrderStatus::PARTIALLY_FILLED,
            },
            MEResult {
                order: order2,
                trades: vec![],
                maker_updates: vec![],
                symbol_id: 1,
                final_status: OrderStatus::NEW,
            },
        ];

        batch_insert_me_results(client.taos(), &results)
            .await
            .expect("Failed to batch insert ME results");

        println!(
            "✅ Batch insert ME results: {} orders, {} trades inserted successfully",
            results.len(),
            results.iter().map(|r| r.trades.len() * 2).sum::<usize>() // buyer + seller
        );
    }

    /// Test that auto-create fallback works when orders/trades tables don't exist
    #[tokio::test]
    #[ignore] // Requires TDengine running
    async fn test_auto_create_orders_trades_tables() {
        let client =
            crate::persistence::TDengineClient::connect("taos+ws://root:taosdata@localhost:6041")
                .await
                .expect("Failed to connect");

        client.init_schema().await.expect("Failed to init schema");

        // Use unique symbol ID to avoid conflicts with other tests
        let test_symbol_id = 9999u32;

        // Drop tables first to simulate missing tables
        let _ = client
            .taos()
            .exec(format!("DROP TABLE IF EXISTS orders_{}", test_symbol_id))
            .await;
        let _ = client
            .taos()
            .exec(format!("DROP TABLE IF EXISTS trades_{}", test_symbol_id))
            .await;

        // Create test order using constructor
        let mut order = InternalOrder::new(
            999999001, // order_id
            99001,     // user_id
            test_symbol_id,
            50000_00000000, // price
            1_00000000,     // qty
            Side::Buy,
        );
        order.order_type = OrderType::Limit;
        order.status = OrderStatus::NEW;
        order.cid = Some("test_autocreate".to_string());

        // Create MEResult with no trades (simpler test)
        let results = vec![crate::messages::MEResult {
            order,
            trades: vec![],
            maker_updates: vec![],
            symbol_id: test_symbol_id,
            final_status: crate::models::OrderStatus::NEW,
        }];

        // This should auto-create the orders table and succeed
        batch_insert_me_results(client.taos(), &results)
            .await
            .expect("Failed to batch insert with auto-create");

        println!("✅ Auto-create orders/trades table test passed");
    }
}
