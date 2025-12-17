// models.rs - Core order and trade types

/// Order side: Buy or Sell
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

/// Order type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Limit,  // Limit order: must specify price
    Market, // Market order: execute at best avail price
}

/// Order status - all possible terminal states for a persisted order
///
/// Per 0x08a design doc, once an order is persisted, it MUST reach
/// one of these terminal states (never disappear or become unknown).
///
/// Reference: Binance API order status enums
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderStatus {
    New,             // Just created, waiting in orderbook
    PartiallyFilled, // Some quantity filled, rest in orderbook
    Filled,          // Fully filled
    Cancelled,       // Cancelled by user
    Rejected,        // Rejected after persistence (e.g., balance check failed)
    Expired,         // Expired by system (e.g., GTD order timeout, IOC/FOK no fill)
                     // TODO: Future implementation
                     // PendingNew,      // Pending in order list (OCO orders) - not implemented
                     // PendingCancel,   // Pending cancel - not implemented (unused in Binance)
                     // ExpiredInMatch,  // Expired due to STP (Self-Trade Prevention) - not implemented
}

// ============================================================
// INTERNAL ORDER (the core order type used throughout the system)
// ============================================================

/// Internal order - used by ME, OrderBook, UBSCore, WAL
///
/// "Internal" = trusted, from Gateway (already validated and scaled)
/// price and qty are raw u64 (already scaled by Gateway)
#[derive(Debug, Clone)]
pub struct InternalOrder {
    pub order_id: u64,
    pub user_id: u64,
    pub symbol_id: u32, // Trading pair
    pub price: u64,     // Raw u64 (already scaled by Gateway)
    pub qty: u64,       // Raw u64 (already scaled by Gateway)
    pub filled_qty: u64,
    pub side: Side,
    pub order_type: OrderType,
    pub status: OrderStatus,
}

impl InternalOrder {
    /// Create a new limit order
    pub fn new(
        order_id: u64,
        user_id: u64,
        symbol_id: u32,
        price: u64,
        qty: u64,
        side: Side,
    ) -> Self {
        Self {
            order_id,
            user_id,
            symbol_id,
            price,
            qty,
            filled_qty: 0,
            side,
            order_type: OrderType::Limit,
            status: OrderStatus::New,
        }
    }

    /// Create a market order
    pub fn market(order_id: u64, user_id: u64, symbol_id: u32, qty: u64, side: Side) -> Self {
        Self {
            order_id,
            user_id,
            symbol_id,
            price: if side == Side::Buy { u64::MAX } else { 0 },
            qty,
            filled_qty: 0,
            side,
            order_type: OrderType::Market,
            status: OrderStatus::New,
        }
    }

    /// Remaining quantity to fill
    #[inline]
    pub fn remaining_qty(&self) -> u64 {
        self.qty - self.filled_qty
    }

    /// Check if order is fully filled
    #[inline]
    pub fn is_filled(&self) -> bool {
        self.filled_qty >= self.qty
    }

    /// Calculate order cost (amount to lock)
    ///
    /// # Arguments
    /// - `qty_unit`: Base asset unit (e.g., 10^8 for BTC) for quote amount calculation
    ///
    /// # Formula
    /// - Buy: `price × qty / qty_unit` = quote amount to lock
    /// - Sell: `qty` = base amount to lock
    ///
    /// # Example (BTC/USDT with base_decimals=8, quote_decimals=6)
    /// ```text
    /// Buy 1 BTC @ 30000 USDT:
    ///   price = 30000_000000 (30000 USDT, 6 decimals)
    ///   qty   = 1_00000000   (1 BTC, 8 decimals)
    ///   qty_unit = 10^8
    ///   cost = 30000_000000 * 1_00000000 / 10^8 = 30000_000000 (30000 USDT)
    ///
    /// Sell 1 BTC:
    ///   cost = 1_00000000 (1 BTC, lock base asset directly)
    /// ```
    ///
    /// # Errors
    /// Returns `CostError::Overflow` if the calculated cost exceeds u64::MAX
    #[inline]
    pub fn calculate_cost(&self, qty_unit: u64) -> Result<u64, CostError> {
        match self.side {
            Side::Buy => {
                // Use u128 to avoid overflow: price * qty can exceed u64::MAX
                let cost_128 = (self.price as u128) * (self.qty as u128) / (qty_unit as u128);
                // Return explicit error if result exceeds u64 range
                if cost_128 > u64::MAX as u128 {
                    Err(CostError::Overflow {
                        price: self.price,
                        qty: self.qty,
                        qty_unit,
                    })
                } else {
                    Ok(cost_128 as u64)
                }
            }
            Side::Sell => Ok(self.qty),
        }
    }
}

// ============================================================
// COST ERROR - Explicit error types for cost calculation
// ============================================================

/// Cost calculation error - explicit error types for financial-grade systems
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostError {
    /// Cost calculation overflowed u64 range
    Overflow { price: u64, qty: u64, qty_unit: u64 },
}

