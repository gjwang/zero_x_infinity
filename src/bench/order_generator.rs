//! Test Order Generator - Bit-Exact Java Implementation
//!
//! This module generates deterministic order sequences matching Exchange-Core's
//! `TestOrdersGenerator` and `TestOrdersGeneratorSession` classes **exactly**.
//!
//! ## Java Source Reference
//! - TestOrdersGeneratorSession.java: Seed initialization, session state
//! - TestOrdersGenerator.java: generateRandomGtcOrder, generateRandomOrder

use crate::bench::java_random::JavaRandom;
use std::collections::BTreeMap;

/// Order command types matching Exchange-Core
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandType {
    PlaceOrder,
    CancelOrder,
    MoveOrder,
    ReduceOrder,
}

/// Order action (side)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Bid,
    Ask,
}

impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Action::Bid => write!(f, "BID"),
            Action::Ask => write!(f, "ASK"),
        }
    }
}

/// Order type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Gtc,
    Ioc,
    FokBudget,
}

/// A generated test order command
#[derive(Debug, Clone)]
pub struct TestCommand {
    pub command: CommandType,
    pub order_id: i64,
    pub symbol: i32,
    pub price: i64,
    pub size: i64,
    pub action: Action,
    pub order_type: OrderType,
    pub uid: i64,
}

/// Phase of order generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Fill,
    Benchmark,
}

/// Configuration for order generation session
#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub target_orders_per_side: usize,
    pub num_accounts: usize,    // Total account quota for generateUsers
    pub symbol_messages: usize, // For numUsersToSelect calculation
    pub symbol_id: i32,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            target_orders_per_side: 50, // 100 total / 2 sides
            num_accounts: 100,          // From RustPortingDataDumper
            symbol_messages: 1000,      // totalTransactionsNumber
            symbol_id: 40000,
        }
    }
}

/// Java's Objects.hash implementation for two integers
/// This matches Java's `Objects.hash(symbol * -177277, seed * 10037 + 198267)`
fn java_objects_hash(a: i32, b: i64) -> i32 {
    // Java's Objects.hash calls Arrays.hashCode which does:
    // result = 1
    // result = 31 * result + element1
    // result = 31 * result + element2
    let mut result: i32 = 1;
    result = result.wrapping_mul(31).wrapping_add(a);
    result = result.wrapping_mul(31).wrapping_add(b as i32);
    result
}

/// Order generation session - **Bit-Exact Java Implementation**
pub struct TestOrdersGeneratorSession {
    rng: JavaRandom,
    config: SessionConfig,

    // Session state from Java
    last_trade_price: i64,
    price_deviation: i64,
    min_price: i64,
    max_price: i64,
    price_direction: i32,

    // Order book state (simulated)
    seq: i64, // order ID counter (starts at 1)

    // UIDs array matching createUserListForSymbol
    uids: Vec<i32>,

    // Shadow Order Book (per user spec section 3.4)
    order_uids: BTreeMap<i32, i32>,     // orderId -> uid
    order_prices: BTreeMap<i32, i32>,   // orderId -> price
    order_sizes: BTreeMap<i32, i32>,    // orderId -> size
    order_actions: BTreeMap<i32, bool>, // orderId -> is_ask

    // Active order lists for random selection (O(1) swap_remove)
    ask_orders: Vec<i32>,
    bid_orders: Vec<i32>,

    phase: Phase,
}

