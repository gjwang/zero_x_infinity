-- 001_init_schema.sql
-- Initial schema for exchange_info_db
-- Naming convention: tables use _tb suffix, database uses _db suffix

-- ============================================================================
-- Users Table
-- ============================================================================
CREATE TABLE IF NOT EXISTS users_tb (
    user_id BIGSERIAL PRIMARY KEY,
    username VARCHAR(64) UNIQUE NOT NULL,
    email VARCHAR(128),
    status SMALLINT NOT NULL DEFAULT 1,  -- 0=disabled, 1=active
    user_flags INT NOT NULL DEFAULT 15,  -- 用户权限位标志
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
-- user_flags 位定义:
--   0x01 = can_login
--   0x02 = can_trade  
--   0x04 = can_withdraw
--   0x08 = can_api_access
--   0x10 = is_vip
--   0x20 = is_kyc_verified
-- 默认 15 (0x0F) = login + trade + withdraw + api

-- ============================================================================
-- Assets Table (BTC, USDT, ETH, etc.)
-- ============================================================================
CREATE TABLE IF NOT EXISTS assets_tb (
    asset_id SERIAL PRIMARY KEY,
    asset VARCHAR(16) UNIQUE NOT NULL
        CONSTRAINT chk_asset_uppercase CHECK (asset = UPPER(asset)),  -- 强制大写
    name VARCHAR(64) NOT NULL,           -- 全称: Bitcoin, Tether USD
    decimals SMALLINT NOT NULL,          -- 精度: 8 for BTC, 6 for USDT
    status SMALLINT NOT NULL DEFAULT 1,  -- 0=disabled, 1=active
    asset_flags INT NOT NULL DEFAULT 7,  -- 资产权限位标志
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
-- asset_flags 位定义:
--   0x01 = can_deposit
--   0x02 = can_withdraw
--   0x04 = can_trade
--   0x08 = is_stable_coin
-- 默认 7 (0x07) = deposit + withdraw + trade

-- ============================================================================
-- Symbols Table (Trading Pairs)
-- ============================================================================
CREATE TABLE IF NOT EXISTS symbols_tb (
    symbol_id SERIAL PRIMARY KEY,
    symbol VARCHAR(32) UNIQUE NOT NULL
        CONSTRAINT chk_symbol_uppercase CHECK (symbol = UPPER(symbol)),  -- 强制大写
    base_asset_id INT NOT NULL REFERENCES assets_tb(asset_id),
    quote_asset_id INT NOT NULL REFERENCES assets_tb(asset_id),
    price_decimals SMALLINT NOT NULL,    -- 价格精度
    qty_decimals SMALLINT NOT NULL,      -- 数量精度
    min_qty BIGINT NOT NULL DEFAULT 0,   -- 最小下单量 (scaled)
    status SMALLINT NOT NULL DEFAULT 1,  -- 0=offline, 1=online, 2=maintenance
    symbol_flags INT NOT NULL DEFAULT 15, -- 交易对权限位标志
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
-- symbol_flags 位定义:
--   0x01 = is_tradable
--   0x02 = is_visible
--   0x04 = allow_market_order
--   0x08 = allow_limit_order
-- 默认 15 (0x0F) = 全部功能

-- ============================================================================
-- Seed Data
-- ============================================================================

-- Initial assets
INSERT INTO assets_tb (asset, name, decimals, asset_flags) VALUES
    ('BTC', 'Bitcoin', 8, 7),
    ('USDT', 'Tether USD', 6, 15),
    ('ETH', 'Ethereum', 8, 7)
ON CONFLICT (asset) DO NOTHING;

-- Initial trading pair
INSERT INTO symbols_tb (symbol, base_asset_id, quote_asset_id, price_decimals, qty_decimals, min_qty, symbol_flags)
SELECT 'BTC_USDT', b.asset_id, q.asset_id, 2, 8, 100000, 15
FROM assets_tb b, assets_tb q 
WHERE b.asset = 'BTC' AND q.asset = 'USDT'
ON CONFLICT (symbol) DO NOTHING;

-- System user (for fees)
INSERT INTO users_tb (user_id, username, email, status, user_flags) VALUES
    (1, 'system', 'system@zero-x-infinity.io', 1, 15)
ON CONFLICT (user_id) DO NOTHING;
