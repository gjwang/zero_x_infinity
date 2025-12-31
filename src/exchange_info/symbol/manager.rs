//! Symbol manager for loading and querying trading pairs

use super::models::Symbol;
use crate::exchange_info::validation;
use sqlx::PgPool;

/// Symbol manager for loading and caching symbols
pub struct SymbolManager;

impl SymbolManager {
    /// Load all active symbols
    pub async fn load_all(pool: &PgPool) -> Result<Vec<Symbol>, sqlx::Error> {
        let rows: Vec<Symbol> = sqlx::query_as(
            r#"SELECT symbol_id, symbol, base_asset_id, quote_asset_id,
                      price_scale, price_precision, qty_scale, qty_precision, min_qty, status, symbol_flags,
                      base_maker_fee, base_taker_fee
               FROM symbols_tb WHERE status = 1"#,
        )
        .fetch_all(pool)
        .await?;

        Ok(rows)
    }

    /// Get symbol by ID
    pub async fn get_by_id(pool: &PgPool, symbol_id: i32) -> Result<Option<Symbol>, sqlx::Error> {
        let row: Option<Symbol> = sqlx::query_as(
            r#"SELECT symbol_id, symbol, base_asset_id, quote_asset_id,
                      price_scale, price_precision, qty_scale, qty_precision, min_qty, status, symbol_flags,
                      base_maker_fee, base_taker_fee
               FROM symbols_tb WHERE symbol_id = $1"#,
        )
        .bind(symbol_id)
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }

    /// Get symbol by symbol name (e.g., "BTC_USDT")
    ///
    /// # Validation
    /// Input must be uppercase and match format ^[A-Z0-9]+_[A-Z0-9]+$
    pub async fn get_by_symbol(pool: &PgPool, symbol: &str) -> Result<Option<Symbol>, sqlx::Error> {
        // Validate input using SymbolName
        let symbol_name = validation::SymbolName::new(symbol)
            .map_err(|e| sqlx::Error::Protocol(format!("Invalid symbol name: {}", e)))?;

        let row: Option<Symbol> = sqlx::query_as(
            r#"SELECT symbol_id, symbol, base_asset_id, quote_asset_id, 
                      price_scale, price_precision, qty_scale, qty_precision, min_qty, status, symbol_flags,
                      base_maker_fee, base_taker_fee
               FROM symbols_tb WHERE symbol = $1"#,
        )
        .bind(symbol_name.as_str())
        .fetch_optional(pool)
        .await?;

        Ok(row)
    }
}
