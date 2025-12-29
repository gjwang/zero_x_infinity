# Phase 0x11-b: Multi-Persona QA Cross-Review

| Date | 2025-12-29 |
| :--- | :--- |
| **Phase** | 0x11-b (Sentinel Hardening & ETH Support) |
| **Participants** | Agent A (激进派), Agent B (保守派), Agent C (安全专家) |
| **Arbitrator** | Agent Leader (主编 / D节点) |

---

## 📋 Review Protocol

Each agent **严格审查** the other two agents' test plans:
1. **Gaps (遗漏)** - Missing test cases
2. **Overlaps (重叠)** - Redundant coverage
3. **Supplements (补充)** - Additional test cases to add
4. **Conflicts (冲突)** - Disagreements → Escalate to D节点

---

# 🔴 Agent A (激进派) Reviews

## A → B: Review of 保守派 Core Flow Tests

### ✅ Strengths
- TC-B01 SegWit Deposit Lifecycle 是 DEF-002 的核心验证，非常重要
- TC-B06 精度处理测试覆盖了 6/18 decimals 的差异
- TC-B07 回归测试确保不破坏已有功能

### ⚠️ Gaps Identified

| Gap | Issue | Recommended Addition | Priority |
| :--- | :--- | :--- | :--- |
| **无 Bech32m 测试** | BTC Taproot 地址 (bc1p...) 未覆盖 | 添加 TC-B09: Taproot 地址处理 | P2 |
| **ETH Gas Price 影响** | 高 Gas 时 Transaction pending 状态 | 添加 TC-B10: Pending Transaction 处理 | P1 |
| **缺少 Batch Deposit** | 多用户同时充值压力测试 | 添加 TC-B11: 100 并发用户充值 | P1 |

### 📋 Suggested Test Case (A → B)

```python
# TC-B09: Taproot Address Handling (Agent A suggests for Agent B)
def test_taproot_address_handling():
    """
    Scenario: 用户发送 BTC 到 Taproot 地址 (bc1p...)
    
    Question: 系统是否支持 Taproot？
    
    If Supported:
      - Expected: 正常入账
    If Not Supported:
      - Expected: 明确拒绝，不静默丢弃
      - Document this as known limitation
    
    Priority: P2 (Future-proofing for BTC ecosystem evolution)
    """
    pass
```

```python
# TC-B11: Concurrent Multi-User Deposit Stress Test
def test_concurrent_100_users():
    """
    Scenario: 100 用户同时请求充值地址并充值
    
    Steps:
    1. 并发创建 100 个用户
    2. 每个用户请求 BTC 地址
    3. 批量生成交易到所有地址
    4. 挖 6 块
    5. 验证所有 100 个用户余额正确
    
    Risk: Sentinel 在高并发下可能漏检
    """
    pass
```

---

## A → C: Review of 安全专家 Security Tests

### ✅ Strengths
- TC-C04 Fake ERC20 Event Injection 覆盖了合约伪造攻击
- TC-C09 Audit Trail 满足金融合规要求
- TC-C02 Private Key 日志检查非常关键

### ⚠️ Gaps Identified

| Gap | Issue | Recommended Addition | Priority |
| :--- | :--- | :--- | :--- |
| **无 Time-based 攻击** | 时间戳操纵攻击未覆盖 | 添加 TC-C10: Block Timestamp 验证 | P1 |
| **缺 DoS 测试** | 海量小额充值耗尽资源 | 添加 TC-C11: Dust Attack Resilience | P0 |
| **无 MEV 攻击考虑** | ETH 交易可能被 Front-run | 添加 TC-C12: Front-running Impact Analysis | P2 |

### 📋 Suggested Test Case (A → C)

```python
# TC-C11: Dust Attack Resilience (Agent A suggests for Agent C)
def test_dust_attack_resilience():
    """
    Security Scenario: 攻击者发送大量 Dust 充值消耗系统资源
    
    Attack Vector:
    1. 发送 10,000 笔 0.00000001 BTC 的充值
    2. 观察系统资源 (DB rows, memory, CPU)
    
    Expected:
    1. 低于 MIN_DEPOSIT_AMOUNT 的充值被忽略
    2. 系统资源保持稳定
    3. 不会创建海量无效记录
    
    Priority: P0 (Architect 在 Critical Review 中明确指出)
    """
    pass
```

