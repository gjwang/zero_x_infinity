# 0x0A-a: ID 规范 (Identity Specification)

本文档定义系统中所有标识符（ID）的命名和格式规范。

---

## 1. 核心原则

| 原则 | 说明 |
|------|------|
| **唯一性** | 每个 ID 在其作用域内必须唯一 |
| **可读性** | ID 应具有业务含义，便于理解 |
| **大小写规范** | Asset/Symbol 代码必须全大写 |
| **不可变性** | ID 一旦创建不应修改 |

---

## 2. Asset ID 规范

### 2.1 格式要求

**规则**：
- ✅ **必须全大写**
- ✅ 仅包含：大写字母（A-Z）、数字（0-9）、下划线（_）
- ✅ 长度：1-16 字符
- ✅ 正则表达式：`^[A-Z0-9_]{1,16}$`

> **⚠️ 实际长度限制**：  
> 由于 Symbol 格式为 `BASE_QUOTE`（最大 32 字符），实际使用中：
> - 如果 Asset 用作 BASE，最大长度 = 32 - 1 (下划线) - QUOTE 长度
> - 如果 Asset 用作 QUOTE，最大长度 = 32 - 1 (下划线) - BASE 长度
> - 示例：如果 BASE 是 16 字符，QUOTE 最多只能 15 字符

### 2.2 示例

✅ **正确**：
```
BTC
USDT
ETH
USDC
BTC2
STABLE_COIN
```

❌ **错误**：
```
btc          # 小写
Btc          # 混合大小写
BTC-USD      # 包含连字符
BTC!         # 包含特殊字符
B            # 单字符（允许）
VERYLONGASSETCODE  # 太长（> 16）
```

### 2.3 验证规则

**数据库层**（基本约束）：
```sql
-- 仅强制大写，复杂格式验证在应用层
ALTER TABLE assets_tb 
ADD CONSTRAINT asset_uppercase_check 
CHECK (asset = UPPER(asset));
```

**应用层**（完整验证）：
```rust
// 严格验证：大写 + 格式 + 长度
AssetName::new("BTC")  // ✅ Ok
AssetName::new("btc")  // ❌ Error: must be uppercase
AssetName::new("BTC-USD")  // ❌ Error: invalid character
```

**API 层**（入口验证）：
```json
// 请求
POST /api/v1/assets
{
  "asset": "btc",  // ❌ 错误
  "name": "Bitcoin"
}

// 响应
{
  "code": 400,
  "msg": "Asset name must be uppercase. Got 'btc', expected 'BTC'",
  "data": null
}
```

> **设计原则**：
> - 数据库层：仅基本约束（大写），保持简单
> - 应用层：完整验证（格式、长度、字符集），灵活可扩展
> - API 层：入口验证，清晰错误提示

---

## 3. Symbol Name 规范

### 3.1 格式要求

**规则**：
- ✅ **必须全大写**
- ✅ **必须使用下划线分隔** `BASE_QUOTE` 格式
- ✅ **必须包含且仅包含一个下划线**
- ✅ 仅包含：大写字母（A-Z）、数字（0-9）、下划线（_）
- ✅ 长度：3-33 字符
- ✅ 正则表达式：`^[A-Z0-9]+_[A-Z0-9]+$`

### 3.2 设计理由

**为什么使用 `BASE_QUOTE` 格式？**

我们选择 `BTC_USDT` 而非其他格式的原因：

#### ❌ 问题 1: `BTCUSDT` 格式（无分隔符）

**问题**：无法可靠地分割 BASE 和 QUOTE

```
BTCUSDT   → BTC + USDT? 或 BTCU + SDT? 或 B + TCUSDT?
ETHUSDT   → ETH + USDT ✓
USDTUSDC  → USDT + USDC? 或 USD + TUSDC?
```

**传统股票市场可行的原因**：
- CURRENCY 种类很少（USD, EUR, CNY 等）
- 可以提前枚举所有 CURRENCY
- 通过后缀匹配识别（如 `AAPL` 默认 USD）

**加密货币市场不可行**：
- CURRENCY 种类极多（USDT, USDC, BUSD, DAI, TUSD...）
- 长度不固定（BTC=3, USDT=4, WBTC=4, SHIB=4）
- 无法预先枚举所有可能的 CURRENCY
- 新币种不断出现

#### ❌ 问题 2: `BTC/USDT` 格式（斜杠分隔）

**问题**：在浏览器和编辑器中体验差

```javascript
// 浏览器 URL
/api/v1/symbols/BTC/USDT  // ❌ 会被解析为两个路径段

// 双击选择
BTC/USDT  // ❌ 只会选中 "BTC" 或 "USDT"，不会选中整体
```

**其他问题**：
- 需要 URL 编码：`BTC%2FUSDT`
- 文件名不友好（某些系统不允许 `/`）
- 复制粘贴体验差

#### ✅ 解决方案: `BTC_USDT` 格式（下划线分隔）

**优势**：
1. **明确分割**：`split('_')` 即可分离 BASE 和 QUOTE
2. **URL 友好**：`/api/v1/symbols/BTC_USDT` 无需编码
3. **双击选择**：浏览器中双击可选中整个 symbol
4. **文件名友好**：可直接用作文件名
5. **可读性好**：视觉上清晰分隔

### 3.3 示例

✅ **正确**：
```
BTC_USDT
ETH_BTC
USDT_USD
BNB_BUSD
ETH2_USDT
1000SHIB_USDT
```

