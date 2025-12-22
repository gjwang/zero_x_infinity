-- 002_create_api_keys.sql
-- API Keys table for authentication
-- Supports multiple key types: Ed25519 (default), HMAC-SHA256, RSA

CREATE TABLE IF NOT EXISTS api_keys_tb (
    key_id         SERIAL PRIMARY KEY,
    user_id        BIGINT NOT NULL REFERENCES users_tb(user_id),
    api_key        VARCHAR(35) UNIQUE NOT NULL,  -- AK_ + 16 hex = 19 chars
    key_type       SMALLINT NOT NULL DEFAULT 1,  -- 1=Ed25519, 2=HMAC-SHA256, 3=RSA
    key_data       BYTEA NOT NULL,               -- Public key or secret hash
    label          VARCHAR(64),                  -- User-defined label
    permissions    INT NOT NULL DEFAULT 1,       -- Bitmask: 1=READ, 2=TRADE, 4=WITHDRAW, 8=TRANSFER
    status         SMALLINT NOT NULL DEFAULT 1,  -- 0=disabled, 1=active
    last_ts_nonce  BIGINT NOT NULL DEFAULT 0,    -- Last used timestamp nonce (replay protection)
    created_at     TIMESTAMPTZ DEFAULT NOW(),
    expires_at     TIMESTAMPTZ DEFAULT NOW() + INTERVAL '1 year',
    last_used_at   TIMESTAMPTZ,
    
    CONSTRAINT chk_api_key_format CHECK (
        api_key ~ '^AK_[0-9A-F]{16}$'  -- AK_ + 16 uppercase hex
    ),
    CONSTRAINT chk_key_data_len CHECK (
        (key_type = 1 AND length(key_data) = 32) OR  -- Ed25519: 32 bytes
        (key_type = 2 AND length(key_data) = 32) OR  -- HMAC: 32 bytes  
        (key_type = 3)                                -- RSA: variable
    )
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_api_keys_user ON api_keys_tb(user_id);
CREATE INDEX IF NOT EXISTS idx_api_keys_status ON api_keys_tb(status) WHERE status = 1;

-- Comments
COMMENT ON TABLE api_keys_tb IS 'API Keys for authentication';
COMMENT ON COLUMN api_keys_tb.key_type IS '1=Ed25519, 2=HMAC-SHA256, 3=RSA';
COMMENT ON COLUMN api_keys_tb.permissions IS 'Bitmask: 1=READ, 2=TRADE, 4=WITHDRAW, 8=TRANSFER';
COMMENT ON COLUMN api_keys_tb.last_ts_nonce IS 'Monotonically increasing nonce for replay protection';
