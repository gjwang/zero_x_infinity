# 🔄 Phase 0x14-c Money Safety: 跨视角交叉审核

> **目的**: 此组件是所有资金操作的基础底座，容不得任何闪失。各 Agent 相互审核，补充盲点。
> **日期**: 2025-12-31
> **流程**: A→审核C, C→审核A, B→审核A&C, Leader D 仲裁冲突

---

# 🔥 Agent A (激进派) 审核 Agent C (安全专家) 的测试

## A→C 审核意见

### ✅ 认可的测试

| C 测试 | A 的评价 |
|--------|----------|
| C-TC-001 溢出资金错算 | ✅ 核心场景，必须保留 |
| C-TC-003 跨路径精度一致性 | ✅ 非常关键，建议升级为 Critical |
| C-TC-004 注入攻击防护 | ✅ Payload 覆盖全面 |

### ⚠️ 需要补充的场景

#### A→C-ADD-001: 链上精度攻击 (Chain Precision Attack)

> **A 视角**: C 漏掉了链上精度与内部精度不一致的攻击向量

**攻击场景**:
```
ETH 链上精度: 18位
系统内部精度: 8位
攻击者存入: 1.000000000000000001 ETH (18位)
系统存储: 1.00000000 ETH (截断后8位)
攻击者提取: 1.00000000 ETH
损失: 0.000000000000000001 ETH 被系统"吞掉"

如果交易所执行大量微小交易，累积损失可观！
```

**补充测试用例**:

```python
class TestChainPrecisionAttack:
    """A→C-ADD-001: 链上精度攻击防护"""
    
    @pytest.mark.critical
    def test_chain_to_internal_truncation_tracked(self):
        """验证链上→内部精度截断被正确追踪"""
        # 模拟 ETH 存款 (18位链上精度)
        chain_amount = "1.000000000123456789"  # 18位
        
        # 存入系统
        deposit_resp = mock_eth_deposit(chain_amount)
        
        # 系统应记录原始链上金额
        assert deposit_resp.json()["on_chain_amount"] == chain_amount
        
        # 内部余额是截断后的 (8位)
        balance = get_balance("ETH")
        assert balance["available"] == "1.00000000"
        
        # 差额应被记录到"精度损失账户"进行审计
        audit_log = get_precision_loss_audit()
        assert "0.000000000123456789" in audit_log
    
    @pytest.mark.critical
    def test_withdrawal_cannot_exceed_internal_balance(self):
        """验证提现不能超过内部余额（即使链上有更多）"""
        # 存入并截断
        mock_eth_deposit("1.000000000999999999")
        
        # 尝试提现原始链上金额
        resp = withdraw(amount="1.000000000999999999")
        
        # 必须拒绝
        assert resp.status_code == 400
        assert "INSUFFICIENT_BALANCE" in resp.json()["code"]
```

---

#### A→C-ADD-002: 时间相关精度攻击 (Time-based Precision Attack)

> **A 视角**: C 没有考虑时间戳与金额混合的攻击

**攻击场景**:
```
内部订单ID格式: timestamp + sequence
如果 timestamp 使用 u64 毫秒，与金额使用相同类型
可能导致类型混淆：
  - 订单ID被误解析为金额
  - 金额被误格式化为订单ID
```

**补充测试用例**:

```python
def test_type_confusion_order_id_vs_amount():
    """A→C-ADD-002: 防止订单ID与金额类型混淆"""
    
    # 构造一个"看起来像金额"的订单ID
    suspicious_order_id = "100000000"  # 1 BTC in scaled form
    
    # 查询这个订单
    resp = get_order(suspicious_order_id)
    
    # 不应返回任何"金额"字段等于这个ID的订单
    if resp.status_code == 200:
        order = resp.json()
        assert str(order.get("quantity", "")) != suspicious_order_id
        assert str(order.get("price", "")) != suspicious_order_id
```

---

#### A→C-ADD-003: 并发精度一致性攻击

> **A 视角**: C 的 C-TC-003 是串行验证，没有考虑并发场景

**攻击场景**:
```
Thread 1: 读取余额 1.12345678
Thread 2: 执行转账，余额变为 1.12345677 (精度丢失)
Thread 1: 基于旧余额计算 → 结果不一致
```

