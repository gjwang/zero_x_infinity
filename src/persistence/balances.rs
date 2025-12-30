use crate::balance::Balance;
use anyhow::Result;
use taos::*;

/// Snapshot user balance to TDengine
pub async fn snapshot_balance(
    taos: &Taos,
    user_id: u64,
    asset_id: u32,
    balance: &Balance,
) -> Result<()> {
    let table_name = format!("balances_{}_{}", user_id, asset_id);

    // Create subtable if not exists
    let create_subtable = format!(
        "CREATE TABLE IF NOT EXISTS {} USING balances TAGS ({}, {})",
        table_name, user_id, asset_id
    );
    taos.exec(&create_subtable)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create balances subtable: {}", e))?;

    // Insert balance snapshot (use accessor methods for private fields)
    let sql = format!(
        "INSERT INTO {} VALUES (NOW, {}, {}, {}, {})",
        table_name,
        balance.avail(),
        balance.frozen(),
        balance.lock_version(),
        balance.settle_version()
    );

    taos.exec(&sql)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to insert balance snapshot: {}", e))?;

    Ok(())
}

/// Batch snapshot balances for multiple users/assets
pub async fn batch_snapshot_balances(
    taos: &Taos,
    snapshots: &[(u64, u32, Balance)], // (user_id, asset_id, balance)
) -> Result<()> {
    if snapshots.is_empty() {
        return Ok(());
    }

    for (user_id, asset_id, balance) in snapshots {
        snapshot_balance(taos, *user_id, *asset_id, balance).await?;
    }

    tracing::debug!("Batch snapshotted {} balances", snapshots.len());
    Ok(())
}

/// Query latest balance for a user and asset
///
/// Note: This is a placeholder implementation.
/// TDengine query API needs proper type handling.
pub async fn query_latest_balance(
    _taos: &Taos,
    _user_id: u64,
    _asset_id: u32,
) -> Result<Option<Balance>> {
    // TODO: Implement proper TDengine query with correct type handling
    // The taos crate's query API requires careful handling of result types
    Ok(None)
}

/// Insert balance values directly (from BalanceEvent)
pub async fn upsert_balance_values(
    taos: &Taos,
    user_id: u64,
    asset_id: u32,
    avail: u64,
    frozen: u64,
) -> Result<()> {
    let table_name = format!("balances_{}_{}", user_id, asset_id);

    // Create subtable if not exists
    let create_subtable = format!(
        "CREATE TABLE IF NOT EXISTS {} USING balances TAGS ({}, {})",
        table_name, user_id, asset_id
    );
    taos.exec(&create_subtable)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create balances subtable: {}", e))?;

    // Insert balance values (use 0 for version fields)
    let sql = format!(
        "INSERT INTO {} VALUES (NOW, {}, {}, 0, 0)",
        table_name, avail, frozen
    );

    taos.exec(&sql)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to insert balance values: {}", e))?;

    Ok(())
}

