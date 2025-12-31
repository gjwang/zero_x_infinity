# 🧪 Phase 0x14-c Money Safety: 多视角测试设计

> **Phase**: 0x14-c Money Type Safety
> **状态**: 测试设计阶段 (Developer 未交付)
> **日期**: 2025-12-31
> **设计方法**: 多角色 QA 协作

---

## 📋 测试设计组织架构

| 角色 | 职责 | 关注领域 |
|------|------|----------|
| 🔥 **Agent A (激进派 QA)** | 边缘测试找漏洞 | 溢出、精度极限、畸形输入 |
| 🛡️ **Agent B (保守派 QA)** | 核心流程稳定性 | 回归测试、正常路径验证 |
| 🔐 **Agent C (安全专家 QA)** | 安全问题审计 | 权限绕过、数据泄露、注入攻击 |
| 📝 **Leader (主编)** | 汇总整理 | 优先级排序、测试矩阵 |

---

# 🔥 Agent A: 激进派 QA - 边缘测试设计

## A.1 设计理念

> **"如果系统能在最极端的输入下正常工作，它就能处理任何正常输入。"**

我的目标是找到那些开发者"认为永远不会发生"的场景，然后证明它们会发生。

---

## A.2 测试用例：精度极限攻击

### A-TC-001: 超精度数值 (Precision Overflow)

| 字段 | 值 |
|------|-----|
| **目标** | 验证系统拒绝超过资产精度的输入 |
| **攻击面** | Gateway 订单接口 |

**测试数据矩阵**:

| Asset | Max Decimals | Input | Expected |
|-------|--------------|-------|----------|
| BTC | 8 | `"1.123456789"` (9位) | 400 PRECISION_EXCEEDED |
| BTC | 8 | `"1.00000001"` (8位) | ✅ Accept |
| USDT | 6 | `"1.1234567"` (7位) | 400 PRECISION_EXCEEDED |
| USDT | 6 | `"0.000001"` (6位) | ✅ Accept |
| ETH | 8 | `"0.000000001"` (9位) | 400 PRECISION_EXCEEDED |

**Python 测试框架**:
```python
@pytest.mark.parametrize("asset,input_val,expected", [
    ("BTC", "1.123456789", 400),      # 9位 > 8位
    ("BTC", "1.12345678", 200),       # 8位 = 8位 ✅
    ("BTC", "0.000000001", 400),      # 9位小数
    ("USDT", "1.1234567", 400),       # 7位 > 6位
    ("USDT", "100.123456", 200),      # 6位 = 6位 ✅
])
def test_precision_boundary(asset, input_val, expected):
    resp = place_order(quantity=input_val, symbol=f"{asset}USDT")
    assert resp.status_code == expected
```

---

### A-TC-002: 数值溢出攻击 (Integer Overflow)

| 字段 | 值 |
|------|-----|
| **目标** | 验证系统检测并拒绝导致 u64 溢出的输入 |
| **风险等级** | 🔴 Critical |

**攻击向量**:

| 场景 | Input | 内部计算 | 风险 |
|------|-------|----------|------|
| 直接溢出 | `"184467440737.09551616"` | > u64::MAX | 系统崩溃 |
| 乘法溢出 | qty=`"1000000000"`, price=`"1000000000"` | qty*price overflow | 资金错算 |
| 精度放大溢出 | `"184467440737"` * 10^8 | 溢出 | 静默截断 |

**测试数据**:
```python
OVERFLOW_CASES = [
    # u64::MAX = 18_446_744_073_709_551_615
    ("18446744073709551616", 400, "AMOUNT_OVERFLOW"),  # u64::MAX + 1
    ("18446744073709551615", 400, "AMOUNT_OVERFLOW"),  # u64::MAX (缩放后溢出)
    ("184467440737.09551616", 400, "AMOUNT_OVERFLOW"), # 缩放后 = u64::MAX + 1
    ("999999999999999999", 400, "AMOUNT_OVERFLOW"),    # 明显超大
    ("1" + "0" * 30, 400, "AMOUNT_OVERFLOW"),          # 10^30
]

@pytest.mark.parametrize("qty,expected_code,expected_error", OVERFLOW_CASES)
def test_overflow_rejection(qty, expected_code, expected_error):
    resp = place_order(quantity=qty)
    assert resp.status_code == expected_code
    assert expected_error in resp.json().get("error", "")
```

