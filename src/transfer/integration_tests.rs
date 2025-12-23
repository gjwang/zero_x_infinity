//! Integration Tests for Transfer FSM
//!
//! These tests verify the complete FSM flow without needing a live database.
//! They use the MockAdapter for both Funding and Trading to simulate scenarios.

#[cfg(test)]
mod integration_tests {
    use std::sync::Arc;

    use crate::transfer::adapters::MockAdapter;
    use crate::transfer::coordinator::TransferCoordinator;
    use crate::transfer::db::TransferDb;
    use crate::transfer::state::TransferState;
    use crate::transfer::types::{ServiceId, TransferRequest};

    /// Helper to create a coordinator with mock adapters for testing
    struct TestHarness {
        coordinator: TransferCoordinator,
        funding: Arc<MockAdapter>,
        trading: Arc<MockAdapter>,
    }

    impl TestHarness {
        fn new(pool: sqlx::PgPool) -> Self {
            // Use unique machine_id based on thread ID to avoid Snowflake collisions
            let thread_id = std::thread::current().id();
            let machine_id = format!("{:?}", thread_id)
                .chars()
                .filter(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse::<u8>()
                .unwrap_or(1)
                % 255
                + 1; // Ensure 1-255 range

            let db = Arc::new(TransferDb::new(pool));
            let funding = Arc::new(MockAdapter::new("funding"));
            let trading = Arc::new(MockAdapter::new("trading"));

            let coordinator = TransferCoordinator::with_machine_id(
                db,
                funding.clone(),
                trading.clone(),
                machine_id,
            );

            Self {
                coordinator,
                funding,
                trading,
            }
        }
    }

    // ========================================================================
    // Happy Path Tests
    // ========================================================================

    /// Test: Funding → Spot transfer completes successfully
    ///
    /// Flow: INIT → SOURCE_PENDING → SOURCE_DONE → TARGET_PENDING → COMMITTED
    #[tokio::test]
    #[ignore = "requires PostgreSQL database"]
    async fn test_funding_to_spot_happy_path() {
        let pool = create_test_pool().await;
        let harness = TestHarness::new(pool);

        // Create transfer request
        let req = TransferRequest::new(
            ServiceId::Funding,
            ServiceId::Trading,
            1001,        // user_id
            1,           // asset_id (BTC)
            100_000_000, // 1.0 BTC
        );

        // Create transfer
        let req_id = harness.coordinator.create(req).await.unwrap();
        assert!(req_id > 0);

        // Execute to completion
        let final_state = harness.coordinator.execute(req_id).await.unwrap();
        assert_eq!(final_state, TransferState::Committed);

        // Verify adapter calls
        assert_eq!(harness.funding.withdraw_count(), 1);
        assert_eq!(harness.trading.deposit_count(), 1);
    }

    /// Test: Spot → Funding transfer completes successfully
    #[tokio::test]
    #[ignore = "requires PostgreSQL database"]
    async fn test_spot_to_funding_happy_path() {
        let pool = create_test_pool().await;
        let harness = TestHarness::new(pool);

        let req = TransferRequest::new(
            ServiceId::Trading,
            ServiceId::Funding,
            1001,
            1,
            50_000_000, // 0.5 BTC
        );

        let req_id = harness.coordinator.create(req).await.unwrap();
        let final_state = harness.coordinator.execute(req_id).await.unwrap();

        assert_eq!(final_state, TransferState::Committed);
        assert_eq!(harness.trading.withdraw_count(), 1);
        assert_eq!(harness.funding.deposit_count(), 1);
    }

    // ========================================================================
    // Failure & Compensation Tests
    // ========================================================================

    /// Test: Source withdraw fails → FAILED state (no compensation needed)
    #[tokio::test]
    #[ignore = "requires PostgreSQL database"]
    async fn test_source_withdraw_fails() {
        let pool = create_test_pool().await;
        let harness = TestHarness::new(pool);

        // Configure funding adapter to fail withdraw
        harness.funding.set_fail_withdraw(true);

        let req =
            TransferRequest::new(ServiceId::Funding, ServiceId::Trading, 1001, 1, 100_000_000);

        let req_id = harness.coordinator.create(req).await.unwrap();
        let final_state = harness.coordinator.execute(req_id).await.unwrap();

        assert_eq!(final_state, TransferState::Failed);

        // No deposit should have been called
        assert_eq!(harness.trading.deposit_count(), 0);

        // No rollback needed since source failed
        assert_eq!(harness.funding.rollback_count(), 0);
    }

