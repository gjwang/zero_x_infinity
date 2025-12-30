//! Benchmark Harness Module
//!
//! Phase 0x14-a: Re-implement Exchange-Core test data generation algorithm in Rust.
//!
//! # Components
//!
//! - [`java_random`] - Java-compatible LCG PRNG
//! - [`order_generator`] - Test order sequence generator
//! - [`golden_verification`] - Golden CSV verification tests

pub mod golden_verification;
pub mod java_random;
pub mod order_generator;
