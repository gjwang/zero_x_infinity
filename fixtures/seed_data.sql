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
-- internal_scale: storage precision (e.g., 8 = 10^8 satoshi)
-- asset_precision: API display precision (can be <= internal_scale)
INSERT INTO assets_tb (asset, name, internal_scale, asset_precision, asset_flags) VALUES
    ('BTC', 'Bitcoin', 8, 6, 7),         -- 8 decimals internal, 6 for display
    ('USDT', 'Tether USD', 6, 2, 15),    -- 6 decimals internal, 2 for display  
    ('ETH', 'Ethereum', 8, 6, 7)         -- 8 decimals internal, 6 for display
ON CONFLICT (asset) DO NOTHING;

-- ============================================================================
-- Trading Pairs (Symbols)
-- ============================================================================
-- price_scale/qty_scale: internal scale factor
-- price_precision/qty_precision: API display precision
INSERT INTO symbols_tb (symbol, base_asset_id, quote_asset_id, 
    price_scale, price_precision, qty_scale, qty_precision, min_qty, symbol_flags)
SELECT 'BTC_USDT', b.asset_id, q.asset_id, 2, 2, 8, 6, 100000, 15
FROM assets_tb b, assets_tb q 
WHERE b.asset = 'BTC' AND q.asset = 'USDT'
ON CONFLICT (symbol) DO NOTHING;

-- ============================================================================
-- API Keys for Testing
-- ============================================================================
-- Test API Key for user_id=1 (system)
-- Private key (hex, for client testing): 9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60
-- This is a well-known test key - DO NOT USE IN PRODUCTION
INSERT INTO api_keys_tb (user_id, api_key, key_type, key_data, label, permissions, status) 
VALUES (
    1, 
    'AK_D4735E3A265E16EE',  -- Test API Key
    1,  -- Ed25519
    E'\\xd75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a',  -- Public key (32 bytes)
    'Test Key',
    15,  -- Full permissions (READ | TRADE | WITHDRAW | TRANSFER)
    1   -- Active
) ON CONFLICT (api_key) DO NOTHING;

-- Test User 1001
INSERT INTO users_tb (user_id, username, email, status, user_flags) VALUES
    (1001, 'user1001', 'user1001@test.com', 1, 15)
ON CONFLICT (user_id) DO NOTHING;

INSERT INTO api_keys_tb (user_id, api_key, key_type, key_data, label, permissions, status)
VALUES (
    1001,
    'AK_0000000000001001',
    1, -- Ed25519
    E'\\xd75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a', -- Reusing system key pair for simplicity
    'Test Key 1001',
    15,
    1
) ON CONFLICT (api_key) DO NOTHING;

-- Test User 1002
INSERT INTO users_tb (user_id, username, email, status, user_flags) VALUES
    (1002, 'user1002', 'user1002@test.com', 1, 15)
ON CONFLICT (user_id) DO NOTHING;

INSERT INTO api_keys_tb (user_id, api_key, key_type, key_data, label, permissions, status)
VALUES (
    1002,
    'AK_0000000000001002',
    1, -- Ed25519
    E'\\xd75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a', -- Reusing system key pair for simplicity
    'Test Key 1002',
    15,
    1
) ON CONFLICT (api_key) DO NOTHING;

-- ============================================================================
-- Balances for Testing
-- ============================================================================
-- User 1001: Buyer (Needs USDT)
-- User 1001: Buyer (Needs USDT)
-- USDT has 6 decimals: 1000000 USDT = 1000000 * 10^6 = 1000000000000 atomic units
INSERT INTO balances_tb (user_id, asset_id, available, frozen, version, account_type)
VALUES (1001, 2, 1000000000000, 0, 1, 1) -- Spot (account_type=1): 1M USDT
ON CONFLICT (user_id, asset_id, account_type) DO UPDATE SET available = EXCLUDED.available;

-- User 1001: Funding balance for transfer testing
-- 1000 USDT = 1000 * 10^6 = 1000000000 atomic units
INSERT INTO balances_tb (user_id, asset_id, available, frozen, version, account_type)
VALUES (1001, 2, 1000000000, 0, 1, 2) -- Funding (account_type=2): 1000 USDT
ON CONFLICT (user_id, asset_id, account_type) DO UPDATE SET available = EXCLUDED.available;

-- User 1002: Seller (Needs BTC)
-- BTC has 8 decimals: 100 BTC = 100 * 10^8 = 10000000000 atomic units
INSERT INTO balances_tb (user_id, asset_id, available, frozen, version, account_type)
VALUES (1002, 1, 10000000000, 0, 1, 1) -- Spot: 100 BTC
ON CONFLICT (user_id, asset_id, account_type) DO UPDATE SET available = EXCLUDED.available;

-- ============================================================================
-- Verification Query (optional for debugging)
-- ============================================================================
-- SELECT 'Assets:' as type, COUNT(*) as count FROM assets_tb
-- UNION ALL
-- SELECT 'Symbols:', COUNT(*) FROM symbols_tb
-- UNION ALL
-- SELECT 'Users:', COUNT(*) FROM users_tb
-- UNION ALL
-- SELECT 'API Keys:', COUNT(*) FROM api_keys_tb;