impl std::fmt::Display for CostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CostError::Overflow {
                price,
                qty,
                qty_unit,
            } => {
                write!(
                    f,
                    "Cost overflow: price={} * qty={} / qty_unit={} exceeds u64::MAX",
                    price, qty, qty_unit
                )
            }
        }
    }
}

impl std::error::Error for CostError {}

// ============================================================
// TRADE
// ============================================================

/// A trade that occurred when orders matched
#[derive(Debug, Clone)]
pub struct Trade {
    pub trade_id: u64,
    pub buyer_order_id: u64,
    pub seller_order_id: u64,
    pub buyer_user_id: u64,
    pub seller_user_id: u64,
    pub price: u64,
    pub qty: u64,
}

impl Trade {
    pub fn new(
        trade_id: u64,
        buyer_order_id: u64,
        seller_order_id: u64,
        buyer_user_id: u64,
        seller_user_id: u64,
        price: u64,
        qty: u64,
    ) -> Self {
        Self {
            trade_id,
            buyer_order_id,
            seller_order_id,
            buyer_user_id,
            seller_user_id,
            price,
            qty,
        }
    }
}

// ============================================================
// ORDER RESULT
// ============================================================

/// Result of adding an order to the book
#[derive(Debug)]
pub struct OrderResult {
    pub order: InternalOrder,
    pub trades: Vec<Trade>,
}

