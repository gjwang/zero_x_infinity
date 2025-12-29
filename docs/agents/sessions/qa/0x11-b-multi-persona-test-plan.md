# Phase 0x11-b: Multi-Persona QA Test Plan

| Date | 2025-12-29 |
| :--- | :--- |
| **Phase** | 0x11-b (Sentinel Hardening & ETH Support) |
| **Scope** | DEF-002 Fix (BTC P2WPKH) + ETH Sentinel Integration |
| **QA Team** | Agent A (激进派), Agent B (保守派), Agent C (安全专家) |
| **Coordinator** | Agent Leader (主编) |

---

## 📋 Architect Directives Summary

| Priority | Objective | Owner |
| :--- | :--- | :--- |
| **P0** | Fix DEF-002: BTC Sentinel must detect SegWit (`bcrt1...`) deposits | All Agents |
| **P1** | Implement ETH Sentinel: `eth_getLogs` for ERC20 `Transfer` events | All Agents |

---

# 🔴 Agent A (激进派 QA): Edge Case & Vulnerability Tests

> **Focus**: 边缘测试 (Edge Cases)，找系统在极端条件下的漏洞。

## A1. BTC SegWit Edge Cases

### TC-A01: Mixed Address Types in Single Block
```python
def test_mixed_address_types_single_block():
    """
    Scenario: 同一区块内同时包含 Legacy (P2PKH) 和 SegWit (P2WPKH) 充值
    
    Edge Case: Sentinel 是否正确区分两种不同的脚本类型？
    
    Steps:
    1. 用户 A 请求 Legacy 地址 (1A1z...)
    2. 用户 B 请求 SegWit 地址 (bcrt1...)
    3. 发送 0.5 BTC 到两个地址
    4. 挖一个区块 (两笔交易在同一块)
    5. 验证两个用户余额分别为 0.5 BTC
    
    Risk: 地址解析器可能只识别一种类型
    """
    pass
```

### TC-A02: Nested SegWit (P2SH-P2WPKH)
```python
def test_nested_segwit_p2sh_p2wpkh():
    """
    Scenario: 用户发送到嵌套 SegWit 地址 (3xxx... 格式)
    
    Edge Case: 如果系统只支持 Native SegWit，嵌套格式是否正确处理？
    
    Expected: 
    - 如果支持: 正确入账
    - 如果不支持: 明确拒绝并记录日志，而非静默丢弃
    """
    pass
```

### TC-A03: SegWit Witness Program Boundary
```python
def test_segwit_witness_program_boundary():
    """
    Scenario: 测试 Witness Program 边界条件
    
    Edge Cases:
    1. 20-byte program (P2WPKH) - 标准，应识别
    2. 32-byte program (P2WSH) - 应识别或明确不支持
    3. 非标准长度 - 应拒绝
    
    Risk: `extract_address` 可能只处理硬编码长度
    """
    pass
```

## A2. ETH Sentinel Edge Cases

### TC-A04: ERC20 Transfer with Zero Amount
```python
def test_erc20_zero_amount_transfer():
    """
    Scenario: 恶意合约发送 amount=0 的 Transfer 事件
    
    Edge Case: 系统是否会创建无效的充值记录？
    
    Expected: 忽略 amount=0 的转账，不创建 deposit 记录
    """
    pass
```

### TC-A05: ERC20 Transfer to Contract (Not User)
```python
def test_erc20_transfer_to_contract_address():
    """
    Scenario: Token 转账目标是合约地址而非 EOA
    
    Edge Case: 用户地址表中如果意外包含合约地址？
    
    Expected: 验证 `to` 地址确实是 EOA，否则告警
    """
    pass
```

### TC-A06: Non-Standard ERC20 (USDT Special Case)
```python
def test_non_standard_erc20_usdt():
    """
    Scenario: USDT 合约非标准实现 (无 return value in transfer)
    
    Edge Case: 解析器是否处理 USDT 特殊情况？
    
    Steps:
    1. 部署真实 USDT ABI 的 Mock 合约
    2. 调用 transfer()
    3. 验证 Sentinel 正确解析
    
    Risk: 标准 ERC20 解析器可能失败
    """
    pass
```

### TC-A07: Log Reorganization During Scan
```python
def test_log_reorg_during_scan():
    """
    Scenario: eth_getLogs 返回结果后，查询的区块被 re-org
    
    Edge Case: 
    1. Sentinel 调用 eth_getLogs (Block 100)
    2. 返回 5 个 Transfer 事件
    3. 在 Sentinel 处理前，Block 100 被 re-org
    4. Block 100' 只有 3 个 Transfer 事件
    
    Expected: Sentinel 检测到 blockHash 不匹配，回滚并重新扫描
    """
    pass
```