/// Fast batch upsert balance events using single SQL statement
///
/// Optimized for high-frequency writes:
/// - One network round-trip (single exec() call)
/// - No Mutex locks
/// - Pre-allocated string buffer
/// - No Stmt set_tbname overhead
///
/// Note: Tables must exist (created during user onboarding, not here)
pub async fn batch_upsert_balance_events(
    taos: &Taos,
    events: &[crate::messages::BalanceEvent],
) -> Result<()> {
    use std::fmt::Write;

    if events.is_empty() {
        return Ok(());
    }

    // Build batch INSERT SQL
    let now_us = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as i64;

    let mut sql = String::with_capacity(events.len() * 100 + 20);
    sql.push_str("INSERT INTO ");

    for (i, event) in events.iter().enumerate() {
        write!(
            sql,
            "trading.balances_{}_{} VALUES({}, {}, {}, 0, 0) ",
            event.user_id,
            event.asset_id,
            now_us + i as i64,
            event.avail_after,
            event.frozen_after
        )
        .unwrap();
    }

    // First attempt
    match taos.exec(&sql).await {
        Ok(_) => return Ok(()),
        Err(e) => {
            let err_str = e.to_string();
            // Check if error is "table does not exist" (TDengine error code 0x2662)
            if !err_str.contains("Table does not exist") && !err_str.contains("0x2662") {
                return Err(anyhow::anyhow!("Batch balance insert failed: {}", e));
            }
            tracing::debug!("Table not found, auto-creating missing balance tables...");
        }
    }

    // Auto-create missing tables and retry
    let mut created = std::collections::HashSet::new();
    for event in events {
        let key = (event.user_id, event.asset_id);
        if !created.contains(&key) {
            let create_sql = format!(
                "CREATE TABLE IF NOT EXISTS trading.balances_{}_{} USING trading.balances TAGS ({}, {})",
                event.user_id, event.asset_id, event.user_id, event.asset_id
            );
            let _ = taos.exec(&create_sql).await; // Ignore error (may already exist)
            created.insert(key);
        }
    }

    // Retry INSERT
    taos.exec(&sql)
        .await
        .map_err(|e| anyhow::anyhow!("Batch balance insert failed after auto-create: {}", e))?;

    Ok(())
}

