use super::error::TransferError;
use super::transfer::{Transfer, TransferRequest, TransferResponse};
use super::types::AccountType;
use crate::account::{AssetManager, Database};
use rust_decimal::prelude::*;
use std::str::FromStr;

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

        // Parse amount (string -> decimal -> raw i64 based on asset precision?)
        // OR standard float?
        // Project uses `i64` scaled. Default scale 1e18? Or asset specific?
        // Asset struct usually has `decimals`.
        // Let's assume we need to parse based on asset decimals.
        // For simplicity in this iteration, assuming input is raw string representation or we parse standard logic.
        // Let's use string parsing to float then scale? NO, float is bad.
        // Use RustDecimal.

        let amount_decimal =
            Decimal::from_str(&req.amount).map_err(|_| TransferError::InvalidAmountFormat)?;

        if amount_decimal <= Decimal::ZERO {
            return Err(TransferError::InvalidAmount);
        }

        // Scale to i64 (e.g. 1.5 * 10^8)
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
        // Fetch as Decimal
        let from_balance_row = sqlx::query!(
            r#"SELECT available as "available: Decimal", version FROM balances_tb 
             WHERE user_id = $1 AND asset_id = $2 AND account_type = $3 
             FOR UPDATE"#,
            user_id,
            asset.asset_id,
            from_account as i16
        )
        .fetch_optional(&mut *tx)
        .await?;

        let available = from_balance_row
            .map(|r| r.available)
            .unwrap_or(Decimal::ZERO);

        if available < amount_decimal {
            return Err(TransferError::InsufficientBalance);
        }

        // Debit Source
        sqlx::query!(
            "UPDATE balances_tb SET available = available - $1, version = version + 1
             WHERE user_id = $2 AND asset_id = $3 AND account_type = $4",
            amount_decimal,
            user_id,
            asset.asset_id,
            from_account as i16
        )
        .execute(&mut *tx)
        .await?;

        // Credit Target
        // Use amount_decimal for balance update
        sqlx::query!(
            "INSERT INTO balances_tb (user_id, asset_id, account_type, available, frozen, version)
             VALUES ($1, $2, $3, $4, 0, 1)
             ON CONFLICT (user_id, asset_id, account_type) 
             DO UPDATE SET available = balances_tb.available + EXCLUDED.available, version = balances_tb.version + 1",
            user_id,
            asset.asset_id,
            to_account as i16,
            amount_decimal
        )
        .execute(&mut *tx)
        .await?;

        // Record Transfer
        // Use amount_scaled (i64) for transfers_tb
        let transfer_rec = sqlx::query_as!(
            Transfer,
            "INSERT INTO transfers_tb (user_id, asset_id, from_account, to_account, amount)
             VALUES ($1, $2, $3, $4, $5)
             RETURNING transfer_id, user_id, asset_id, from_account, to_account, amount, created_at",
            user_id,
            asset.asset_id,
            from_account as i16,
            to_account as i16,
            amount_scaled
        )
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
}