**补充测试用例**:

```python
import concurrent.futures

def test_concurrent_precision_consistency():
    """A→C-ADD-003: 并发操作下精度一致性"""
    
    initial_balance = "100.12345678"
    deposit(amount=initial_balance)
    
    def read_balance():
        return get_balance("BTC")["available"]
    
    def transfer_small():
        transfer(amount="0.00000001", to="user2")
        return get_balance("BTC")["available"]
    
    # 并发执行 100 次读写
    with concurrent.futures.ThreadPoolExecutor(max_workers=10) as executor:
        futures = []
        for i in range(50):
            futures.append(executor.submit(read_balance))
            futures.append(executor.submit(transfer_small))
        
        results = [f.result() for f in futures]
    
    # 所有余额读取必须是有效的 8 位精度数字
    for balance in results:
        if balance:
            assert len(balance.split('.')[-1]) == 8
            assert balance.replace('.', '').isdigit()
```

---

#### A→C-ADD-004: 负数绕过攻击

> **A 视角**: C 的注入测试没有覆盖负数变体

**攻击向量**:

| Input | 意图 | 系统应行为 |
|-------|------|------------|
| `-1.0` | 负数直接输入 | 400 拒绝 |
| `--1.0` | 双负号 | 400 拒绝 |
| `-0.0` | 负零 | 400 拒绝 |
| `"-1.0"` | 字符串包装负数 | 400 拒绝 |
| `1.0e-308` | 极小数 (接近零) | 视为零拒绝? |
| `1.0 - 2.0` | 表达式注入 | 400 拒绝 |

```python
NEGATIVE_BYPASS_PAYLOADS = [
    ("-1.0", "NEGATIVE"),
    ("--1.0", "INVALID_FORMAT"),
    ("-0.0", "ZERO_OR_NEGATIVE"),
    ("-0.00000001", "NEGATIVE"),
    ("1.0 - 2.0", "INVALID_FORMAT"),
    ("(-1)", "INVALID_FORMAT"),
]

@pytest.mark.parametrize("payload,expected_error", NEGATIVE_BYPASS_PAYLOADS)
def test_negative_bypass_attacks(payload, expected_error):
    """A→C-ADD-004: 负数绕过攻击"""
    resp = place_order(quantity=payload)
    assert resp.status_code == 400
```

---

## A→C 审核结论

| 项目 | 结论 |
|------|------|
| C 原有测试 | ✅ 全部保留，无需删除 |
| 补充测试 | +4 个用例 (A→C-ADD-001 ~ 004) |
| 优先级调整 | C-TC-003 建议升级为 🔴 Critical |
| 冲突 | 无 |

---

# 🔐 Agent C (安全专家) 审核 Agent A (激进派) 的测试

## C→A 审核意见

### ✅ 认可的测试

| A 测试 | C 的评价 |
|--------|----------|
| A-TC-001 超精度拒绝 | ✅ 安全关键，覆盖完整 |
| A-TC-002 溢出攻击 | ✅ 必须保留，建议增加 checked arithmetic 验证 |
| A-TC-003 畸形格式 | ✅ 覆盖广泛，无遗漏 |

### ⚠️ 需要补充的场景

#### C→A-ADD-001: 审计日志完整性 (Audit Trail Integrity)

> **C 视角**: A 的测试关注功能拒绝，但没有验证失败操作是否被正确审计

**安全需求**:
```
金融系统的每一次失败操作都必须被记录，以便：
1. 发现攻击模式
2. 法律合规要求
3. 安全事件响应
```

**补充测试用例**:

```python
class TestAuditTrailIntegrity:
    """C→A-ADD-001: 失败操作审计完整性"""
    
    @pytest.mark.security
    def test_overflow_attempt_logged(self):
        """验证溢出攻击尝试被记录"""
        # 发起溢出攻击
        resp = place_order(quantity="18446744073709551616")
        assert resp.status_code == 400
        
        # 检查审计日志
        audit = get_security_audit_log(
            event_type="AMOUNT_OVERFLOW",
            time_range="last_1_minute"
        )
        
        assert len(audit) >= 1
        assert "18446744073709551616" in audit[0]["raw_input"]
        assert audit[0]["user_id"] is not None
        assert audit[0]["ip_address"] is not None
        assert audit[0]["timestamp"] is not None
    
    @pytest.mark.security
    def test_precision_attack_logged(self):
        """验证精度攻击尝试被记录"""
        resp = place_order(quantity="1.123456789")  # 9位
        assert resp.status_code == 400
        
        audit = get_security_audit_log(event_type="PRECISION_EXCEEDED")
        assert len(audit) >= 1
    
    @pytest.mark.security
    def test_malformed_input_logged(self):
        """验证畸形输入尝试被记录"""
        resp = place_order(quantity=".5")
        assert resp.status_code == 400
        
        audit = get_security_audit_log(event_type="INVALID_FORMAT")
        assert len(audit) >= 1
```

