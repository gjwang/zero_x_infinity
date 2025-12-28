# ADR-003: Real Chain Sentinel & Re-org Recovery Protocol

## Status
Proposed

## Context
Phase 0x11 implemented a "Mock Chain" where the test script pushed deposit events to the Gateway. For Phase 0x11-a, we must integrate with real Bitcoin (Regtest) and Ethereum (Anvil) nodes. This requires a transition from a **Push Model** to a **Pull Model** (Sentinel service) and a robust strategy for handling blockchain re-orgs.

## Decision
We will implement a standalone **Sentinel Service** that polls blockchain RPC nodes.

### 1. The Sentinel Pull-Model
- **Independent Loops**: Each chain (BTC, ETH) will have its own scanning loop.
- **Cursor Tracking**: Scanning progress will be persisted in a `chain_cursor` table (height + hash).
- **Atomic Updates**: Cursor updates and deposit records MUST be committed in a single DB transaction.

### 2. Pipeline Integration
- **Deterministic Injection**: Sentinel detects a transaction, but DOES NOT update balances directly.
- **OrderAction::Deposit**: Deposits are injected into the Ring Buffer as a specialized `OrderAction`. This ensures balance updates are serial and deterministic within `UBSCore`, preventing race conditions between trading and funding.

### 3. Re-org Recovery Protocol
- **Parent Hash Validation**: Before scanning block `N`, Sentinel verifies that the parent hash of block `N` matches the stored hash of block `N-1`.
- **Shallow Re-org Handling**: Sentinel rolls back the `chain_cursor` to the last known canonical block and invalidates pending/confirming deposits in the orphaned fork.
- **Deep Re-org (> MAX_REORG_DEPTH blocks)**: Treat as a system-blocking event. For Phase 0x11-a, this requires manual intervention. The automated "Clawback" protocol is moved to future work.

## Future Work (Out of Scope for 0x11-a)
- **High-Performance Scanning**: Bloom Filters for million-user address matching. Initial implementation will use standard HashMap lookups.
- **Automated Clawback**: Deterministic injection of administrative balance reversals for deep re-orgs.

### 4. Precision & Truncation
- To ensure 100% reconciliation match between chain and ledger, we enforce a **Truncation Protocol**: Chain amounts are converted to integer units (Satoshi/Wei) and scaled to the exchange's internal precision (10^6 or 10^18) using fixed-point arithmetic. Floating point is strictly forbidden.

## Consequences
- **Positive**: High reliability, deterministic balance state, automatic recovery from minor fork events.
- **Negative**: Increased DB load (cursor updates), complexity in managing the Sentinel-to-Pipeline bridge.
