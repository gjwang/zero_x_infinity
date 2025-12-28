-- 008_users_auth.sql
-- Add password authentication support to users_tb
-- Part of Phase 0x10.6 Essential Services

ALTER TABLE users_tb 
ADD COLUMN IF NOT EXISTS password_hash VARCHAR(255),
ADD COLUMN IF NOT EXISTS salt VARCHAR(64);

COMMENT ON COLUMN users_tb.password_hash IS 'Argon2id password hash';
COMMENT ON COLUMN users_tb.salt IS 'Salt for password hashing (if not embedded in hash)';
