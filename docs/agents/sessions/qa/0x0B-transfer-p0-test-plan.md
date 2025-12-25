# P0 Test Cases Implementation Plan

> **Target**: `scripts/test_transfer_e2e.sh`  
> **Focus**: 12 Critical (P0) Test Cases  
> **Estimated Effort**: 2-3 days

---

## Test Case Template Structure

Each test follows this pattern:
```python
def test_scenario_name():
    # 1. Preconditions: Setup test state
    # 2. Action: Execute transfer
    # 3. Assertions: Verify response + state
    # 4. Cleanup: (if needed)
```

---

## 📋 P0 Test Cases (Priority Order)

### **TC-P0-01: Insufficient Balance**
**Priority**: P0 (Critical)  
**Category**: Error Handling  
**Estimated Time**: 30 minutes

**Preconditions**:
- User 1001 has 30 USDT in Funding account

**Test Steps**:
1. Attempt transfer: Funding → Spot, 100 USDT
2. Verify API returns error
3. Verify balance unchanged

**Expected Result**:
```json
{
  "error": "INSUFFICIENT_BALANCE",
  "message": "Insufficient balance in source account"
}
```

**Python Code Template**:
```python
# TC-P0-01: Insufficient Balance
print("  [TC-P0-01] Insufficient Balance Test...")
# Setup: Create user with only 30 USDT
PGPASSWORD db_exec("""
    DELETE FROM balances_tb WHERE user_id = 1002;
    INSERT INTO balances_tb (user_id, asset_id, account_type, available)
    VALUES (1002, 2, 2, 30000000);  -- 30 USDT
""")

resp = client.post('/api/v1/private/transfer',
    json_body={'from': 'funding', 'to': 'spot', 'asset': 'USDT', 'amount': '100'},
    headers={'X-User-ID': '1002'})

if resp.status_code == 400 and 'INSUFFICIENT_BALANCE' in resp.text:
    print("    ✓ PASS: Correctly rejected")
    tests_passed += 1
else:
    print(f"    ✗ FAIL: Expected 400 with INSUFFICIENT_BALANCE, got {resp.status_code}")
    tests_failed += 1

# Verify balance unchanged
balance_after = get_balance(1002, 2, 2)
assert balance_after == 30.0, "Balance should not change on failure"
```

**Acceptance Criteria**:
- [ ] API returns 400 status code
- [ ] Error message contains "INSUFFICIENT_BALANCE"
- [ ] Source balance remains unchanged
- [ ] FSM state = `FAILED` (state=-10)
- [ ] No transfer record in COMMITTED state

---

### **TC-P0-02: Invalid Amount - Zero**
**Priority**: P0 (Critical)  
**Category**: Input Validation  
**Estimated Time**: 20 minutes

**Test Steps**:
1. Attempt transfer with amount = "0"
2. Verify rejected before hitting database

**Expected Result**:
```json
{
  "error": "INVALID_AMOUNT",
  "message": "Transfer amount must be greater than 0"
}
```

**Python Code Template**:
```python
# TC-P0-02: Invalid Amount - Zero
print("  [TC-P0-02] Invalid Amount (Zero)...")
resp = client.post('/api/v1/private/transfer',
    json_body={'from': 'funding', 'to': 'spot', 'asset': 'USDT', 'amount': '0'},
    headers={'X-User-ID': '1001'})

if resp.status_code == 400 and 'INVALID_AMOUNT' in resp.text:
    print("    ✓ PASS")
    tests_passed += 1
else:
    print(f"    ✗ FAIL: {resp.status_code}")
    tests_failed += 1
```

---

### **TC-P0-03: Invalid Amount - Negative**
**Priority**: P0 (Critical)  
**Category**: Input Validation  
**Estimated Time**: 15 minutes

**Test Steps**:
1. Attempt transfer with amount = "-10"
2. Verify rejected

