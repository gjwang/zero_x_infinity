//! Transfer Database Layer
//!
//! PostgreSQL-based persistence for FSM transfer state.
//! All state updates use atomic CAS (Compare-And-Swap) operations.

use sqlx::{PgPool, Row};
use std::time::Duration;

use super::error::TransferError;
use super::state::TransferState;
use super::types::{InternalTransferId, ServiceId, TransferRecord, TransferType};

/// Transfer database operations
pub struct TransferDb {
    pool: PgPool,
}

impl TransferDb {
    /// Create a new TransferDb with the given connection pool
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new transfer record in INIT state
    ///
    /// This function is idempotent: if a transfer with the same cid already exists,
    /// it returns the existing transfer's database id instead of creating a new one.
    /// This prevents double-spend vulnerabilities from duplicate requests.
    pub async fn create(&self, record: &TransferRecord) -> Result<i64, TransferError> {
        // IDEMPOTENCY CHECK: If cid provided, check if transfer already exists
        #[allow(clippy::collapsible_if)]
        if let Some(cid) = &record.cid {
            if let Some(existing) = self.get_by_cid(cid).await? {
                // Found existing transfer with same cid - return its id (idempotent)
                tracing::info!(
                    transfer_id = %existing.transfer_id,
                    cid = %cid,
                    "Transfer with cid already exists - returning existing record (idempotent)"
                );

                // Get database id for the existing transfer
                let db_id = sqlx::query_scalar::<_, i64>(
                    "SELECT id FROM fsm_transfers_tb WHERE transfer_id = $1",
                )
                .bind(existing.transfer_id.to_string())
                .fetch_one(&self.pool)
                .await?;

                return Ok(db_id);
            }
        }

        // No existing transfer found - create new one
        let id = sqlx::query_scalar::<_, i64>(
            r#"
            INSERT INTO fsm_transfers_tb 
                (transfer_id, cid, user_id, asset_id, amount, transfer_type, source_type, state, created_at, updated_at)
            VALUES 
                ($1, $2, $3, $4, $5, $6, $7, $8, NOW(), NOW())
            RETURNING id
            "#,
        )
        .bind(record.transfer_id.to_string())
        .bind(&record.cid)
        .bind(record.user_id as i64)
        .bind(record.asset_id as i32)
        .bind(rust_decimal::Decimal::from(record.amount))
        .bind(record.transfer_type.id())
        .bind(record.source.id())
        .bind(record.state.id())
        .fetch_one(&self.pool)
        .await?;

        Ok(id)
    }

