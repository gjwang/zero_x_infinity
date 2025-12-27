-- 012_audit_log_trace_id.sql
-- UX-10: Add trace_id column for evidence chain
-- ULID format: 26 characters

ALTER TABLE admin_audit_log 
ADD COLUMN IF NOT EXISTS trace_id VARCHAR(26);

-- Index for trace_id lookups
CREATE INDEX IF NOT EXISTS idx_audit_trace_id ON admin_audit_log(trace_id);

-- Comment
COMMENT ON COLUMN admin_audit_log.trace_id IS 'ULID trace ID for request tracking (UX-10)';
