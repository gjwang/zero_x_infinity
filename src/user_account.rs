// user_account.rs - User account and balance management

use rustc_hash::FxHashMap;

/// Asset balance for a user
/// Tracks avail and frozen (locked in orders) amounts
#[derive(Debug, Clone, Default)]
pub struct Balance {
    pub avail: u64,  // Available for new orders (short name for efficient JSON output)
    pub frozen: u64, // Locked in pending orders
}

impl Balance {
    pub fn new() -> Self {
        Self::default()
    }

    /// Total balance = avail + frozen
    #[inline]
    pub fn total(&self) -> u64 {
        self.avail + self.frozen
    }

    /// Deposit funds (adds to avail)
    /// Returns false if overflow would occur - critical for financial systems
    pub fn deposit(&mut self, amount: u64) -> bool {
        match self.avail.checked_add(amount) {
            Some(new_avail) => {
                self.avail = new_avail;
                true
            }
            None => false, // Overflow! This is a bug that needs investigation
        }
    }

    /// Withdraw funds (from avail only)
    pub fn withdraw(&mut self, amount: u64) -> bool {
        if self.avail >= amount {
            self.avail -= amount;
            true
        } else {
            false
        }
    }

    /// Freeze funds for an order (move from avail to frozen)
    pub fn freeze(&mut self, amount: u64) -> bool {
        if self.avail >= amount {
            self.avail -= amount;
            self.frozen += amount;
            true
        } else {
            false
        }
    }

    /// Unfreeze funds (move from frozen back to avail, e.g., order cancelled)
    pub fn unfreeze(&mut self, amount: u64) -> bool {
        if self.frozen >= amount {
            self.frozen -= amount;
            self.avail += amount;
            true
        } else {
            false
        }
    }

    /// Consume frozen funds (order executed, funds leave the account)
    pub fn consume_frozen(&mut self, amount: u64) -> bool {
        if self.frozen >= amount {
            self.frozen -= amount;
            true
        } else {
            false
        }
    }

    /// Receive funds from a trade (adds to avail)
    /// Returns false if overflow would occur
    pub fn receive(&mut self, amount: u64) -> bool {
        match self.avail.checked_add(amount) {
            Some(new_avail) => {
                self.avail = new_avail;
                true
            }
            None => false,
        }
    }
}

/// A user account holding multiple asset balances
/// Uses FxHashMap for O(1) asset lookup - faster than std HashMap for integer keys
/// (FxHashMap uses a simpler, faster hash function optimized for small keys)
#[derive(Debug, Clone)]
pub struct UserAccount {
    pub user_id: u64,
    balances: FxHashMap<u32, Balance>, // asset_id -> Balance
}

impl UserAccount {
    pub fn new(user_id: u64) -> Self {
        Self {
            user_id,
            balances: FxHashMap::default(),
        }
    }

    /// Get balance for an asset (creates if not exists)
    pub fn get_balance_mut(&mut self, asset_id: u32) -> &mut Balance {
        self.balances.entry(asset_id).or_insert_with(Balance::new)
    }

    /// Get balance for an asset (read-only)
    pub fn get_balance(&self, asset_id: u32) -> Option<&Balance> {
        self.balances.get(&asset_id)
    }

    /// Deposit funds into the account
    pub fn deposit(&mut self, asset_id: u32, amount: u64) {
        self.get_balance_mut(asset_id).deposit(amount);
    }

    /// Withdraw funds from the account
    pub fn withdraw(&mut self, asset_id: u32, amount: u64) -> bool {
        self.get_balance_mut(asset_id).withdraw(amount)
    }

    /// Get avail balance for an asset
    pub fn avail(&self, asset_id: u32) -> u64 {
        self.balances.get(&asset_id).map(|b| b.avail).unwrap_or(0)
    }

    /// Get frozen balance for an asset
    pub fn frozen(&self, asset_id: u32) -> u64 {
        self.balances.get(&asset_id).map(|b| b.frozen).unwrap_or(0)
    }
}

/// User account manager - holds all user accounts
#[derive(Debug, Default)]
pub struct AccountManager {
    accounts: FxHashMap<u64, UserAccount>, // user_id -> UserAccount
}

