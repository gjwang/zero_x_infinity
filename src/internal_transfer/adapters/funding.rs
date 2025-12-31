//! Funding Account Adapter
//!
//! PostgreSQL-based adapter for Funding account balance operations.
//! Uses `balances_tb` with `account_type = FUNDING (2)`.

use async_trait::async_trait;
use sqlx::PgPool;
use tracing::{debug, error, warn};

use super::ServiceAdapter;
use crate::internal_transfer::db::{OpType, check_operation, record_operation};
use crate::internal_transfer::types::{InternalTransferId, OpResult, ScaledAmount, ServiceId};

/// Funding account adapter
///
/// Implements balance operations directly against PostgreSQL.
pub struct FundingAdapter {
    pool: PgPool,
}

/// Account type ID for Funding in balances_tb
const FUNDING_ACCOUNT_TYPE: i16 = 2;

impl FundingAdapter {
    /// Create a new FundingAdapter
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ServiceAdapter for FundingAdapter {
    fn name(&self) -> &'static str {
        "Funding"
    }

    async fn withdraw(
        &self,
        transfer_id: InternalTransferId,
        user_id: u64,
        asset_id: u32,
        amount: ScaledAmount,
    ) -> OpResult {
        debug!(
            transfer_id = %transfer_id,
            user_id = user_id,
            asset_id = asset_id,
            amount = *amount,
            "Funding withdraw"
        );

        // Check idempotency - was this already processed?
        match check_operation(
            &self.pool,
            transfer_id,
            OpType::Withdraw,
            ServiceId::Funding,
        )
        .await
        {
            Ok(Some(result)) => {
                debug!(transfer_id = %transfer_id, result = %result, "Funding withdraw already processed");
                return if result == "SUCCESS" {
                    OpResult::Success
                } else {
                    OpResult::Failed("Previously failed".to_string())
                };
            }
            Err(e) => {
                warn!(transfer_id = %transfer_id, error = %e, "Failed to check idempotency");
                return OpResult::Pending;
            }
            Ok(None) => {}
        }

        // Start transaction
        let mut tx = match self.pool.begin().await {
            Ok(tx) => tx,
            Err(e) => {
                error!(transfer_id = %transfer_id, error = %e, "Failed to start transaction");
                return OpResult::Pending;
            }
        };

        // Lock and check balance with SELECT FOR UPDATE
        let balance_row = sqlx::query(
            r#"
            SELECT available, status FROM balances_tb
            WHERE user_id = $1 AND asset_id = $2 AND account_type = $3
            FOR UPDATE
            "#,
        )
        .bind(user_id as i64)
        .bind(asset_id as i32)
        .bind(FUNDING_ACCOUNT_TYPE)
        .fetch_optional(&mut *tx)
        .await;

        // Get asset decimals
        let _decimals: i16 = match sqlx::query_scalar!(
            "SELECT internal_scale FROM assets_tb WHERE asset_id = $1",
            asset_id as i32
        )
        .fetch_optional(&mut *tx)
        .await
        {
            Ok(Some(d)) => d,
            Ok(None) => {
                let _ = tx.rollback().await;
                debug!(transfer_id = %transfer_id, "Funding withdraw failed: Asset not found");
                return OpResult::Failed("Asset not found".to_string());
            }
            Err(e) => {
                let _ = tx.rollback().await;
                debug!(transfer_id = %transfer_id, error = %e, "Failed to fetch asset decimals");
                return OpResult::Pending;
            }
        };

        use crate::db::SafeRow;
        let (available, account_status) = match balance_row {
            Ok(Some(row)) => {
                let available: i64 = match row.try_get_log("available") {
                    Some(v) => v,
                    None => {
                        error!(transfer_id = %transfer_id, "Critical: available column missing in balances_tb");
                        let _ = tx.rollback().await;
                        return OpResult::Pending;
                    }
                };
                let status: i16 = match row.try_get_log("status") {
                    Some(v) => v,
                    None => {
                        error!(transfer_id = %transfer_id, "Critical: status column missing in balances_tb");
                        let _ = tx.rollback().await;
                        return OpResult::Pending;
                    }
                };
                (available, status)
            }
            Ok(None) => {
                // Account doesn't exist
                let _ = tx.rollback().await;
                let _ = record_operation(
                    &self.pool,
                    transfer_id,
                    OpType::Withdraw,
                    ServiceId::Funding,
                    "FAILED",
                    Some("Account not found"),
                )
                .await;
                debug!(transfer_id = %transfer_id, "Funding withdraw failed: Account not found");
                return OpResult::Failed("Account not found".to_string());
            }
            Err(e) => {
                error!(transfer_id = %transfer_id, error = %e, "Failed to query balance");
                let _ = tx.rollback().await;
                return OpResult::Pending;
            }
        };

        // === Defense-in-Depth Layer 3: Adapter Account Status Check (§1.5.5) ===
        // Status codes: 1=ACTIVE, 2=FROZEN, 3=DISABLED
        if account_status == 2 {
            let _ = tx.rollback().await;
            let _ = record_operation(
                &self.pool,
                transfer_id,
                OpType::Withdraw,
                ServiceId::Funding,
                "FAILED",
                Some("Account is frozen"),
            )
            .await;
            debug!(transfer_id = %transfer_id, "Funding withdraw failed: Account is frozen");
            return OpResult::Failed("Account is frozen".to_string());
        }

        if account_status == 3 {
            let _ = tx.rollback().await;
            let _ = record_operation(
                &self.pool,
                transfer_id,
                OpType::Withdraw,
                ServiceId::Funding,
                "FAILED",
                Some("Account is disabled"),
            )
            .await;
            debug!(transfer_id = %transfer_id, "Funding withdraw failed: Account is disabled");
            return OpResult::Failed("Account is disabled".to_string());
        }

        // Check sufficient balance
        // use atomic units directly
        if available < *amount as i64 {
            let _ = tx.rollback().await;
            let _ = record_operation(
                &self.pool,
                transfer_id,
                OpType::Withdraw,
                ServiceId::Funding,
                "FAILED",
                Some("Insufficient balance"),
            )
            .await;
            debug!(transfer_id = %transfer_id, available = %available, amount = %amount, "Funding withdraw failed: Insufficient balance");
            return OpResult::Failed("Insufficient balance".to_string());
        }

        // Deduct balance
        let update_result = sqlx::query(
            r#"
            UPDATE balances_tb
            SET available = available - $1, version = version + 1
            WHERE user_id = $2 AND asset_id = $3 AND account_type = $4
            "#,
        )
        .bind(*amount as i64)
        .bind(user_id as i64)
        .bind(asset_id as i32)
        .bind(FUNDING_ACCOUNT_TYPE)
        .execute(&mut *tx)
        .await;

        if let Err(e) = update_result {
            error!(transfer_id = %transfer_id, error = %e, "Failed to update balance");
            let _ = tx.rollback().await;
            return OpResult::Pending;
        }

        // Commit transaction
        if let Err(e) = tx.commit().await {
            error!(transfer_id = %transfer_id, error = %e, "Failed to commit transaction");
            return OpResult::Pending;
        }

        // Record success for idempotency
        let _ = record_operation(
            &self.pool,
            transfer_id,
            OpType::Withdraw,
            ServiceId::Funding,
            "SUCCESS",
            None,
        )
        .await;

        debug!(transfer_id = %transfer_id, "Funding withdraw successful");
        OpResult::Success
    }