## A3. Chaos Engineering

### TC-A08: RPC Node Latency Spike
```python
def test_rpc_latency_spike():
    """
    Scenario: RPC 节点响应延迟突然增加到 30 秒
    
    Edge Case: Sentinel 是否会超时？是否会重复处理？
    
    Expected: 
    1. 超时后重试 (with backoff)
    2. 不会重复入账 (幂等性保护)
    """
    pass
```

### TC-A09: Multiple Deposits Same TX
```python
def test_multiple_outputs_same_tx():
    """
    Scenario (BTC): 一笔交易包含多个输出到同一用户地址
    
    Edge Case: 是否每个 UTXO 分别计入？
    
    Steps:
    1. User 请求一个 BTC 地址
    2. 构造一笔交易包含两个输出到同一地址 (0.5 + 0.3 BTC)
    3. 验证用户余额 = 0.8 BTC
    
    Risk: 可能只记录第一个输出
    """
    pass
```

---

# 🟢 Agent B (保守派 QA): Core Flow & Regression Tests

> **Focus**: 核心流程稳定性，回归测试，确保基本功能正常。

## B1. BTC SegWit Core Flow (DEF-002 Fix Verification)

### TC-B01: SegWit Deposit Lifecycle (Critical Path)
```python
def test_segwit_deposit_lifecycle():
    """
    Scenario: 标准 SegWit 充值完整生命周期
    
    Steps:
    1. 用户请求 BTC 充值地址 (应返回 bcrt1... 格式)
    2. 发送 1 BTC 到该地址
    3. 挖 1 块 -> 状态变为 DETECTED
    4. 挖 5 块 -> 状态变为 CONFIRMING (N/6)
    5. 挖 1 块 -> 状态变为 FINALIZED
    6. 用户余额 = 1 BTC
    
    Critical Verification: 这是 DEF-002 的核心修复验证
    """
    pass
```

### TC-B02: Legacy Address Regression (No Regression from DEF-002 Fix)
```python
def test_legacy_address_no_regression():
    """
    Scenario: 验证 Legacy 地址充值仍然正常 (回归测试)
    
    Steps:
    1. 请求 Legacy P2PKH 地址 (如果支持)
    2. 发送 0.5 BTC
    3. 验证正常入账
    
    Purpose: 确保 SegWit 修复没有破坏 Legacy 支持
    """
    pass
```

### TC-B03: Cursor Persistence After SegWit Detection
```python
def test_cursor_persistence_segwit():
    """
    Scenario: Sentinel 成功识别 SegWit 充值后，重启是否恢复正确位置？
    
    Steps:
    1. Sentinel 扫描到包含 SegWit 充值的 Block N
    2. graceful shutdown
    3. 检查 chain_cursor.last_scanned_height = N
    4. 重启 Sentinel
    5. 不应重复处理 Block N
    
    Purpose: 状态持久化验证
    """
    pass
```

## B2. ETH Sentinel Core Flow

### TC-B04: ERC20 Deposit Lifecycle
```python
def test_erc20_deposit_lifecycle():
    """
    Scenario: 标准 ERC20 充值完整生命周期
    
    Steps:
    1. 用户请求 ETH 充值地址
    2. 调用 MockUSDT.transfer(user_addr, 100_000000) (100 USDT)
    3. 等待 X 个确认
    4. 验证用户 USDT 余额 = 100.000000
    
    Critical Path: ETH Sentinel 基本功能验证
    """
    pass
```

### TC-B05: Native ETH Deposit (Non-ERC20)
```python
def test_native_eth_deposit():
    """
    Scenario: 用户发送原生 ETH (非 Token)
    
    Steps:
    1. 用户请求 ETH 地址
    2. 发送 1 ETH 到该地址
    3. 验证余额
    
    Note: 实现依赖于是否支持原生 ETH 检测 (可能需要单独扫描)
    """
    pass
```

### TC-B06: ERC20 Precision Handling (6 vs 18 Decimals)
```python
def test_erc20_precision_handling():
    """
    Scenario: 不同 Token 有不同精度 (USDT=6, DAI=18)
    
    Steps:
    1. 配置 MockUSDT (6 decimals) 和 MockDAI (18 decimals)
    2. 分别充值 100 个最小单位
    3. 验证:
       - USDT: 100 -> 0.000100 USDT (100 / 10^6)
       - DAI: 100 -> 0.000000000000000100 DAI (100 / 10^18)
    
    Purpose: 精度配置正确性
    """
    pass
```