---

### A-TC-003: 畸形格式攻击 (Malformed Input)

| 字段 | 值 |
|------|-----|
| **目标** | 验证系统拒绝非标准数字格式 |
| **设计依据** | `money-type-safety.md` 3.2节: 严格解析规范 |

**测试矩阵**:

| Input | Category | Expected | 备注 |
|-------|----------|----------|------|
| `.5` | 缺少整数部分 | 400 INVALID_FORMAT | 必须是 `0.5` |
| `5.` | 缺少小数部分 | 400 INVALID_FORMAT | 必须是 `5.0` |
| `1,000.00` | 千分位分隔符 | 400 INVALID_FORMAT | 不接受逗号 |
| `1 000.00` | 空格分隔 | 400 INVALID_FORMAT | 不接受空格 |
| `+1.5` | 显式正号 | 400 or 200? | **待确认** |
| `1.5e8` | 科学计数法 | 400 INVALID_FORMAT | 不接受科学计数法 |
| ` 1.5` | 前导空格 | 400 INVALID_FORMAT | 不接受前后空格 |
| `1.5 ` | 尾随空格 | 400 INVALID_FORMAT | 不接受前后空格 |
| `""` | 空字符串 | 400 INVALID_FORMAT | 必须非空 |
| `null` | JSON null | 400 MISSING_FIELD | 必填字段 |
| `1.5.0` | 多个小数点 | 400 INVALID_FORMAT | 非法格式 |
| `0x1F` | 十六进制 | 400 INVALID_FORMAT | 仅接受十进制 |
| `Infinity` | 特殊值 | 400 INVALID_FORMAT | 非法 |
| `NaN` | 特殊值 | 400 INVALID_FORMAT | 非法 |

```python
MALFORMED_INPUTS = [
    (".5", "INVALID_FORMAT"),
    ("5.", "INVALID_FORMAT"),
    ("1,000.00", "INVALID_FORMAT"),
    ("1 000", "INVALID_FORMAT"),
    ("1.5e8", "INVALID_FORMAT"),
    (" 1.5", "INVALID_FORMAT"),
    ("1.5 ", "INVALID_FORMAT"),
    ("", "INVALID_FORMAT"),
    ("1.5.0", "INVALID_FORMAT"),
    ("0x1F", "INVALID_FORMAT"),
    ("Infinity", "INVALID_FORMAT"),
    ("NaN", "INVALID_FORMAT"),
    ("-0", "ZERO_NOT_ALLOWED"),  # 负零？
]

@pytest.mark.parametrize("input_val,expected_error", MALFORMED_INPUTS)
def test_malformed_input_rejection(input_val, expected_error):
    resp = place_order(quantity=input_val)
    assert resp.status_code == 400
    assert expected_error in resp.json().get("code", "")
```

---

### A-TC-004: 零值边界攻击 (Zero Value Edge Cases)

| 字段 | 值 |
|------|-----|
| **目标** | 验证零值在不同上下文中的处理 |
| **设计依据** | `money-type-safety.md` 3.3节: 默认严格 + 显式入口 |

**测试矩阵**:

| 场景 | Input | Field | Expected | 原因 |
|------|-------|-------|----------|------|
| 订单数量 | `"0"` | quantity | 400 ZERO_NOT_ALLOWED | 数量必须非零 |
| 订单价格 | `"0"` | price | 400 ZERO_NOT_ALLOWED | 价格必须非零 |
| 提现手续费 | `"0"` | fee | ✅ 200 OK | 手续费可为零 |
| 极小非零 | `"0.00000001"` | quantity | ✅ 200 OK | 最小有效值 |
| 负零 | `"-0"` | quantity | 400 | 负数或零? |
| 伪零 | `"0.00000000"` | quantity | 400 ZERO_NOT_ALLOWED | 等效于零 |

```python
def test_zero_quantity_rejected():
    """订单数量零值必须被拒绝"""
    resp = place_order(quantity="0")
    assert resp.status_code == 400
    assert "ZERO_NOT_ALLOWED" in resp.json()["code"]

def test_zero_fee_accepted():
    """提现手续费零值应被接受"""
    resp = withdraw(amount="100", fee="0")
    assert resp.status_code == 200

def test_minimum_quantity_accepted():
    """最小非零值应被接受"""
    resp = place_order(quantity="0.00000001")  # 1 satoshi
    assert resp.status_code == 200
```

