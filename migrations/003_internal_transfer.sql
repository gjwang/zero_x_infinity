-- 003_internal_transfer.sql

-- 1. Add account_type to balances_tb
-- 
-- IMPORTANT: balances_tb only stores Funding account balances (account_type=2).
-- Spot (Trading) balances are managed by UBSCore in RAM, NOT in this table.
--
-- account_type values:
--   1 = Spot (UNUSED in PostgreSQL - UBSCore RAM manages Spot balances)
--   2 = Funding (ACTIVE - deposit/withdraw, stored here)
--
-- 1. Add account_type to balances_tb (Idempotent)
DO $$ 
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_name='balances_tb' AND column_name='account_type') THEN
        ALTER TABLE balances_tb ADD COLUMN account_type SMALLINT NOT NULL DEFAULT 1;
    END IF;
END $$;

-- 2. Update Unique Constraint
-- Old: UNIQUE (user_id, asset_id)
-- New: UNIQUE (user_id, asset_id, account_type)
-- 2. Update Unique Constraint (Idempotent)
DO $$
BEGIN
    -- Drop old constraint if exists
    IF EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'balances_tb_user_id_asset_id_key') THEN
        ALTER TABLE balances_tb DROP CONSTRAINT balances_tb_user_id_asset_id_key;
    END IF;

    -- Add new constraint if not exists
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'balances_tb_user_id_asset_id_account_type_key') THEN
        ALTER TABLE balances_tb ADD CONSTRAINT balances_tb_user_id_asset_id_account_type_key UNIQUE (user_id, asset_id, account_type);
    END IF;
END $$;

-- 3. Create transfers_tb table
-- Tracks internal transfers between account types
-- 3. Create transfers_tb table (Idempotent)
CREATE TABLE IF NOT EXISTS transfers_tb (
    transfer_id BIGSERIAL PRIMARY KEY,
    req_id VARCHAR(64) NOT NULL, -- Idempotency Key
    user_id BIGINT NOT NULL,
    asset_id INTEGER NOT NULL,
    amount BIGINT NOT NULL, -- Logical amount (scaled)
    from_type SMALLINT NOT NULL,
    to_type SMALLINT NOT NULL,
    status SMALLINT NOT NULL DEFAULT 0, -- 0=PENDING, 1=SUCCESS, 2=FAILED
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    CONSTRAINT transfers_tb_req_id_key UNIQUE (req_id)
);

-- Index for querying user history
-- Index for querying user history
CREATE INDEX IF NOT EXISTS idx_transfers_user_id ON transfers_tb(user_id);
