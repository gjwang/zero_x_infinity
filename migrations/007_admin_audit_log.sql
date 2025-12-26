-- 007_admin_audit_log.sql
-- Admin Dashboard Audit Log Table (Phase 0x0F)

CREATE TABLE IF NOT EXISTS admin_audit_log (
    id BIGSERIAL PRIMARY KEY,
    admin_id BIGINT NOT NULL,
    admin_username VARCHAR(64),
    ip_address VARCHAR(45) NOT NULL,  -- IPv6 support
    action VARCHAR(32) NOT NULL,       -- GET/POST/PUT/DELETE
    path VARCHAR(256) NOT NULL,        -- /admin/asset/1
    entity_type VARCHAR(32),           -- asset/symbol/vip_level
    entity_id BIGINT,
    old_value JSONB,
    new_value JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Indexes for query performance
CREATE INDEX IF NOT EXISTS idx_audit_admin_id ON admin_audit_log(admin_id);
CREATE INDEX IF NOT EXISTS idx_audit_created_at ON admin_audit_log(created_at);
CREATE INDEX IF NOT EXISTS idx_audit_entity ON admin_audit_log(entity_type, entity_id);

COMMENT ON TABLE admin_audit_log IS 'Admin dashboard operation audit log';
COMMENT ON COLUMN admin_audit_log.old_value IS 'Previous state before modification (JSON)';
COMMENT ON COLUMN admin_audit_log.new_value IS 'New state after modification (JSON)';
