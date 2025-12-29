use super::error::TransferError;
use super::transfer::Transfer;
use super::transfer::{TransferRequest, TransferResponse};
use super::types::AccountType;
use crate::account::{AssetManager, Database};
use rust_decimal::prelude::*;
use sqlx::Row;
use std::str::FromStr;
use utoipa::ToSchema;

pub struct TransferService;

impl TransferService {
    /// Execute an internal transfer
    pub async fn execute(
        db: &Database,
        user_id: i64,
        req: TransferRequest,
    ) -> Result<TransferResponse, TransferError> {
        // 1. Validation
        let from_account =
            AccountType::from_str(&req.from).map_err(|_| TransferError::InvalidAccountType)?;
        let to_account =
            AccountType::from_str(&req.to).map_err(|_| TransferError::InvalidAccountType)?;

        if from_account == to_account {
            return Err(TransferError::SameAccount);
        }

        // Get asset from DB via AssetManager
        let asset = AssetManager::get_by_asset(db.pool(), &req.asset)
            .await
            .map_err(TransferError::DatabaseError)?
            .ok_or_else(|| TransferError::InvalidAsset(req.asset.clone()))?;

        // Scale Amount: String -> i64
        // Logic: Parse as Decimal first to handle "1.23", then multiply by 10^decimals, then round/truncate to i64
        let amount_decimal =
            Decimal::from_str(&req.amount).map_err(|_| TransferError::InvalidAmountFormat)?;

        if amount_decimal <= Decimal::ZERO {
            return Err(TransferError::InvalidAmount);
        }

        let scale_factor = Decimal::from(10u64.pow(asset.decimals as u32));
        let amount_scaled = (amount_decimal * scale_factor)
            .to_i64()
            .ok_or(TransferError::InvalidAmount)?;

        if amount_scaled <= 0 {
            return Err(TransferError::InvalidAmount);
        }

        // 2. Transaction
        let mut tx = db.pool().begin().await?;

        // Lock Source Balance
        let from_balance_row = sqlx::query(
            "SELECT available, version FROM balances_tb 
             WHERE user_id = $1 AND asset_id = $2 AND account_type = $3 
             FOR UPDATE",
        )
        .bind(user_id)
        .bind(asset.asset_id)
        .bind(from_account as i16)
        .fetch_optional(&mut *tx)
        .await?;

        let available: i64 = from_balance_row
            .as_ref()
            .map(|r| r.get("available"))
            .unwrap_or(0);

        if available < amount_scaled {
            return Err(TransferError::InsufficientBalance);
        }

        // Debit Source
        sqlx::query(
            "UPDATE balances_tb SET available = available - $1, version = version + 1
             WHERE user_id = $2 AND asset_id = $3 AND account_type = $4",
        )
        .bind(amount_scaled)
        .bind(user_id)
        .bind(asset.asset_id)
        .bind(from_account as i16)
        .execute(&mut *tx)
        .await?;

        // Credit Target
        sqlx::query(
            "INSERT INTO balances_tb (user_id, asset_id, account_type, available, frozen, version)
             VALUES ($1, $2, $3, $4, 0, 1)
             ON CONFLICT (user_id, asset_id, account_type) 
             DO UPDATE SET available = balances_tb.available + EXCLUDED.available, version = balances_tb.version + 1",
        )
        .bind(user_id)
        .bind(asset.asset_id)
        .bind(to_account as i16)
        .bind(amount_scaled)
        .execute(&mut *tx)
        .await?;

        // Record Transfer
        let transfer_rec: Transfer = sqlx::query_as(
            "INSERT INTO transfers_tb (user_id, asset_id, from_account, to_account, amount)
             VALUES ($1, $2, $3, $4, $5)
             RETURNING transfer_id, user_id, asset_id, from_account, to_account, amount, created_at",
        )
        .bind(user_id)
        .bind(asset.asset_id)
        .bind(from_account as i16)
        .bind(to_account as i16)
        .bind(amount_scaled)
        .fetch_one(&mut *tx)
        .await?;

        tx.commit().await?;

        // 3. Response
        Ok(TransferResponse {
            transfer_id: transfer_rec.transfer_id.to_string(),
            status: "success".to_string(), // Initial design said "pending" but it's atomic success here
            from: req.from,
            to: req.to,
            asset: req.asset,
            amount: req.amount,
            timestamp: transfer_rec.created_at.timestamp_millis(),
        })
    }

    /// Get all balances for a user (all account types)
    /// Returns balances from PostgreSQL balances_tb
    pub async fn get_all_balances(
        pool: &sqlx::PgPool,
        user_id: i64,
    ) -> Result<Vec<BalanceInfo>, TransferError> {
        tracing::debug!("DB: get_all_balances for user_id: {}", user_id);
        let rows = sqlx::query(
            r#"
            SELECT b.user_id, b.asset_id, b.account_type, b.available, b.frozen,
                   a.asset as asset_name, a.decimals
            FROM balances_tb b
            JOIN assets_tb a ON b.asset_id = a.asset_id
            WHERE b.user_id = $1
            ORDER BY b.asset_id, b.account_type
            "#,
        )
        .bind(user_id)
        .fetch_all(pool)
        .await
        .map_err(|e| TransferError::DatabaseError(e))?;

        tracing::info!(
            "DB: get_all_balances found {} rows for user_id: {}",
            rows.len(),
            user_id
        );
        let mut balances = Vec::new();
        for row in rows {
            let asset_id: i32 = row.get("asset_id");
            let account_type: i16 = row.get("account_type");
            // READ as i64 (BIGINT)
            let available: i64 = row.get("available");
            let frozen: i64 = row.get("frozen");
            let asset_name: String = row.get("asset_name");
            let decimals: i16 = row.get("decimals");

            let account_type_name = match account_type {
                1 => "spot",
                2 => "funding",
                _ => "unknown",
            };

            balances.push(BalanceInfo {
                asset_id: asset_id as u32,
                asset: asset_name,
                account_type: account_type_name.to_string(),
                available: unscale_amount(available, decimals as u32),
                frozen: unscale_amount(frozen, decimals as u32),
            });
        }

        Ok(balances)
    }
}

/// Balance info for API response
#[derive(Debug, Clone, serde::Serialize, ToSchema)]
pub struct BalanceInfo {
    pub asset_id: u32,
    pub asset: String,
    pub account_type: String,
    pub available: String,
    pub frozen: String,
}

/// Unscale amount: convert from stored i64 scaled value to human-readable string
/// e.g., stored=100000000, decimals=8 -> "1.00000000"
fn unscale_amount(stored: i64, decimals: u32) -> String {
    let scale_factor = Decimal::from(10u64.pow(decimals));
    let stored_decimal = Decimal::from(stored);
    let human_val = stored_decimal / scale_factor;
    // Format with fixed precision matching the asset decimals
    format!("{:.prec$}", human_val, prec = decimals as usize)
}