    async fn deposit(
        &self,
        transfer_id: InternalTransferId,
        user_id: u64,
        asset_id: u32,
        amount: ScaledAmount,
    ) -> OpResult {
        debug!(
            transfer_id = %transfer_id,
            user_id = user_id,
            asset_id = asset_id,
            amount = *amount,
            "Funding deposit"
        );

        // Check idempotency
        match check_operation(&self.pool, transfer_id, OpType::Deposit, ServiceId::Funding).await {
            Ok(Some(result)) => {
                debug!(transfer_id = %transfer_id, result = %result, "Funding deposit already processed");
                return if result == "SUCCESS" {
                    OpResult::Success
                } else {
                    OpResult::Failed("Previously failed".to_string())
                };
            }
            Err(e) => {
                warn!(transfer_id = %transfer_id, error = %e, "Failed to check idempotency");
                return OpResult::Pending;
            }
            Ok(None) => {}
        }

        // Get asset decimals
        let _decimals: i16 = match sqlx::query_scalar!(
            "SELECT internal_scale FROM assets_tb WHERE asset_id = $1",
            asset_id as i32
        )
        .fetch_optional(&self.pool)
        .await
        {
            Ok(Some(d)) => d,
            Ok(None) => {
                debug!(transfer_id = %transfer_id, "Funding deposit failed: Asset not found");
                return OpResult::Failed("Asset not found".to_string());
            }
            Err(e) => {
                debug!(transfer_id = %transfer_id, error = %e, "Failed to fetch asset decimals");
                return OpResult::Pending;
            }
        };

        // UPSERT balance (create if not exists, or add to existing)
        // UPSERT balance (create if not exists, or add to existing)
        let result = sqlx::query(
            r#"
            INSERT INTO balances_tb (user_id, asset_id, account_type, available, frozen, version)
            VALUES ($1, $2, $3, $4, 0, 1)
            ON CONFLICT (user_id, asset_id, account_type)
            DO UPDATE SET available = balances_tb.available + EXCLUDED.available,
                          version = balances_tb.version + 1
            "#,
        )
        .bind(user_id as i64)
        .bind(asset_id as i32)
        .bind(FUNDING_ACCOUNT_TYPE)
        .bind(*amount as i64)
        .execute(&self.pool)
        .await;

        match result {
            Ok(_) => {
                let _ = record_operation(
                    &self.pool,
                    transfer_id,
                    OpType::Deposit,
                    ServiceId::Funding,
                    "SUCCESS",
                    None,
                )
                .await;
                debug!(transfer_id = %transfer_id, "Funding deposit successful");
                OpResult::Success
            }
            Err(e) => {
                error!(transfer_id = %transfer_id, error = %e, "Failed to deposit");
                OpResult::Pending
            }
        }
    }

