-- 003_internal_transfer.sql
-- Support for internal transfers (Funding <-> Spot)

-- 1. Add account_type to balances_tb
-- 1=Spot (Default), 2=Funding
ALTER TABLE balances_tb ADD COLUMN IF NOT EXISTS account_type SMALLINT NOT NULL DEFAULT 1;

-- 2. Update unique constraint on balances_tb
-- Drop old constraint on (user_id, asset_id)
ALTER TABLE balances_tb DROP CONSTRAINT IF EXISTS balances_tb_user_id_asset_id_key;
-- Add new constraint on (user_id, asset_id, account_type)
ALTER TABLE balances_tb ADD CONSTRAINT balances_tb_unique 
    UNIQUE(user_id, asset_id, account_type);

-- 3. Create transfers table
CREATE TABLE IF NOT EXISTS transfers_tb (
    transfer_id     BIGSERIAL PRIMARY KEY,
    user_id         BIGINT NOT NULL REFERENCES users_tb(user_id),
    asset_id        INTEGER NOT NULL REFERENCES assets_tb(asset_id),
    from_account    SMALLINT NOT NULL,  -- 1=Spot, 2=Funding
    to_account      SMALLINT NOT NULL,
    amount          BIGINT NOT NULL,    -- Transfer amount
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    
    CONSTRAINT chk_amount_positive CHECK (amount > 0),
    CONSTRAINT chk_diff_accounts CHECK (from_account != to_account),
    CONSTRAINT chk_valid_accounts CHECK (from_account IN (1, 2) AND to_account IN (1, 2))
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_transfers_user ON transfers_tb(user_id);
CREATE INDEX IF NOT EXISTS idx_transfers_created ON transfers_tb(created_at);

-- Comments
COMMENT ON TABLE transfers_tb IS 'Internal transfer history between accounts';
COMMENT ON COLUMN balances_tb.account_type IS '1=Spot, 2=Funding';
