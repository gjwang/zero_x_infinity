# Pipeline Service Migration

> Refactoring Date: 2025-12-18
> Branch: `0x08-h-performance-monitoring`

## Motivation

### The Problem: Tight Coupling

The original `pipeline_mt.rs` had 4 large `spawn_*_stage` functions (~467 lines) that bundled **threading with business logic**:

```rust
// Before: Can't test or reuse without spawning threads
fn spawn_me_stage(...) -> JoinHandle<OrderBook> {
    thread::spawn(move || {
        // 175 lines of matching logic
    })
}
```

This design had several issues:

| Issue | Impact |
|-------|--------|
| **No unit testing** | Can't test logic without threads |
| **No reusability** | Can't use in single-threaded mode |
| **Duplicated patterns** | Spin-loop, backpressure repeated |
| **Hard to extend** | Adding polling mode requires rewrite |

### The Solution: Service-Oriented Architecture

Extract each stage's business logic into a **service struct**:

```rust
// After: Logic separated, caller controls execution
pub struct MatchingService { book: OrderBook, ... }

impl MatchingService {
    pub fn run(&mut self, shutdown: &ShutdownSignal) { ... }
    pub fn into_inner(self) -> OrderBook { self.book }
}

// Caller decides how to run
thread::spawn(move || { service.run(&s); service.into_inner() })
```

## Benefits

1. **Testability** - Services can be tested without threads
2. **Reusability** - Same services for ST/MT/simulation
3. **Clean separation** - `pipeline_mt.rs` reduced from 720 to 250 lines
4. **Future-proof** - Easy to add `poll()` method for cooperative scheduling

## Implementation

### Incremental Migration Strategy

The first attempt to migrate all services at once **failed** (lost ~2000 trades). The successful approach:

1. Migrate **one service at a time**
2. Run full comparison test after each phase
3. Commit only if tests pass
4. Keep original functions until all phases complete

### Migration Phases

| Phase | Service | Status |
|-------|---------|--------|
| 1 | `IngestionService` | ✅ |
| 2 | `UBSCoreService` | ✅ |
| 3 | `MatchingService` | ✅ |
| 4 | `SettlementService` | ✅ |
| Cleanup | Remove old spawn functions | ✅ |

### Files Changed

| File | Change |
|------|--------|
| `src/pipeline_services.rs` | **NEW** - 4 service structs (~640 lines) |
| `src/pipeline_mt.rs` | **REDUCED** - Now ~250 lines (was 720) |
| `src/lib.rs` | Added module declaration |

## Validation

All tests pass with **exact correctness**:

```
Trades: 667,567 vs 667,567 ✅ PASS
Final balances: ✅ MATCH (0 differences)
```

## Service API

Each service follows the same pattern:

```rust
pub struct XxxService {
    // Owned component
    component: Component,
    // Shared resources
    queues: Arc<MultiThreadQueues>,
    stats: Arc<PipelineStats>,
}

impl XxxService {
    /// Create new service
    pub fn new(...) -> Self { ... }
    
    /// Run in blocking mode (for MT)
    pub fn run(&mut self, shutdown: &ShutdownSignal) { ... }
    
    /// Extract inner component after completion
    pub fn into_inner(self) -> Component { ... }
}
```

## Future Work

- [ ] Add `poll()` methods for single-threaded cooperative execution
- [ ] Move `MarketContext` to shared location
- [ ] Add service-level unit tests
