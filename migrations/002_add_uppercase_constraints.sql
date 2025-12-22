-- Migration: Add uppercase CHECK constraints
-- Created: 2025-12-22
-- Description: Add simplified uppercase validation constraints for assets and symbols
--
-- Design principle: Keep database constraints simple (uppercase only)
-- Complex validation (format, length) is handled in application layer

-- ============================================================================
-- Assets Table: Enforce Uppercase
-- ============================================================================

ALTER TABLE assets_tb 
ADD CONSTRAINT asset_uppercase_check 
CHECK (asset = UPPER(asset));

COMMENT ON CONSTRAINT asset_uppercase_check ON assets_tb IS 
'Ensures asset codes are uppercase. Format validation handled in application layer.';

-- ============================================================================
-- Symbols Table: Enforce Uppercase
-- ============================================================================

ALTER TABLE symbols_tb 
ADD CONSTRAINT symbol_uppercase_check 
CHECK (symbol = UPPER(symbol));

COMMENT ON CONSTRAINT symbol_uppercase_check ON symbols_tb IS 
'Ensures symbol codes are uppercase. Format validation handled in application layer.';

-- ============================================================================
-- Verification Queries
-- ============================================================================

-- Test 1: Should FAIL - lowercase asset
-- INSERT INTO assets_tb (asset, name, decimals, status, asset_flags) 
-- VALUES ('btc', 'Bitcoin', 8, 1, 7);
-- Expected: ERROR: new row violates check constraint "asset_uppercase_check"

-- Test 2: Should SUCCEED - uppercase asset
-- INSERT INTO assets_tb (asset, name, decimals, status, asset_flags) 
-- VALUES ('BTC', 'Bitcoin', 8, 1, 7);

-- Test 3: Should FAIL - lowercase symbol
-- INSERT INTO symbols_tb (symbol, base_asset_id, quote_asset_id, price_decimals, qty_decimals, min_qty, status, symbol_flags)
-- VALUES ('btc_usdt', 1, 2, 2, 6, 1000, 1, 15);
-- Expected: ERROR: new row violates check constraint "symbol_uppercase_check"

-- Test 4: Should SUCCEED - uppercase symbol
-- INSERT INTO symbols_tb (symbol, base_asset_id, quote_asset_id, price_decimals, qty_decimals, min_qty, status, symbol_flags)
-- VALUES ('BTC_USDT', 1, 2, 2, 6, 1000, 1, 15);