```python
# TC-C10: Block Timestamp Verification
def test_block_timestamp_manipulation():
    """
    Security Scenario: 恶意矿工操纵区块时间戳
    
    Attack Vector:
    1. 区块时间戳设置为未来 2 小时
    2. 验证 Sentinel 是否检测异常
    
    Expected: 
    - 异常时间戳触发告警
    - 不影响充值处理，但记录警告
    """
    pass
```

---

# 🟢 Agent B (保守派) Reviews

## B → A: Review of 激进派 Edge Case Tests

### ✅ Strengths
- TC-A01 Mixed Address Types 确保多格式兼容
- TC-A07 Log Reorg During Scan 覆盖了 ETH 的关键边缘情况
- TC-A09 Multiple Outputs Same TX 是 UTXO 模型特有问题

### ⚠️ Concerns

| Concern | Issue | Recommendation |
| :--- | :--- | :--- |
| **TC-A07 风险** | Re-org 测试可能破坏测试环境 | 必须添加 cleanup/reset 步骤 |
| **TC-A08 不可控** | RPC 延迟模拟难以精确控制 | 使用 Mock RPC 而非真实节点 |
| **缺少基线验证** | 边缘测试后如何确认系统恢复正常？ | 添加 Post-Edge Health Check |

### ⚠️ Gaps Identified

| Gap | Issue | Recommended Addition | Priority |
| :--- | :--- | :--- | :--- |
| **无 Empty Block 测试** | 区块没有交易时的处理 | 添加 TC-A10: Empty Block Scanning | P2 |
| **缺 Orphan Detection** | 孤块检测逻辑 | 添加 TC-A11: Orphan Block Identification | P1 |
| **无 RPC Version 兼容** | 不同节点版本 RPC 差异 | 添加 TC-A12: RPC Compatibility Check | P2 |

### 📋 Suggested Test Case (B → A)

```python
# TC-A10: Empty Block Scanning (Agent B suggests for Agent A)
def test_empty_block_scanning():
    """
    Scenario: 区块不包含任何交易
    
    Edge Case: Sentinel 是否正确更新 cursor 而不报错？
    
    Steps:
    1. 当前 cursor 在 Block N
    2. 挖一个空块 N+1
    3. 验证 cursor 更新到 N+1
    4. 不应有任何错误日志
    
    Purpose: 边界条件处理
    """
    pass
```

```python
# TC-A13: Post-Chaos Health Check (mandatory after each chaos test)
def test_post_chaos_health_check():
    """
    After ANY destructive test (re-org, node kill, etc.):
    
    Steps:
    1. Verify Sentinel process is running
    2. Verify chain_cursor is sane (height ≤ actual chain height)
    3. Perform a fresh deposit and verify it works
    
    Purpose: 确保混沌测试不会永久破坏环境
    """
    pass
```

---

## B → C: Review of 安全专家 Security Tests

### ✅ Strengths
- TC-C01 Address Isolation 是资金安全的核心
- TC-C08 Internal Endpoint Auth 保护了内部接口
- TC-C03 Malformed Script 防止解析器崩溃

### ⚠️ Gaps Identified

| Gap | Issue | Recommended Addition | Priority |
| :--- | :--- | :--- | :--- |
| **无 Rate Limit 测试** | 地址生成 API 应有频率限制 | 添加 TC-C13: Address Generation Rate Limit | P1 |
| **缺 Session Hijacking** | JWT Token 被盗用场景 | 添加 TC-C14: Stolen Token Detection | P1 |
| **无 Error Leak 测试** | 错误信息是否泄露内部细节 | 添加 TC-C15: Error Response Sanitization | P1 |

### 📋 Suggested Test Case (B → C)

