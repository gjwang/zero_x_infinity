-- =============================================================================
-- Seed Data for Testing
-- =============================================================================
-- This file contains test data. Apply after 001_init_schema.sql
-- Usage: psql -d exchange_info_db -f fixtures/seed_data.sql
-- =============================================================================

-- ============================================================================
-- System User (for fees and internal operations)
-- ============================================================================
INSERT INTO users_tb (user_id, username, email, status, user_flags) VALUES
    (1, 'system', 'system@zero-x-infinity.io', 1, 15)
ON CONFLICT (user_id) DO NOTHING;

-- ============================================================================
-- Assets
-- ============================================================================
INSERT INTO assets_tb (asset, name, decimals, asset_flags) VALUES
    ('BTC', 'Bitcoin', 8, 7),
    ('USDT', 'Tether USD', 6, 15),
    ('ETH', 'Ethereum', 8, 7)
ON CONFLICT (asset) DO NOTHING;

-- ============================================================================
-- Trading Pairs (Symbols)
-- ============================================================================
INSERT INTO symbols_tb (symbol, base_asset_id, quote_asset_id, price_decimals, qty_decimals, min_qty, symbol_flags)
SELECT 'BTC_USDT', b.asset_id, q.asset_id, 2, 8, 100000, 15
FROM assets_tb b, assets_tb q 
WHERE b.asset = 'BTC' AND q.asset = 'USDT'
ON CONFLICT (symbol) DO NOTHING;

-- ============================================================================
-- Verification Query (optional for debugging)
-- ============================================================================
-- SELECT 'Assets:' as type, COUNT(*) as count FROM assets_tb
-- UNION ALL
-- SELECT 'Symbols:', COUNT(*) FROM symbols_tb
-- UNION ALL
-- SELECT 'Users:', COUNT(*) FROM users_tb;