**Python Code Template**:
```python
# TC-P0-03: Invalid Amount - Negative
print("  [TC-P0-03] Invalid Amount (Negative)...")
resp = client.post('/api/v1/private/transfer',
    json_body={'from': 'funding', 'to': 'spot', 'asset': 'USDT', 'amount': '-10'},
    headers={'X-User-ID': '1001'})

assert resp.status_code == 400 and 'INVALID_AMOUNT' in resp.text
print("    ✓ PASS")
tests_passed += 1
```

---

### **TC-P0-04: Precision Overflow**
**Priority**: P0 (Critical)  
**Category**: Edge Case / Security  
**Estimated Time**: 25 minutes

**Background**: USDT has 6 decimal places. Input with 9 decimals should be rejected.

**Test Steps**:
1. Attempt transfer with amount = "1.123456789" (9 decimals)
2. Verify rejected

**Expected Result**:
```json
{
  "error": "PRECISION_OVERFLOW",
  "message": "Amount exceeds asset precision (6 decimals)"
}
```

**Python Code Template**:
```python
# TC-P0-04: Precision Overflow
print("  [TC-P0-04] Precision Overflow (9 decimals for USDT)...")
resp = client.post('/api/v1/private/transfer',
    json_body={'from': 'funding', 'to': 'spot', 'asset': 'USDT', 'amount': '1.123456789'},
    headers={'X-User-ID': '1001'})

if resp.status_code == 400 and ('PRECISION' in resp.text or 'INVALID_AMOUNT' in resp.text):
    print("    ✓ PASS")
    tests_passed += 1
else:
    print(f"    ✗ FAIL: Expected precision error, got {resp.status_code}")
    tests_failed += 1
```

---

### **TC-P0-05: Same Account Transfer**
**Priority**: P0 (Critical)  
**Category**: Business Logic Validation  
**Estimated Time**: 15 minutes

**Test Steps**:
1. Attempt transfer: Funding → Funding (same account)
2. Verify rejected

**Expected Result**:
```json
{
  "error": "SAME_ACCOUNT",
  "message": "Source and target accounts cannot be the same"
}
```

**Python Code Template**:
```python
# TC-P0-05: Same Account Transfer
print("  [TC-P0-05] Same Account Transfer (Funding → Funding)...")
resp = client.post('/api/v1/private/transfer',
    json_body={'from': 'funding', 'to': 'funding', 'asset': 'USDT', 'amount': '10'},
    headers={'X-User-ID': '1001'})

assert resp.status_code == 400 and 'SAME_ACCOUNT' in resp.text
print("    ✓ PASS")
tests_passed += 1
```

---

### **TC-P0-06: Invalid Asset**
**Priority**: P0 (Critical)  
**Category**: Asset Validation  
**Estimated Time**: 20 minutes

**Test Steps**:
1. Attempt transfer with non-existent asset "FAKE"
2. Verify rejected

**Expected Result**:
```json
{
  "error": "INVALID_ASSET",
  "message": "Asset not found or not supported"
}
```

**Python Code Template**:
```python
# TC-P0-06: Invalid Asset
print("  [TC-P0-06] Invalid Asset (FAKE)...")
resp = client.post('/api/v1/private/transfer',
    json_body={'from': 'funding', 'to': 'spot', 'asset': 'FAKE', 'amount': '10'},
    headers={'X-User-ID': '1001'})

assert resp.status_code == 400 and 'INVALID_ASSET' in resp.text
print("    ✓ PASS")
tests_passed += 1
```

---

### **TC-P0-07: Idempotency - Duplicate CID**
**Priority**: P0 (Critical)  
**Category**: Idempotency / Security  
**Estimated Time**: 45 minutes

**Test Steps**:
1. Submit transfer with cid="client-123"
2. Wait for COMMITTED
3. Submit SAME request with SAME cid
4. Verify returns original result, balance changes ONCE

**Expected Behavior**:
- First request: Creates transfer, balance changes
- Second request: Returns existing result, balance UNCHANGED