```python
# TC-C13: Address Generation Rate Limit (Agent B suggests for Agent C)
def test_address_generation_rate_limit():
    """
    Security Scenario: 攻击者快速生成大量地址 (Address Poisoning)
    
    Steps:
    1. 同一用户 1 分钟内请求 100 个地址
    2. 验证触发 Rate Limit
    3. 返回 429 Too Many Requests
    
    Risk: 无限制会导致地址池膨胀
    Note: Architect Critical Review 中明确提到此风险
    """
    pass
```

```python
# TC-C15: Error Response Sanitization
def test_error_response_no_internal_details():
    """
    Security Scenario: 错误响应不应包含内部信息
    
    Steps:
    1. 触发各种错误 (invalid address, DB error, etc.)
    2. 检查响应不包含:
       - Stack traces
       - File paths
       - SQL queries
       - Internal IPs
    
    Expected: 用户友好的通用错误消息
    """
    pass
```

---

# 🔒 Agent C (安全专家) Reviews

## C → A: Review of 激进派 Edge Case Tests

### ✅ Strengths
- TC-A06 USDT 非标准实现覆盖了真实世界问题
- TC-A07 Log Reorg 是 51% 攻击防护的关键
- TC-A09 Multiple Outputs 防止 UTXO 遗漏

### ⚠️ Security Concerns

| Concern | Security Risk | Recommendation |
| :--- | :--- | :--- |
| **TC-A08 模拟攻击** | 测试代码本身可能成为攻击向量 | 测试脚本不得包含真实攻击实现 |
| **Chaos 测试暴露** | Re-org 测试逻辑不应出现在生产环境 | 添加编译时 flag 隔离 |
| **日志安全** | 边缘测试可能产生敏感日志 | 确保测试日志不被持久化 |

### ⚠️ Gaps Identified

| Gap | Security Risk | Recommended Addition | Priority |
| :--- | :--- | :--- | :--- |
| **无 Supply Verification** | 充值金额与链上不一致 | 添加 TC-A14: Amount Supply Verification | P0 |
| **缺 Confirmations Bypass** | 0 确认充值攻击 | 添加 TC-A15: Zero-Conf Attack Prevention | P0 |
| **无 Replay 测试** | 同一 TX 在不同链上重放 | 添加 TC-A16: Cross-Chain TX Replay | P1 |

### 📋 Suggested Test Case (C → A)

```python
# TC-A15: Zero-Confirmation Attack Prevention (Agent C suggests for Agent A)
def test_zero_conf_attack_prevention():
    """
    Security Scenario: 攻击者尝试利用 0 确认充值
    
    Attack Vector:
    1. 发送大额 BTC 交易
    2. 交易进入 mempool，状态 DETECTED
    3. 立即尝试提款或交易
    4. 同时广播 Double-Spend 交易取消原交易
    
    Expected:
    1. DETECTED 状态不增加可用余额
    2. 提款/交易请求被拒绝
    3. 只有 FINALIZED 状态才能使用资金
    
    Priority: P0 (核心安全要求)
    """
    pass
```

```python
# TC-A14: Amount Supply Verification
def test_amount_supply_verification():
    """
    Security Scenario: 验证充值金额与链上数据一致
    
    Steps:
    1. 发送 1.23456789 BTC 到用户地址
    2. Sentinel 检测到充值
    3. 独立查询链上 UTXO 金额
    4. 验证 Sentinel 记录金额 == 链上金额
    
    Risk: 解析错误可能导致金额被篡改
    """
    pass
```

---

## C → B: Review of 保守派 Core Flow Tests

### ✅ Strengths
- TC-B01 SegWit Lifecycle 是 DEF-002 验证的关键
- TC-B03 Cursor Persistence 防止重复记账
- TC-B08 Idempotent Processing 保护双花

### ⚠️ Security Gaps

