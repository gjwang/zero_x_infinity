# Trade Fee System - QA Test Execution Plan

> **Task**: Verify Trade Fee System (Phase 0x0C) implementation  
> **Approach**: Execute existing tests, validate critical scenarios, generate report  
> **Mode**: Read-only testing (no code changes)

---

## 1. Current Test Status

### Test Coverage Summary
From `docs/test/0x0C-trade-fee-test-checklist.md`:

| Category | Passed ✅ | Pending ⚠️ E2E | Not Supported ❌ | Total |
|----------|----------|---------------|----------------|-------|
| Unit Tests (U01-U13) | 9 | 4 | 0 | 13 |
| Integration (I01-I11) | 5 | 5 | 1 | 11 |
| Asset Conservation (C01-C07) | 4 | 3 | 0 | 7 |
| Database (D01-D08) | 4 | 4 | 0 | 8 |
| API Tests (A01-A08) | 5 | 3 | 0 | 8 |
| Edge Cases (E01-E07) | 4 | 2 | 1 | 7 |
| Performance (P01-P09) | 1 | 8 | 0 | 9 |
| Security (S01-S04) | 2 | 2 | 0 | 4 |
| Regression (R01-R03) | 2 | 1 | 0 | 3 |
| **Total** | **36** | **32** | **2** | **70** |

**Pass Rate**: 51% (36/70) - Unit tests mostly complete, E2E tests pending

### Existing Test Infrastructure

**Unit Tests**:
- `src/ubscore.rs::test_fee_calculation_accuracy` ✅
- `src/ubscore.rs::test_settle_trade_maker_role` ✅
- `src/ubscore.rs::test_settle_trade_conservation` ✅

**E2E Test Script**:
- `scripts/test_fee_e2e.sh` - Full API verification
  - Issue: Path problem with `inject_orders.py` (needs fix)
  - Flow: Clears DB → Starts Gateway → Injects orders → Queries API → Verifies fee fields

---

## 2. Proposed QA Execution Plan

### Phase 1: Unit Test Verification ✅
**Goal**: Confirm existing unit tests still pass

**Test Command**:
```bash
cargo test test_fee_calculation_accuracy --release
cargo test test_settle_trade_maker_role --release
cargo test test_settle_trade_conservation --release
```

**Acceptance Criteria**:
- All 3 tests pass
- No compilation warnings
- Fee calculation formulas match specifications:
  - `fee = (amount * rate) / 10^6`
  - Maker/Taker role assignment correct
  - Asset conservation law holds (Σ debits = Σ credits)

---

### Phase 2: E2E Test Execution ⚠️
**Goal**: Run existing E2E test and validate real API responses

**Test Command**:
```bash
./scripts/test_fee_e2e.sh
```

**Known Issue**: Path error for `inject_orders.py`
- Script calls: `scripts/lib/inject_orders.py`
- Actual path: `scripts/inject_orders.py`
- **Fix Required**: Update path in test script (Line 139)

**Acceptance Criteria**:
- Test passes (5/5 checks)
- API response contains:
  - `fee` field (decimal string)
  - `fee_asset` field (asset symbol)
  - `role` field ("MAKER" or "TAKER")
- At least one trade has `fee > 0`

---

### Phase 3: Manual Spot Checks 🔍
**Goal**: Verify critical fee scenarios manually

#### Test 3.1: Basic Fee Calculation
**Setup**: Place a buy order for 1 BTC @ 100,000 USDT (Taker)

**Expected**:
- Base taker fee rate: 0.20% (2000/10^6)
- Gross received: 1 BTC
- Fee: 0.002 BTC (1 BTC × 0.002)
- Net received: 0.998 BTC

**Verification**:
```bash
# Query user trades
curl -X GET "http://localhost:8080/api/v1/private/trades?limit=1" \
  -H "X-User-ID: 1001"

# Check response
{
  "data": [{
    "qty": "1.00000000",
    "fee": "0.00200000",
    "fee_asset": "BTC",
    "role": "TAKER"
  }]
}
```

#### Test 3.2: VIP Discount (if VIP configured)
**Setup**: User with VIP level 3 (70% discount → 0.20% × 0.7 = 0.14%)

**Expected**:
- Fee: 0.0014 BTC (1 BTC × 0.0014)

**Note**: Requires VIP configuration in database. Skip if no VIP users.

#### Test 3.3: Asset Conservation Check
**Query TDengine**:
```sql
SELECT 
  SUM(debit_amt) as total_debits,
  SUM(credit_amt) as total_credits
FROM balance_events
WHERE trade_id = <TRADE_ID>;

-- Expected: total_debits = total_credits
```

---

## 3. Gap Analysis

### Critical Tests Passed ✅
- [x] Fee calculation accuracy (U01-U07)
- [x] Maker/Taker role assignment (U08-U10)
- [x] Asset conservation (C01-C04)
- [x] API response format (A01-A04)
- [x] WebSocket push format (A05)

### E2E Tests Pending ⚠️
**High Priority**:
- [ ] Buyer fee deduction from BTC (U11)
- [ ] Seller fee deduction from USDT (U12)
- [ ] VIP discount end-to-end (I06)
- [ ] Zero-fee symbol (E01)
- [ ] Fee ledger reconciliation (C06)

**Medium Priority**:
- [ ] Performance TPS test (P01-P04)
- [ ] Query performance (D05-D08)

**Not Supported** (Out of Scope):
- ❌ Hot config reload (I08)
- ❌ Event replay recovery (E07)

---

## 4. Verification Strategy

### Automated Tests
1. **Unit Tests**: `cargo test` (all fee-related tests)
2. **E2E Script**: `./scripts/test_fee_e2e.sh` (after path fix)

### Manual Validation
3. **API Response Check**: Query `/api/v1/private/trades` and verify fee fields
4. **TDengine Query**: Check asset conservation in `balance_events` table
5. **Revenue Account**: Query REVENUE account balance (user_id=0)

### Acceptance Checklist
- [ ] All unit tests pass
- [ ] E2E test passes (5/5 checks)
- [ ] Fee conservation verified (Σ = 0)
- [ ] API includes fee/fee_asset/role fields
- [ ] At least 1 trade has fee > 0
- [ ] Revenue account accumulates fees correctly

---

## 5. Known Issues & Blockers

### Blocker 1: E2E Script Path Error
**Issue**: `inject_orders.py` path incorrect in `test_fee_e2e.sh:139`  
**Impact**: E2E test fails at order injection step  
**Fix**: Update path from `scripts/lib/inject_orders.py` to `scripts/inject_orders.py`

### Non-Blocker: VIP Testing
**Issue**: VIP discount tests require VIP user setup in database  
**Workaround**: Skip VIP tests if no VIP users configured  
**Priority**: P2 (nice to have, not critical)

---

## 6. Deliverables

### Test Report
Will generate: `fee_system_test_report.md` containing:
- Test execution results (pass/fail counts)
- Fee calculation verification
- Asset conservation audit results
- API response samples
- Gap analysis vs. test checklist
- QA sign-off decision

### Recommendations
- Fix E2E script path
- Implement missing E2E tests (32 pending)
- Performance benchmarks (if production-critical)

---

## 7. Estimated Effort

| Phase | Task | Time |
|-------|------|------|
| 1 | Run unit tests | 15 min |
| 2 | Fix E2E script & run | 30 min |
| 3 | Manual spot checks | 30 min |
| 4 | Generate report | 30 min |
| **Total** | | **~2 hours** |

---

**QA Approach**: Pragmatic testing - validate existing tests pass, spot-check critical scenarios, document gaps for future work. No code changes in this task.