❌ **错误**：
```
btc_usdt     # 小写
BTC-USDT     # 使用连字符（非法）
Btc_Usdt     # 混合大小写
BTCUSDT      # 缺少下划线分隔符（非法！）
BTC__USDT    # 双下划线（非法）
_BTCUSDT     # 开头下划线（非法）
BTCUSDT_     # 结尾下划线（非法）
BT           # 太短（< 3）
BTC_USDT_EUR # 多个下划线（非法！仅允许一个）
```

### 3.4 验证规则

**数据库层**（基本约束）：
```sql
-- 仅强制大写，复杂格式验证在应用层
ALTER TABLE symbols_tb 
ADD CONSTRAINT symbol_uppercase_check 
CHECK (symbol = UPPER(symbol));
```

**应用层**（完整验证）：
```rust
// 严格验证：大写 + 格式 + 长度 + 单个下划线
SymbolName::new("BTC_USDT")  // ✅ Ok
SymbolName::new("BTCUSDT")   // ❌ Error: missing underscore separator
SymbolName::new("BTC-USDT")  // ❌ Error: invalid character '-'
SymbolName::new("BTC_USDT_EUR") // ❌ Error: multiple underscores
```

> **设计原则**：
> - 数据库层：仅基本约束（大写），保持简单
> - 应用层：完整验证（格式、长度、字符集），灵活可扩展

---

## 4. User ID 规范

### 4.1 格式要求

**类型**：`BIGINT` (64-bit signed integer)

**范围**：
- 最小值：`1001`（保留 1-1000 用于系统账户）
- 最大值：`9223372036854775807` (2^63 - 1)

### 4.2 生成策略

**推荐**：使用数据库自增序列
```sql
CREATE SEQUENCE user_id_seq START WITH 1001;
```

**禁止**：
- ❌ 使用连续整数（安全风险）
- ❌ 暴露用户总数信息

---

## 5. 验证实施

### 5.1 四层验证

```
┌─────────────────────────────────────┐
│  1. API 层：严格拒绝不规范输入      │
│     - 返回 400 错误                 │
│     - 提供明确错误信息              │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│  2. 应用层：验证函数                │
│     - AssetCode::validate()         │
│     - SymbolCode::validate()        │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│  3. Repository 层：内部检查         │
│     - 确保传入数据已验证            │
└─────────────────────────────────────┘
              ↓
┌─────────────────────────────────────┐
│  4. 数据库层：CHECK 约束            │
│     - 最后一道防线                  │
│     - 保证数据完整性                │
└─────────────────────────────────────┘
```

### 5.2 错误处理

**原则**：
- ✅ **不转换** - 不自动转大写
- ✅ **严格拒绝** - 不符合规范立即报错
- ✅ **清晰提示** - 告知用户正确格式

**示例错误消息**：
```
Asset code must be uppercase and contain only A-Z, 0-9, underscore.
Got: 'btc'
Expected format: 'BTC'
```

---

## 6. 测试要求

### 6.1 单元测试

```rust
#[test]
fn test_asset_code_uppercase_required() {
    assert!(AssetCode::validate("BTC").is_ok());
    assert!(AssetCode::validate("btc").is_err());
    assert!(AssetCode::validate("Btc").is_err());
}

#[test]
fn test_asset_code_invalid_chars() {
    assert!(AssetCode::validate("BTC-USD").is_err());
    assert!(AssetCode::validate("BTC!").is_err());
}

#[test]
fn test_asset_code_length() {
    assert!(AssetCode::validate("B").is_err());      // too short
    assert!(AssetCode::validate("VERYLONGCODE").is_err()); // too long
}
```

### 6.2 集成测试

```rust
#[tokio::test]
async fn test_database_rejects_lowercase() {
    let result = sqlx::query("INSERT INTO assets_tb (asset, name, decimals) VALUES ($1, $2, $3)")
        .bind("btc")  // 小写
        .bind("Bitcoin")
        .bind(8i16)
        .execute(pool)
        .await;
    
    assert!(result.is_err(), "Database should reject lowercase");
}
```

---

## 7. 迁移指南

### 7.1 现有数据清理

```sql
-- 检查不规范数据
SELECT asset_id, asset 
FROM assets_tb 
WHERE asset != UPPER(asset);

-- 清理（如果需要）
UPDATE assets_tb 
SET asset = UPPER(asset) 
WHERE asset != UPPER(asset);
```

### 7.2 添加约束

```sql
-- 添加 CHECK 约束
ALTER TABLE assets_tb 
ADD CONSTRAINT asset_code_uppercase_check 
CHECK (asset = UPPER(asset) AND asset ~ '^[A-Z0-9_]{1,16}$');

ALTER TABLE symbols_tb 
ADD CONSTRAINT symbol_code_uppercase_check 
CHECK (symbol = UPPER(symbol) AND symbol ~ '^[A-Z0-9_]{3,32}$');
```

---

## 8. 常见问题

### Q: 为什么不自动转大写？

**A**: 严格验证有以下好处：
1. **明确规范** - 强制用户遵守规则
2. **避免歧义** - 不存在隐式转换
3. **数据一致** - 100% 保证大写
4. **调试友好** - 错误来源清晰

### Q: 如果用户输入小写怎么办？

**A**: API 返回 400 错误，明确告知正确格式：
```json
{
  "code": 400,
  "msg": "Asset code must be uppercase. Got 'btc', expected 'BTC'"
}
```

### Q: 内部代码需要验证吗？

**A**: 是的。所有层都应验证，确保数据完整性。

---

**最后更新**: 2025-12-22  
**版本**: 1.1  
**状态**: 强制执行