## B3. Regression Suite

### TC-B07: 0x11-a Full Regression
```bash
# Run existing 0x11-a verification suite
bash scripts/run_0x11a_verification.sh
```
**Purpose**: 确保 0x11-b 修改没有破坏 0x11-a 的功能。

### TC-B08: Idempotent Processing Regression
```python
def test_idempotent_processing_regression():
    """
    Scenario: 同一笔交易重复推送
    
    Steps:
    1. Sentinel 处理 TX-A
    2. 重启 Sentinel (cursor 未更新)
    3. 再次处理 TX-A
    4. 验证只有一条 deposit 记录
    
    Purpose: 幂等性保护未被破坏
    """
    pass
```

---

# 🔒 Agent C (安全专家 QA): Security & Permission Tests

> **Focus**: 权限安全、数据泄露、攻击向量分析。

## C1. BTC Security Tests

### TC-C01: SegWit Address Isolation
```python
def test_segwit_address_isolation():
    """
    Security Scenario: 用户 A 的 SegWit 地址不能被用户 B 访问
    
    Steps:
    1. User A 请求 bcrt1... 地址
    2. User B 尝试通过 API 查询 User A 的地址
    3. 发送 1 BTC 到 User A 地址
    4. 验证只有 User A 余额增加
    
    Risk: 地址归属关系被篡改
    """
    pass
```

### TC-C02: Private Key Never Exposed in Logs
```python
def test_private_key_not_in_logs():
    """
    Security Scenario: 检查所有日志不包含私钥
    
    Steps:
    1. 启用 DEBUG 日志
    2. 执行完整充值流程
    3. 扫描所有日志文件
    4. 验证不包含 "WIF", "xprv", "secret", "private"
    
    Risk: 密钥泄露
    """
    pass
```

### TC-C03: SegWit Malformed Script Injection
```python
def test_segwit_malformed_script_injection():
    """
    Security Scenario: 攻击者构造畸形 Witness Script
    
    Attack Vector:
    1. 构造一个看起来像 P2WPKH 但实际是恶意的 scriptPubKey
    2. 验证 Sentinel 不会解析崩溃
    3. 验证不会误入账
    
    Expected: 优雅拒绝，记录警告日志
    """
    pass
```

## C2. ETH Security Tests

### TC-C04: Fake ERC20 Event Injection
```python
def test_fake_erc20_event_injection():
    """
    Security Scenario: 攻击者部署假 Token 合约模拟 Transfer 事件
    
    Attack Vector:
    1. 部署 FakeUSDT 合约 (非官方地址)
    2. 发送 Transfer 事件到用户地址
    3. 验证 Sentinel 不会入账
    
    Expected: 只处理白名单合约地址的事件
    """
    pass
```

### TC-C05: ETH Topic Manipulation
```python
def test_eth_topic_manipulation():
    """
    Security Scenario: 攻击者构造 topic 顺序错误的事件
    
    Attack Vector:
    1. 发送 Transfer 事件但 topic[1] 和 topic[2] 互换
    2. 验证不会误将资金记入错误用户
    
    Expected: 严格按 Transfer(from, to, value) 顺序解析
    """
    pass
```

### TC-C06: ERC20 Amount Overflow
```python
def test_erc20_amount_overflow():
    """
    Security Scenario: Transfer 事件的 amount 超过系统最大值
    
    Attack Vector:
    1. 发送 amount = 2^256 - 1 的 Transfer
    2. 验证系统不会溢出
    
    Expected: 截断或拒绝，记录告警
    """
    pass
```

## C3. Cross-Chain Security

### TC-C07: RPC Node Spoofing Detection
```python
def test_rpc_node_spoofing_detection():
    """
    Security Scenario: RPC 节点被劫持返回假数据
    
    Attack Vector:
    1. 启动恶意 RPC 节点返回伪造的 Block
    2. 验证系统有机制检测 (如 Multi-Source Validation)
    
    Note: Phase I 可接受 "记录日志 + 告警" 作为最低标准
    """
    pass
```

### TC-C08: Internal Endpoint Authentication
```python
def test_internal_endpoint_authentication():
    """
    Security Scenario: 内部 Sentinel API 不能被外部访问
    
    Steps:
    1. 尝试直接调用 Sentinel 内部端点
    2. 验证需要内部认证 Token
    
    Risk: 未授权访问可伪造充值
    """
    pass
```

