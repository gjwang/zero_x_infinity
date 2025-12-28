-- 004_fsm_transfers.sql
-- Internal Transfer FSM State Machine
-- 
-- This migration creates the FSM-based transfer tracking table for
-- distributed 2-phase commit between Funding (PostgreSQL) and Trading (UBSCore).

-- Drop old simple transfers_tb if upgrading (optional, comment out if you want to preserve history)
-- DROP TABLE IF EXISTS transfers_tb;

-- FSM Transfer Table
-- Tracks all internal transfers with full state machine lifecycle
CREATE TABLE IF NOT EXISTS fsm_transfers_tb (
    transfer_id   BIGSERIAL PRIMARY KEY,
    
    -- Identifiers
    req_id        VARCHAR(64) UNIQUE NOT NULL,  -- Server-generated Snowflake ID
    cid           VARCHAR(64) UNIQUE,            -- Client idempotency key (optional)
    
    -- Transfer details
    user_id       BIGINT NOT NULL,
    asset_id      INTEGER NOT NULL,
    amount        NUMERIC(40, 0) NOT NULL,
    
    -- Direction
    transfer_type SMALLINT NOT NULL,             -- 1 = Funding→Spot, 2 = Spot→Funding
    source_type   SMALLINT NOT NULL,             -- 1 = Funding, 2 = Trading
    
    -- FSM State
    -- State IDs (from design doc):
    --   0  = INIT
    --   10 = SOURCE_PENDING
    --   20 = SOURCE_DONE
    --   30 = TARGET_PENDING
    --   40 = COMMITTED (Terminal)
    --  -10 = FAILED (Terminal)
    --  -20 = COMPENSATING
    --  -30 = ROLLED_BACK (Terminal)
    state         SMALLINT NOT NULL DEFAULT 0,
    
    -- Error tracking
    error_message TEXT,
    retry_count   INTEGER NOT NULL DEFAULT 0,
    
    -- Timestamps
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for Recovery Worker: find non-terminal transfers
-- Excludes: COMMITTED (40), FAILED (-10), ROLLED_BACK (-30)
CREATE INDEX IF NOT EXISTS idx_fsm_transfers_active_state 
    ON fsm_transfers_tb(state, updated_at) 
    WHERE state NOT IN (40, -10, -30);

-- Index for user history queries
CREATE INDEX IF NOT EXISTS idx_fsm_transfers_user_id 
    ON fsm_transfers_tb(user_id, created_at DESC);

-- Index for cid lookups (idempotency)
CREATE INDEX IF NOT EXISTS idx_fsm_transfers_cid 
    ON fsm_transfers_tb(cid) 
    WHERE cid IS NOT NULL;

-- Transfer operations table for adapter idempotency
-- Records individual operations (withdraw/deposit/rollback) for each transfer
CREATE TABLE IF NOT EXISTS transfer_operations_tb (
    op_id         BIGSERIAL PRIMARY KEY,
    req_id        VARCHAR(64) NOT NULL,          -- References fsm_transfers_tb.req_id
    op_type       VARCHAR(16) NOT NULL,          -- 'WITHDRAW', 'DEPOSIT', 'ROLLBACK', 'COMMIT'
    service_type  SMALLINT NOT NULL,             -- 1 = Funding, 2 = Trading
    result        VARCHAR(16) NOT NULL,          -- 'SUCCESS', 'FAILED', 'PENDING'
    error_message TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    -- Unique constraint for idempotency
    CONSTRAINT transfer_operations_unique UNIQUE (req_id, op_type, service_type)
);

-- Index for lookups
CREATE INDEX IF NOT EXISTS idx_transfer_ops_req_id 
    ON transfer_operations_tb(req_id);

-- Comment on tables
COMMENT ON TABLE fsm_transfers_tb IS 'FSM-based internal transfer state machine for Funding <-> Trading transfers';
COMMENT ON TABLE transfer_operations_tb IS 'Idempotency records for transfer adapter operations';
