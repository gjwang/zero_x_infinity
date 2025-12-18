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

        let balance = Balance::new(
            100_00000000, // avail
            10_00000000,  // frozen
        );

        snapshot_balance(client.taos(), 1001, 1, &balance)
            .await
            .expect("Failed to snapshot balance");
    }
}
