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

/// Batch upsert balance events (efficient multi-row insert)
///
/// Groups events by user+asset, creates subtables if needed,
/// then inserts all values in one SQL statement.
/// Uses static cache to avoid repeated CREATE TABLE calls.
pub async fn batch_upsert_balance_events(
    taos: &Taos,
    events: &[crate::messages::BalanceEvent],
) -> Result<()> {
    use once_cell::sync::Lazy;
    use std::sync::Mutex;

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

    // Build batch INSERT statement
    // TDengine supports: INSERT INTO t1 VALUES (...) t2 VALUES (...) ...
    let mut sql = String::from("INSERT INTO ");
    for event in events {
        let table_name = format!("balances_{}_{}", event.user_id, event.asset_id);
        sql.push_str(&format!(
            "{} VALUES (NOW, {}, {}, 0, 0) ",
            table_name, event.avail_after, event.frozen_after
        ));
    }

    taos.exec(&sql)
        .await
        .map_err(|e| anyhow::anyhow!("Batch balance insert failed: {}", e))?;

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
