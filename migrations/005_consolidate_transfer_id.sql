-- 005_consolidate_transfer_id.sql
-- Tech Debt Fix: Rename req_id to transfer_id, keep standard BIGSERIAL PK
-- NOTE: This drops existing data. Only for development phase.

-- Drop old tables
DROP TABLE IF EXISTS transfer_operations_tb;
DROP TABLE IF EXISTS fsm_transfers_tb;

-- FSM Transfer Table (new schema)
CREATE TABLE fsm_transfers_tb (
    id            BIGSERIAL PRIMARY KEY,       -- Standard auto-increment PK
    transfer_id   VARCHAR(26) UNIQUE NOT NULL, -- ULID (was req_id)
    cid           VARCHAR(64) UNIQUE,          -- Client idempotency key
    
    user_id       BIGINT NOT NULL,
    asset_id      INTEGER NOT NULL,
    amount        DECIMAL(30, 8) NOT NULL,
    
    transfer_type SMALLINT NOT NULL,           -- 1=Funding→Spot, 2=Spot→Funding
    source_type   SMALLINT NOT NULL,           -- 1=Funding, 2=Trading
    
    state         SMALLINT NOT NULL DEFAULT 0,
    error_message TEXT,
    retry_count   INTEGER NOT NULL DEFAULT 0,
    
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes
CREATE INDEX idx_fsm_transfers_active_state 
    ON fsm_transfers_tb(state, updated_at) 
    WHERE state NOT IN (40, -10, -30);

CREATE INDEX idx_fsm_transfers_user_id 
    ON fsm_transfers_tb(user_id, created_at DESC);

CREATE INDEX idx_fsm_transfers_cid 
    ON fsm_transfers_tb(cid) 
    WHERE cid IS NOT NULL;

-- Transfer operations table
CREATE TABLE transfer_operations_tb (
    op_id         BIGSERIAL PRIMARY KEY,
    transfer_id   VARCHAR(26) NOT NULL,        -- References fsm_transfers_tb.transfer_id
    op_type       VARCHAR(16) NOT NULL,
    service_type  SMALLINT NOT NULL,
    result        VARCHAR(16) NOT NULL,
    error_message TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    CONSTRAINT transfer_operations_unique UNIQUE (transfer_id, op_type, service_type)
);

CREATE INDEX idx_transfer_ops_transfer_id ON transfer_operations_tb(transfer_id);

COMMENT ON COLUMN fsm_transfers_tb.id IS 'Auto-increment PK for count estimation';
COMMENT ON COLUMN fsm_transfers_tb.transfer_id IS 'ULID unique identifier (was req_id)';