// ============================================================
// TESTS
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Test constants matching real config
    const BTC_DECIMALS: u32 = 8;
    const USDT_DECIMALS: u32 = 6;
    const QTY_UNIT: u64 = 10u64.pow(BTC_DECIMALS); // 10^8 for BTC

    /// Helper to create a Buy order
    fn buy_order(price: u64, qty: u64) -> InternalOrder {
        InternalOrder::new(1, 1, 0, price, qty, Side::Buy)
    }

    /// Helper to create a Sell order
    fn sell_order(price: u64, qty: u64) -> InternalOrder {
        InternalOrder::new(1, 1, 0, price, qty, Side::Sell)
    }

    // ============================================================
    // Buy Order Tests - cost = price × qty / qty_unit
    // ============================================================

    #[test]
    fn test_buy_cost_1_btc_at_30000_usdt() {
        // Buy 1 BTC @ 30000 USDT
        // price = 30000 * 10^6 = 30_000_000_000 (30000 USDT in internal units)
        // qty = 1 * 10^8 = 100_000_000 (1 BTC in satoshi)
        // cost = 30_000_000_000 * 100_000_000 / 10^8 = 30_000_000_000 (30000 USDT)
        let price = 30000u64 * 10u64.pow(USDT_DECIMALS);
        let qty = 1u64 * 10u64.pow(BTC_DECIMALS);
        let order = buy_order(price, qty);

        let cost = order.calculate_cost(QTY_UNIT);
        assert_eq!(cost, Ok(30_000_000_000)); // 30000 USDT in internal units
    }

    #[test]
    fn test_buy_cost_0_1_btc_at_30000_usdt() {
        // Buy 0.1 BTC @ 30000 USDT
        // price = 30000 * 10^6 = 30_000_000_000
        // qty = 0.1 * 10^8 = 10_000_000
        // cost = 30_000_000_000 * 10_000_000 / 10^8 = 3_000_000_000 (3000 USDT)
        let price = 30000u64 * 10u64.pow(USDT_DECIMALS);
        let qty = 10_000_000u64; // 0.1 BTC
        let order = buy_order(price, qty);

        let cost = order.calculate_cost(QTY_UNIT);
        assert_eq!(cost, Ok(3_000_000_000)); // 3000 USDT
    }

    #[test]
    fn test_buy_cost_0_00001_btc_at_50000_usdt() {
        // Buy 0.00001 BTC @ 50000 USDT
        // price = 50000 * 10^6 = 50_000_000_000
        // qty = 0.00001 * 10^8 = 1000 satoshi
        // cost = 50_000_000_000 * 1000 / 10^8 = 500_000 (0.5 USDT)
        let price = 50000u64 * 10u64.pow(USDT_DECIMALS);
        let qty = 1000u64; // 0.00001 BTC
        let order = buy_order(price, qty);

        let cost = order.calculate_cost(QTY_UNIT);
        assert_eq!(cost, Ok(500_000)); // 0.5 USDT
    }

    #[test]
    fn test_buy_cost_with_decimal_price() {
        // Buy 1 BTC @ 30000.50 USDT
        // price = 30000.50 * 10^6 = 30_000_500_000
        // qty = 1 * 10^8 = 100_000_000
        // cost = 30_000_500_000 * 100_000_000 / 10^8 = 30_000_500_000 (30000.50 USDT)
        let price = 30_000_500_000u64; // 30000.50 USDT
        let qty = 100_000_000u64; // 1 BTC
        let order = buy_order(price, qty);

        let cost = order.calculate_cost(QTY_UNIT);
        assert_eq!(cost, Ok(30_000_500_000)); // 30000.50 USDT
    }

    // ============================================================
    // Sell Order Tests - cost = qty (no division needed)
    // ============================================================

    #[test]
    fn test_sell_cost_1_btc() {
        // Sell 1 BTC - cost is just the qty
        let qty = 100_000_000u64; // 1 BTC
        let order = sell_order(30_000_000_000, qty);

        let cost = order.calculate_cost(QTY_UNIT);
        assert_eq!(cost, Ok(100_000_000)); // 1 BTC in satoshi
    }

    #[test]
    fn test_sell_cost_0_5_btc() {
        // Sell 0.5 BTC
        let qty = 50_000_000u64; // 0.5 BTC
        let order = sell_order(25_000_000_000, qty);

        let cost = order.calculate_cost(QTY_UNIT);
        assert_eq!(cost, Ok(50_000_000)); // 0.5 BTC
    }

    #[test]
    fn test_sell_cost_ignores_price() {
        // Sell order cost should be independent of price
        let qty = 100_000_000u64;
        let order1 = sell_order(10_000_000_000, qty); // @ 10000 USDT
        let order2 = sell_order(100_000_000_000, qty); // @ 100000 USDT

        assert_eq!(order1.calculate_cost(QTY_UNIT), Ok(qty));
        assert_eq!(order2.calculate_cost(QTY_UNIT), Ok(qty));
    }

    // ============================================================
    // Edge Cases and Overflow Tests
    // ============================================================

    #[test]
    fn test_buy_cost_overflow_returns_error() {
        // Test overflow protection
        // price = u64::MAX, qty = u64::MAX → even with u128 intermediate,
        // result exceeds u64::MAX → returns explicit error
        let order = buy_order(u64::MAX, u64::MAX);
        let result = order.calculate_cost(QTY_UNIT);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            CostError::Overflow {
                price: u64::MAX,
                qty: u64::MAX,
                qty_unit: QTY_UNIT,
            }
        );
    }

    #[test]
    fn test_buy_cost_near_overflow_boundary() {
        // Test near overflow boundary - but with u128 intermediate, this works
        // price = 10^18, qty = 10^8 → 10^26 / 10^8 = 10^18 (fits in u64)
        let price = 1_000_000_000_000_000_000u64; // 10^18
        let qty = 100_000_000u64; // 10^8
        let order = buy_order(price, qty);
        let cost = order.calculate_cost(QTY_UNIT);
        // 10^18 * 10^8 / 10^8 = 10^18 = 1_000_000_000_000_000_000
        assert_eq!(cost, Ok(1_000_000_000_000_000_000));
    }

    #[test]
    fn test_buy_cost_real_world_overflow_case() {
        // CRITICAL: Real-world case from production test data
        // Order #21: Buy 2.562844 BTC @ 84956.01 USDT
        //
        // With naive u64: price * qty = 2.177×10^19 > u64::MAX (1.84×10^19)
        //   → wrapping overflow → 33,261,559,755 (WRONG!)
        //
        // With u128 intermediate: 217,729,000,492 (CORRECT!)
        //
        // This test ensures we don't silently under-lock funds due to overflow.
        let price = 84_956_010_000u64; // 84956.01 USDT (6 decimals)
        let qty = 256_284_400u64; // 2.562844 BTC (8 decimals)
        let qty_unit = 100_000_000u64; // 10^8

        let order = buy_order(price, qty);
        let cost = order.calculate_cost(qty_unit);

        // Correct result: 217,729,000,492 (about 217,729 USDT)
        // NOT 33,261,559,755 (which would be from u64 wrapping overflow)
        assert_eq!(cost, Ok(217_729_000_492));

        // Verify this would overflow in naive u64 multiplication
        assert!(
            price.checked_mul(qty).is_none(),
            "This case should overflow u64"
        );
    }

    #[test]
    fn test_buy_cost_zero_price() {
        // Zero price → cost = 0
        let order = buy_order(0, 100_000_000);
        assert_eq!(order.calculate_cost(QTY_UNIT), Ok(0));
    }

    #[test]
    fn test_buy_cost_zero_qty() {
        // Zero qty → cost = 0
        let order = buy_order(30_000_000_000, 0);
        assert_eq!(order.calculate_cost(QTY_UNIT), Ok(0));
    }

    #[test]
    fn test_sell_cost_zero_qty() {
        // Zero qty → cost = 0
        let order = sell_order(30_000_000_000, 0);
        assert_eq!(order.calculate_cost(QTY_UNIT), Ok(0));
    }

    // ============================================================
    // Cross-validation with main.rs formula
    // ============================================================

    #[test]
    fn test_buy_cost_matches_main_formula() {
        // Verify calculate_cost matches the formula used in main.rs:
        // let cost = input.price * input.qty / qty_unit;
        let price = 42_123_456_789u64; // 42123.456789 USDT
        let qty = 123_456_789u64; // 1.23456789 BTC
        let qty_unit = QTY_UNIT;

        let order = buy_order(price, qty);
        let calculated_cost = order.calculate_cost(qty_unit);

        // Manual calculation (same as main.rs)
        let expected_cost = price * qty / qty_unit;
        assert_eq!(calculated_cost, Ok(expected_cost));
    }
}