| Gap | Security Risk | Recommended Addition | Priority |
| :--- | :--- | :--- | :--- |
| **无 Confirmation 竞态** | 确认数更新时的并发问题 | 添加 TC-B12: Confirmation Race Condition | P1 |
| **缺 Balance 快照** | 入账时余额状态验证 | 添加 TC-B13: Pre/Post Balance Snapshot | P1 |
| **无 Finalization 回滚保护** | FINALIZED 后不可回滚 | 添加 TC-B14: Finalized Status Immutability | P0 |

### 📋 Suggested Test Case (C → B)

```python
# TC-B14: Finalized Status Immutability (Agent C suggests for Agent B)
def test_finalized_status_cannot_rollback():
    """
    Security Scenario: FINALIZED 状态的充值不能被回滚
    
    Steps:
    1. 完成一笔充值直到 FINALIZED
    2. 尝试通过内部 API 将状态改回 CONFIRMING
    3. 尝试通过 DB 直接修改状态
    
    Expected:
    1. API 调用失败 (403 或业务错误)
    2. DB 直接修改触发告警 (如有审计日志)
    3. 用户余额不受影响
    
    Priority: P0 (防止内部篡改)
    """
    pass
```

```python
# TC-B12: Confirmation Race Condition
def test_confirmation_race_condition():
    """
    Security Scenario: 两个 Sentinel 实例同时更新确认数
    
    Steps:
    1. 启动两个 Sentinel 实例 (模拟错误配置)
    2. 两者同时扫描同一区块
    3. 验证只产生一条记录，确认数正确
    
    Risk: 竞态条件可能导致重复入账
    """
    pass
```

---

# ⚖️ Agent Leader (D节点): Conflict Resolution & Final Consolidation

## 1. Conflicts Identified

### Conflict 1: Chaos Test Environment Isolation

| Agent | Position |
| :--- | :--- |
| **Agent A** | TC-A07, TC-A08 需要真实 Re-org 和 RPC 延迟模拟 |
| **Agent B** | Chaos 测试必须使用 Mock，避免破坏环境 |
| **Agent C** | Chaos 测试代码不应进入生产构建 |

**Leader Ruling**: ✅ **Compromise - Layered Approach**

```
Decision:
1. 创建独立的 chaos/ 目录，与主测试分离
2. 使用 Docker 容器化运行，每次测试后销毁
3. Chaos 测试脚本添加 #![cfg(feature = "chaos_test")] 编译隔离
4. 添加 TC-A13 Post-Chaos Health Check 作为 mandatory teardown
```

---

### Conflict 2: Minimum Deposit Threshold

| Agent | Position |
| :--- | :--- |
| **Agent A** | 不应硬编码阈值，应可配置 |
| **Agent C** | 必须有强制最低值防止 Dust Attack (TC-C11) |

**Leader Ruling**: ✅ **Accept Both**

```
Decision:
1. MIN_DEPOSIT_AMOUNT 可配置 (YAML)
2. 但配置值必须 >= ABSOLUTE_MIN (硬编码安全下限)
3. 测试两种场景:
   - TC-C11: Default threshold 防 Dust
   - TC-A-NEW: 配置为 0 时系统是否正确拒绝
```

---

### Conflict 3: Rate Limit Threshold for Address Generation

| Agent | Position |
| :--- | :--- |
| **Agent A** | 100/minute 供压力测试 |
| **Agent C** | 10/minute 保守安全 |

**Leader Ruling**: ✅ **Accept Agent C (Production), Agent A (Stress Only)**

```
Decision:
1. 生产配置: 10 addresses/minute/user (Agent C)
2. 压力测试: 临时调整为 1000/minute 进行 Load Test
3. 添加 TC-C13 验证默认限制生效
```

---

## 2. Consolidated Additions from Cross-Review

Based on all agents' reviews, the following test cases are **officially added**:

