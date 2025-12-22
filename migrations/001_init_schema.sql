-- 001_init_schema.sql
-- Initial schema for account management

-- Users table
CREATE TABLE IF NOT EXISTS users (
    user_id BIGSERIAL PRIMARY KEY,
    username VARCHAR(64) UNIQUE NOT NULL,
    email VARCHAR(128),
    status SMALLINT NOT NULL DEFAULT 1,  -- 1=active, 0=disabled
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Assets table (BTC, USDT, ETH, etc.)
CREATE TABLE IF NOT EXISTS assets (
    asset_id SERIAL PRIMARY KEY,
    asset VARCHAR(16) UNIQUE NOT NULL,   -- 资产代码: BTC, USDT, ETH
    name VARCHAR(64) NOT NULL,           -- e.g., "Bitcoin", "Tether"
    decimals SMALLINT NOT NULL,          -- e.g., 8 for BTC, 6 for USDT
    status SMALLINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Symbols table (trading pairs)
CREATE TABLE IF NOT EXISTS symbols (
    symbol_id SERIAL PRIMARY KEY,
    symbol VARCHAR(32) UNIQUE NOT NULL,  -- 交易对: BTC_USDT
    base_asset_id INT NOT NULL REFERENCES assets(asset_id),
    quote_asset_id INT NOT NULL REFERENCES assets(asset_id),
    price_decimals SMALLINT NOT NULL,    -- price precision
    qty_decimals SMALLINT NOT NULL,      -- qty precision
    min_qty BIGINT NOT NULL DEFAULT 0,   -- minimum order qty (scaled)
    status SMALLINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Seed initial assets
INSERT INTO assets (asset, name, decimals) VALUES
    ('BTC', 'Bitcoin', 8),
    ('USDT', 'Tether USD', 6),
    ('ETH', 'Ethereum', 8)
ON CONFLICT (asset) DO NOTHING;

-- Seed initial trading pair
INSERT INTO symbols (symbol, base_asset_id, quote_asset_id, price_decimals, qty_decimals, min_qty)
SELECT 'BTC_USDT', b.asset_id, q.asset_id, 2, 8, 100000  -- min 0.001 BTC
FROM assets b, assets q 
WHERE b.asset = 'BTC' AND q.asset = 'USDT'
ON CONFLICT (symbol) DO NOTHING;

-- Seed system user (for fees)
INSERT INTO users (user_id, username, email, status) VALUES
    (1, 'system', 'system@zero-x-infinity.io', 1)
ON CONFLICT (user_id) DO NOTHING;
