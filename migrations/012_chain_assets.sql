-- Up Migration (ADR-005 Compliant)

-- 1. Create chains_tb (Infrastructure)
CREATE TABLE chains_tb (
    chain_slug VARCHAR(32) PRIMARY KEY,  -- 'ETH', 'BTC' (Business ID)
    chain_name VARCHAR(64) NOT NULL,     -- 'Ethereum Anvil'
    rpc_urls TEXT[] NOT NULL,            -- Array of endpoints
    network_id VARCHAR(32),              -- '31337', 'regtest' (EIP-155 / SLIP-44)
    scan_start_height BIGINT NOT NULL DEFAULT 0,
    confirmation_blocks INT NOT NULL DEFAULT 1,
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- 2. Create chain_assets_tb (Physical Extension to assets_tb)
CREATE TABLE chain_assets_tb (
    id SERIAL PRIMARY KEY,
    chain_slug VARCHAR(32) NOT NULL REFERENCES chains_tb(chain_slug),
    asset_id INT NOT NULL REFERENCES assets_tb(asset_id),
    
    -- Physical Properties Only
    contract_address VARCHAR(128),  -- NULL for Native, 0x... for Tokens
    decimals SMALLINT NOT NULL,     -- Chain-specific decimals (e.g. 6 for USDT)
    
    -- Chain-Specific Overrides (Business Rules - stored as atomic units e.g. Satoshis/Wei)
    min_deposit BIGINT DEFAULT 0,
    min_withdraw BIGINT DEFAULT 0,
    withdraw_fee BIGINT DEFAULT 0,
    
    -- Operational Status
    is_active BOOLEAN DEFAULT FALSE,  -- SECURITY: Default inactive for safe listing
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    -- Constraints
    UNIQUE(chain_slug, asset_id),        -- 1 Logical Asset per Chain
    UNIQUE(chain_slug, contract_address) -- 1 Contract = 1 Asset (per chain)
);

-- 3. Seed Chains (Dev Environment Defaults)
INSERT INTO chains_tb (chain_slug, chain_name, rpc_urls, network_id, scan_start_height, confirmation_blocks)
VALUES 
    ('BTC', 'Bitcoin Regtest', ARRAY['http://127.0.0.1:18443'], 'regtest', 0, 1),
    ('ETH', 'Ethereum Anvil', ARRAY['http://127.0.0.1:8545'], '31337', 0, 1)
ON CONFLICT (chain_slug) DO NOTHING;

-- 4. Seed Chain Assets
-- Note: Assuming asset_ids from seed_data.sql (BTC=1, ETH=2, USDT=3)
-- Using subqueries to be robust against ID shifts.

-- BTC Native
INSERT INTO chain_assets_tb (chain_slug, asset_id, contract_address, decimals)
SELECT 'BTC', asset_id, NULL, 8 
FROM assets_tb WHERE asset = 'BTC'
ON CONFLICT DO NOTHING;

-- ETH Native
INSERT INTO chain_assets_tb (chain_slug, asset_id, contract_address, decimals)
SELECT 'ETH', asset_id, NULL, 18
FROM assets_tb WHERE asset = 'ETH'
ON CONFLICT DO NOTHING;

-- USDT on ETH (Mock Address)
INSERT INTO chain_assets_tb (chain_slug, asset_id, contract_address, decimals)
SELECT 'ETH', asset_id, '0xdac17f958d2ee523a2206206994597c13d831ec7', 6
FROM assets_tb WHERE asset = 'USDT'
ON CONFLICT DO NOTHING;

-- 5. Create user_chain_addresses (ADR-006: User Address Decoupling)
-- Enables "Hot Listing" - users don't need new addresses for new tokens on same chain
CREATE TABLE user_chain_addresses (
    user_id BIGINT NOT NULL,
    chain_slug VARCHAR(32) NOT NULL REFERENCES chains_tb(chain_slug),
    address VARCHAR(255) NOT NULL,
    
    -- Metadata
    memo_tag VARCHAR(64),          -- For XRP/EOS destination tags
    created_at TIMESTAMPTZ DEFAULT NOW(),
    
    -- Constraints: One address per user per chain (simplified MVP model)
    PRIMARY KEY (user_id, chain_slug),
    UNIQUE (chain_slug, address)   -- Reverse lookup must be unique
);

-- Down Migration
-- DROP TABLE user_chain_addresses;
-- DROP TABLE chain_assets_tb;
-- DROP TABLE chains_tb;