**Python Code Template**:
```python
# TC-P0-07: Idempotency - Duplicate CID
print("  [TC-P0-07] Idempotency (Duplicate CID)...")

# Get initial balance
balance_before = get_balance(1001, 2, 2)

# First request with cid
resp1 = client.post('/api/v1/private/transfer',
    json_body={
        'from': 'funding', 
        'to': 'spot', 
        'asset': 'USDT', 
        'amount': '20',
        'cid': 'client-idempotency-test-001'
    },
    headers={'X-User-ID': '1001'})

assert resp1.status_code == 200
req_id_1 = resp1.json()['data']['req_id']
print(f"    First request: req_id={req_id_1}")

balance_after_1 = get_balance(1001, 2, 2)
assert balance_after_1 == balance_before - 20, "Balance should decrease by 20"

# Second request with SAME cid
resp2 = client.post('/api/v1/private/transfer',
    json_body={
        'from': 'funding', 
        'to': 'spot', 
        'asset': 'USDT', 
        'amount': '20',  # Same parameters
        'cid': 'client-idempotency-test-001'  # Same CID
    },
    headers={'X-User-ID': '1001'})

assert resp2.status_code == 200
req_id_2 = resp2.json()['data']['req_id']
print(f"    Second request: req_id={req_id_2}")

# CRITICAL: Should return same req_id
if req_id_1 == req_id_2:
    print("    ✓ PASS: Same req_id returned")
    tests_passed += 1
else:
    print(f"    ✗ FAIL: Different req_id ({req_id_1} vs {req_id_2})")
    tests_failed += 1

# Balance should NOT change again
balance_after_2 = get_balance(1001, 2, 2)
if balance_after_2 == balance_after_1:
    print("    ✓ PASS: Balance unchanged on duplicate")
    tests_passed += 1
else:
    print(f"    ✗ FAIL: Balance changed again ({balance_after_1} → {balance_after_2})")
    tests_failed += 1
```

**Acceptance Criteria**:
- [ ] Second request returns same `req_id`
- [ ] Balance changes only once (not twice)
- [ ] Only one transfer record in database
- [ ] Both responses have `state=COMMITTED`

---

### **TC-P0-08: FSM State Verification**
**Priority**: P0 (Critical)  
**Category**: State Machine Integrity  
**Estimated Time**: 40 minutes

**Test Steps**:
1. Query `/api/v1/private/transfer/{req_id}` during/after transfer
2. Verify state progression: INIT → SOURCE_PENDING → SOURCE_DONE → TARGET_PENDING → COMMITTED
3. Verify database `fsm_transfers_tb` matches API response

**Python Code Template**:
```python
# TC-P0-08: FSM State Verification
print("  [TC-P0-08] FSM State Progression...")

# Initiate transfer
resp = client.post('/api/v1/private/transfer',
    json_body={'from': 'funding', 'to': 'spot', 'asset': 'USDT', 'amount': '15'},
    headers={'X-User-ID': '1001'})

req_id = resp.json()['data']['req_id']
final_state = resp.json()['data']['state']

print(f"    Transfer initiated: req_id={req_id}, final_state={final_state}")

# Query transfer status
status_resp = client.get(f'/api/v1/private/transfer/{req_id}', 
                         headers={'X-User-ID': '1001'})

if status_resp.status_code == 200:
    status_data = status_resp.json()['data']
    print(f"    API state: {status_data['state']}")
    
    # Verify database state matches
    db_state = PGPASSWORD psql_query(f"""
        SELECT state FROM fsm_transfers_tb WHERE req_id = '{req_id}'
    """)
    
    # State mapping: 40 = COMMITTED
    if status_data['state'] == 'COMMITTED' and db_state == 40:
        print("    ✓ PASS: API and DB state match")
        tests_passed += 1
    else:
        print(f"    ✗ FAIL: State mismatch (API={status_data['state']}, DB={db_state})")
        tests_failed += 1
else:
    print(f"    ✗ FAIL: Cannot query status ({status_resp.status_code})")
    tests_failed += 1
```

**Acceptance Criteria**:
- [ ] GET `/api/v1/private/transfer/{req_id}` returns 200
- [ ] Returned state matches database `fsm_transfers_tb.state`
- [ ] Final state is `COMMITTED` (state=40)

---