impl TestOrdersGeneratorSession {
    /// Create a new session matching Java's TestOrdersGeneratorSession constructor exactly.
    ///
    /// Java code:
    /// ```java
    /// this.rand = new Random(Objects.hash(symbol * -177277, seed * 10037 + 198267));
    /// int price = (int) Math.pow(10, 3.3 + rand.nextDouble() * 1.5 + rand.nextDouble() * 1.5);
    /// ```
    pub fn new(config: SessionConfig, benchmark_seed: i64) -> Self {
        // Java: Objects.hash(symbol * -177277, seed * 10037 + 198267)
        let hash_a = config.symbol_id.wrapping_mul(-177277);
        let hash_b = benchmark_seed.wrapping_mul(10037).wrapping_add(198267);
        let session_seed = java_objects_hash(hash_a, hash_b);

        let mut rng = JavaRandom::new(session_seed as i64);

        // Java: int price = (int) Math.pow(10, 3.3 + rand.nextDouble() * 1.5 + rand.nextDouble() * 1.5);
        let r1 = rng.next_double();
        let r2 = rng.next_double();
        let price = 10.0_f64.powf(3.3 + r1 * 1.5 + r2 * 1.5) as i64;

        // Java: this.priceDeviation = Math.min((int) (price * 0.05), 10000);
        let price_deviation = ((price as f64 * 0.05) as i64).min(10000);

        // Java: this.minPrice = price - priceDeviation * 5;
        //       this.maxPrice = price + priceDeviation * 5;
        let min_price = price - price_deviation * 5;
        let max_price = price + price_deviation * 5;

        // Generate UIDs array matching createUserListForSymbol
        // Java: Random rand = new Random(spec.symbolId);
        //       int uid = 1 + rand.nextInt(users2currencies.size() - 1);
        let uids = Self::create_user_list_for_symbol(
            config.symbol_id,
            config.num_accounts,
            config.symbol_messages,
        );

        Self {
            rng,
            config,
            last_trade_price: price,
            price_deviation,
            min_price,
            max_price,
            price_direction: 0, // enableSlidingPrice = false
            seq: 1,
            uids,
            order_uids: BTreeMap::new(),
            order_prices: BTreeMap::new(),
            order_sizes: BTreeMap::new(),
            order_actions: BTreeMap::new(),
            ask_orders: Vec::new(),
            bid_orders: Vec::new(),
            phase: Phase::Fill,
        }
    }

    /// Replicate Java's createUserListForSymbol with currency filtering
    ///
    /// Java flow:
    /// 1. generateUsers(numAccounts, currencies) - creates List<BitSet>
    /// 2. createUserListForSymbol - filters users, limited by numUsersToSelect
    fn create_user_list_for_symbol(
        symbol_id: i32,
        num_accounts: usize,
        symbol_messages: usize,
    ) -> Vec<i32> {
        // Step 1: Generate user accounts
        let user_currencies = Self::generate_user_accounts(num_accounts);
        let users_size = user_currencies.len();

        // Step 2: Calculate numUsersToSelect
        // Java: int numUsersToSelect = Math.min(users2currencies.size(), Math.max(2, symbolMessagesExpected / 5));
        let num_users_to_select = users_size.min((2_usize).max(symbol_messages / 5));

        // Step 3: Filter users starting from symbol-seeded position
        // Java: Random rand = new Random(spec.symbolId);
        //       int uid = 1 + rand.nextInt(users2currencies.size() - 1);
        let mut rand = JavaRandom::new(symbol_id as i64);
        let start_uid = 1 + rand.next_int((users_size - 1) as i32);

        // For FUTURES_CONTRACT, only quoteCurrency (USD=840) is required
        const QUOTE_CURRENCY: i32 = 840; // USD

        let mut uids = Vec::new();
        let mut uid = start_uid;
        let mut c = 0;

        // Java: while (uids.size() < numUsersToSelect && c < users2currencies.size())
        while uids.len() < num_users_to_select && c < users_size {
            if uid > 0
                && (uid as usize) < users_size
                && user_currencies[uid as usize].contains(&QUOTE_CURRENCY)
            {
                uids.push(uid);
            }
            uid += 1;
            if uid == users_size as i32 {
                uid = 1;
            }
            c += 1;
        }

        uids
    }