---

### A-TC-005: 跨资产精度混淆攻击

| 字段 | 值 |
|------|-----|
| **目标** | 验证不同资产的精度隔离 |
| **风险** | 使用 BTC 精度处理 USDT 金额可能导致资金错算 |

**攻击场景**:

| Symbol | Base | Quote | Base Decimals | Quote Decimals | Input | 风险 |
|--------|------|-------|---------------|----------------|-------|------|
| BTCUSDT | BTC | USDT | 8 | 6 | qty=`"1.12345678"` | 正常 |
| BTCUSDT | BTC | USDT | 8 | 6 | price=`"50000.123456"` | 6位OK |
| BTCUSDT | BTC | USDT | 8 | 6 | price=`"50000.1234567"` | 7位应拒绝 |
| ETHBTC | ETH | BTC | 8 | 8 | price=`"0.12345678"` | 8位OK |

```python
def test_cross_asset_precision_isolation():
    """验证 base 和 quote 精度独立验证"""
    # BTCUSDT: BTC(8位) / USDT(6位)
    
    # Base 精度验证
    resp = place_order(symbol="BTCUSDT", quantity="1.123456789")  # 9位
    assert resp.status_code == 400
    
    # Quote 精度验证
    resp = place_order(symbol="BTCUSDT", price="50000.1234567")  # 7位
    assert resp.status_code == 400
    
    # 正确精度
    resp = place_order(symbol="BTCUSDT", quantity="1.12345678", price="50000.123456")
    assert resp.status_code == 200
```

---

### A-TC-006: 显示精度 vs 存储精度攻击

| 字段 | 值 |
|------|-----|
| **目标** | 验证系统不会因显示精度截断导致资金损失 |
| **设计依据** | `money-type-safety.md` 关于截断策略 |

**场景**: 
- 存储精度: 8位
- 显示精度: 4位  
- 输入: `"1.12345678"` (8位)
- 存储: `112345678` (正确)
- 显示: `"1.1234"` (截断显示)
- **风险**: 如果 Response 用截断值覆盖，会丢失精度

```python
def test_display_truncation_does_not_lose_funds():
    """验证显示截断不影响存储精度"""
    # 下单 1.12345678 BTC
    order = place_order(quantity="1.12345678")
    order_id = order.json()["orderId"]
    
    # 查询订单详情
    detail = get_order(order_id)
    
    # 验证原始精度保留
    assert detail.json()["quantity"] == "1.12345678"  # 不是 "1.1234"
```

---

## A.3 CI 审计脚本测试

### A-TC-007: 绕过审计脚本攻击

| 字段 | 值 |
|------|-----|
| **目标** | 验证审计脚本无法被绕过 |

**绕过手法矩阵**:

| 手法 | 示例代码 | 应被检测? |
|------|----------|-----------|
| 直接使用 | `10u64.pow(8)` | ✅ |
| 变量替换 | `let n=10u64; n.pow(8)` | ❓ 可能漏检 |
| 宏展开 | `pow_ten!(8)` | ❓ 需要宏检测 |
| 常量定义 | `const SCALE: u64 = 100000000;` | ❓ 需要检测 |
| 注释伪装 | `// 10u64.pow(8)` 换行后 `10u64.pow(8)` | ❓ |
| 字符串拼接 | `"10u64" + ".pow(8)"` | N/A (编译不过) |

```bash
# 测试审计脚本检测能力
echo "Testing audit script bypass..."

# 创建测试文件
cat > /tmp/test_bypass.rs << 'EOF'
// 手法1: 直接使用
let x = 10u64.pow(8);

// 手法2: 变量替换
let n = 10u64;
let y = n.pow(8);

// 手法3: 常量
const SCALE: u64 = 100_000_000;
EOF

# 运行审计
./scripts/audit_money_safety.sh /tmp/test_bypass.rs
```

---

# 🛡️ Agent B: 保守派 QA - 核心流程验证

## B.1 设计理念

> **"确保系统在标准场景下 100% 可靠，是所有测试的基石。"**

我的目标是验证正常业务流程的稳定性，确保边缘测试不会破坏核心功能。

---

## B.2 测试用例：核心转换正确性

### B-TC-001: 标准金额转换准确性

| 字段 | 值 |
|------|-----|
| **目标** | 验证标准输入的转换精确无误 |
| **优先级** | P0 |