### **TC-P0-09: Invalid Account Type**
**Priority**: P0 (Critical)  
**Category**: Input Validation  
**Estimated Time**: 20 minutes

**Test Steps**:
1. Attempt transfer with from="margin" (unsupported account type)
2. Verify rejected

**Python Code Template**:
```python
# TC-P0-09: Invalid Account Type
print("  [TC-P0-09] Invalid Account Type (margin)...")
resp = client.post('/api/v1/private/transfer',
    json_body={'from': 'margin', 'to': 'spot', 'asset': 'USDT', 'amount': '10'},
    headers={'X-User-ID': '1001'})

assert resp.status_code == 400 and ('INVALID_ACCOUNT' in resp.text or 'UNSUPPORTED' in resp.text)
print("    ✓ PASS")
tests_passed += 1
```

---

### **TC-P0-10: Disabled Asset Transfer**
**Priority**: P0 (Critical)  
**Category**: Asset Lifecycle  
**Estimated Time**: 30 minutes

**Preconditions**:
- Disable USDT in `assets_tb`: `UPDATE assets_tb SET status = 0 WHERE asset_id = 2`

**Test Steps**:
1. Attempt transfer with disabled USDT
2. Verify rejected
3. Re-enable USDT

**Python Code Template**:
```python
# TC-P0-10: Disabled Asset Transfer
print("  [TC-P0-10] Disabled Asset Transfer...")

# Disable USDT
PGPASSWORD db_exec("UPDATE assets_tb SET status = 0 WHERE asset_id = 2")

resp = client.post('/api/v1/private/transfer',
    json_body={'from': 'funding', 'to': 'spot', 'asset': 'USDT', 'amount': '10'},
    headers={'X-User-ID': '1001'})

# Re-enable USDT
PGPASSWORD db_exec("UPDATE assets_tb SET status = 1 WHERE asset_id = 2")

assert resp.status_code == 400 and ('ASSET' in resp.text or 'SUSPENDED' in resp.text)
print("    ✓ PASS")
tests_passed += 1
```

---

### **TC-P0-11: Target Rollback on Explicit Fail** ⚠️ **(Mock Required)**
**Priority**: P0 (Critical)  
**Category**: FSM Rollback Logic  
**Estimated Time**: 3-4 hours (requires failure injection)

**Background**: This is the MOST CRITICAL test - verifies compensation logic works.

**Challenge**: Requires simulating UBSCore rejection (TARGET_PENDING → COMPENSATING).

**Options**:
1. **Mock UBSCore Adapter** - Inject failure in `TradingAdapter`
2. **Trigger Business Failure** - Create UBSCore state that rejects deposit
3. **Manual Database Manipulation** - Force FSM into COMPENSATING state

**Test Steps** (Option 1 - Recommended):
1. Modify `src/transfer/adapters/trading.rs` to inject failure for test req_id
2. Initiate transfer
3. Verify state progression: INIT → SOURCE_DONE → TARGET_PENDING → COMPENSATING → ROLLED_BACK
4. Verify source balance refunded

**Implementation Note**:
```rust
// In src/transfer/adapters/trading.rs (for testing only)
pub async fn deposit(&self, req: DepositRequest) -> AdapterResult {
    // TEST INJECTION POINT
    if req.req_id.contains("FORCE_FAIL") {
        return Err(AdapterError::ExplicitFail("Test injection".to_string()));
    }
    // ... normal logic
}
```

**Python Code Template**:
```python
# TC-P0-11: Target Rollback on Explicit Fail
print("  [TC-P0-11] Target Rollback (Compensating → RolledBack)...")

balance_before = get_balance(1001, 2, 2)

# Use special marker to trigger test failure
resp = client.post('/api/v1/private/transfer',
    json_body={
        'from': 'funding', 
        'to': 'spot', 
        'asset': 'USDT', 
        'amount': '30',
        'cid': 'FORCE_FAIL_TEST_001'  # Marker for adapter
    },
    headers={'X-User-ID': '1001'})

# Should eventually reach ROLLED_BACK
final_state = resp.json()['data']['state']
if final_state == 'ROLLED_BACK':
    print("    ✓ PASS: Reached ROLLED_BACK state")
    tests_passed += 1
else:
    print(f"    ✗ FAIL: Expected ROLLED_BACK, got {final_state}")
    tests_failed += 1

# Balance should be refunded (unchanged)
balance_after = get_balance(1001, 2, 2)
if balance_after == balance_before:
    print("    ✓ PASS: Balance refunded")
    tests_passed += 1
else:
    print(f"    ✗ FAIL: Balance changed ({balance_before} → {balance_after})")
    tests_failed += 1
```

