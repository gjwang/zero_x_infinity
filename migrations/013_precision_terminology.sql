-- 013_precision_terminology.sql
-- Precision Terminology Refactor
-- 
-- RATIONALE:
-- The old naming (decimals, price_decimals) caused confusion between:
--   1. Internal storage scale (10^decimals for Decimal↔u64 conversion)
--   2. API precision (max decimals allowed in API input/output)
--
-- This migration:
--   1. Renames columns to clarify internal scale role
--   2. Adds explicit API precision columns
--
-- See: docs/standards/money-type-safety.md Section 2.5

-- ============================================================================
-- ASSETS TABLE: Add asset_precision column
-- ============================================================================

-- Step 1: Rename decimals → internal_scale
ALTER TABLE assets_tb RENAME COLUMN decimals TO internal_scale;

-- Step 2: Add asset_precision column (API boundary precision)
-- Default: same as internal_scale (can be configured to fewer decimals)
ALTER TABLE assets_tb ADD COLUMN asset_precision SMALLINT;
UPDATE assets_tb SET asset_precision = internal_scale;
ALTER TABLE assets_tb ALTER COLUMN asset_precision SET NOT NULL;

-- Add comment for documentation
COMMENT ON COLUMN assets_tb.internal_scale IS 'Internal storage scale factor (e.g., 8 for BTC = 10^8 satoshi)';
COMMENT ON COLUMN assets_tb.asset_precision IS 'API precision for input validation and output formatting';

-- ============================================================================
-- SYMBOLS TABLE: Add price_precision column
-- ============================================================================

-- Step 1: Rename price_decimals → price_scale
ALTER TABLE symbols_tb RENAME COLUMN price_decimals TO price_scale;

-- Step 2: Add price_precision column
ALTER TABLE symbols_tb ADD COLUMN price_precision SMALLINT;
UPDATE symbols_tb SET price_precision = price_scale;
ALTER TABLE symbols_tb ALTER COLUMN price_precision SET NOT NULL;

-- Step 3: Rename qty_decimals → qty_scale for consistency
ALTER TABLE symbols_tb RENAME COLUMN qty_decimals TO qty_scale;

-- Step 4: Add qty_precision column
ALTER TABLE symbols_tb ADD COLUMN qty_precision SMALLINT;
UPDATE symbols_tb SET qty_precision = qty_scale;
ALTER TABLE symbols_tb ALTER COLUMN qty_precision SET NOT NULL;

-- Add comments
COMMENT ON COLUMN symbols_tb.price_scale IS 'Internal price scale factor';
COMMENT ON COLUMN symbols_tb.price_precision IS 'API price precision for input/output';
COMMENT ON COLUMN symbols_tb.qty_scale IS 'Internal quantity scale factor (usually = base_asset.internal_scale)';
COMMENT ON COLUMN symbols_tb.qty_precision IS 'API quantity precision for input/output';