**黄金测试数据**:

| Input (String) | Asset | Decimals | Expected (u64) | 验证方式 |
|----------------|-------|----------|----------------|----------|
| `"1.0"` | BTC | 8 | `100_000_000` | 数学验证 |
| `"0.00000001"` | BTC | 8 | `1` | 最小单位 |
| `"21000000.0"` | BTC | 8 | `2_100_000_000_000_000` | BTC总量 |
| `"100.0"` | USDT | 6 | `100_000_000` | 标准金额 |
| `"0.000001"` | USDT | 6 | `1` | 最小单位 |
| `"50000.00"` | USDT | 6 | `50_000_000_000` | 标准价格 |

```rust
#[test]
fn test_standard_conversion_accuracy() {
    let cases = [
        ("1.0", 8, 100_000_000u64),
        ("0.00000001", 8, 1u64),
        ("21000000.0", 8, 2_100_000_000_000_000u64),
        ("100.0", 6, 100_000_000u64),
        ("0.000001", 6, 1u64),
    ];
    
    for (input, decimals, expected) in cases {
        let result = money::parse_decimal(input, decimals).unwrap();
        assert_eq!(*result, expected, "Failed for input: {}", input);
    }
}
```

---

### B-TC-002: 往返转换一致性 (Round-trip)

| 字段 | 值 |
|------|-----|
| **目标** | 验证 parse → format → parse 结果一致 |
| **定律** | `parse(format(parse(x))) == parse(x)` |

```rust
#[test]
fn test_roundtrip_consistency() {
    let test_values = [
        "1.5", "0.00000001", "100.123456", "99999.99999999",
    ];
    
    for original in test_values {
        let parsed1 = money::parse_decimal(original, 8).unwrap();
        let formatted = money::format_amount(*parsed1, 8, 8);
        let parsed2 = money::parse_decimal(&formatted, 8).unwrap();
        
        assert_eq!(parsed1, parsed2, "Round-trip failed for: {}", original);
    }
}
```

---

### B-TC-003: SymbolManager 精度获取

| 字段 | 值 |
|------|-----|
| **目标** | 验证 SymbolManager 返回正确的精度配置 |

```rust
#[test]
fn test_symbol_manager_decimals() {
    let mgr = SymbolManager::new_from_db().unwrap();
    
    // BTC: 8位精度
    let btc_decimals = mgr.get_asset_decimals("BTC").unwrap();
    assert_eq!(btc_decimals, 8);
    
    // USDT: 6位精度
    let usdt_decimals = mgr.get_asset_decimals("USDT").unwrap();
    assert_eq!(usdt_decimals, 6);
    
    // BTCUSDT 交易对
    let symbol = mgr.get_symbol_info("BTCUSDT").unwrap();
    assert_eq!(symbol.base_decimals, 8);
    assert_eq!(symbol.quote_decimals, 6);
}
```

---

## B.3 回归测试

### B-TC-004: 现有功能回归验证

| 字段 | 值 |
|------|-----|
| **目标** | 确保 Money Safety 改造不破坏现有功能 |
| **方法** | 运行全量测试套件 |

```bash
#!/bin/bash
# 回归测试脚本

echo "🧪 Running Regression Tests..."

# 1. 单元测试
cargo test --lib
if [ $? -ne 0 ]; then
    echo "❌ Unit tests failed"
    exit 1
fi

# 2. 集成测试
cargo test --test '*'
if [ $? -ne 0 ]; then
    echo "❌ Integration tests failed"
    exit 1
fi

# 3. Money 模块专项
cargo test money::
if [ $? -ne 0 ]; then
    echo "❌ Money module tests failed"
    exit 1
fi

# 4. 订单 API 回归
python3 scripts/tests/test_order_api.py
if [ $? -ne 0 ]; then
    echo "❌ Order API tests failed"
    exit 1
fi

echo "✅ All regression tests passed!"
```

---

### B-TC-005: API 响应格式一致性

| 字段 | 值 |
|------|-----|
| **目标** | 验证 API 响应中的金额格式符合规范 |

**验证规则**:
- 金额字段必须是 String 类型
- 格式必须是标准十进制 (如 `"1.50000000"`)
- 尾部零不能省略 (确保精度可见)