/// Batch insert balance events to balance_events table (Event Sourcing)
///
/// This writes the FULL event record for audit trail, fee tracking, etc.
/// Different from batch_upsert_balance_events which only updates balance snapshots.
///
/// Note: Uses dual TAGs (user_id, account_type) per design doc 4.2
pub async fn batch_insert_balance_events(
    taos: &Taos,
    events: &[crate::messages::BalanceEvent],
    account_type: u8, // 1=Spot, 2=Funding
) -> Result<()> {
    use std::fmt::Write;

    if events.is_empty() {
        return Ok(());
    }

    let now_us = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as i64;

    // Build batch INSERT SQL
    // Schema: ts, event_type, trade_id, source_id, asset_id, delta, avail_after, frozen_after, from_user
    let mut sql = String::with_capacity(events.len() * 150 + 20);
    sql.push_str("INSERT INTO ");

    for (i, event) in events.iter().enumerate() {
        // Table name: balance_events_{user_id}_{account_type}
        // Schema: ts, event_type, trade_id, source_id, asset_id, delta, avail_after, frozen_after, from_user, fee_amount
        write!(
            sql,
            "balance_events_{}_{} VALUES({}, {}, {}, {}, {}, {}, {}, {}, 0, {}) ",
            event.user_id,
            account_type,
            now_us + i as i64,
            event.event_type as u8, // Maps to TINYINT
            event.source_id,        // trade_id for Trade events
            event.source_id,        // source_id
            event.asset_id,
            event.delta,
            event.avail_after,
            event.frozen_after,
            // from_user = 0 (placeholder)
            event.fee_amount, // Fee deducted (Settle events only)
        )
        .unwrap();
    }

    // First attempt
    match taos.exec(&sql).await {
        Ok(_) => return Ok(()),
        Err(e) => {
            let err_str = e.to_string();
            if !err_str.contains("Table does not exist") && !err_str.contains("0x2662") {
                return Err(anyhow::anyhow!("Batch balance_events insert failed: {}", e));
            }
            tracing::debug!("balance_events table not found, auto-creating...");
        }
    }

    // Auto-create missing tables and retry
    let mut created = std::collections::HashSet::new();
    for event in events {
        let key = (event.user_id, account_type);
        if !created.contains(&key) {
            let create_sql = format!(
                "CREATE TABLE IF NOT EXISTS balance_events_{}_{} USING balance_events TAGS ({}, {})",
                event.user_id, account_type, event.user_id, account_type
            );
            let _ = taos.exec(&create_sql).await;
            created.insert(key);
        }
    }

    // Retry INSERT
    taos.exec(&sql).await.map_err(|e| {
        anyhow::anyhow!(
            "Batch balance_events insert failed after auto-create: {}",
            e
        )
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires TDengine running
    async fn test_snapshot_balance() {
        let client =
            crate::persistence::TDengineClient::connect("taos+ws://root:taosdata@localhost:6041")
                .await
                .expect("Failed to connect");

        client.init_schema().await.expect("Failed to init schema");

        let mut balance = Balance::default();
        balance.deposit(100_00000000).expect("Failed to deposit");
        balance.lock(10_00000000).expect("Failed to lock");

        snapshot_balance(client.taos(), 1001, 1, &balance)
            .await
            .expect("Failed to snapshot balance");
    }

    #[tokio::test]
    #[ignore] // Requires TDengine running
    async fn test_batch_upsert_balance_events() {
        let client =
            crate::persistence::TDengineClient::connect("taos+ws://root:taosdata@localhost:6041")
                .await
                .expect("Failed to connect");

        client.init_schema().await.expect("Failed to init schema");

        // Create test balance events using the new() constructor
        use crate::messages::{BalanceEventType, SourceType};

        let events = vec![
            crate::messages::BalanceEvent::new(
                1001,                   // user_id
                1,                      // asset_id
                BalanceEventType::Lock, // event_type
                1,                      // version
                SourceType::Order,      // source_type
                100,                    // source_id
                -10_00000000,           // delta
                90_00000000,            // avail_after
                10_00000000,            // frozen_after
                0,                      // ingested_at_ns
            ),
            crate::messages::BalanceEvent::new(
                1001,                   // user_id
                1,                      // asset_id
                BalanceEventType::Lock, // event_type
                2,                      // version
                SourceType::Order,      // source_type
                101,                    // source_id
                -10_00000000,           // delta
                80_00000000,            // avail_after
                20_00000000,            // frozen_after
                0,                      // ingested_at_ns
            ),
            crate::messages::BalanceEvent::new(
                1002,                   // user_id
                1,                      // asset_id
                BalanceEventType::Lock, // event_type
                1,                      // version
                SourceType::Order,      // source_type
                102,                    // source_id
                -5_00000000,            // delta
                45_00000000,            // avail_after
                5_00000000,             // frozen_after
                0,                      // ingested_at_ns
            ),
        ];

        batch_upsert_balance_events(client.taos(), &events)
            .await
            .expect("Failed to batch upsert balance events");

        println!(
            "✅ Batch upsert balance events: {} events inserted successfully",
            events.len()
        );
    }

    /// Test that auto-create fallback works when tables don't exist
    #[tokio::test]
    #[ignore] // Requires TDengine running
    async fn test_auto_create_balance_tables() {
        let client =
            crate::persistence::TDengineClient::connect("taos+ws://root:taosdata@localhost:6041")
                .await
                .expect("Failed to connect");

        client.init_schema().await.expect("Failed to init schema");

        // Use unique user/asset IDs to avoid conflicts with other tests
        let test_user_id = 9999001u64;
        let test_asset_id = 99u32;

        // Drop the table first to simulate missing table
        let drop_sql = format!(
            "DROP TABLE IF EXISTS balances_{}_{}",
            test_user_id, test_asset_id
        );
        let _ = client.taos().exec(&drop_sql).await;

        // Create test event for the dropped table
        use crate::messages::{BalanceEventType, SourceType};
        let events = vec![crate::messages::BalanceEvent::new(
            test_user_id,
            test_asset_id,
            BalanceEventType::Lock,
            1,
            SourceType::Order,
            1000,
            -10_00000000,
            90_00000000,
            10_00000000,
            0,
        )];

        // This should auto-create the table and succeed
        batch_upsert_balance_events(client.taos(), &events)
            .await
            .expect("Failed to batch upsert with auto-create");

        println!("✅ Auto-create balance table test passed");
    }
}
