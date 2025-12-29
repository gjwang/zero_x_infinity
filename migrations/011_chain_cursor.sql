-- Phase 0x11-a: Chain Cursor for Sentinel Service
-- Tracks blockchain scanning progress per chain

-- Track scanning progress per chain
CREATE TABLE IF NOT EXISTS chain_cursor (
    chain_slug VARCHAR(32) PRIMARY KEY, -- "btc", "eth", "trx"
    last_scanned_height BIGINT NOT NULL,
    last_scanned_hash VARCHAR(128) NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Enhance deposit_history for real chain tracking
-- These columns may already exist, so we use IF NOT EXISTS pattern via DO block
DO $$
BEGIN
    -- Add chain_slug column
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_name = 'deposit_history' AND column_name = 'chain_slug') THEN
        ALTER TABLE deposit_history ADD COLUMN chain_slug VARCHAR(32);
    END IF;
    
    -- Add block_height column (may already exist from mock phase)
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_name = 'deposit_history' AND column_name = 'block_height') THEN
        ALTER TABLE deposit_history ADD COLUMN block_height BIGINT;
    END IF;
    
    -- Add block_hash column
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_name = 'deposit_history' AND column_name = 'block_hash') THEN
        ALTER TABLE deposit_history ADD COLUMN block_hash VARCHAR(128);
    END IF;
    
    -- Add tx_index column
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_name = 'deposit_history' AND column_name = 'tx_index') THEN
        ALTER TABLE deposit_history ADD COLUMN tx_index INT DEFAULT 0;
    END IF;
    
    -- Add confirmations column
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns 
                   WHERE table_name = 'deposit_history' AND column_name = 'confirmations') THEN
        ALTER TABLE deposit_history ADD COLUMN confirmations INT DEFAULT 0;
    END IF;
END $$;

-- Index for efficient re-org checking
CREATE INDEX IF NOT EXISTS idx_deposit_chain_height 
ON deposit_history(chain_slug, block_height);

-- Index for confirmation monitoring
CREATE INDEX IF NOT EXISTS idx_deposit_confirming 
ON deposit_history(status) WHERE status IN ('DETECTED', 'CONFIRMING');
