-- 010_deposit_withdraw.sql
-- Phase 0x11: Deposit & Withdraw History

-- 1. Deposit History (Idempotent: tx_hash + output_index ideally, but simple tx_hash for now)
-- We use tx_hash as PK.
CREATE TABLE IF NOT EXISTS deposit_history (
    tx_hash VARCHAR(128) PRIMARY KEY,
    user_id BIGINT NOT NULL,
    asset VARCHAR(32) NOT NULL,
    amount NUMERIC(40, 0) NOT NULL,
    status VARCHAR(32) NOT NULL, -- CONFIRMING, SUCCESS
    block_height BIGINT DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Index for user lookups
CREATE INDEX IF NOT EXISTS idx_deposit_user ON deposit_history(user_id);


-- 2. Withdraw History
CREATE TABLE IF NOT EXISTS withdraw_history (
    request_id VARCHAR(64) PRIMARY KEY, -- UUID
    user_id BIGINT NOT NULL,
    asset VARCHAR(32) NOT NULL,
    amount NUMERIC(40, 0) NOT NULL,
    fee NUMERIC(40, 0) NOT NULL,
    to_address VARCHAR(255) NOT NULL,
    tx_hash VARCHAR(128), -- Nullable initially
    status VARCHAR(32) NOT NULL, -- PENDING, PROCESSING, SUCCESS, FAILED
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_withdraw_user ON withdraw_history(user_id);


-- 3. User Addresses (Warm Wallet Model)
-- One address per network per asset (or shared if possible, but distinct here)
CREATE TABLE IF NOT EXISTS user_addresses (
    user_id BIGINT NOT NULL,
    asset VARCHAR(32) NOT NULL,
    network VARCHAR(32) NOT NULL, -- BTC, ETH, TRON
    address VARCHAR(255) NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (user_id, asset, network)
);

-- Ensure we can look up owner by address (for deposit scanning)
CREATE INDEX IF NOT EXISTS idx_address_lookup ON user_addresses(address);