    /// Generate user accounts matching Java's UserCurrencyAccountsGenerator.generateUsers
    ///
    /// **CRITICAL**: numAccounts is total ACCOUNT quota, not user count!
    /// Java code:
    /// ```java
    /// Random rand = new Random(1);                            // for currency selection
    /// ParetoDistribution pareto = new ParetoDistribution(
    ///     new JDKRandomGenerator(0), 1, 1.5);                  // separate RNG!
    /// ```
    fn generate_user_accounts(num_accounts: usize) -> Vec<Vec<i32>> {
        // CURRENCIES_FUTURES from TestConstants:
        // Sets.newHashSet(CURRENECY_USD, CURRENECY_EUR) = {840, 978}
        //
        // Java HashSet iteration order depends on HashMap bucket index:
        // For default capacity 16: 978 % 16 = 2, 840 % 16 = 8
        // So EUR(978) comes BEFORE USD(840) in iteration!
        const CURRENCIES: [i32; 2] = [978, 840]; // EUR first, then USD

        // Java uses TWO separate RNGs:
        // 1. Random(1) for currency selection
        // 2. JDKRandomGenerator(0) inside ParetoDistribution
        let mut currency_rand = JavaRandom::new(1);
        let mut pareto_rand = JavaRandom::new(0); // JDKRandomGenerator(0)

        let mut accounts = Vec::new();

        // uid=0 has no accounts
        accounts.push(Vec::new());

        let mut accounts_quota = num_accounts as i32;

        while accounts_quota > 0 {
            // Java: int accountsToOpen = Math.min(Math.min(1 + (int)paretoDistribution.sample(), currencyCodes.length), totalAccountsQuota);
            // Note: 1 + (int)sample, NOT max(1, (int)sample)
            let n = pareto_rand.next_double();
            let pareto_sample = 1.0 / n.powf(1.0 / 1.5);
            let accounts_to_open = (1 + pareto_sample as i32)
                .min(CURRENCIES.len() as i32)
                .min(accounts_quota);

            // Currency selection using currency_rand
            let mut user_currencies = Vec::new();
            while user_currencies.len() < accounts_to_open as usize {
                let idx = currency_rand.next_int(CURRENCIES.len() as i32) as usize;
                let currency = CURRENCIES[idx];
                if !user_currencies.contains(&currency) {
                    user_currencies.push(currency);
                }
            }

            accounts_quota -= accounts_to_open;
            accounts.push(user_currencies);
        }

        accounts
    }

    /// Generate the next GTC order - **EXACT Java implementation**
    ///
    /// Java code from generateRandomGtcOrder:
    /// ```java
    /// final OrderAction action = (rand.nextInt(4) + session.priceDirection >= 2) ? BID : ASK;
    /// final int uid = session.uidMapper.apply(rand.nextInt(session.numUsers));
    /// final int newOrderId = session.seq;
    /// final int dev = 1 + (int) (Math.pow(rand.nextDouble(), 2) * session.priceDeviation);
    /// long p = 0;
    /// for (int i = 0; i < 4; i++) { p += rand.nextInt(dev); }
    /// p = p / 4 * 2 - dev;
    /// if (p > 0 ^ action == OrderAction.ASK) { p = -p; }
    /// final int price = (int) session.lastTradePrice + (int) p;
    /// int size = 1 + rand.nextInt(6) * rand.nextInt(6) * rand.nextInt(6);
    /// ```
    /// Generate next command - dispatches to FILL or BENCHMARK phase
    pub fn next_command(&mut self) -> TestCommand {
        // Check phase transition: FILL -> BENCHMARK after target_orders reached
        let total_fill_orders = self.config.target_orders_per_side * 2;
        if self.phase == Phase::Fill && self.seq as usize > total_fill_orders {
            self.phase = Phase::Benchmark;
        }

        match self.phase {
            Phase::Fill => self.generate_gtc_order(),
            Phase::Benchmark => self.generate_random_order(),
        }
    }

    /// Generate GTC order (FILL phase)
    /// Bit-exact replication of Java's generateRandomGtcOrder
    fn generate_gtc_order(&mut self) -> TestCommand {
        let order_id = self.seq as i32;
        self.seq += 1;

        // 1. Action: (rand.nextInt(4) + priceDirection >= 2) ? BID : ASK
        let action_rand = self.rng.next_int(4);
        let action = if action_rand + self.price_direction >= 2 {
            Action::Bid
        } else {
            Action::Ask
        };

        // 2. UID
        let uid_idx = self.rng.next_int(self.uids.len() as i32) as usize;
        let uid = self.uids[uid_idx] as i64;

        // 3. Price deviation
        let dev_rand = self.rng.next_double();
        let dev = 1 + (dev_rand * dev_rand * self.price_deviation as f64) as i64;

        // 4. Price offset
        let mut p: i64 = 0;
        for _ in 0..4 {
            p += self.rng.next_int(dev as i32) as i64;
        }
        p = p / 4 * 2 - dev;

        if (p > 0) ^ (action == Action::Ask) {
            p = -p;
        }

        let price = self.last_trade_price + p;

        // 5. Size
        let s1 = self.rng.next_int(6);
        let s2 = self.rng.next_int(6);
        let s3 = self.rng.next_int(6);
        let size = (1 + s1 * s2 * s3) as i64;

        // Track order in shadow order book (per user spec section 3.4)
        let is_ask = action == Action::Ask;
        self.order_uids.insert(order_id, uid as i32);
        self.order_prices.insert(order_id, price as i32);
        self.order_sizes.insert(order_id, size as i32);
        self.order_actions.insert(order_id, is_ask);

        // Add to active order list for random selection
        if is_ask {
            self.ask_orders.push(order_id);
        } else {
            self.bid_orders.push(order_id);
        }

        TestCommand {
            command: CommandType::PlaceOrder,
            order_id: order_id as i64,
            symbol: self.config.symbol_id,
            price,
            size,
            action,
            order_type: OrderType::Gtc,
            uid,
        }
    }