| ID | Test Case | Owner | Source | Priority |
| :--- | :--- | :--- | :--- | :--- |
| TC-B09 | Taproot Address Handling | Agent B | A → B | P2 |
| TC-B11 | Concurrent 100 Users | Agent B | A → B | P1 |
| TC-B12 | Confirmation Race Condition | Agent B | C → B | P1 |
| TC-B14 | Finalized Status Immutability | Agent B | C → B | P0 |
| TC-A10 | Empty Block Scanning | Agent A | B → A | P2 |
| TC-A11 | Orphan Block Identification | Agent A | B → A | P1 |
| TC-A13 | Post-Chaos Health Check | Agent A | B → A | P0 |
| TC-A14 | Amount Supply Verification | Agent A | C → A | P0 |
| TC-A15 | Zero-Conf Attack Prevention | Agent A | C → A | P0 |
| TC-C10 | Block Timestamp Verification | Agent C | A → C | P1 |
| TC-C11 | Dust Attack Resilience | Agent C | A → C | P0 |
| TC-C13 | Address Generation Rate Limit | Agent C | B → C | P1 |
| TC-C15 | Error Response Sanitization | Agent C | B → C | P1 |

---

## 3. Updated Test Count Summary

| Agent | Original | Added via Cross-Review | Final Total |
| :--- | :---: | :---: | :---: |
| Agent A (激进派) | 9 | +5 | **14** |
| Agent B (保守派) | 8 | +4 | **12** |
| Agent C (安全专家) | 9 | +4 | **13** |
| **Total** | **26** | **+13** | **39** |

### Priority Breakdown (Final)

| Priority | Count | Description |
| :--- | :---: | :--- |
| **P0** | 13 | 必须通过才能发布 |
| **P1** | 16 | 应该通过，可有限制条件 |
| **P2** | 10 | 最好通过，可文档化为已知限制 |

---

## 4. Final P0 Critical Path (Updated)

| # | ID | Test Case | Agent | Rationale |
| :--- | :--- | :--- | :--- | :--- |
| 1 | TC-B01 | SegWit Deposit Lifecycle | B | DEF-002 核心验证 |
| 2 | TC-B04 | ERC20 Deposit Lifecycle | B | ETH Sentinel 核心 |
| 3 | TC-B07 | 0x11-a Full Regression | B | 防止回归 |
| 4 | TC-B14 | Finalized Status Immutability | B | 防止内部篡改 |
| 5 | TC-A13 | Post-Chaos Health Check | A | Chaos 测试安全网 |
| 6 | TC-A14 | Amount Supply Verification | A | 金额一致性 |
| 7 | TC-A15 | Zero-Conf Attack Prevention | A | 防双花 |
| 8 | TC-C01 | SegWit Address Isolation | C | 资金隔离 |
| 9 | TC-C04 | Fake ERC20 Event Injection | C | 防伪造 |
| 10 | TC-C09 | Audit Trail for Deposits | C | 合规审计 |
| 11 | TC-C11 | Dust Attack Resilience | C | 防 DoS |

**Total P0**: 11 tests (was 8, added 3 critical security tests from cross-review)

---

## 5. Action Items

| Owner | Action | Deadline |
| :--- | :--- | :--- |
| Agent A | 添加 TC-A10, A11, A13, A14, A15 到 `agent_a_edge_cases.py` | Before Dev Handover |
| Agent B | 添加 TC-B09, B11, B12, B14 到 `agent_b_core_flow.py` | Before Dev Handover |
| Agent C | 添加 TC-C10, C11, C13, C15 到 `agent_c_security.py` | Before Dev Handover |
| Leader | 更新 `run_all_0x11b.sh` 包含所有 39 个测试 | After all agents |
| Leader | 更新主设计文档反映新增 P0 测试 | After consolidation |

---

## 6. Sign-off

| Agent | Review Complete | Signature |
| :--- | :---: | :--- |
| Agent A (激进派) | ✅ | Reviewed B & C, provided 5 supplements |
| Agent B (保守派) | ✅ | Reviewed A & C, provided 5 supplements |
| Agent C (安全专家) | ✅ | Reviewed A & B, provided 6 supplements |
| Leader (D节点) | ✅ | Resolved 3 conflicts, consolidated 13 additions |

---

*Cross-Review Completed: 2025-12-29*
*Total Test Cases: 39 (26 original + 13 from cross-review)*
*Arbitration Node: Agent Leader (D节点/主编)*
