# Handover: 0x14-b Matching Engine Parity (Spot)

**Role**: Architect -> Developer
**Date**: 2025-12-30
**Priority**: P0 (Blocker for Benchmark Verification)

---

## 1. Objective
Implement a feature-complete Spot Matching Engine in Rust that supports the `TestOrdersGenerator` requirements, specifically `IOC` (Immediate-or-Cancel) orders and `TimeInForce` logic.

**Design Spec**: [docs/src/0x14-b-order-commands.md](../../../../docs/src/0x14-b-order-commands.md)

## 2. Developer Tasks

### 2.1 Model Extensions
- [ ] **Modify `src/models.rs`**:
    - Add `pub enum TimeInForce { GTC, IOC, FOK }`.
    - Add `time_in_force` field to `InternalOrder`.
    - Update `InternalOrder::new` constructor.

### 2.2 Matching Engine Logic
- [ ] **Create `src/engine/mod.rs` & `src/engine/matching.rs`**:
    - Implement `Engine` struct wrapping `OrderBook`.
    - Implement `process_order(&mut self, order: InternalOrder) -> OrderResult`.
    - **Logic**:
        - If `Limit` + `GTC`: Try match, rest remainder.
        - If `Limit` + `IOC`: Try match, expire remainder (DO NOT REST).
        - If `Market`: Match at best available, expire remainder.

### 2.3 Command Support
- [ ] **Implement Complexity**:
    - `CancelOrder`: Remove from book.
    - `ReduceOrder`: `order.qty -= delta`. (Ideally preserve priority, or re-insert if easier for MVP).
    - `MoveOrder`: `Cancel(old_id)` -> `Place(new_id)`.

## 3. QA Verification Steps
1.  **Test GTC**:
    ```rust
    // Logic: Order rests in book
    let res = engine.process_order(buy_100_gtc);
    assert_eq!(engine.book.best_bid(), Some(100));
    ```
2.  **Test IOC**:
    ```rust
    // Logic: Order matches 60, expires 40. Book remains empty.
    let res = engine.process_order(buy_100_ioc); // against sell_60
    assert_eq!(res.trades.len(), 1);
    assert_eq!(res.trades[0].qty, 60);
    assert_eq!(engine.book.best_bid(), None); // Remainder expired
    ```

## 4. Constraint Checklist
- [ ] No `unwrap()` in critical path. Use `Result` or silent handling.
- [ ] `process_order` must be deterministic based on `seq_id`.
- [ ] Do not implement Futures/Margin logic yet (0x14-c).

---

> **Ready for Implementation.**
