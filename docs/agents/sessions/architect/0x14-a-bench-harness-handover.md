# 🏛️ Architect → 💻 Developer / 🧪 QA Handover

**Date**: 2025-12-30
**Branch**: `0x14-a-bench-harness`
**Phase**: V - Extreme Optimization (Metal Mode)
**Chapter**: 0x14-a Benchmark Harness

---

## 📋 Task Summary

| Item | Value |
|------|-------|
| **Goal** | Re-implement Exchange-Core test data generation algorithm in Rust |
| **Deliverable** | Rust code that generates identical datasets to Java reference |
| **Success Criteria** | Generated data matches `golden_*.csv` byte-for-byte |

---

## ✅ Architect Deliverables (Complete)

| Artifact | Location | Status |
|----------|----------|--------|
| Methodology Overview | `docs/src/0x14-extreme-optimization.md` | ✅ Done |
| Implementation Spec | `docs/src/0x14-a-bench-harness.md` | ✅ Done |
| Golden Data | `docs/exchange_core_verification_kit/golden_data/` | ✅ Done |
| Reference Docs | `docs/exchange_core_verification_kit/` | ✅ Done |

---

## 💻 Developer Tasks

### Step 1: Create Module Structure
- [ ] Create `src/bench/mod.rs`
- [ ] Add `pub mod bench;` to `src/lib.rs`

### Step 2: Implement JavaRandom LCG PRNG
- [ ] Create `src/bench/java_random.rs`
- [ ] Unit test: first 100 numbers match Java output

### Step 3: Implement TestOrdersGenerator
- [ ] Create `src/bench/order_generator.rs`
- [ ] Pareto distribution for symbol/user weights
- [ ] Order generation logic (GTC, IOC, Cancel, Move, Reduce)

### Step 4: Golden CSV Verification
- [ ] `#[test] fn test_golden_single_pair_margin()`
- [ ] `#[test] fn test_golden_single_pair_exchange()`

---

## 🧪 QA Tasks

### Verification Points
- [ ] All 1,100 rows in `golden_single_pair_margin.csv` match
- [ ] All 1,100 rows in `golden_single_pair_exchange.csv` match
- [ ] `order_id`, `price`, `size`, `uid` fields are bit-exact

### Edge Cases
- [ ] Seed = 0 behavior
- [ ] Large seed values (i64 boundary)
- [ ] Pareto distribution randomness

---

## 📦 Reference Materials

1. **LCG PRNG Spec**: `docs/exchange_core_verification_kit/TEST_REPRODUCTION_PACKAGE.md` (Section 1.1)
2. **Seed Derivation**: `TEST_REPRODUCTION_PACKAGE.md` (Section 1.2)
3. **Order Generation Logic**: `exchange_core_test_analysis.md` (Section 3)
4. **Golden Data Format**: `golden_data/*.csv`

---

## ❌ Out of Scope (This Chapter)

- Benchmark measurement (Criterion)
- Performance optimization
- Zero-Copy implementation
- CPU affinity

---

**Architect Sign-off**: ✅ Ready for Developer implementation