---

#### C→A-ADD-002: 速率限制防护 (Rate Limiting)

> **C 视角**: A 的边缘测试可能被滥用为 DoS 攻击向量

**安全需求**:
```
如果攻击者大量发送畸形输入：
- 即使每个都被正确拒绝
- 也可能消耗服务器资源 (解析/验证/日志)
- 需要速率限制防护
```

**补充测试用例**:

```python
import time

class TestRateLimitingProtection:
    """C→A-ADD-002: 畸形输入速率限制"""
    
    @pytest.mark.security
    def test_malformed_input_rate_limited(self):
        """验证畸形输入被速率限制"""
        start = time.time()
        blocked_count = 0
        
        # 快速发送 100 个畸形请求
        for i in range(100):
            resp = place_order(quantity=f".{i}")
            if resp.status_code == 429:  # Too Many Requests
                blocked_count += 1
        
        elapsed = time.time() - start
        
        # 应该在合理时间内被限制
        assert blocked_count > 50, "Rate limiting not effective"
        assert elapsed < 5, "Requests not being rate limited efficiently"
    
    @pytest.mark.security
    def test_overflow_attempts_rate_limited(self):
        """验证溢出攻击被速率限制"""
        overflow_payloads = [f"1{'0' * i}" for i in range(20, 40)]
        
        blocked = 0
        for payload in overflow_payloads * 5:  # 100 次尝试
            resp = place_order(quantity=payload)
            if resp.status_code == 429:
                blocked += 1
        
        assert blocked > 0, "Overflow attempts should trigger rate limiting"
```

---

#### C→A-ADD-003: 时序侧信道防护 (Timing Side Channel)

> **C 视角**: A 的测试没有验证错误响应时间一致性

**安全风险**:
```
如果不同错误类型响应时间不同：
- "用户不存在" 快速返回
- "密码错误" 慢速返回
攻击者可据此枚举有效用户

同理，金额验证：
- "格式错误" 快速返回
- "精度超限" 需要解析后验证，较慢
可能泄露输入格式有效性信息
```

**补充测试用例**:

```python
import time
import statistics

class TestTimingSideChannel:
    """C→A-ADD-003: 时序侧信道防护"""
    
    @pytest.mark.security
    def test_error_response_timing_consistency(self):
        """验证错误响应时间一致"""
        test_cases = [
            (".5", "INVALID_FORMAT"),
            ("1.123456789", "PRECISION_EXCEEDED"),
            ("-1.0", "NEGATIVE"),
            ("abc", "INVALID_FORMAT"),
            ("18446744073709551616", "OVERFLOW"),
        ]
        
        timings = {}
        for payload, error_type in test_cases:
            times = []
            for _ in range(20):
                start = time.perf_counter()
                resp = place_order(quantity=payload)
                elapsed = time.perf_counter() - start
                times.append(elapsed)
            
            timings[error_type] = statistics.mean(times)
        
        # 所有错误类型的平均响应时间应在 2x 范围内
        min_time = min(timings.values())
        max_time = max(timings.values())
        
        assert max_time < min_time * 2, \
            f"Timing variance too high: {timings}"
```

---

#### C→A-ADD-004: 错误消息一致性 (Error Message Consistency)

> **C 视角**: A 的测试验证了拒绝，但没有验证错误消息不泄露信息

**补充测试用例**:

```python
class TestErrorMessageSafety:
    """C→A-ADD-004: 错误消息安全性"""
    
    @pytest.mark.security
    def test_precision_error_no_internal_details(self):
        """精度错误不泄露内部精度配置"""
        resp = place_order(quantity="1.123456789")
        error = resp.json()
        
        # 不应泄露具体精度配置
        error_str = str(error)
        assert "decimals=8" not in error_str.lower()
        assert "10^8" not in error_str
        assert "100000000" not in error_str
        
        # 应有用户友好的消息
        assert "precision" in error.get("message", "").lower() or \
               "too many decimal places" in error.get("message", "").lower()
    
    @pytest.mark.security
    def test_overflow_error_no_max_value(self):
        """溢出错误不泄露最大值"""
        resp = place_order(quantity="999999999999999999999")
        error = resp.json()
        
        # 不应泄露 u64::MAX
        error_str = str(error)
        assert "18446744073709551615" not in error_str
        assert "u64" not in error_str.lower()
        assert "MAX" not in error_str
```

---

## C→A 审核结论

| 项目 | 结论 |
|------|------|
| A 原有测试 | ✅ 全部保留 |
| 补充测试 | +4 个用例 (C→A-ADD-001 ~ 004) |
| 优先级调整 | 无 |
| 冲突 | 无 |

---

# 🛡️ Agent B (保守派) 审核 Agent A & C 的测试

## B→A 审核意见

### 关注点：边缘测试不应破坏核心流程

#### B→A-ADD-001: 边缘测试后的系统状态验证

> **B 视角**: A 的边缘测试验证了拒绝，但没有验证系统状态未被污染

```python
class TestSystemStateAfterEdgeCases:
    """B→A-ADD-001: 边缘测试后系统状态验证"""
    
    @pytest.mark.regression
    def test_system_clean_after_overflow_attempts(self):
        """溢出攻击后系统状态正常"""
        # 获取初始状态
        initial_orders = get_all_orders()
        initial_balance = get_balance("BTC")
        
        # 发起多次溢出攻击
        for i in range(10):
            place_order(quantity=f"1{'0' * (20+i)}")
        
        # 验证状态未变化
        assert get_all_orders() == initial_orders
        assert get_balance("BTC") == initial_balance
    
    @pytest.mark.regression
    def test_normal_order_works_after_malformed_inputs(self):
        """畸形输入后正常订单仍可工作"""
        # 发送畸形输入
        malformed_inputs = [".5", "5.", "1,000", "NaN", "Infinity"]
        for inp in malformed_inputs:
            place_order(quantity=inp)
        
        # 验证正常订单仍可工作
        resp = place_order(quantity="1.5", price="50000.0")
        assert resp.status_code == 200
```

---

## B→C 审核意见

### 关注点：安全测试不应过于严格导致误拒

#### B→C-ADD-001: 合法边界值不被误拒

> **B 视角**: C 的安全测试可能过于严格，拒绝一些合法输入

```python
class TestNoFalsePositives:
    """B→C-ADD-001: 安全检查不误拒合法输入"""
    
    @pytest.mark.regression
    def test_legitimate_large_amounts_accepted(self):
        """合法大额不被误判为溢出"""
        # 这些都是合法的大额
        large_amounts = [
            "100000.0",       # 10万 BTC (约 30亿美元)
            "1000000.0",      # 100万 BTC (实际不存在，但格式合法)
            "21000000.0",     # 2100万 BTC (总供应量)
        ]
        
        for amount in large_amounts:
            resp = place_order(quantity=amount, price="1.0")
            # 可能因余额不足失败，但不应是 OVERFLOW
            if resp.status_code == 400:
                assert "OVERFLOW" not in resp.json().get("code", "")
    
    @pytest.mark.regression
    def test_legitimate_small_amounts_accepted(self):
        """合法小额不被误判为零"""
        small_amounts = [
            "0.00000001",  # 1 satoshi
            "0.00000002",
            "0.0001",
        ]
        
        for amount in small_amounts:
            resp = place_order(quantity=amount, price="50000.0")
            # 不应被判为 ZERO_NOT_ALLOWED
            if resp.status_code == 400:
                assert "ZERO" not in resp.json().get("code", "")
```

---

## B→全体 审核意见

### B-GLOBAL-001: 测试数据隔离

> **B 视角**: 所有 Agent 的测试必须相互隔离，不能相互影响