    async fn rollback(&self, transfer_id: InternalTransferId) -> OpResult {
        debug!(transfer_id = %transfer_id, "Funding rollback");

        // Check idempotency
        match check_operation(
            &self.pool,
            transfer_id,
            OpType::Rollback,
            ServiceId::Funding,
        )
        .await
        {
            Ok(Some(_)) => {
                debug!(transfer_id = %transfer_id, "Funding rollback already processed");
                return OpResult::Success;
            }
            Err(e) => {
                warn!(transfer_id = %transfer_id, error = %e, "Failed to check idempotency");
                return OpResult::Pending;
            }
            Ok(None) => {}
        }

        // Get original withdraw details from transfer record
        // This requires looking up the original transfer to reverse it
        // For now, we'll query the fsm_transfers_tb
        let transfer_row = sqlx::query(
            r#"
            SELECT user_id, asset_id, amount FROM fsm_transfers_tb WHERE transfer_id = $1
            "#,
        )
        .bind(transfer_id.to_string())
        .fetch_optional(&self.pool)
        .await;

        use crate::db::SafeRow;
        let (user_id, asset_id, amount) = match transfer_row {
            Ok(Some(row)) => {
                let user_id: i64 = match row.try_get_log("user_id") {
                    Some(v) => v,
                    None => {
                        error!(transfer_id = %transfer_id, "Critical: user_id missing in fsm_transfers_tb");
                        return OpResult::Pending;
                    }
                };
                let asset_id: i32 = match row.try_get_log("asset_id") {
                    Some(v) => v,
                    None => {
                        error!(transfer_id = %transfer_id, "Critical: asset_id missing in fsm_transfers_tb");
                        return OpResult::Pending;
                    }
                };
                let amount: i64 = match row.try_get_log("amount") {
                    Some(v) => v,
                    None => {
                        error!(transfer_id = %transfer_id, "Critical: amount missing in fsm_transfers_tb");
                        return OpResult::Pending;
                    }
                };
                (user_id, asset_id, amount)
            }
            Ok(None) => {
                error!(transfer_id = %transfer_id, "Transfer not found for rollback");
                return OpResult::Failed("Transfer not found".to_string());
            }
            Err(e) => {
                error!(transfer_id = %transfer_id, error = %e, "Failed to query transfer");
                return OpResult::Pending;
            }
        };

        // Get asset decimals
        let _decimals: i16 = match sqlx::query_scalar!(
            "SELECT internal_scale FROM assets_tb WHERE asset_id = $1",
            asset_id as i32
        )
        .fetch_optional(&self.pool)
        .await
        {
            Ok(Some(d)) => d,
            Ok(None) => return OpResult::Failed("Asset not found for rollback".to_string()),
            Err(_) => return OpResult::Pending,
        };

        // Credit back the withdrawn amount (same as deposit logic)
        let result = sqlx::query(
            r#"
            UPDATE balances_tb
            SET available = available + $1, version = version + 1
            WHERE user_id = $2 AND asset_id = $3 AND account_type = $4
            "#,
        )
        .bind(amount) // amount is i64 here from rollback query
        .bind(user_id)
        .bind(asset_id)
        .bind(FUNDING_ACCOUNT_TYPE)
        .execute(&self.pool)
        .await;

        match result {
            Ok(_) => {
                let _ = record_operation(
                    &self.pool,
                    transfer_id,
                    OpType::Rollback,
                    ServiceId::Funding,
                    "SUCCESS",
                    None,
                )
                .await;
                debug!(transfer_id = %transfer_id, "Funding rollback successful");
                OpResult::Success
            }
            Err(e) => {
                error!(transfer_id = %transfer_id, error = %e, "Failed to rollback");
                OpResult::Pending
            }
        }
    }

    async fn commit(&self, transfer_id: InternalTransferId) -> OpResult {
        debug!(transfer_id = %transfer_id, "Funding commit");

        // For Funding, commit is a no-op (we don't use locks)
        // Just record for audit trail
        let _ = record_operation(
            &self.pool,
            transfer_id,
            OpType::Commit,
            ServiceId::Funding,
            "SUCCESS",
            None,
        )
        .await;

        OpResult::Success
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_funding_account_type() {
        assert_eq!(super::FUNDING_ACCOUNT_TYPE, 2);
    }
}
