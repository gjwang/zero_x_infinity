# Architect to Developer Handover: Phase 0x11

| **Phase** | 0x11 Deposit & Withdraw |
| :--- | :--- |
| **Priority** | P1 (Critical features) |
| **Status** | Ready for Implementation |

## 1. Core Documentation
*   **Design Spec**: [`docs/src/0x11-deposit-withdraw.md`](../../../src/0x11-deposit-withdraw.md)
*   **Definition of Done**: [`docs/src/0x11-acceptance-checklist.md`](../../../src/0x11-acceptance-checklist.md) (STRICT)

## 2. Technical Specifications (Reference Implementation)

### 2.1 Database Schema (Proposed)
The Architect has defined the following Schema for `migrations/0x11_deposit_withdraw.sql`. Developer MUST implement this exact schema.

```sql
-- 1. Deposit History (Idempotent: tx_hash PK)
CREATE TABLE IF NOT EXISTS deposit_history (
    tx_hash VARCHAR(128) PRIMARY KEY,
    user_id BIGINT NOT NULL,
    asset VARCHAR(32) NOT NULL,
    amount DECIMAL(30, 8) NOT NULL,
    status VARCHAR(32) NOT NULL, -- CONFIRMING, SUCCESS
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- 2. Withdraw History
CREATE TABLE IF NOT EXISTS withdraw_history (
    request_id VARCHAR(64) PRIMARY KEY,
    user_id BIGINT NOT NULL,
    asset VARCHAR(32) NOT NULL,
    amount DECIMAL(30, 8) NOT NULL,
    fee DECIMAL(30, 8) NOT NULL,
    to_address VARCHAR(255) NOT NULL,
    tx_hash VARCHAR(128),
    status VARCHAR(32) NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- 3. User Addresses (Warm Wallet)
CREATE TABLE IF NOT EXISTS user_addresses (
    user_id BIGINT NOT NULL,
    asset VARCHAR(32) NOT NULL,
    network VARCHAR(32) NOT NULL,
    address VARCHAR(255) NOT NULL,
    PRIMARY KEY (user_id, asset, network)
);
```

### 2.2 Chain Adapter Interface (Trait Definition)
Required Trait signature for `src/funding/chain_adapter.rs`:

```rust
#[async_trait]
pub trait ChainClient: Send + Sync + Debug {
    // Must return Result<String, ChainError>
    async fn generate_address(&self, user_id: u64) -> Result<String, ChainError>;
    
    // Validate address format (Base58 for BTC, Hex for ETH)
    fn validate_address(&self, address: &str) -> bool;
    
    // Broadcast withdrawal
    async fn broadcast_withdraw(&self, to: &str, amt: &str) -> Result<String, ChainError>;
}
```

### 2.3 Pipeline Integration (CRITICAL)
The Architect mandates using the high-performance Ring Buffer Pipeline for all Balance Operations.
Do **NOT** update balances via direct DB writes (except for initial migration logging).

*   **Deposit**: Must inject `OrderAction::Deposit(BalanceUpdate)` into the `order_queue`.
    *   This requires adding `Deposit` variant to `src/pipeline.rs`.
    *   This requires handling `Deposit` in `src/ubscore.rs` `apply_order_action`.
*   **Withdraw**: Must inject `OrderAction::WithdrawLock(...)` (to be defined) into the pipeline to safely lock funds.
*   **Rationale**: Ensures strict serialization of balance events and prevents race conditions between Trading and Funding.

## 3. Strict Constraints
1.  **NO Real Chain Integration**: Use `MockBtcChain` and `MockEvmChain` structs.
2.  **Idempotency**: The `deposit_history.tx_hash` PRIMARY KEY is the primary defense against Double Spending. Do NOT remove it.