```python
import pytest
import uuid

@pytest.fixture(autouse=True)
def test_isolation():
    """确保每个测试使用独立的测试用户和数据"""
    test_id = str(uuid.uuid4())[:8]
    
    # 创建测试用户
    user = create_test_user(f"qa_test_{test_id}")
    
    # 设置初始余额
    set_test_balance(user, "BTC", "1000.0")
    set_test_balance(user, "USDT", "10000000.0")
    
    yield user
    
    # 清理
    cleanup_test_user(user)
```

### B-GLOBAL-002: 测试顺序无关性

> **B 视角**: 测试必须可以任意顺序执行

```bash
# 随机顺序执行
pytest tests/0x14c/ --random-order

# 反向顺序执行
pytest tests/0x14c/ --reverse

# 单独执行每个测试
for test in $(pytest tests/0x14c/ --collect-only -q); do
    pytest "$test" || exit 1
done
```

---

## B 审核结论

| 项目 | 结论 |
|------|------|
| A 测试 | ✅ 保留，+1 系统状态验证 |
| C 测试 | ✅ 保留，+1 误拒检查 |
| 全局 | +2 测试基础设施要求 |
| 冲突 | 无 |

---

# 📝 Leader D: 仲裁与最终决议

## D.1 冲突审查

### 审查结果：无直接冲突

经审查，三位 Agent 的测试设计**无相互矛盾之处**：

| 交叉审核 | 结果 | 冲突 |
|----------|------|------|
| A→C | +4 补充 | 无 |
| C→A | +4 补充 | 无 |
| B→A | +1 补充 | 无 |
| B→C | +1 补充 | 无 |
| B→全局 | +2 基础设施 | 无 |

---

## D.2 优先级仲裁

### 关于 C-TC-003 优先级升级

**A 建议**: 将 C-TC-003 (跨路径精度一致性) 从 Critical 升级为 🔴 P0-Critical

**Leader 裁决**: ✅ **同意**

```
理由：
1. 精度不一致是资金损失的直接原因
2. 跨路径问题难以通过单元测试覆盖
3. 该场景在生产中曾导致真实损失（行业案例）
```

---

## D.3 最终测试矩阵

### 原始测试
| Agent | 原有测试 |
|-------|----------|
| A | 7 |
| B | 6 |
| C | 7 |
| **小计** | **20** |

### 交叉审核补充
| 补充来源 | 测试数 |
|----------|--------|
| A→C | +4 |
| C→A | +4 |
| B→A | +1 |
| B→C | +1 |
| B→全局 | +2 |
| **小计** | **+12** |

### 最终总计
| 类别 | 数量 |
|------|------|
| 原始测试 | 20 |
| 交叉补充 | 12 |
| **总计** | **32** |

---

## D.4 最终优先级分布

### 🔴 P0 - 必须通过 (阻塞发布) [10个]

| ID | 测试用例 | Owner | 来源 |
|----|----------|-------|------|
| B-TC-001 | 标准金额转换准确性 | B | 原始 |
| B-TC-002 | 往返转换一致性 | B | 原始 |
| A-TC-001 | 超精度数值拒绝 | A | 原始 |
| A-TC-002 | 数值溢出攻击防护 | A | 原始 |
| C-TC-001 | 溢出不导致资金错算 | C | 原始 |
| C-TC-003 | 跨路径精度一致性 | C | 原始→升级 |
| A→C-ADD-001 | 链上精度攻击防护 | A | 补充 |
| A→C-ADD-003 | 并发精度一致性 | A | 补充 |
| C→A-ADD-001 | 审计日志完整性 | C | 补充 |
| B→A-ADD-001 | 边缘测试后系统状态验证 | B | 补充 |

### 🟡 P1 - 应该通过 (关键功能) [12个]

| ID | 测试用例 | Owner | 来源 |
|----|----------|-------|------|
| A-TC-003 | 畸形格式拒绝 | A | 原始 |
| A-TC-004 | 零值边界处理 | A | 原始 |
| B-TC-004 | 回归测试通过 | B | 原始 |
| C-TC-002 | 无隐藏 dust | C | 原始 |
| C-TC-004 | 注入攻击防护 | C | 原始 |
| A→C-ADD-002 | 时间相关精度攻击 | A | 补充 |
| A→C-ADD-004 | 负数绕过攻击 | A | 补充 |
| C→A-ADD-002 | 速率限制防护 | C | 补充 |
| C→A-ADD-003 | 时序侧信道防护 | C | 补充 |
| C→A-ADD-004 | 错误消息一致性 | C | 补充 |
| B→C-ADD-001 | 合法输入不被误拒 | B | 补充 |
| B-GLOBAL-001 | 测试数据隔离 | B | 补充 |