**Acceptance Criteria**:
- [ ] FSM reaches COMPENSATING state (state=-20)
- [ ] FSM reaches ROLLED_BACK state (state=-30)
- [ ] Source balance is refunded (unchanged from initial)
- [ ] Database has refund operation record

---

### **TC-P0-12: Asset Transfer Flag Disabled**
**Priority**: P0 (Critical)  
**Category**: Asset Permissions  
**Estimated Time**: 25 minutes

**Preconditions**:
- Remove CAN_INTERNAL_TRANSFER flag: `UPDATE assets_tb SET asset_flags = asset_flags & ~16 WHERE asset_id = 2`

**Test Steps**:
1. Attempt transfer with USDT (flag disabled)
2. Verify rejected with TRANSFER_NOT_ALLOWED
3. Re-enable flag

**Python Code Template**:
```python
# TC-P0-12: Asset Transfer Flag Disabled
print("  [TC-P0-12] Asset Transfer Flag Disabled...")

# Disable transfer flag for USDT
PGPASSWORD db_exec("UPDATE assets_tb SET asset_flags = asset_flags & ~16 WHERE asset_id = 2")

resp = client.post('/api/v1/private/transfer',
    json_body={'from': 'funding', 'to': 'spot', 'asset': 'USDT', 'amount': '10'},
    headers={'X-User-ID': '1001'})

# Re-enable flag
PGPASSWORD db_exec("UPDATE assets_tb SET asset_flags = asset_flags | 16 WHERE asset_id = 2")

assert resp.status_code == 400 and 'NOT_ALLOWED' in resp.text
print("    ✓ PASS")
tests_passed += 1
```

---

## 📊 Implementation Summary

| Test ID | Category | Effort | Complexity | Mock Required |
|---------|----------|--------|------------|---------------|
| TC-P0-01 | Error Handling | 30m | Low | No |
| TC-P0-02 | Input Validation | 20m | Low | No |
| TC-P0-03 | Input Validation | 15m | Low | No |
| TC-P0-04 | Edge Case | 25m | Medium | No |
| TC-P0-05 | Business Logic | 15m | Low | No |
| TC-P0-06 | Asset Validation | 20m | Low | No |
| TC-P0-07 | Idempotency | 45m | Medium | No |
| TC-P0-08 | FSM Verification | 40m | Medium | No |
| TC-P0-09 | Input Validation | 20m | Low | No |
| TC-P0-10 | Asset Lifecycle | 30m | Medium | No |
| TC-P0-11 | Rollback Logic | 3-4h | **High** | **Yes** |
| TC-P0-12 | Asset Permissions | 25m | Medium | No |

**Total Estimated Effort**: **8-10 hours** (including TC-P0-11)

---

## 🔧 Implementation Approach

### Option A: Extend `test_transfer_e2e.sh` (Recommended)
- Add new Python test functions to existing script
- Maintain current structure
- Easier integration

### Option B: Separate Test Module
- Create `scripts/test_transfer_p0.sh`
- Modular, cleaner separation
- Can run independently

**Recommendation**: Option A (extend existing script) for faster delivery.

---

## ✅ Acceptance Criteria for Plan

- [x] All 12 P0 test cases documented
- [x] Test templates with code samples provided
- [x] Effort estimates included
- [x] Acceptance criteria defined per test
- [ ] Plan approved by user/team
- [ ] Ready to proceed to implementation (Phase B2)

---

*P0 Test Plan Created: 2024-12-25 22:31*
