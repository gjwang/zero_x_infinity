use crate::models::{Side, Trade};
use anyhow::{Context, Result};
use taos::*;

/// Insert a single trade into TDengine
///
/// For each trade, we insert TWO records (one for buyer, one for seller)
pub async fn insert_trade(taos: &Taos, trade: &Trade, symbol_id: u32) -> Result<()> {
    let table_name = format!("trades_{}", symbol_id);

    // Create subtable if not exists
    let create_subtable = format!(
        "CREATE TABLE IF NOT EXISTS {} USING trades TAGS ({})",
        table_name, symbol_id
    );
    taos.exec(&create_subtable)
        .await
        .map_err(|e| anyhow::anyhow!("{}: {}", "Failed to create trades subtable", e))?;

    // Insert buyer's trade record
    let sql_buyer = format!(
        "INSERT INTO {} VALUES (NOW, {}, {}, {}, {}, {}, {}, {}, {})",
        table_name,
        trade.trade_id,
        trade.buyer_order_id,
        trade.buyer_user_id,
        Side::Buy as u8,
        trade.price,
        trade.qty,
        trade.fee,
        trade.role
    );

    // Insert seller's trade record
    let sql_seller = format!(
        "INSERT INTO {} VALUES (NOW, {}, {}, {}, {}, {}, {}, {}, {})",
        table_name,
        trade.trade_id,
        trade.seller_order_id,
        trade.seller_user_id,
        Side::Sell as u8,
        trade.price,
        trade.qty,
        trade.fee,
        trade.role
    );

    taos.exec(&sql_buyer)
        .await
        .map_err(|e| anyhow::anyhow!("{}: {}", "Failed to insert buyer trade", e))?;
    taos.exec(&sql_seller)
        .await
        .map_err(|e| anyhow::anyhow!("{}: {}", "Failed to insert seller trade", e))?;

    Ok(())
}

/// Batch insert trades for better performance
pub async fn batch_insert_trades(taos: &Taos, trades: &[Trade], symbol_id: u32) -> Result<()> {
    if trades.is_empty() {
        return Ok(());
    }

    for trade in trades {
        insert_trade(taos, trade, symbol_id).await?;
    }

    tracing::debug!("Batch inserted {} trades", trades.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires TDengine running
    async fn test_insert_trade() {
        let client =
            crate::persistence::TDengineClient::connect("taos+ws://root:taosdata@localhost:6041")
                .await
                .expect("Failed to connect");

        client.init_schema().await.expect("Failed to init schema");

        let trade = Trade::new(
            1,              // trade_id
            100,            // buyer_order_id
            101,            // seller_order_id
            1001,           // buyer_user_id
            1002,           // seller_user_id
            85000_00000000, // price
            1_000000,       // qty
        );

        insert_trade(client.taos(), &trade, 1)
            .await
            .expect("Failed to insert trade");
    }
}
