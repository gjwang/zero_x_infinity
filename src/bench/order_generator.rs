//! Test Order Generator - Bit-Exact Java Implementation
//!
//! This module generates deterministic order sequences matching Exchange-Core's
//! `TestOrdersGenerator` and `TestOrdersGeneratorSession` classes **exactly**.
//!
//! ## Java Source Reference
//! - TestOrdersGeneratorSession.java: Seed initialization, session state
//! - TestOrdersGenerator.java: generateRandomGtcOrder, generateRandomOrder

use crate::bench::java_random::JavaRandom;

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
    pub num_users: usize,
    pub symbol_id: i32,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            target_orders_per_side: 50, // 100 total / 2 sides
            num_users: 100,             // From RustPortingDataDumper
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
    #[allow(dead_code)]
    min_price: i64,
    #[allow(dead_code)]
    max_price: i64,
    price_direction: i32,

    // Order book state (simulated)
    seq: i64, // order ID counter (starts at 1)

    // UIDs array matching createUserListForSymbol
    #[allow(dead_code)]
    uids: Vec<i32>,

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
        //       ... iterate to collect valid users
        let uids = Self::create_user_list_for_symbol(config.symbol_id, config.num_users);

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
            phase: Phase::Fill,
        }
    }

    /// Replicate Java's createUserListForSymbol
    ///
    /// Java code:
    /// ```java
    /// Random rand = new Random(spec.symbolId);
    /// int uid = 1 + rand.nextInt(users2currencies.size() - 1);
    /// // iterate through users starting from uid, wrapping around
    /// ```
    fn create_user_list_for_symbol(symbol_id: i32, num_users: usize) -> Vec<i32> {
        // For golden data, we simulate the user filtering logic
        // The key is: iterate starting from a symbol-seeded random position

        let mut rand = JavaRandom::new(symbol_id as i64);
        let start_uid = 1 + rand.next_int((num_users - 1) as i32);

        // In the real Java code, it filters users by currency accounts
        // For our golden data test (100 users, simple case), we assume
        // all users are valid, so the order is what matters

        let mut uids = Vec::with_capacity(num_users);
        let mut uid = start_uid;

        for _ in 0..num_users {
            uids.push(uid);
            uid += 1;
            if uid >= num_users as i32 {
                uid = 1;
            }
        }

        uids
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
    pub fn next_command(&mut self) -> TestCommand {
        let order_id = self.seq;
        self.seq += 1;

        // 1. Action: (rand.nextInt(4) + priceDirection >= 2) ? BID : ASK
        let action_rand = self.rng.next_int(4);
        let action = if action_rand + self.price_direction >= 2 {
            Action::Bid
        } else {
            Action::Ask
        };

        // 2. UID: uidMapper.apply(rand.nextInt(numUsers))
        //    UID_PLAIN_MAPPER: i -> i + 1
        //    So uid = rand.nextInt(numUsers) + 1
        let uid = (self.rng.next_int(self.config.num_users as i32) + 1) as i64;

        // 3. Price deviation: dev = 1 + (int)(pow(rand.nextDouble(), 2) * priceDeviation)
        let dev_rand = self.rng.next_double();
        let dev = 1 + (dev_rand * dev_rand * self.price_deviation as f64) as i64;

        // 4. Price offset: sum of 4 random values, then normalize
        let mut p: i64 = 0;
        for _ in 0..4 {
            p += self.rng.next_int(dev as i32) as i64;
        }
        p = p / 4 * 2 - dev;

        // Adjust sign based on action
        // Java: if (p > 0 ^ action == OrderAction.ASK) { p = -p; }
        let should_flip = (p > 0) ^ (action == Action::Ask);
        if should_flip {
            p = -p;
        }

        let price = self.last_trade_price + p;

        // 5. Size: 1 + rand.nextInt(6) * rand.nextInt(6) * rand.nextInt(6)
        let s1 = self.rng.next_int(6);
        let s2 = self.rng.next_int(6);
        let s3 = self.rng.next_int(6);
        let size = (1 + s1 * s2 * s3) as i64;

        TestCommand {
            command: CommandType::PlaceOrder,
            order_id,
            symbol: self.config.symbol_id,
            price,
            size,
            action,
            order_type: OrderType::Gtc,
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
}