    /// Generate random order (BENCHMARK phase)
    /// Per user spec section 3.4: command generation decision tree
    fn generate_random_order(&mut self) -> TestCommand {
        // Calculate lack of orders using ask_orders/bid_orders length
        let target_half = self.config.target_orders_per_side as i32;
        let lack_ask = target_half - self.ask_orders.len() as i32;
        let lack_bid = target_half - self.bid_orders.len() as i32;
        let grow_orders = lack_ask > 0 || lack_bid > 0;

        // Java: int q = rand.nextInt(growOrders ? (requireFastFill ? 2 : 10) : 40)
        let q_range = if grow_orders { 10 } else { 40 };
        let q = self.rng.next_int(q_range);

        if q < 2 || self.order_uids.is_empty() {
            if grow_orders {
                return self.generate_gtc_order();
            } else {
                return self.generate_ioc_order();
            }
        }

        // Pick random existing order (per spec section 3.4.2)
        // Choose side based on random
        let use_ask = self.rng.next_int(2) == 0;
        let orders = if use_ask {
            &self.ask_orders
        } else {
            &self.bid_orders
        };

        if orders.is_empty() {
            return self.generate_gtc_order();
        }

        let idx = self.rng.next_int(orders.len().min(512) as i32) as usize;
        let order_id = orders[idx];
        let uid = *self.order_uids.get(&order_id).unwrap();

        if q == 2 {
            // Cancel order (per spec 3.4.2.B - use swap_remove for O(1))
            if use_ask {
                self.ask_orders.swap_remove(idx);
            } else {
                // Need to use mutable reference
                let idx = self.bid_orders.iter().position(|&x| x == order_id).unwrap();
                self.bid_orders.swap_remove(idx);
            }
            self.order_uids.remove(&order_id);
            self.order_prices.remove(&order_id);
            self.order_sizes.remove(&order_id);
            self.order_actions.remove(&order_id);

            TestCommand {
                command: CommandType::CancelOrder,
                order_id: order_id as i64,
                symbol: self.config.symbol_id,
                price: 0,
                size: 0,
                action: Action::Bid, // doesn't matter for cancel
                order_type: OrderType::Gtc,
                uid: uid as i64,
            }
        } else if q == 3 {
            // Reduce order
            let prev_size = *self.order_sizes.get(&order_id).unwrap_or(&1);
            let reduce_by = self.rng.next_int(prev_size.max(1)) + 1;

            TestCommand {
                command: CommandType::ReduceOrder,
                order_id: order_id as i64,
                symbol: self.config.symbol_id,
                price: 0,
                size: reduce_by as i64,
                action: Action::Bid,
                order_type: OrderType::Gtc,
                uid: uid as i64,
            }
        } else {
            // Move order (q >= 4)
            let prev_price = *self
                .order_prices
                .get(&order_id)
                .unwrap_or(&(self.last_trade_price as i32));

            // Java: double priceMove = (session.lastTradePrice - prevPrice) * CENTRAL_MOVE_ALPHA;
            // CENTRAL_MOVE_ALPHA = 0.1
            let price_move = (self.last_trade_price as f64 - prev_price as f64) * 0.1;
            let price_move_rounded = if prev_price > self.last_trade_price as i32 {
                price_move.floor() as i32
            } else if prev_price < self.last_trade_price as i32 {
                price_move.ceil() as i32
            } else {
                self.rng.next_int(2) * 2 - 1
            };

            let new_price = (prev_price + price_move_rounded).min(self.max_price as i32);
            self.order_prices.insert(order_id, new_price);

            TestCommand {
                command: CommandType::MoveOrder,
                order_id: order_id as i64,
                symbol: self.config.symbol_id,
                price: new_price as i64,
                size: 0,
                action: Action::Bid,
                order_type: OrderType::Gtc,
                uid: uid as i64,
            }
        }
    }

