-- 006_trade_fee.sql
-- Add base fee rates to symbols (10^6 precision: 1000 = 0.10%)

-- Symbol base fee rates
ALTER TABLE symbols_tb ADD COLUMN base_maker_fee INTEGER NOT NULL DEFAULT 1000;  -- 0.10%
ALTER TABLE symbols_tb ADD COLUMN base_taker_fee INTEGER NOT NULL DEFAULT 2000;  -- 0.20%

COMMENT ON COLUMN symbols_tb.base_maker_fee IS 'Base maker fee rate (10^6 precision: 1000 = 0.10%)';
COMMENT ON COLUMN symbols_tb.base_taker_fee IS 'Base taker fee rate (10^6 precision: 1000 = 0.10%)';

-- VIP discount levels table
CREATE TABLE IF NOT EXISTS vip_levels_tb (
    level           SMALLINT PRIMARY KEY,
    discount_percent SMALLINT NOT NULL DEFAULT 100,  -- 100 = no discount, 50 = 50% off
    min_volume      DECIMAL(30, 8) DEFAULT 0,        -- Trading volume for upgrade
    description     VARCHAR(64)
);

-- Insert default VIP levels
INSERT INTO vip_levels_tb (level, discount_percent, description) VALUES
    (0, 100, 'Normal'),
    (1, 90, 'VIP 1'),
    (2, 80, 'VIP 2'),
    (3, 70, 'VIP 3'),
    (4, 60, 'VIP 4'),
    (5, 50, 'VIP 5')
ON CONFLICT (level) DO NOTHING;

-- User VIP level
ALTER TABLE users_tb ADD COLUMN IF NOT EXISTS vip_level SMALLINT NOT NULL DEFAULT 0;