    /// Test: Target deposit fails (Funding source) → COMPENSATING → ROLLED_BACK
    #[tokio::test]
    #[ignore = "requires PostgreSQL database"]
    async fn test_target_deposit_fails_funding_source() {
        let pool = create_test_pool().await;
        let harness = TestHarness::new(pool);

        // Configure trading adapter to fail deposit
        harness.trading.set_fail_deposit(true);

        let req = TransferRequest::new(
            ServiceId::Funding, // Source is Funding
            ServiceId::Trading,
            1001,
            1,
            100_000_000,
        );

        let req_id = harness.coordinator.create(req).await.unwrap();
        let final_state = harness.coordinator.execute(req_id).await.unwrap();

        // Should rollback since source is Funding
        assert_eq!(final_state, TransferState::RolledBack);

        // Verify rollback was called
        assert_eq!(harness.funding.rollback_count(), 1);
    }

    /// Test: Target deposit fails (Trading source) → stays in TARGET_PENDING (infinite retry)
    ///
    /// CRITICAL: This tests the "Trading Cannot Rollback" invariant!
    #[tokio::test]
    #[ignore = "requires PostgreSQL database"]
    async fn test_target_deposit_fails_trading_source_no_rollback() {
        let pool = create_test_pool().await;
        let harness = TestHarness::new(pool);

        // Configure funding adapter to fail deposit
        harness.funding.set_fail_deposit(true);

        let req = TransferRequest::new(
            ServiceId::Trading, // Source is Trading - CANNOT rollback!
            ServiceId::Funding,
            1001,
            1,
            100_000_000,
        );

        let req_id = harness.coordinator.create(req).await.unwrap();

        // Step manually to observe behavior (execute would loop forever)
        let _ = harness.coordinator.step(req_id).await.unwrap(); // INIT → SOURCE_PENDING
        let _ = harness.coordinator.step(req_id).await.unwrap(); // SOURCE_PENDING → SOURCE_DONE
        let state = harness.coordinator.step(req_id).await.unwrap(); // Try target deposit → stays TARGET_PENDING

        // Should stay in TARGET_PENDING (not compensating!)
        assert_eq!(state, TransferState::TargetPending);

        // CRITICAL: No rollback should EVER be called when source is Trading
        assert_eq!(harness.trading.rollback_count(), 0);
    }

    // ========================================================================
    // Idempotency Tests
    // ========================================================================

    /// Test: Duplicate cid returns same req_id (idempotent create)
    #[tokio::test]
    #[ignore = "requires PostgreSQL database"]
    async fn test_duplicate_cid_idempotent() {
        let pool = create_test_pool().await;
        let harness = TestHarness::new(pool);

        let req1 = TransferRequest::with_cid(
            ServiceId::Funding,
            ServiceId::Trading,
            1001,
            1,
            100_000_000,
            "client-idempotency-key-123".to_string(),
        );

        let req_id1 = harness.coordinator.create(req1).await.unwrap();

        // Second request with same cid should return same req_id
        let req2 = TransferRequest::with_cid(
            ServiceId::Funding,
            ServiceId::Trading,
            1001,
            1,
            100_000_000,
            "client-idempotency-key-123".to_string(),
        );

        let req_id2 = harness.coordinator.create(req2).await.unwrap();

        assert_eq!(req_id1, req_id2);
    }

    // ========================================================================
    // Validation Tests
    // ========================================================================

    /// Test: Zero amount rejected
    #[tokio::test]
    #[ignore = "requires PostgreSQL database"]
    async fn test_zero_amount_rejected() {
        let pool = create_test_pool().await;
        let harness = TestHarness::new(pool);

        let req = TransferRequest::new(
            ServiceId::Funding,
            ServiceId::Trading,
            1001,
            1,
            0, // Zero amount!
        );

        let result = harness.coordinator.create(req).await;
        assert!(result.is_err());
    }

    /// Test: Same account rejected
    #[tokio::test]
    #[ignore = "requires PostgreSQL database"]
    async fn test_same_account_rejected() {
        let pool = create_test_pool().await;
        let harness = TestHarness::new(pool);

        let req = TransferRequest::new(
            ServiceId::Funding,
            ServiceId::Funding, // Same!
            1001,
            1,
            100_000_000,
        );

        let result = harness.coordinator.create(req).await;
        assert!(result.is_err());
    }

    // ========================================================================
    // Helper Functions
    // ========================================================================

    async fn create_test_pool() -> sqlx::PgPool {
        let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
            "postgres://postgres:postgres@localhost:5432/zero_x_infinity_test".to_string()
        });

        sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
            .expect("Failed to connect to test database")
    }
}
