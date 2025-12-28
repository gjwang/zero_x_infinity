-- Manual Seed Data for E2E Test (Corrected Schema + Explicit Timestamps + Fees)

-- Assets (Add status=1, created_at=NOW())
INSERT INTO assets_tb (asset, name, decimals, asset_flags, status, created_at) VALUES
    ('BTC', 'Bitcoin', 8, 7, 1, NOW()),
    ('USDT', 'Tether USD', 6, 15, 1, NOW()),
    ('ETH', 'Ethereum', 8, 7, 1, NOW())
ON CONFLICT (asset) DO UPDATE SET status=1;

-- Symbols (Add status=1, created_at=NOW(), fees=default)
INSERT INTO symbols_tb (symbol, base_asset_id, quote_asset_id, price_decimals, qty_decimals, min_qty, symbol_flags, status, created_at, base_maker_fee, base_taker_fee)
SELECT 'BTC_USDT', b.asset_id, q.asset_id, 2, 8, 100000, 15, 1, NOW(), 1000, 2000
FROM assets_tb b, assets_tb q 
WHERE b.asset = 'BTC' AND q.asset = 'USDT'
ON CONFLICT (symbol) DO NOTHING;

-- Balances (Add status=1, account_type=1, timestamps)
-- User 1001: Buyer (Needs USDT)
INSERT INTO balances_tb (user_id, asset_id, available, frozen, version, account_type, status, created_at, updated_at)
VALUES (1001, (SELECT asset_id FROM assets_tb WHERE asset='USDT'), 1000000.00000000, 0.00000000, 1, 1, 1, NOW(), NOW())
ON CONFLICT (user_id, asset_id, account_type) DO NOTHING;

-- User 1002: Seller (Needs BTC)
INSERT INTO balances_tb (user_id, asset_id, available, frozen, version, account_type, status, created_at, updated_at)
VALUES (1002, (SELECT asset_id FROM assets_tb WHERE asset='BTC'), 100.00000000, 0.00000000, 1, 1, 1, NOW(), NOW())
ON CONFLICT (user_id, asset_id, account_type) DO NOTHING;
