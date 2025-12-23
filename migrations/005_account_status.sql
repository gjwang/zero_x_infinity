-- 005_account_status.sql
-- Add account status field for transfer validation (Phase 0x0B-a §1.5.5)

-- Add account status column to balances_tb
-- 1 = ACTIVE (default), 2 = FROZEN, 3 = DISABLED
ALTER TABLE balances_tb 
ADD COLUMN IF NOT EXISTS status SMALLINT NOT NULL DEFAULT 1;

-- Add comment for documentation
COMMENT ON COLUMN balances_tb.status IS 'Account status: 1=ACTIVE, 2=FROZEN, 3=DISABLED';

-- Create index for status queries (useful for admin queries)
CREATE INDEX IF NOT EXISTS idx_balances_status ON balances_tb(status) WHERE status != 1;
