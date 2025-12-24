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
    #[inline]
    pub fn qty_unit(&self) -> u64 {
        10u64.pow(self.base_decimals)
    }
}

#[derive(Debug, Clone)]
pub struct AssetInfo {
    pub asset_id: u32,
    pub decimals: u32,
    pub display_decimals: u32, // Max allowed decimals for display/input
    pub name: String,
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
}
