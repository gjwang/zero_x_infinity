//! Test Order Generator
//!
//! This module generates deterministic order sequences matching Exchange-Core's
//! `TestOrdersGenerator` and `TestOrdersGeneratorSession` classes.
//!
//! # Command Types
//!
//! - PLACE_ORDER (GTC/IOC/FOK)
//! - CANCEL_ORDER
//! - MOVE_ORDER
//! - REDUCE_ORDER

use crate::bench::java_random::{JavaRandom, derive_session_seed};

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

/// Order type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    /// Good Till Cancel
    Gtc,
    /// Immediate Or Cancel
    Ioc,
    /// Fill Or Kill (Budget version)
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
    /// Pre-fill phase: build up order book depth
    Fill,
    /// Benchmark phase: mixed command generation
    Benchmark,
}

/// Configuration for order generation session
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Target number of orders per side (ask/bid)
    pub target_orders_per_side: usize,
    /// Number of users
    pub num_users: usize,
    /// Central price
    pub central_price: i64,
    /// Price range (spread from central)
    pub price_range: i64,
    /// Symbol ID
    pub symbol_id: i32,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            target_orders_per_side: 500,
            num_users: 2000,
            central_price: 34400,
            price_range: 1000,
            symbol_id: 40000,
        }
    }
}

/// Order generation session for a single symbol
pub struct TestOrdersGeneratorSession {
    rng: JavaRandom,
    config: SessionConfig,
    order_id_counter: i64,
    ask_orders: i64,
    bid_orders: i64,
    phase: Phase,
}

impl TestOrdersGeneratorSession {
    /// Create a new session with the given configuration and benchmark seed.
    pub fn new(config: SessionConfig, benchmark_seed: i64) -> Self {
        let session_seed = derive_session_seed(config.symbol_id, benchmark_seed);
        Self {
            rng: JavaRandom::new(session_seed),
            config,
            order_id_counter: 0,
            ask_orders: 0,
            bid_orders: 0,
            phase: Phase::Fill,
        }
    }

    /// Check if we need more orders on a side
    fn needs_fill(&self) -> bool {
        let target = self.config.target_orders_per_side as i64;
        self.ask_orders < target || self.bid_orders < target
    }

    /// Generate the next order command
    pub fn next_command(&mut self) -> TestCommand {
        self.order_id_counter += 1;
        let order_id = self.order_id_counter;

        // Generate user ID using Pareto-like distribution
        let uid = self.generate_uid();

        // During fill phase, always generate GTC orders
        if self.needs_fill() {
            self.phase = Phase::Fill;
            return self.generate_gtc_order(order_id, uid);
        }

        self.phase = Phase::Benchmark;

        // Benchmark phase: decide command type
        let decision = self.rng.next_int(8);

        match decision {
            0 | 1 => {
                // Might need to grow, or generate IOC
                if self.needs_growth() {
                    self.generate_gtc_order(order_id, uid)
                } else {
                    self.generate_ioc_order(order_id, uid)
                }
            }
            2 => self.generate_cancel_command(order_id, uid),
            3 => self.generate_reduce_command(order_id, uid),
            _ => self.generate_move_command(order_id, uid),
        }
    }

    /// Generate a GTC limit order
    fn generate_gtc_order(&mut self, order_id: i64, uid: i64) -> TestCommand {
        let action = if self.bid_orders <= self.ask_orders {
            self.bid_orders += 1;
            Action::Bid
        } else {
            self.ask_orders += 1;
            Action::Ask
        };

        let price = self.generate_price(action);
        let size = self.generate_size();

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

    /// Generate an IOC order
    fn generate_ioc_order(&mut self, order_id: i64, uid: i64) -> TestCommand {
        let action = if self.rng.next_boolean() {
            Action::Bid
        } else {
            Action::Ask
        };

        let price = self.generate_price(action);
        let size = self.generate_size();

        // 31:1 ratio between IOC and FOK_BUDGET
        let order_type = if self.rng.next_int(32) == 0 {
            OrderType::FokBudget
        } else {
            OrderType::Ioc
        };

        TestCommand {
            command: CommandType::PlaceOrder,
            order_id,
            symbol: self.config.symbol_id,
            price,
            size,
            action,
            order_type,
            uid,
        }
    }

    /// Generate a cancel command
    fn generate_cancel_command(&mut self, order_id: i64, uid: i64) -> TestCommand {
        // In real implementation, this would reference an existing order
        TestCommand {
            command: CommandType::CancelOrder,
            order_id,
            symbol: self.config.symbol_id,
            price: 0,
            size: 0,
            action: Action::Bid,
            order_type: OrderType::Gtc,
            uid,
        }
    }

    /// Generate a move command
    fn generate_move_command(&mut self, order_id: i64, uid: i64) -> TestCommand {
        let price = self.generate_price(Action::Bid);
        TestCommand {
            command: CommandType::MoveOrder,
            order_id,
            symbol: self.config.symbol_id,
            price,
            size: 0,
            action: Action::Bid,
            order_type: OrderType::Gtc,
            uid,
        }
    }

    /// Generate a reduce command
    fn generate_reduce_command(&mut self, order_id: i64, uid: i64) -> TestCommand {
        let size = self.generate_size();
        TestCommand {
            command: CommandType::ReduceOrder,
            order_id,
            symbol: self.config.symbol_id,
            price: 0,
            size,
            action: Action::Bid,
            order_type: OrderType::Gtc,
            uid,
        }
    }

    /// Check if order book needs to grow
    fn needs_growth(&self) -> bool {
        let target = self.config.target_orders_per_side as i64;
        let current = (self.ask_orders + self.bid_orders) / 2;
        current < target
    }

    /// Generate price based on action and central price
    fn generate_price(&mut self, action: Action) -> i64 {
        let central = self.config.central_price;
        let range = self.config.price_range;

        // Generate offset from central price
        let offset = (self.rng.next_double() * range as f64) as i64;

        match action {
            Action::Bid => central - offset / 2,
            Action::Ask => central + offset / 2,
        }
    }

    /// Generate order size using cubic distribution (favors small sizes)
    fn generate_size(&mut self) -> i64 {
        // Formula: 1 + rand(6) * rand(6) * rand(6)
        // Range: 1-216, heavily skewed toward small values
        let a = self.rng.next_int(6) + 1;
        let b = self.rng.next_int(6) + 1;
        let c = self.rng.next_int(6) + 1;
        (a * b * c) as i64
    }

    /// Generate user ID using Pareto-like distribution
    fn generate_uid(&mut self) -> i64 {
        // Simple approximation: most activity from few users
        let r = self.rng.next_double();
        let pareto = 1.0 / r.powf(1.0 / 1.5); // shape = 1.5
        let uid = (pareto.min(self.config.num_users as f64)) as i64;
        uid.max(1)
    }

    /// Get current phase
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
    fn test_generate_commands() {
        let config = SessionConfig {
            target_orders_per_side: 10,
            ..Default::default()
        };
        let mut session = TestOrdersGeneratorSession::new(config, 1);

        // Generate 30 commands
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

        // Same seed should produce same sequence
        for _ in 0..20 {
            let cmd1 = session1.next_command();
            let cmd2 = session2.next_command();
            assert_eq!(cmd1.order_id, cmd2.order_id);
            assert_eq!(cmd1.price, cmd2.price);
            assert_eq!(cmd1.size, cmd2.size);
        }
    }
}
