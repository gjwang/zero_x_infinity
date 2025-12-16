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

    /// Load initial state (simulating DB load)
    pub fn load_from_db() -> Self {
        let mut manager = SymbolManager::new();
        //TODO: refactor: we do NOT need quantity decimal any more,juse use get_asset_decimal
        // Add Assets FIRST (symbols depend on them)
        manager.add_asset(1, 8, 3, "BTC"); // BTC: 8 decimals, 3 precision
        manager.add_asset(2, 8, 2, "USDT"); // USDT: 8 decimals, 2 precision
        manager.add_asset(3, 8, 4, "ETH"); // ETH: 8 decimals, 4 precision

        // BTC_USDT: Base 1 (BTC), Quote 2 (USDT), Price Decimal 2
        manager
            .insert_symbol("BTC_USDT", 0, 1, 2, 2, 2)
            .expect("BTC_USDT init failed");
        // ETH_USDT: Base 3 (ETH), Quote 2 (USDT), Price Decimal 2
        manager
            .insert_symbol("ETH_USDT", 1, 3, 2, 2, 2)
            .expect("ETH_USDT init failed");

        manager
    }
}