### 🟢 P2 - 建议通过 (完善性) [10个]

| ID | 测试用例 | Owner | 来源 |
|----|----------|-------|------|
| A-TC-005 | 跨资产精度隔离 | A | 原始 |
| A-TC-006 | 显示精度不丢失存储精度 | A | 原始 |
| A-TC-007 | 审计脚本绕过检测 | A | 原始 |
| B-TC-003 | SymbolManager 精度获取 | B | 原始 |
| B-TC-005 | API 响应格式一致 | B | 原始 |
| B-TC-006 | 迁移文件功能验证 | B | 原始 |
| C-TC-005 | 内部表示不泄露 | C | 原始 |
| C-TC-006 | 错误消息不泄露敏感信息 | C | 原始 |
| C-TC-007 | 审计脚本不可被禁用 | C | 原始 |
| B-GLOBAL-002 | 测试顺序无关性 | B | 补充 |

---

## D.5 签字区

| 角色 | 原始设计 | 交叉审核 | 最终确认 |
|------|----------|----------|----------|
| 🔥 Agent A (激进派) | ✅ 7 用例 | ✅ +4 for C | ✅ 审核完成 |
| 🛡️ Agent B (保守派) | ✅ 6 用例 | ✅ +2 for A&C, +2 全局 | ✅ 审核完成 |
| 🔐 Agent C (安全专家) | ✅ 7 用例 | ✅ +4 for A | ✅ 审核完成 |
| 📝 Leader D (主编) | N/A | N/A | ✅ **仲裁完成** |

---

## D.6 附录：完整测试清单

```
Phase 0x14-c Money Safety - 完整测试清单 (32项)
═══════════════════════════════════════════════════

🔴 P0 Critical (10项)
├── B-TC-001   标准金额转换准确性
├── B-TC-002   往返转换一致性
├── A-TC-001   超精度数值拒绝
├── A-TC-002   数值溢出攻击防护
├── C-TC-001   溢出不导致资金错算
├── C-TC-003   跨路径精度一致性 (升级)
├── A→C-ADD-001 链上精度攻击防护
├── A→C-ADD-003 并发精度一致性
├── C→A-ADD-001 审计日志完整性
└── B→A-ADD-001 边缘测试后系统状态验证

🟡 P1 Important (12项)
├── A-TC-003   畸形格式拒绝
├── A-TC-004   零值边界处理
├── B-TC-004   回归测试通过
├── C-TC-002   无隐藏 dust
├── C-TC-004   注入攻击防护
├── A→C-ADD-002 时间相关精度攻击
├── A→C-ADD-004 负数绕过攻击
├── C→A-ADD-002 速率限制防护
├── C→A-ADD-003 时序侧信道防护
├── C→A-ADD-004 错误消息一致性
├── B→C-ADD-001 合法输入不被误拒
└── B-GLOBAL-001 测试数据隔离

🟢 P2 Nice-to-have (10项)
├── A-TC-005   跨资产精度隔离
├── A-TC-006   显示精度不丢失存储精度
├── A-TC-007   审计脚本绕过检测
├── B-TC-003   SymbolManager 精度获取
├── B-TC-005   API 响应格式一致
├── B-TC-006   迁移文件功能验证
├── C-TC-005   内部表示不泄露
├── C-TC-006   错误消息不泄露敏感信息
├── C-TC-007   审计脚本不可被禁用
└── B-GLOBAL-002 测试顺序无关性
```

---

> **Leader D 批示**: 
> 
> 本测试设计经过 4 位 QA Agent 的共同努力，从 20 项扩展到 32 项测试用例，
> 覆盖边缘场景、核心流程、安全审计三大维度，并通过交叉审核确保无盲点。
> 
> **准入标准**: Developer 交付后，必须 P0 全绿方可进入 P1 验证。
> 
> **签发日期**: 2025-12-31
