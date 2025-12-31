use crate::money::{MoneyFormatter, ScaledAmount};
use rustc_hash::FxHashMap;

#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub symbol: String,
    pub symbol_id: u32,
    pub base_asset_id: u32,
    pub quote_asset_id: u32,
    pub price_decimal: u32,
    pub price_display_decimal: u32,
    /// Base asset decimals (e.g., 8 for BTC = satoshi)
    /// Stored here for fast access without additional lookup
    pub base_decimals: u32,
    /// Base maker fee rate (10^6 precision: 1000 = 0.10%)
    pub base_maker_fee: u64,
    /// Base taker fee rate (10^6 precision: 2000 = 0.20%)
    pub base_taker_fee: u64,
}

impl SymbolInfo {
    /// Get qty_unit (base asset unit) - e.g., 10^8 for BTC
    ///
    /// Returns ScaledAmount for type safety. Use `*qty_unit()` when you need u64.
    /// Delegates to money::unit_amount() as the single source of truth.
    #[inline]
    pub fn qty_unit(&self) -> ScaledAmount {
        crate::money::unit_amount(self.base_decimals)
    }

    /// Get price_unit (price decimals unit) - e.g., 10^2 for 2 decimal places
    ///
    /// Returns ScaledAmount for type safety. Use `*price_unit()` when you need u64.
    /// Delegates to money::unit_amount() as the single source of truth.
    #[inline]
    pub fn price_unit(&self) -> ScaledAmount {
        crate::money::unit_amount(self.price_decimal)
    }

    /// Calculate quote quantity from price and quantity
    ///
    /// Formula: (price * qty) / qty_unit
    /// Returns raw u64 amount in quote asset decimals
    #[inline]
    pub fn calculate_quote_qty(&self, price: u64, qty: u64) -> u64 {
        (price * qty) / *self.qty_unit()
    }

    // ========================================================================
    // Intent-based Display APIs: Raw u64 → Decimal for human-readable output
    // Encapsulates price_unit/qty_unit so callers don't handle scaling directly
    // ========================================================================

    /// Convert raw scaled price (u64) to Decimal for display
    ///
    /// Example: 85000_00 (scaled) with price_decimals=2 → Decimal(85000.00)
    #[inline]
    pub fn price_as_decimal(&self, price: u64) -> rust_decimal::Decimal {
        rust_decimal::Decimal::from(price) / rust_decimal::Decimal::from(*self.price_unit())
    }

    /// Convert raw scaled quantity (u64) to Decimal for display
    ///
    /// Example: 1_00000000 (scaled) with base_decimals=8 → Decimal(1.0)
    #[inline]
    pub fn qty_as_decimal(&self, qty: u64) -> rust_decimal::Decimal {
        rust_decimal::Decimal::from(qty) / rust_decimal::Decimal::from(*self.qty_unit())
    }

    /// Calculate quote value as Decimal for display
    ///
    /// Formula: price_decimal * qty_decimal
    /// Use this for PublicTrade/Ticker quote_qty display
    #[inline]
    pub fn format_quote_value(&self, price: u64, qty: u64) -> rust_decimal::Decimal {
        self.price_as_decimal(price) * self.qty_as_decimal(qty)
    }
}

#[derive(Debug, Clone)]
pub struct AssetInfo {
    pub asset_id: u32,
    pub decimals: u32,
    pub display_decimals: u32, // Max allowed decimals for display/input
    pub name: String,
}

impl AssetInfo {
    // ========================================================================
    // Intent-based API: Decimal → ScaledAmount
    // Encapsulates conversion details; caller only needs to express intent
    // ========================================================================

    /// Parse amount (rejects zero). For quantities, prices, etc.
    pub fn parse_amount(
        &self,
        d: rust_decimal::Decimal,
    ) -> Result<ScaledAmount, crate::money::MoneyError> {
        crate::money::parse_decimal(d, self.decimals)
    }

    /// Parse amount (allows zero). For fees, discounts, etc.
    /// Caller explicitly opts into allowing zero.
    pub fn parse_amount_allow_zero(
        &self,
        d: rust_decimal::Decimal,
    ) -> Result<ScaledAmount, crate::money::MoneyError> {
        crate::money::parse_decimal_allow_zero(d, self.decimals)
    }

    /// Format amount for display
    pub fn format_amount(&self, amount: ScaledAmount) -> String {
        crate::money::format_amount(*amount, self.decimals, self.display_decimals)
    }
}