```python
def test_api_response_format():
    """验证 API 响应格式"""
    # 下单
    order = place_order(quantity="1.5", price="50000.0")
    data = order.json()
    
    # 验证字段是字符串
    assert isinstance(data["quantity"], str)
    assert isinstance(data["price"], str)
    
    # 验证格式正确 (8位小数)
    assert data["quantity"] == "1.50000000"
    assert data["price"] == "50000.000000"  # 6位
    
    # 余额查询
    balance = get_balances()
    for item in balance.json():
        assert isinstance(item["available"], str)
        assert isinstance(item["locked"], str)
```

---

## B.4 存量代码迁移验证

### B-TC-006: 迁移文件功能验证

| 字段 | 值 |
|------|-----|
| **目标** | 验证迁移后的文件功能正常 |

**迁移清单与验证**:

| File | 验证方法 |
|------|----------|
| `persistence/queries.rs` | 运行 `cargo test persistence::` |
| `sentinel/eth.rs` | ETH 存款检测 E2E 测试 |
| `models.rs` | 运行 `cargo test models::` |
| `csv_io.rs` | CSV 导入导出测试 |
| `websocket/service.rs` | WebSocket 深度推送测试 |

```bash
#!/bin/bash
# 迁移验证脚本

FILES_TO_VERIFY=(
    "persistence::queries"
    "sentinel::eth"
    "models"
    "csv_io"
    "websocket::service"
)

for module in "${FILES_TO_VERIFY[@]}"; do
    echo "Testing $module..."
    cargo test "$module::" --lib
    if [ $? -ne 0 ]; then
        echo "❌ $module tests failed after migration"
        exit 1
    fi
done

echo "✅ All migrated modules verified!"
```

---

# 🔐 Agent C: 安全专家 QA - 安全审计

## C.1 设计理念

> **"在金融系统中，安全漏洞就是资金损失。"**

我的目标是识别可能导致资金损失、数据泄露或权限绕过的安全风险。

---

## C.2 测试用例：溢出攻击防护

### C-TC-001: 整数溢出导致资金错算

| 字段 | 值 |
|------|-----|
| **威胁等级** | 🔴 Critical |
| **攻击目标** | 通过溢出使大额变小额或小额变大额 |

**攻击向量**:

```
正常: qty=1000, price=50000 → turnover = 50,000,000
攻击: 如果 qty * price 溢出并回绕 → turnover 可能变成很小的数
```

**测试用例**:

```python
def test_overflow_does_not_cause_fund_miscalculation():
    """验证溢出不会导致资金错算"""
    
    # 尝试构造溢出
    large_qty = "18446744073"  # 接近 u64::MAX / 10^8
    large_price = "1000000"
    
    # 应该被拒绝，而不是溢出后接受
    resp = place_order(quantity=large_qty, price=large_price)
    
    # 如果接受了，检查是否有异常
    if resp.status_code == 200:
        order = resp.json()
        # 验证成交金额计算正确
        assert order["turnover"] == expected_turnover(large_qty, large_price)
    else:
        # 应该是溢出错误
        assert resp.status_code == 400
        assert "OVERFLOW" in resp.json().get("code", "")
```

---

### C-TC-002: 精度攻击导致 Dust 残留

| 字段 | 值 |
|------|-----|
| **威胁等级** | 🟡 Medium |
| **攻击目标** | 通过精度差异制造无法提取的 dust 余额 |

**攻击场景**:
1. 用户存入极小金额 (低于最小显示精度)
2. 系统接受但无法显示完整余额
3. 用户无法提取这些 "隐藏" 资金

```python
def test_no_hidden_dust():
    """验证不存在隐藏的 dust 余额"""
    
    # 存入最小单位
    deposit(amount="0.00000001")  # 1 satoshi
    
    # 查询余额
    balance = get_balance("BTC")
    
    # 验证显示完整
    assert balance["available"] == "0.00000001"
    
    # 验证可以全额提取
    withdraw(amount="0.00000001")
    balance_after = get_balance("BTC")
    assert balance_after["available"] == "0.00000000"
```

---

### C-TC-003: 不一致精度导致套利

| 字段 | 值 |
|------|-----|
| **威胁等级** | 🔴 Critical |
| **攻击目标** | 利用不同路径的精度差异套利 |

**攻击场景**:
```
路径A: 存款 1.999999999 BTC → 截断存储 1.99999999 (8位)
路径B: API 显示 1.99999999 → 用户认为有 1.99999999
路径C: 内部计算时使用 2.0 (四舍五入) → 多算 0.00000001
```