    /// Get a transfer record by transfer_id
    pub async fn get(
        &self,
        transfer_id: InternalTransferId,
    ) -> Result<Option<TransferRecord>, TransferError> {
        let row = sqlx::query(
            r#"
            SELECT transfer_id, transfer_id, cid, user_id, asset_id, amount, 
                   transfer_type, source_type, state, error_message, retry_count,
                   created_at, updated_at
            FROM fsm_transfers_tb
            WHERE transfer_id = $1
            "#,
        )
        .bind(transfer_id.to_string())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(self.row_to_record(&row)?)),
            None => Ok(None),
        }
    }

    /// Get a transfer record by client idempotency key (cid)
    pub async fn get_by_cid(&self, cid: &str) -> Result<Option<TransferRecord>, TransferError> {
        let row = sqlx::query(
            r#"
            SELECT transfer_id, transfer_id, cid, user_id, asset_id, amount, 
                   transfer_type, source_type, state, error_message, retry_count,
                   created_at, updated_at
            FROM fsm_transfers_tb
            WHERE cid = $1
            "#,
        )
        .bind(cid)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => Ok(Some(self.row_to_record(&row)?)),
            None => Ok(None),
        }
    }

    /// Atomic CAS update: Update state only if current state matches expected
    ///
    /// Returns true if update succeeded, false if state didn't match (another worker modified it)
    pub async fn update_state_if(
        &self,
        transfer_id: InternalTransferId,
        expected_state: TransferState,
        new_state: TransferState,
    ) -> Result<bool, TransferError> {
        let result = sqlx::query(
            r#"
            UPDATE fsm_transfers_tb 
            SET state = $1, updated_at = NOW()
            WHERE transfer_id = $2 AND state = $3
            "#,
        )
        .bind(new_state.id())
        .bind(transfer_id.to_string())
        .bind(expected_state.id())
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Atomic CAS update with error message
    pub async fn update_state_with_error(
        &self,
        transfer_id: InternalTransferId,
        expected_state: TransferState,
        new_state: TransferState,
        error: &str,
    ) -> Result<bool, TransferError> {
        let result = sqlx::query(
            r#"
            UPDATE fsm_transfers_tb 
            SET state = $1, error_message = $2, updated_at = NOW()
            WHERE transfer_id = $3 AND state = $4
            "#,
        )
        .bind(new_state.id())
        .bind(error)
        .bind(transfer_id.to_string())
        .bind(expected_state.id())
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Increment retry count for a transfer
    pub async fn increment_retry(
        &self,
        transfer_id: InternalTransferId,
    ) -> Result<(), TransferError> {
        sqlx::query(
            r#"
            UPDATE fsm_transfers_tb 
            SET retry_count = retry_count + 1, updated_at = NOW()
            WHERE transfer_id = $1
            "#,
        )
        .bind(transfer_id.to_string())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Find stale transfers (stuck in non-terminal states for too long)
    ///
    /// Used by recovery worker to resume processing
    pub async fn find_stale(
        &self,
        threshold: Duration,
    ) -> Result<Vec<TransferRecord>, TransferError> {
        let threshold_secs = threshold.as_secs() as i64;

        let rows = sqlx::query(
            r#"
            SELECT transfer_id, transfer_id, cid, user_id, asset_id, amount, 
                   transfer_type, source_type, state, error_message, retry_count,
                   created_at, updated_at
            FROM fsm_transfers_tb
            WHERE state NOT IN ($1, $2, $3)
              AND updated_at < NOW() - INTERVAL '1 second' * $4
            ORDER BY updated_at ASC
            LIMIT 100
            "#,
        )
        .bind(TransferState::Committed.id())
        .bind(TransferState::Failed.id())
        .bind(TransferState::RolledBack.id())
        .bind(threshold_secs)
        .fetch_all(&self.pool)
        .await?;

        let mut records = Vec::with_capacity(rows.len());
        for row in rows {
            records.push(self.row_to_record(&row)?);
        }

        Ok(records)
    }

    /// Convert database row to TransferRecord
    fn row_to_record(&self, row: &sqlx::postgres::PgRow) -> Result<TransferRecord, TransferError> {
        let transfer_id_str: String = row.get("transfer_id");
        let transfer_id: InternalTransferId = transfer_id_str
            .parse()
            .map_err(|_| TransferError::SystemError("Invalid transfer_id format".to_string()))?;

        let state_id: i16 = row.get("state");
        let state = TransferState::from_id(state_id)
            .ok_or_else(|| TransferError::SystemError(format!("Invalid state ID: {}", state_id)))?;

        let source_id: i16 = row.get("source_type");
        let source = ServiceId::from_id(source_id).ok_or_else(|| {
            TransferError::SystemError(format!("Invalid source_type: {}", source_id))
        })?;

        let transfer_type_id: i16 = row.get("transfer_type");
        let transfer_type = TransferType::from_id(transfer_type_id).ok_or_else(|| {
            TransferError::SystemError(format!("Invalid transfer_type: {}", transfer_type_id))
        })?;

        // Derive target from source and transfer_type
        let target = match transfer_type {
            TransferType::FundingToSpot => ServiceId::Trading,
            TransferType::SpotToFunding => ServiceId::Funding,
        };

        let amount: rust_decimal::Decimal = row.get("amount");
        // Use trunc() to drop decimal places, then convert to i64/u64
        // DB stores amount like 50000000.00000000, we need 50000000
        use rust_decimal::prelude::ToPrimitive;

        // P1 Fix: Warn if amount has unexpected fractional part (precision loss detection)
        if amount.fract() != rust_decimal::Decimal::ZERO {
            tracing::warn!(
                transfer_id = %transfer_id_str,
                amount = %amount,
                "Transfer amount has fractional part - truncating"
            );
        }

        let amount_u64 = amount.trunc().to_i64().unwrap_or(0) as u64;

        let created_at: chrono::DateTime<chrono::Utc> = row.get("created_at");
        let updated_at: chrono::DateTime<chrono::Utc> = row.get("updated_at");

        Ok(TransferRecord {
            transfer_id,
            cid: row.get("cid"),
            source,
            target,
            transfer_type,
            user_id: row.get::<i64, _>("user_id") as u64,
            asset_id: row.get::<i32, _>("asset_id") as u32,
            amount: amount_u64,
            state,
            error: row.get("error_message"),
            retry_count: row.get("retry_count"),
            created_at: created_at.timestamp_millis(),
            updated_at: updated_at.timestamp_millis(),
        })
    }
}

// === Adapter Operation Recording (Idempotency) ===

/// Operation type for idempotency tracking
#[derive(Debug, Clone, Copy)]
pub enum OpType {
    Withdraw,
    Deposit,
    Rollback,
    Commit,
}

impl OpType {
    pub fn as_str(&self) -> &'static str {
        match self {
            OpType::Withdraw => "WITHDRAW",
            OpType::Deposit => "DEPOSIT",
            OpType::Rollback => "ROLLBACK",
            OpType::Commit => "COMMIT",
        }
    }
}

/// Record an adapter operation for idempotency
pub async fn record_operation(
    pool: &PgPool,
    transfer_id: InternalTransferId,
    op_type: OpType,
    service: ServiceId,
    result: &str,
    error: Option<&str>,
) -> Result<bool, TransferError> {
    let insert_result = sqlx::query(
        r#"
        INSERT INTO transfer_operations_tb (transfer_id, op_type, service_type, result, error_message)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (transfer_id, op_type, service_type) DO NOTHING
        "#,
    )
    .bind(transfer_id.to_string())
    .bind(op_type.as_str())
    .bind(service.id())
    .bind(result)
    .bind(error)
    .execute(pool)
    .await?;

    // Returns true if inserted (new operation), false if already existed
    Ok(insert_result.rows_affected() > 0)
}

/// Check if an operation was already recorded
pub async fn check_operation(
    pool: &PgPool,
    transfer_id: InternalTransferId,
    op_type: OpType,
    service: ServiceId,
) -> Result<Option<String>, TransferError> {
    let row = sqlx::query(
        r#"
        SELECT result FROM transfer_operations_tb
        WHERE transfer_id = $1 AND op_type = $2 AND service_type = $3
        "#,
    )
    .bind(transfer_id.to_string())
    .bind(op_type.as_str())
    .bind(service.id())
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.get("result")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_op_type_as_str() {
        assert_eq!(OpType::Withdraw.as_str(), "WITHDRAW");
        assert_eq!(OpType::Deposit.as_str(), "DEPOSIT");
        assert_eq!(OpType::Rollback.as_str(), "ROLLBACK");
        assert_eq!(OpType::Commit.as_str(), "COMMIT");
    }
}