/// Manages symbol-to-ID and ID-to-symbol mappings
#[derive(Debug, Clone)]
pub struct SymbolManager {
    pub symbol_to_id: FxHashMap<String, u32>,
    pub id_to_symbol: FxHashMap<u32, String>,
    pub symbol_info: FxHashMap<u32, SymbolInfo>,
    pub assets: FxHashMap<u32, AssetInfo>,
}

impl Default for SymbolManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolManager {
    pub fn new() -> Self {
        SymbolManager {
            symbol_to_id: FxHashMap::default(),
            id_to_symbol: FxHashMap::default(),
            symbol_info: FxHashMap::default(),
            assets: FxHashMap::default(),
        }
    }

    pub fn insert(
        &mut self,
        symbol: &str,
        id: u32,
        base_asset_id: u32,
        quote_asset_id: u32,
    ) -> Result<(), &'static str> {
        self.insert_symbol(symbol, id, base_asset_id, quote_asset_id, 2, 2)
    }

    pub fn insert_symbol(
        &mut self,
        symbol: &str,
        id: u32,
        base_asset_id: u32,
        quote_asset_id: u32,
        price_decimal: u32,
        price_display_decimal: u32,
    ) -> Result<(), &'static str> {
        self.insert_symbol_with_fees(
            symbol,
            id,
            base_asset_id,
            quote_asset_id,
            price_decimal,
            price_display_decimal,
            1000, // Default maker fee: 0.10%
            2000, // Default taker fee: 0.20%
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn insert_symbol_with_fees(
        &mut self,
        symbol: &str,
        id: u32,
        base_asset_id: u32,
        quote_asset_id: u32,
        price_decimal: u32,
        price_display_decimal: u32,
        base_maker_fee: u64,
        base_taker_fee: u64,
    ) -> Result<(), &'static str> {
        // Lookup base_decimals from assets - return error if not found
        let base_decimals = self
            .assets
            .get(&base_asset_id)
            .map(|a| a.decimals)
            .ok_or("base_asset_id not found in assets")?;

        self.symbol_to_id.insert(symbol.to_string(), id);
        self.id_to_symbol.insert(id, symbol.to_string());
        self.symbol_info.insert(
            id,
            SymbolInfo {
                symbol: symbol.to_string(),
                symbol_id: id,
                base_asset_id,
                quote_asset_id,
                price_decimal,
                price_display_decimal,
                base_decimals,
                base_maker_fee,
                base_taker_fee,
            },
        );
        Ok(())
    }

    pub fn get_symbol_id(&self, symbol: &str) -> Option<u32> {
        self.symbol_to_id.get(symbol).copied()
    }

    pub fn get_symbol(&self, id: u32) -> Option<&String> {
        self.id_to_symbol.get(&id)
    }

    pub fn get_symbol_info(&self, symbol: &str) -> Option<&SymbolInfo> {
        let id = self.get_symbol_id(symbol)?;
        self.symbol_info.get(&id)
    }

    pub fn get_symbol_info_by_id(&self, id: u32) -> Option<&SymbolInfo> {
        self.symbol_info.get(&id)
    }

    pub fn add_asset(&mut self, asset_id: u32, decimals: u32, display_decimals: u32, name: &str) {
        self.assets.insert(
            asset_id,
            AssetInfo {
                asset_id,
                decimals,
                display_decimals,
                name: name.to_string(),
            },
        );
    }

    pub fn get_asset_name(&self, asset_id: u32) -> Option<String> {
        self.assets.get(&asset_id).map(|a| a.name.clone())
    }

    pub fn get_asset_decimal(&self, asset_id: u32) -> Option<u32> {
        self.assets.get(&asset_id).map(|a| a.decimals)
    }

    pub fn get_asset_display_decimals(&self, asset_id: u32) -> Option<u32> {
        self.assets.get(&asset_id).map(|a| a.display_decimals)
    }

    pub fn get_asset_id(&self, name: &str) -> Option<u32> {
        self.assets
            .values()
            .find(|a| a.name == name)
            .map(|a| a.asset_id)
    }

    /// Get the number of configured symbols
    pub fn symbol_count(&self) -> usize {
        self.symbol_info.len()
    }

    /// Iterate over all symbols
    pub fn iter_symbols(&self) -> impl Iterator<Item = (&u32, &SymbolInfo)> {
        self.symbol_info.iter()
    }

    // ========================================================================
    // Layer 2: Money Conversion Methods (Intent-based API)
    // Delegates to crate::money core functions
    // ========================================================================

    /// Format quantity for display (internal ScaledAmount → String)
    ///
    /// Uses base_asset.decimals and display_decimals from SymbolManager.
    pub fn format_qty(&self, value: ScaledAmount, symbol_id: u32) -> Option<String> {
        crate::money::format_qty(value, symbol_id, self).ok()
    }

    /// Format price for display (internal ScaledAmount → String)
    ///
    /// Uses price_decimal and price_display_decimal from SymbolInfo.
    pub fn format_price(&self, value: ScaledAmount, symbol_id: u32) -> Option<String> {
        crate::money::format_price(value, symbol_id, self).ok()
    }

    /// Parse quantity string (client String → internal ScaledAmount)
    ///
    /// Validates precision against base_asset.decimals.
    pub fn parse_qty(&self, amount_str: &str, symbol_id: u32) -> Option<ScaledAmount> {
        crate::money::parse_qty(amount_str, symbol_id, self).ok()
    }

    /// Parse price string (client String → internal ScaledAmount)
    ///
    /// Validates precision against price_decimal.
    pub fn parse_price(&self, price_str: &str, symbol_id: u32) -> Option<ScaledAmount> {
        crate::money::parse_price(price_str, symbol_id, self).ok()
    }

    // ========================================================================
    // Layer 2.5: Decimal → ScaledAmount (Intent-based API for ReduceOrder/MoveOrder)
    // ========================================================================

    /// Convert Decimal quantity to ScaledAmount (intent-based API)
    ///
    /// For ReduceOrder and similar operations where Decimal is already parsed.
    /// Encapsulates decimals lookup - caller only needs symbol_id.
    pub fn decimal_to_qty(
        &self,
        qty: rust_decimal::Decimal,
        symbol_id: u32,
    ) -> Result<u64, &'static str> {
        let symbol_info = self
            .get_symbol_info_by_id(symbol_id)
            .ok_or("symbol not found")?;
        let asset = self
            .assets
            .get(&symbol_info.base_asset_id)
            .ok_or("base asset not found")?;
        asset
            .parse_amount(qty)
            .map(|s| *s)
            .map_err(|_| "invalid quantity")
    }

    /// Convert Decimal price to ScaledAmount (intent-based API)
    ///
    /// For MoveOrder and similar operations where Decimal is already parsed.
    /// Encapsulates decimals lookup - caller only needs symbol_id.
    pub fn decimal_to_price(
        &self,
        price: rust_decimal::Decimal,
        symbol_id: u32,
    ) -> Result<u64, &'static str> {
        let symbol_info = self
            .get_symbol_info_by_id(symbol_id)
            .ok_or("symbol not found")?;
        crate::money::parse_decimal(price, symbol_info.price_decimal)
            .map(|s| *s)
            .map_err(|_| "invalid price")
    }

    pub fn money_formatter(&self, symbol_id: u32) -> Option<MoneyFormatter<'_>> {
        MoneyFormatter::new(self, symbol_id)
    }

    // ========================================================================
    // Layer 3: DisplayAmount Factory Methods (API Response Formatting)
    // These are the ONLY legitimate ways to create DisplayAmount instances
    // ========================================================================

    /// Format quantity as DisplayAmount for API response
    ///
    /// This is the only way to create a DisplayAmount for quantity fields.
    /// Ensures all quantity output goes through controlled formatting.
    pub fn display_qty(
        &self,
        value: ScaledAmount,
        symbol_id: u32,
    ) -> Option<crate::gateway::types::DisplayAmount> {
        self.format_qty(value, symbol_id)
            .map(crate::gateway::types::DisplayAmount::new)
    }

    /// Format price as DisplayAmount for API response
    ///
    /// This is the only way to create a DisplayAmount for price fields.
    /// Uses price_display_decimal for appropriate truncation.
    pub fn display_price(
        &self,
        value: ScaledAmount,
        symbol_id: u32,
    ) -> Option<crate::gateway::types::DisplayAmount> {
        self.format_price(value, symbol_id)
            .map(crate::gateway::types::DisplayAmount::new)
    }

    /// Format u64 price as DisplayAmount for API response
    ///
    /// Convenience method for raw u64 prices.
    pub fn display_price_u64(
        &self,
        value: u64,
        symbol_id: u32,
    ) -> Option<crate::gateway::types::DisplayAmount> {
        self.display_price(ScaledAmount::from(value), symbol_id)
    }

    /// Format asset amount as DisplayAmount for API response
    ///
    /// For balance/funding responses where asset_id is known.
    pub fn display_asset_amount(
        &self,
        value: ScaledAmount,
        asset_id: u32,
    ) -> Option<crate::gateway::types::DisplayAmount> {
        let asset = self.assets.get(&asset_id)?;
        let formatted = crate::money::format_amount(*value, asset.decimals, asset.display_decimals);
        Some(crate::gateway::types::DisplayAmount::new(formatted))
    }
}