```python
def test_consistent_precision_across_paths():
    """验证所有路径使用一致的精度"""
    
    # 存款
    deposit_resp = deposit("1.99999999")
    deposit_amount = deposit_resp.json()["amount"]
    
    # API 查询
    balance = get_balance("BTC")["available"]
    
    # 内部划转
    transfer("1.99999999", from_account="spot", to_account="funding")
    funding_balance = get_funding_balance("BTC")["available"]
    
    # 所有路径必须完全一致
    assert deposit_amount == balance == funding_balance == "1.99999999"
```

---

## C.3 注入攻击防护

### C-TC-004: 金额字段注入攻击

| 字段 | 值 |
|------|-----|
| **威胁等级** | 🟡 Medium |
| **攻击目标** | 通过金额字段注入恶意内容 |

**攻击向量**:

| 注入类型 | Payload | 目标 |
|----------|---------|------|
| SQL 注入 | `"1.0; DROP TABLE orders--"` | 数据库 |
| JSON 注入 | `"1.0\", \"admin\": true"` | 权限绕过 |
| 日志注入 | `"1.0\nINFO: admin login"` | 日志伪造 |
| 路径遍历 | `"../../etc/passwd"` | 文件访问 |

```python
INJECTION_PAYLOADS = [
    '1.0; DROP TABLE orders--',
    '1.0", "admin": true',
    '1.0\nINFO: admin_login',
    '../../etc/passwd',
    '<script>alert(1)</script>',
    '${7*7}',
    '{{7*7}}',
]

@pytest.mark.parametrize("payload", INJECTION_PAYLOADS)
def test_injection_resistance(payload):
    """验证金额字段不接受注入 payload"""
    resp = place_order(quantity=payload)
    
    # 必须被 400 拒绝
    assert resp.status_code == 400
    
    # 验证 payload 不在响应中回显 (防止 XSS)
    assert payload not in resp.text
```

---

## C.4 信息泄露防护

### C-TC-005: 内部精度不暴露给客户端

| 字段 | 值 |
|------|-----|
| **威胁等级** | 🟢 Low |
| **目标** | 验证内部 u64 表示不会泄露 |

```python
def test_internal_representation_not_exposed():
    """验证内部 u64 表示不会泄露给客户端"""
    
    # 下单
    order = place_order(quantity="1.5")
    data = order.json()
    
    # quantity 必须是 String，不是 Number
    raw_response = requests.get(f"/orders/{data['orderId']}").text
    
    # 不应该包含内部的 150000000
    assert "150000000" not in raw_response
    
    # 应该包含格式化的 "1.50000000"
    assert "1.50000000" in raw_response or "1.5" in raw_response
```

---

### C-TC-006: 错误消息不泄露敏感信息

| 字段 | 值 |
|------|-----|
| **威胁等级** | 🟡 Medium |
| **目标** | 验证错误消息不包含堆栈跟踪或内部细节 |

```python
def test_error_message_safe():
    """验证错误消息不泄露内部信息"""
    
    # 触发错误
    resp = place_order(quantity="invalid")
    error = resp.json()
    
    # 不应包含敏感信息
    forbidden_patterns = [
        "stack trace",
        "panic",
        "src/",
        "line ",
        ".rs:",
        "RUST_BACKTRACE",
        "postgres",
        "connection",
    ]
    
    for pattern in forbidden_patterns:
        assert pattern.lower() not in str(error).lower(), \
            f"Error message contains sensitive pattern: {pattern}"
```

---

## C.5 CI 审计安全性

### C-TC-007: 审计脚本不可被禁用

| 字段 | 值 |
|------|-----|
| **威胁等级** | 🟡 Medium |
| **目标** | 验证 CI 中的审计步骤不能被跳过 |

**验证项**:
- [ ] 审计脚本在 CI 必选步骤中
- [ ] 审计失败会阻止合并
- [ ] 无法通过 `[skip ci]` 绕过审计
- [ ] PR 必须通过审计才能合并

---

# 📝 Leader: 主编汇总

## L.1 测试矩阵总览