impl AccountManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get or create a user account
    pub fn get_account_mut(&mut self, user_id: u64) -> &mut UserAccount {
        self.accounts
            .entry(user_id)
            .or_insert_with(|| UserAccount::new(user_id))
    }

    /// Get a user account (read-only)
    pub fn get_account(&self, user_id: u64) -> Option<&UserAccount> {
        self.accounts.get(&user_id)
    }

    /// Deposit funds to a user's account
    pub fn deposit(&mut self, user_id: u64, asset_id: u32, amount: u64) {
        self.get_account_mut(user_id).deposit(asset_id, amount);
    }

    /// Check if user has sufficient avail balance
    pub fn has_sufficient_balance(&self, user_id: u64, asset_id: u32, amount: u64) -> bool {
        self.accounts
            .get(&user_id)
            .map(|acc| acc.avail(asset_id) >= amount)
            .unwrap_or(false)
    }

    /// Freeze funds for an order
    pub fn freeze(&mut self, user_id: u64, asset_id: u32, amount: u64) -> bool {
        self.get_account_mut(user_id)
            .get_balance_mut(asset_id)
            .freeze(amount)
    }

    /// Unfreeze funds (order cancelled)
    pub fn unfreeze(&mut self, user_id: u64, asset_id: u32, amount: u64) -> bool {
        self.get_account_mut(user_id)
            .get_balance_mut(asset_id)
            .unfreeze(amount)
    }

    /// Settle a trade: buyer receives base asset, seller receives quote asset
    /// The frozen funds are consumed, and received funds are added
    pub fn settle_trade(
        &mut self,
        buyer_id: u64,
        seller_id: u64,
        base_asset_id: u32,
        quote_asset_id: u32,
        base_amount: u64,  // quantity traded
        quote_amount: u64, // price * quantity
    ) {
        // Buyer: spent quote asset (frozen -> consumed), receives base asset
        self.get_account_mut(buyer_id)
            .get_balance_mut(quote_asset_id)
            .consume_frozen(quote_amount);
        self.get_account_mut(buyer_id)
            .get_balance_mut(base_asset_id)
            .receive(base_amount);

        // Seller: spent base asset (frozen -> consumed), receives quote asset
        self.get_account_mut(seller_id)
            .get_balance_mut(base_asset_id)
            .consume_frozen(base_amount);
        self.get_account_mut(seller_id)
            .get_balance_mut(quote_asset_id)
            .receive(quote_amount);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BTC: u32 = 1;
    const USDT: u32 = 2;

    #[test]
    fn test_balance_deposit_withdraw() {
        let mut balance = Balance::new();

        balance.deposit(1000);
        assert_eq!(balance.avail, 1000);
        assert_eq!(balance.frozen, 0);

        assert!(balance.withdraw(300));
        assert_eq!(balance.avail, 700);

        assert!(!balance.withdraw(800)); // Insufficient
        assert_eq!(balance.avail, 700);
    }

    #[test]
    fn test_balance_freeze_unfreeze() {
        let mut balance = Balance::new();
        balance.deposit(1000);

        assert!(balance.freeze(400));
        assert_eq!(balance.avail, 600);
        assert_eq!(balance.frozen, 400);

        assert!(balance.unfreeze(200));
        assert_eq!(balance.avail, 800);
        assert_eq!(balance.frozen, 200);

        assert!(!balance.freeze(900)); // Insufficient avail
    }

    #[test]
    fn test_user_account() {
        let mut account = UserAccount::new(1);

        account.deposit(BTC, 10_00000000); // 10 BTC
        account.deposit(USDT, 100000_00000000); // 100,000 USDT

        assert_eq!(account.avail(BTC), 10_00000000);
        assert_eq!(account.avail(USDT), 100000_00000000);
    }

    #[test]
    fn test_account_manager_settle_trade() {
        let mut manager = AccountManager::new();

        // User 1 (buyer): has USDT, wants BTC
        manager.deposit(1, USDT, 100000_00000000); // 100,000 USDT

        // User 2 (seller): has BTC, wants USDT
        manager.deposit(2, BTC, 10_00000000); // 10 BTC

        // Freeze funds for orders
        // Buyer freezes USDT (to buy 1 BTC at 50,000 USDT)
        assert!(manager.freeze(1, USDT, 50000_00000000));
        // Seller freezes BTC
        assert!(manager.freeze(2, BTC, 1_00000000));

        // Settle trade: 1 BTC @ 50,000 USDT
        manager.settle_trade(
            1, // buyer
            2, // seller
            BTC,
            USDT,
            1_00000000,     // 1 BTC
            50000_00000000, // 50,000 USDT
        );

        // Check balances after trade
        // Buyer: -50,000 USDT (was frozen, now consumed), +1 BTC
        assert_eq!(manager.get_account(1).unwrap().avail(USDT), 50000_00000000); // 100k - 50k frozen = 50k avail
        assert_eq!(manager.get_account(1).unwrap().frozen(USDT), 0);
        assert_eq!(manager.get_account(1).unwrap().avail(BTC), 1_00000000);

        // Seller: -1 BTC (was frozen, now consumed), +50,000 USDT
        assert_eq!(manager.get_account(2).unwrap().avail(BTC), 9_00000000); // 10 - 1 frozen = 9 avail
        assert_eq!(manager.get_account(2).unwrap().frozen(BTC), 0);
        assert_eq!(manager.get_account(2).unwrap().avail(USDT), 50000_00000000);
    }
}