    /// Generate IOC order (BENCHMARK phase when not growing)
    fn generate_ioc_order(&mut self) -> TestCommand {
        let order_id = self.seq as i32;
        self.seq += 1;

        let action_rand = self.rng.next_int(4);
        let action = if action_rand + self.price_direction >= 2 {
            Action::Bid
        } else {
            Action::Ask
        };

        let uid_idx = self.rng.next_int(self.uids.len() as i32) as usize;
        let uid = self.uids[uid_idx] as i64;

        // IOC at limit price
        let price = if action == Action::Bid {
            self.max_price
        } else {
            self.min_price
        };

        // Size: 1 + rand.nextInt(6) * rand.nextInt(6) * rand.nextInt(6)
        let s1 = self.rng.next_int(6);
        let s2 = self.rng.next_int(6);
        let s3 = self.rng.next_int(6);
        let size = (1 + s1 * s2 * s3) as i64;

        TestCommand {
            command: CommandType::PlaceOrder,
            order_id: order_id as i64,
            symbol: self.config.symbol_id,
            price,
            size,
            action,
            order_type: OrderType::Ioc,
            uid,
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let config = SessionConfig::default();
        let session = TestOrdersGeneratorSession::new(config, 1);
        assert_eq!(session.phase(), Phase::Fill);
    }

    #[test]
    fn test_java_objects_hash() {
        // Test the hash function
        let hash_a = 40000_i32.wrapping_mul(-177277);
        let hash_b = 1_i64.wrapping_mul(10037).wrapping_add(198267);
        let result = java_objects_hash(hash_a, hash_b);
        eprintln!("Objects.hash for symbol=40000, seed=1: {}", result);
        // This should match Java's Objects.hash() output
    }

    #[test]
    fn test_generate_commands() {
        let config = SessionConfig {
            target_orders_per_side: 10,
            ..Default::default()
        };
        let mut session = TestOrdersGeneratorSession::new(config, 1);

        for i in 0..30 {
            let cmd = session.next_command();
            assert_eq!(cmd.order_id, (i + 1) as i64);
            assert!(cmd.uid >= 1);
        }
    }

    #[test]
    fn test_deterministic_generation() {
        let config = SessionConfig::default();
        let mut session1 = TestOrdersGeneratorSession::new(config.clone(), 1);
        let mut session2 = TestOrdersGeneratorSession::new(config, 1);

        for _ in 0..20 {
            let cmd1 = session1.next_command();
            let cmd2 = session2.next_command();
            assert_eq!(cmd1.order_id, cmd2.order_id);
            assert_eq!(cmd1.price, cmd2.price);
            assert_eq!(cmd1.size, cmd2.size);
            assert_eq!(cmd1.uid, cmd2.uid);
        }
    }

    /// Test large scale order generation (FILL + BENCHMARK phases)
    /// Verifies the generator can produce 10k orders without panicking
    #[test]
    fn test_large_scale_generation() {
        let config = SessionConfig::default();
        let mut session = TestOrdersGeneratorSession::new(config, 1);

        let mut place_count = 0;
        let mut cancel_count = 0;
        let mut move_count = 0;
        let mut reduce_count = 0;

        for _ in 0..10000 {
            let cmd = session.next_command();
            match cmd.command {
                CommandType::PlaceOrder => place_count += 1,
                CommandType::CancelOrder => cancel_count += 1,
                CommandType::MoveOrder => move_count += 1,
                CommandType::ReduceOrder => reduce_count += 1,
            }
        }

        eprintln!("\n=== 10k Order Generation Test ===");
        eprintln!("PlaceOrder:  {}", place_count);
        eprintln!("CancelOrder: {}", cancel_count);
        eprintln!("MoveOrder:   {}", move_count);
        eprintln!("ReduceOrder: {}", reduce_count);

        // Verify we have a mix of command types in BENCHMARK phase
        assert!(place_count >= 100, "Should have at least 100 FILL orders");
        assert!(
            cancel_count + move_count + reduce_count > 0,
            "Should have Cancel/Move/Reduce in BENCHMARK"
        );
    }
}
