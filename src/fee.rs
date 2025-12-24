//! Fee calculation utilities
//!
//! All fee rates use 10^6 precision: 1000 = 0.10%

/// Fee rate precision (10^6 = 1,000,000)
pub const FEE_PRECISION: u64 = 1_000_000;

/// Default maker fee rate (1000 = 0.10%)
pub const DEFAULT_MAKER_FEE: u64 = 1000;

/// Default taker fee rate (2000 = 0.20%)
pub const DEFAULT_TAKER_FEE: u64 = 2000;

/// Calculate fee from amount and rate.
///
/// Uses u128 intermediate to prevent overflow.
///
/// # Arguments
/// * `amount` - Amount in scaled units (e.g., satoshis for BTC)
/// * `rate` - Fee rate in 10^6 precision (1000 = 0.10%)
///
/// # Returns
/// Fee amount in same units as input amount
///
/// # Example
/// ```
/// use zero_x_infinity::fee::calculate_fee;
/// // 1 BTC (100_000_000 satoshis) * 0.20% = 200_000 satoshis
/// let fee = calculate_fee(100_000_000, 2000);
/// assert_eq!(fee, 200_000);
/// ```
#[inline]
pub fn calculate_fee(amount: u64, rate: u64) -> u64 {
    let fee = (amount as u128 * rate as u128) / FEE_PRECISION as u128;
    // Minimum fee is 1 if amount > 0 and rate > 0
    if fee == 0 && amount > 0 && rate > 0 {
        1
    } else {
        fee as u64
    }
}

/// Calculate fee with VIP discount.
///
/// # Arguments
/// * `amount` - Amount in scaled units
/// * `base_rate` - Base fee rate (10^6 precision)
/// * `discount_percent` - VIP discount (100 = no discount, 50 = 50% off)
#[inline]
pub fn calculate_fee_with_discount(amount: u64, base_rate: u64, discount_percent: u8) -> u64 {
    let effective_rate = base_rate * discount_percent as u64 / 100;
    calculate_fee(amount, effective_rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_fee_basic() {
        // 1 BTC (100M satoshis) * 0.20% = 200,000 satoshis
        assert_eq!(calculate_fee(100_000_000, 2000), 200_000);

        // 1 BTC * 0.10% = 100,000 satoshis
        assert_eq!(calculate_fee(100_000_000, 1000), 100_000);
    }

    #[test]
    fn test_calculate_fee_small_amount() {
        // Small amount that would round to 0 -> minimum fee is 1
        assert_eq!(calculate_fee(100, 1000), 1); // 100 * 0.10% = 0.1 -> 1
        assert_eq!(calculate_fee(1, 1000), 1); // 1 * 0.10% = 0.001 -> 1
    }

    #[test]
    fn test_calculate_fee_zero() {
        // Zero amount = zero fee
        assert_eq!(calculate_fee(0, 1000), 0);
        // Zero rate = zero fee
        assert_eq!(calculate_fee(100_000, 0), 0);
    }

    #[test]
    fn test_calculate_fee_with_discount() {
        // VIP 5 (50% discount): rate = 2000 * 50 / 100 = 1000
        // 1 BTC * 0.10% = 100,000 satoshis
        assert_eq!(calculate_fee_with_discount(100_000_000, 2000, 50), 100_000);

        // No discount (100%)
        assert_eq!(calculate_fee_with_discount(100_000_000, 2000, 100), 200_000);
    }

    #[test]
    fn test_no_overflow() {
        // Large amount close to u64::MAX should not overflow
        let large_amount: u64 = 10_000_000_000_000_000_000; // 10^19
        let fee = calculate_fee(large_amount, 2000);
        assert_eq!(fee, 20_000_000_000_000_000); // 0.20% of 10^19
    }
}
