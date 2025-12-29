-- 011_rename_network_to_chain_id.sql
-- Phase 0x11-b: Rename 'network' column to 'chain_id' for naming consistency
-- 
-- Rationale: Sentinel code uses 'chain_id' terminology (from ChainScanner.chain_id()),
-- so the database should match for consistency.

-- 1. Rename column in user_addresses table
ALTER TABLE user_addresses RENAME COLUMN network TO chain_id;

-- 2. Drop old primary key and recreate with new column name
-- Note: In PostgreSQL, renaming a column automatically updates PK constraint
-- No action needed for PK

-- 3. Update deposit_history to add chain_id if not present
ALTER TABLE deposit_history ADD COLUMN IF NOT EXISTS chain_id VARCHAR(32);