| 分类 | Agent | 测试数 | 优先级分布 |
|------|-------|--------|------------|
| 边缘测试 | A (激进派) | 7 | P0:3, P1:2, P2:2 |
| 核心验证 | B (保守派) | 6 | P0:4, P1:2 |
| 安全审计 | C (安全专家) | 7 | Critical:3, Medium:3, Low:1 |
| **总计** | | **20** | |

---

## L.2 优先级排序

### 🔴 P0 - 必须通过 (阻塞发布)

| ID | 测试用例 | Owner |
|----|----------|-------|
| B-TC-001 | 标准金额转换准确性 | Agent B |
| B-TC-002 | 往返转换一致性 | Agent B |
| A-TC-001 | 超精度数值拒绝 | Agent A |
| A-TC-002 | 数值溢出攻击防护 | Agent A |
| C-TC-001 | 溢出不导致资金错算 | Agent C |
| C-TC-003 | 跨路径精度一致性 | Agent C |

### 🟡 P1 - 应该通过 (关键功能)

| ID | 测试用例 | Owner |
|----|----------|-------|
| A-TC-003 | 畸形格式拒绝 | Agent A |
| A-TC-004 | 零值边界处理 | Agent A |
| B-TC-004 | 回归测试通过 | Agent B |
| C-TC-002 | 无隐藏 dust | Agent C |
| C-TC-004 | 注入攻击防护 | Agent C |

### 🟢 P2 - 建议通过 (完善性)

| ID | 测试用例 | Owner |
|----|----------|-------|
| A-TC-005 | 跨资产精度隔离 | Agent A |
| A-TC-006 | 显示精度不丢失存储精度 | Agent A |
| B-TC-005 | API 响应格式一致 | Agent B |
| C-TC-005 | 内部表示不泄露 | Agent C |

---

## L.3 执行计划

```
Phase 1: 环境准备
├── [ ] 确认审计脚本存在
├── [ ] 确认 Gateway 可启动
└── [ ] 准备测试数据

Phase 2: P0 测试执行
├── [ ] 运行所有 P0 测试
├── [ ] 记录失败用例
└── [ ] 生成初始报告

Phase 3: P1/P2 测试执行
├── [ ] 运行 P1 测试
├── [ ] 运行 P2 测试
└── [ ] 汇总所有结果

Phase 4: 报告生成
├── [ ] 生成 Defect Report
├── [ ] 生成 Coverage Report
└── [ ] 提交给 Developer
```

---

## L.4 测试脚本模板

```python
#!/usr/bin/env python3
"""
0x14-c Money Safety QA Test Suite
Generated from multi-agent test design
"""

import pytest
import requests
from lib.api_client import APIClient

# 测试配置
BASE_URL = "http://localhost:8080"
client = APIClient(BASE_URL)

#
# ============ Agent A: 边缘测试 ============
#
class TestEdgeCases:
    """激进派 QA 边缘测试"""
    
    @pytest.mark.p0
    def test_precision_overflow(self):
        """A-TC-001"""
        ...
    
    @pytest.mark.p0
    def test_integer_overflow(self):
        """A-TC-002"""
        ...

#
# ============ Agent B: 核心验证 ============
#
class TestCoreFlow:
    """保守派 QA 核心流程"""
    
    @pytest.mark.p0
    def test_standard_conversion(self):
        """B-TC-001"""
        ...

#
# ============ Agent C: 安全审计 ============
#
class TestSecurity:
    """安全专家 QA 审计"""
    
    @pytest.mark.critical
    def test_overflow_fund_safety(self):
        """C-TC-001"""
        ...


if __name__ == "__main__":
    pytest.main([__file__, "-v", "--tb=short"])
```

---

## L.5 准入标准 (Definition of Ready)

Developer 交付后，QA 开始执行测试的前提:

- [ ] `scripts/audit_money_safety.sh` 脚本存在且可执行
- [ ] CI workflow 已更新包含审计步骤
- [ ] 所有 `10u64.pow` 违规已修复或加入白名单
- [ ] `cargo test` 全量通过 (370+)
- [ ] Gateway 可正常启动并响应请求

---

## L.6 签字区

| 角色 | 签名 | 日期 |
|------|------|------|
| Agent A (激进派) | ✅ 测试设计完成 | 2025-12-31 |
| Agent B (保守派) | ✅ 测试设计完成 | 2025-12-31 |
| Agent C (安全专家) | ✅ 测试设计完成 | 2025-12-31 |
| Leader (主编) | ✅ 汇总审核完成 | 2025-12-31 |
