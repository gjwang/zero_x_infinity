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

/// Batch upsert balance events using Stmt parameter binding
///
/// Uses prepared statement with parameter binding for better performance.
/// Skips SQL parsing by sending binary data directly.
/// Uses static cache to avoid repeated CREATE TABLE calls.
pub async fn batch_upsert_balance_events(
    taos: &Taos,
    events: &[crate::messages::BalanceEvent],
) -> Result<()> {
    use once_cell::sync::Lazy;
    use std::sync::Mutex;
    use taos::AsyncBindable;
    use taos_query::prelude::ColumnView;

    // Cache of already-created tables (survives across calls)
    static CREATED_TABLES: Lazy<Mutex<std::collections::HashSet<(u64, u32)>>> =
        Lazy::new(|| Mutex::new(std::collections::HashSet::new()));

    if events.is_empty() {
        return Ok(());
    }

    // Find tables that need creation (not in cache)
    let mut new_tables: Vec<(u64, u32)> = Vec::new();
    {
        let created = CREATED_TABLES.lock().unwrap();
        for event in events {
            let key = (event.user_id, event.asset_id);
            if !created.contains(&key) && !new_tables.contains(&key) {
                new_tables.push(key);
            }
        }
    }

    // Create only NEW subtables
    for (user_id, asset_id) in &new_tables {
        let table_name = format!("balances_{}_{}", user_id, asset_id);
        let create_subtable = format!(
            "CREATE TABLE IF NOT EXISTS {} USING balances TAGS ({}, {})",
            table_name, user_id, asset_id
        );
        if taos.exec(&create_subtable).await.is_ok() {
            // Add to cache on success
            CREATED_TABLES.lock().unwrap().insert((*user_id, *asset_id));
        }
    }

    // Group events by (user_id, asset_id) for batch binding
    let mut grouped: std::collections::HashMap<(u64, u32), Vec<&crate::messages::BalanceEvent>> =
        std::collections::HashMap::new();
    for event in events {
        grouped
            .entry((event.user_id, event.asset_id))
            .or_default()
            .push(event);
    }

    // Use Stmt for each table group
    // balances schema: ts TIMESTAMP, avail BIGINT UNSIGNED, frozen BIGINT UNSIGNED,
    //                  lock_version BIGINT, settle_version BIGINT
    let mut stmt: taos::Stmt = <taos::Stmt as taos::AsyncBindable<Taos>>::init(taos)
        .await
        .map_err(|e| anyhow::anyhow!("Stmt init failed: {}", e))?;
    stmt.prepare("INSERT INTO ? VALUES(?, ?, ?, ?, ?)")
        .await
        .map_err(|e| anyhow::anyhow!("Stmt prepare failed: {}", e))?;

    for ((user_id, asset_id), table_events) in grouped {
        let table_name = format!("balances_{}_{}", user_id, asset_id);
        stmt.set_tbname(&table_name)
            .await
            .map_err(|e| anyhow::anyhow!("Stmt set_tbname failed: {}", e))?;

        // Build columns for batch binding
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        let timestamps: Vec<i64> = (0..table_events.len()).map(|i| now_ms + i as i64).collect();
        let avails: Vec<u64> = table_events.iter().map(|e| e.avail_after).collect();
        let frozens: Vec<u64> = table_events.iter().map(|e| e.frozen_after).collect();
        let lock_versions: Vec<u64> = vec![0; table_events.len()];
        let settle_versions: Vec<u64> = vec![0; table_events.len()];

        let params = vec![
            ColumnView::from_millis_timestamp(timestamps),
            ColumnView::from_unsigned_big_ints(avails),
            ColumnView::from_unsigned_big_ints(frozens),
            ColumnView::from_unsigned_big_ints(lock_versions),
            ColumnView::from_unsigned_big_ints(settle_versions),
        ];

        stmt.bind(&params)
            .await
            .map_err(|e| anyhow::anyhow!("Stmt bind failed: {}", e))?;
        stmt.add_batch()
            .await
            .map_err(|e| anyhow::anyhow!("Stmt add_batch failed: {}", e))?;
    }

    stmt.execute()
        .await
        .map_err(|e| anyhow::anyhow!("Stmt execute failed: {}", e))?;
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
}