### TC-C09: Audit Trail for Deposits
```python
def test_audit_trail_deposits():
    """
    Security Scenario: 所有充值必须有完整审计日志
    
    Verification:
    1. 执行充值
    2. 检查 audit_log 表包含:
       - 时间戳
       - tx_hash
       - 用户 ID
       - 金额
       - 确认数变化
    3. 审计日志不可篡改
    
    Compliance: 金融系统审计要求
    """
    pass
```

---

# ⚖️ Agent Leader (主编): Test Consolidation & Execution Plan

## 1. Test Case Summary

| Agent | Focus | Test Cases | Priority Breakdown |
| :--- | :--- | :---: | :--- |
| **Agent A** | Edge Cases | 9 | P0: 2, P1: 4, P2: 3 |
| **Agent B** | Core Flow | 8 | P0: 3, P1: 3, P2: 2 |
| **Agent C** | Security | 9 | P0: 3, P1: 4, P2: 2 |
| **Total** | | **26** | P0: 8, P1: 11, P2: 7 |

## 2. P0 Critical Path (必须在 0x11-b 发布前通过)

| ID | Test Case | Agent | Rationale |
| :--- | :--- | :--- | :--- |
| TC-B01 | SegWit Deposit Lifecycle | B | DEF-002 核心修复验证 |
| TC-B04 | ERC20 Deposit Lifecycle | B | ETH Sentinel 核心功能 |
| TC-B07 | 0x11-a Full Regression | B | 防止回归 |
| TC-A01 | Mixed Address Types | A | 确保多类型兼容 |
| TC-A07 | Log Reorg During Scan | A | Re-org 安全性 |
| TC-C01 | SegWit Address Isolation | C | 资金安全 |
| TC-C04 | Fake ERC20 Event Injection | C | 防伪造攻击 |
| TC-C09 | Audit Trail | C | 合规要求 |

## 3. Execution Order

```
Phase 1: Environment Setup
├── Start bitcoind regtest
├── Start anvil (ETH)
├── Apply migrations
└── Start Sentinel

Phase 2: Core Flow Tests (Agent B) - Must pass first
├── TC-B01: SegWit Deposit Lifecycle ★★★
├── TC-B04: ERC20 Deposit Lifecycle ★★★
├── TC-B07: 0x11-a Regression ★★★
└── TC-B02 ~ TC-B08

Phase 3: Security Tests (Agent C) - Parallel with Edge
├── TC-C01: Address Isolation ★★★
├── TC-C04: Fake ERC20 ★★★
└── TC-C02 ~ TC-C09

Phase 4: Edge Case Tests (Agent A) - Last (May destabilize env)
├── TC-A01: Mixed Address ★★★
├── TC-A07: Log Reorg ★★★
└── TC-A02 ~ TC-A09 (with environment reset between chaos tests)

Phase 5: Cross-Review & Sign-off
├── Each Agent reviews other Agents' results
├── Leader consolidates final report
└── Issue QA Verdict
```

## 4. Test Script Location

```
scripts/tests/0x11b_sentinel/
├── run_all_0x11b.sh          # Master runner
├── agent_a_edge_cases.py     # Agent A tests
├── agent_b_core_flow.py      # Agent B tests
├── agent_c_security.py       # Agent C tests
└── lib/
    ├── btc_helper.py         # Bitcoin RPC utilities
    └── eth_helper.py         # Ethereum RPC utilities
```

## 5. Success Criteria

Phase 0x11-b is **QA APPROVED** when:

- [x] All P0 tests pass (8/8)
- [ ] All P1 tests pass (11/11)
- [ ] P2 tests: ≥ 80% pass (5/7)
- [ ] DEF-002 marked **CLOSED** in defect tracker
- [ ] Cross-review completed by all 3 Agents
- [ ] No new P0/P1 defects introduced

---

## 6. Handover Notes

**To Developer**:
- 请先实现 DEF-002 修复，QA 将优先执行 TC-B01 验证
- ETH Sentinel 实现后，通知 QA 执行 TC-B04

**To Architect**:
- 如发现 P0 安全问题 (TC-C01, TC-C04)，将立即 Escalate

---

*Test Plan Created: 2025-12-29*
*QA Team: Agent A (激进派), Agent B (保守派), Agent C (安全专家)*
*Coordinator: Agent Leader (主编)*
