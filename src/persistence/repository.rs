//! Repository traits for data access abstraction
//!
//! This module provides traits that abstract data access, enabling:
//! - Testability through mock implementations
//! - Flexibility to swap data sources
//! - Clean separation between business logic and infrastructure

use anyhow::Result;
use async_trait::async_trait;

use super::queries::{BalanceApiData, OrderApiData, PublicTradeApiData, TradeApiData};

// ============================================================================
// Order Repository
// ============================================================================

/// Repository trait for Order data access
#[async_trait]
pub trait OrderRepository: Send + Sync {
    /// Get a single order by ID
    async fn get_order(&self, order_id: u64) -> Result<Option<OrderApiData>>;

    /// Get orders for a user with limit
    async fn get_orders(&self, user_id: u64, limit: usize) -> Result<Vec<OrderApiData>>;
}

// ============================================================================
// Balance Repository
// ============================================================================

/// Repository trait for Balance data access
#[async_trait]
pub trait BalanceRepository: Send + Sync {
    /// Get balance for a specific asset
    async fn get_balance(&self, user_id: u64, asset_id: u32) -> Result<Option<BalanceApiData>>;

    /// Get all balances for a user
    async fn get_all_balances(&self, user_id: u64) -> Result<Vec<BalanceApiData>>;
}

// ============================================================================
// Trade Repository
// ============================================================================

/// Repository trait for Trade data access
#[async_trait]
pub trait TradeRepository: Send + Sync {
    /// Get trades for a symbol with limit
    async fn get_trades(&self, limit: usize) -> Result<Vec<TradeApiData>>;

    /// Get public trades for market data
    async fn get_public_trades(
        &self,
        limit: usize,
        from_id: Option<i64>,
    ) -> Result<Vec<PublicTradeApiData>>;
}

// ============================================================================
// TDengine Implementation
// ============================================================================

use std::sync::Arc;
use taos::Taos;

use crate::symbol_manager::SymbolManager;

/// TDengine-backed order repository implementation
pub struct TDengineOrderRepository {
    taos: Arc<Taos>,
    symbol_mgr: Arc<SymbolManager>,
    symbol_id: u32,
}

impl TDengineOrderRepository {
    pub fn new(taos: Arc<Taos>, symbol_mgr: Arc<SymbolManager>, symbol_id: u32) -> Self {
        Self {
            taos,
            symbol_mgr,
            symbol_id,
        }
    }
}

#[async_trait]
impl OrderRepository for TDengineOrderRepository {
    async fn get_order(&self, order_id: u64) -> Result<Option<OrderApiData>> {
        super::queries::query_order(&self.taos, order_id, self.symbol_id, &self.symbol_mgr).await
    }

    async fn get_orders(&self, user_id: u64, limit: usize) -> Result<Vec<OrderApiData>> {
        super::queries::query_orders(&self.taos, user_id, self.symbol_id, limit, &self.symbol_mgr)
            .await
    }
}

/// TDengine-backed balance repository implementation
pub struct TDengineBalanceRepository {
    taos: Arc<Taos>,
    symbol_mgr: Arc<SymbolManager>,
}

impl TDengineBalanceRepository {
    pub fn new(taos: Arc<Taos>, symbol_mgr: Arc<SymbolManager>) -> Self {
        Self { taos, symbol_mgr }
    }
}

#[async_trait]
impl BalanceRepository for TDengineBalanceRepository {
    async fn get_balance(&self, user_id: u64, asset_id: u32) -> Result<Option<BalanceApiData>> {
        super::queries::query_balance(&self.taos, user_id, asset_id, &self.symbol_mgr).await
    }

    async fn get_all_balances(&self, user_id: u64) -> Result<Vec<BalanceApiData>> {
        super::queries::query_all_balances(&self.taos, user_id, &self.symbol_mgr).await
    }
}

/// TDengine-backed trade repository implementation
pub struct TDengineTradeRepository {
    taos: Arc<Taos>,
    symbol_mgr: Arc<SymbolManager>,
    symbol_id: u32,
}

impl TDengineTradeRepository {
    pub fn new(taos: Arc<Taos>, symbol_mgr: Arc<SymbolManager>, symbol_id: u32) -> Self {
        Self {
            taos,
            symbol_mgr,
            symbol_id,
        }
    }
}

#[async_trait]
impl TradeRepository for TDengineTradeRepository {
    async fn get_trades(&self, limit: usize) -> Result<Vec<TradeApiData>> {
        super::queries::query_trades(&self.taos, self.symbol_id, limit, &self.symbol_mgr).await
    }

    async fn get_public_trades(
        &self,
        limit: usize,
        from_id: Option<i64>,
    ) -> Result<Vec<PublicTradeApiData>> {
        super::queries::query_public_trades(
            &self.taos,
            self.symbol_id,
            limit,
            from_id,
            &self.symbol_mgr,
        )
        .await
    }
}
