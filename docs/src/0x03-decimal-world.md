# 0x03: Decimal World

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📦 Code Changes**: [View Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.2-the-curse-of-float...v0.3-decimal-world)

In the previous chapter, we refactored all `f64` to `u64`, solving the floating-point precision issues. But this introduced a new problem: **Clients use decimals, while we use integers internally. How do we convert between them?**

### 1. The Decimal Conversion Problem

When a user places an order, the input price might be `"100.50"` and quantity `"10.5"`. However, our engine uses `u64` integers:

```rust
pub struct Order {
    pub id: u64,
    pub price: u64,   // Integer representation
    pub qty: u64,     // Integer representation
    pub side: Side,
}
```

**Core Question**: How to perform lossless conversion between decimal strings and u64?

The answer is the **Fixed Decimal** scheme:

```rust
/// Convert decimal string to u64
/// e.g., "100.50" with 2 decimals -> 10050
fn parse_decimal(s: &str, decimals: u32) -> u64 {
    let multiplier = 10u64.pow(decimals);
    // ... Parsing Logic
}

/// Convert u64 back to decimal string for display
/// e.g., 10050 with 2 decimals -> "100.50"
fn format_decimal(value: u64, decimals: u32) -> String {
    let multiplier = 10u64.pow(decimals);
    let int_part = value / multiplier;
    let dec_part = value % multiplier;
    format!("{}.{:0>width$}", int_part, dec_part, width = decimals as usize)
}
```

### 2. The u64 Max Value (Range Analysis)

The maximum value of `u64` is:

```text
u64::MAX = 18,446,744,073,709,551,615
```

If we use **8 decimal places** (similar to Bitcoin's satoshi), the maximum representable value is:

```text
184,467,440,737.09551615
```

This means:
*   For Price: We can represent up to ~**184 Billion**. (If Bitcoin hits this price, we'll upgrade...)
*   For Quantity: It can hold the entire total supply of BTC (21 million).

#### Decimals Configuration for Different Assets

Different blockchain assets have different native precisions:

| Asset | Native Decimals | Smallest Unit |
|-------|----------------|---------|
| BTC | 8 | 1 satoshi = 0.00000001 BTC |
| USDT (ERC20) | 6 | 0.000001 USDT |
| ETH | 18 | 1 wei = 0.000000000000000001 ETH |

**The Question**: ETH natively uses 18 decimals. Will we lose precision if we use only 8?

The answer is: **It is sufficient for an Exchange**. Because:
*   With 8 decimals, the smallest supported unit is `0.00000001 ETH`.
*   There's no real need to trade `0.000000000000000001 ETH` (value ≈ $0.000000000000003).

So we can choose a **reasonable internal precision**, not necessarily identical to the native chain.

Thus, we need a **SymbolManager** to manage:
*   Internal precision (`decimals`) for each asset.
*   User display precision (`display_decimals`).
*   Price precision configuration for trading pairs.
*   Conversion between on-chain and internal precision during Deposit/Withdrawal.

#### ETH Decimals Analysis: 8 vs 12 bits

Let's analyze the maximum ETH amount representable by `u64` under different decimal configs:

| Decimals | Multiplier | Max Value by u64 | Sufficient? |
|----------|-----|-------------------|-------|
| 8 | 10^8 | **184,467,440,737 ETH** | ✅ Huge margin |
| 9 | 10^9 | **18,446,744,073 ETH** | ✅ Huge margin |
| 10 | 10^10 | **1,844,674,407 ETH** | ✅ > Total Supply |
| 11 | 10^11 | **184,467,440 ETH** | ✅ Just enough (~120M) |
| 12 | 10^12 | **18,446,744 ETH** | ❌ < Total Supply! |
| 18 | 10^18 | **18.44 ETH** | ❌ Absolutely not enough |

> ETH Total Supply ≈ **120 Million ETH**

**Why we chose 8 decimals for ETH?**

*   `0.00000001 ETH` ≈ `$0.00000003`, far below any meaningful trade size.
*   Max capacity 184 Billion ETH > Total Supply (120M).
*   Just convert precision during Deposit/Withdrawal.

**Configuration Example**:

```rust
// BTC: 8 decimals (Same as satoshi)
manager.add_asset(1, 8, 3, "BTC");

// USDT: 8 decimals (Native is 6, we align to 8 internally)
manager.add_asset(2, 8, 2, "USDT");

// ETH: 8 decimals (Safe range, sufficient precision)
manager.add_asset(3, 8, 4, "ETH");
```

### 3. Symbol Configuration

Different trading pairs have different precision requirements:

| Symbol | Price Decimals | Qty Display Decimals | Example |
|--------|---------------|---------------------|---------|
| BTC_USDT | 2 | 3 | Buy 0.001 BTC @ $65000.00 |
| ETH_USDT | 2 | 4 | Buy 0.0001 ETH @ $3500.00 |
| DOGE_USDT | 6 | 0 | Buy 100 DOGE @ $0.123456 |

We use `SymbolManager` to manage these configs:

```rust
#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub symbol: String,
    pub symbol_id: u32,
    pub base_asset_id: u32,
    pub quote_asset_id: u32,
    pub price_decimal: u32,         // Decimals for Price
    pub price_display_decimal: u32, // Display decimals for Price
}

#[derive(Debug, Clone)]
pub struct AssetInfo {
    pub asset_id: u32,
    pub decimals: u32,         // Internal precision (usually 8)
    pub display_decimals: u32, // Max decimals for input/display
    pub name: String,
}
```

### 4. decimals vs display_decimals

Distinguishing these two concepts is crucial:

#### `decimals` (Internal Precision)
*   Determines the multiplier for `u64`.
*   Usually **8** (like satoshi).
*   This is internal storage format, invisible to users.

#### `display_decimals` (Display Precision)
*   Determines how many decimal places users can see/input.
*   E.g., BTC displays 3 digits: `0.001 BTC`.
*   USDT displays 2 digits: `100.00 USDT`.

**Why separate them?**
1.  **UX**: Users don't need to see 8 decimal places.
2.  **Validation**: Limit user input precision.
3.  **Cleanliness**: Avoid trailing zeros.

### 5. Program Output

Output after `cargo run`:

```text
--- 0xInfinity: Stage 3 (Decimal World) ---
Symbol: BTC_USDT (ID: 0)
Price Decimals: 2, Qty Display Decimals: 3

[1] Makers coming in...
    Order 1: Sell 10.000 BTC @ $100.00
    Order 2: Sell 5.000 BTC @ $102.00
    Order 3: Sell 5.000 BTC @ $101.00

[2] Taker eats liquidity...
    Order 4: Buy 12.000 BTC @ $101.50
MATCH: Buy 4 eats Sell 1 @ Price 10000 (Qty: 10000)
MATCH: Buy 4 eats Sell 3 @ Price 10100 (Qty: 2000)

[3] More makers...
    Order 5: Buy 10.000 BTC @ $99.00

--- End of Simulation ---

--- u64 Range Demo ---
u64::MAX = 18446744073709551615
With 8 decimals, max representable value = 184467440737.09551615
```

Observation:
*   User input is decimal string `"100.00"`.
*   Internal storage is integer `10000`.
*   Display converts back to `"100.00"`.

This is the core of **Decimal World**: **Seamless lossless conversion between Decimal Strings and u64 Integers**.

### 📖 True Story: JavaScript Number Overflow

During development, we encountered a bizarre bug:

> **Symptom**: The backend returned raw ETH amount (in wei). During testing with small amounts (0.00x ETH), frontend worked fine. But once the amount hit ~**0.009 ETH**, the number started losing precision and **became incorrect**!

**Root Cause**: JavaScript's `Number` type uses IEEE 754 double-precision floats. The maximum safe integer is `2^53 - 1`:

```javascript
> console.log(Number.MAX_SAFE_INTEGER);
9007199254740991                          // ~ 9 * 10^15

// 1 ETH = 10^18 wei
> const oneEthInWei = 1000000000000000000;

// The Issue: When wei amount exceeds MAX_SAFE_INTEGER
> const smallAmount = 1000000000000000;     // 0.001 ETH = 10^15 wei ✅ Safe
> const dangerAmount = 9007199254740992;    // ~ 0.009 ETH ⚠️ Just exceeded limit!
> const tenEthInWei = 10000000000000000000; // 10 ETH = 10^19 wei ❌ Overflow!

// Verify Precision Loss: Adding 1 has no effect!
> console.log(tenEthInWei + 1);
10000000000000000000                       // No +1!
> console.log(tenEthInWei === tenEthInWei + 1);
true                                       // 😱 WHAT?!
```

**Why ~0.009 ETH?**

```javascript
> console.log(Number.MAX_SAFE_INTEGER / 1e18);
0.009007199254740991                       // 0.009 ETH is the safety limit!
```

**Solution**:

```javascript
// ✅ Solution 1: Backend returns String, Frontend uses BigInt
> const weiString = "10000000000000000000";  // String from backend
> const weiBigInt = BigInt(weiString);       // Convert to BigInt
> console.log((weiBigInt + 1n).toString());
10000000000000000001                       // ✅ Correct!

// ✅ Solution 2: Use libraries like ethers.js
// import { formatEther, parseEther } from 'ethers';
// const eth = formatEther(weiBigInt);  // "10.0"
```

### Summary

This chapter solved:

1.  ✅ **Decimal Conversion**: `parse_decimal()` and `format_decimal()` for bidirectional lossless conversion.
2.  ✅ **u64 Range**: Max value 184 Billion (at 8 decimals), sufficient for any financial scenario.
3.  ✅ **Symbol Config**: `SymbolManager` handles precision settings per pair.
4.  ✅ **Precision Definitions**: Distinct `decimals` (internal) vs `display_decimals` (UI).

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📦 代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.2-the-curse-of-float...v0.3-decimal-world)

在上一章中，我们将所有的 `f64` 重构为 `u64`，解决了浮点数的精度问题。但这引入了一个新的问题：**客户端使用的是十进制，而我们内部使用的是整数，如何进行转换？**

### 1. 十进制转换问题 (The Decimal Conversion Problem)

用户在下单时，输入的价格是 `"100.50"`，数量是 `"10.5"`。但我们的引擎内部使用的是 `u64` 整数：

```rust
pub struct Order {
    pub id: u64,
    pub price: u64,   // 整数表示
    pub qty: u64,     // 整数表示
    pub side: Side,
}
```

**核心问题**：如何在十进制字符串和 u64 之间进行无损转换？

答案是使用 **固定小数位数（Fixed Decimal）** 方案：

```rust
/// 将十进制字符串转换为 u64
/// e.g., "100.50" with 2 decimals -> 10050
fn parse_decimal(s: &str, decimals: u32) -> u64 {
    let multiplier = 10u64.pow(decimals);
    // ... 解析逻辑
}

/// 将 u64 转换回十进制字符串用于显示
/// e.g., 10050 with 2 decimals -> "100.50"
fn format_decimal(value: u64, decimals: u32) -> String {
    let multiplier = 10u64.pow(decimals);
    let int_part = value / multiplier;
    let dec_part = value % multiplier;
    format!("{}.{:0>width$}", int_part, dec_part, width = decimals as usize)
}
```

### 2. u64 的最大值问题 (u64 Max Value)

`u64` 的最大值是：

```text
u64::MAX = 18,446,744,073,709,551,615
```

如果我们使用 **8 位小数**（类似比特币的 satoshi），可以表示的最大值是：

```text
184,467,440,737.09551615
```

这意味着：
- 对于价格：可以表示到约 **1844 亿**，某天比特币需要这么大价格表示的时候再升级吧....
- 对于数量：可以装进去全部比特币BTC总量了（总供应量 2100 万）


#### 不同资产的 Decimals 配置

不同的区块链资产有不同的原生精度：

| Asset | Native Decimals | 最小单位 |
|-------|----------------|---------|
| BTC | 8 | 1 satoshi = 0.00000001 BTC |
| USDT (ERC20) | 6 | 0.000001 USDT |
| ETH | 18 | 1 wei = 0.000000000000000001 ETH |

**问题来了**：但是 ETH 原生是 18 位小数，但我们只用 8 位会丢失精度吗？

答案是：**在交易所场景下足够使用**。因为：
- 定义8位的时候交易所交易的最小支持精度是 `0.00000001 ETH`, 足够了
- 没有必要支持交易 `0.000000000000000001 ETH`（价值约 $0.000000000000003）

所以我们可以选择一个**合理的内部精度**，不一定要和原生链一致。

因此，我们需要一个**资产和币对的基本配置管理器**（`SymbolManager`），用于：
- 管理每个资产的内部精度（decimals）
- 管理用户可见的显示精度（display_decimals）
- 管理交易对的价格精度配置
- 在入金/提币时进行链上精度和内部精度的转换

#### ETH Decimals 分析：8 到 12 位的选择

让我们分析不同 decimals 配置下，u64 能表示的最大 ETH 数量：

| Decimals | 乘数 | u64 能表示的最大值 | 够用？ |
|----------|-----|-------------------|-------|
| 8 | 10^8 | **184,467,440,737 ETH** | ✅ 远超总供应量 |
| 9 | 10^9 | **18,446,744,073 ETH** | ✅ 远超总供应量 |
| 10 | 10^10 | **1,844,674,407 ETH** | ✅ 超过总供应量 |
| 11 | 10^11 | **184,467,440 ETH** | ✅ 刚好超过总供应量 (~120M) |
| 12 | 10^12 | **18,446,744 ETH** | ❌ 小于总供应量！ |
| 18 | 10^18 | **18.44 ETH** | ❌ 完全不够用 |

> ETH 当前总供应量约 **1.2 亿 ETH**

**分析**：

- **8 位小数**：最大 1844 亿 ETH，余量巨大，精度 `0.00000001 ETH` 对交易所足够
- **10 位小数**：最大 18 亿 ETH，精度更高
- **12 位小数**：最大 1800 万 ETH，精度最高，⚠️ 但小于总供应量

**为什么 ETH 选择 8 位小数？**

虽然 ETH 原生是 18 位小数（wei），但对于交易所来说：
- `0.00000001 ETH` ≈ `$0.00000003`，远小于任何有意义的交易金额
- 最大可表示 1844 亿 ETH，远超总供应量（1.2 亿）
- 入金/提币时进行精度转换即可

**配置示例**：

```rust
// BTC: 8 位小数（和链上 satoshi 一致）
manager.add_asset(1, 8, 3, "BTC");

// USDT: 8 位小数
manager.add_asset(2, 8, 2, "USDT");

// ETH: 8 位小数（精度足够，范围安全）
manager.add_asset(3, 8, 4, "ETH");
```

### 3. 交易对配置 (Symbol Configuration)

不同的交易对可能有不同的精度要求：

| Symbol | Price Decimals | Qty Display Decimals | Example |
|--------|---------------|---------------------|---------|
| BTC_USDT | 2 | 3 | 买 0.001 BTC @ $65000.00 |
| ETH_USDT | 2 | 4 | 买 0.0001 ETH @ $3500.00 |
| DOGE_USDT | 6 | 0 | 买 100 DOGE @ $0.123456 |

我们使用 `SymbolManager` 来管理这些配置：

```rust
#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub symbol: String,
    pub symbol_id: u32,
    pub base_asset_id: u32,
    pub quote_asset_id: u32,
    pub price_decimal: u32,         // 价格的小数位数
    pub price_display_decimal: u32, // 价格显示的小数位数
}

#[derive(Debug, Clone)]
pub struct AssetInfo {
    pub asset_id: u32,
    pub decimals: u32,         // 内部精度（通常是 8）
    pub display_decimals: u32, // 显示/输入的最大小数位数
    pub name: String,
}
```

### 4. decimals vs display_decimals

这里有两个概念需要区分：

#### `decimals` (内部精度)
- 决定了 u64 乘以多少
- 通常是 **8**（类似 satoshi）
- 这是内部存储精度，用户看不到

#### `display_decimals` (显示精度)
- 决定了用户可以输入/看到多少位小数
- 例如 BTC 显示 3 位：`0.001 BTC`
- USDT 显示 2 位：`100.00 USDT`

**为什么要分开？**

1. **用户体验**：用户不需要看到 8 位小数的精度
2. **输入验证**：可以限制用户输入的小数位数
3. **显示简洁**：避免显示过多无意义的零

### 5. 运行结果

运行 `cargo run` 后的输出：

```text
--- 0xInfinity: Stage 3 (Decimal World) ---
Symbol: BTC_USDT (ID: 0)
Price Decimals: 2, Qty Display Decimals: 3

[1] Makers coming in...
    Order 1: Sell 10.000 BTC @ $100.00
    Order 2: Sell 5.000 BTC @ $102.00
    Order 3: Sell 5.000 BTC @ $101.00

[2] Taker eats liquidity...
    Order 4: Buy 12.000 BTC @ $101.50
MATCH: Buy 4 eats Sell 1 @ Price 10000 (Qty: 10000)
MATCH: Buy 4 eats Sell 3 @ Price 10100 (Qty: 2000)

[3] More makers...
    Order 5: Buy 10.000 BTC @ $99.00

--- End of Simulation ---

--- u64 Range Demo ---
u64::MAX = 18446744073709551615
With 8 decimals, max representable value = 184467440737.09551615
```

可以看到：
- 用户输入的是十进制字符串 `"100.00"`
- 内部存储为整数 `10000`
- 显示时又转换回 `"100.00"`

这就是 **Decimal World** 的核心：**在十进制和 u64 整数之间无缝转换**。

### 📖 真实踩坑故事：JavaScript Number 溢出

在我们的开发过程中，曾经遇到过一个非常诡异的 bug：

> **现象**：后端返回给前端的是原始 ETH 数量（单位 wei）。在开发测试阶段，因为测试金额非常小（0.00x 个 ETH 级别），前端都能正常显示和处理。但上线后只要金额稍大一点（实际上超过约 **0.009 ETH**），数字就开始出现精度问题，**变成一个不正确的数值**！

**根本原因**：JavaScript 的 `Number` 类型使用 IEEE 754 双精度浮点数，最大安全整数是 `2^53 - 1`：

```javascript
> console.log(Number.MAX_SAFE_INTEGER);
9007199254740991                          // 约 9 * 10^15

// 1 ETH = 10^18 wei
> const oneEthInWei = 1000000000000000000;

// 问题演示：当 wei 数量超过 MAX_SAFE_INTEGER 时
> const smallAmount = 1000000000000000;     // 0.001 ETH = 10^15 wei ✅ 安全
> const dangerAmount = 9007199254740992;    // 约 0.009 ETH ⚠️ 刚好超过安全范围
> const tenEthInWei = 10000000000000000000; // 10 ETH = 10^19 wei ❌ 溢出！

// 验证精度丢失：加 1 后值不变！
> console.log(tenEthInWei + 1);
10000000000000000000                       // 没有 +1!

> console.log(tenEthInWei + 2);
10000000000000000000                       // 还是一样!

> console.log(tenEthInWei + 1000);
10000000000000000000                       // 加 1000 也还是一样!

> console.log(tenEthInWei === tenEthInWei + 1);
true                                       // 😱 见鬼了！
```

**为什么超过约 0.009 个 ETH 就出问题？**

```javascript
> console.log(Number.MAX_SAFE_INTEGER / 1e18);
0.009007199254740991                       // 约 0.009 ETH 就是安全边界！

// 虽然输出看起来正确，但实际上精度已经丢失，验证方法：
> const nineEth = 9n * 10n ** 18n;         // BigInt 表示 9 ETH
> const nineEthNum = Number(nineEth);      // 转为 Number
> console.log(nineEthNum);
9000000000000000000                        // 看起来正确...

> console.log(nineEthNum + 1);
9000000000000000000                        // 但是 +1 没有效果！

> console.log(nineEthNum === nineEthNum + 1);
true                                       // 证明精度已丢失
```

**正确的处理方案**：

```javascript
// ✅ 方案 1: 后端返回字符串，前端用 BigInt 处理
> const weiString = "10000000000000000000";  // 后端返回字符串
> const weiBigInt = BigInt(weiString);       // 转为 BigInt
> console.log(weiBigInt.toString());
10000000000000000000                       // ✅ 精确！

// BigInt 可以正确进行算术运算
> console.log((weiBigInt + 1n).toString());
10000000000000000001                       // ✅ +1 正确！

// ✅ 方案 2: 使用专业库如 ethers.js
// import { formatEther, parseEther } from 'ethers';
// const eth = formatEther(weiBigInt);  // "10.0"
```

### Summary

本章解决了以下问题：

1. ✅ **十进制转换**：`parse_decimal()` 和 `format_decimal()` 实现双向无损转换
2. ✅ **u64 范围**：最大值 1844 亿（8 位小数），足够应对任何金融场景
3. ✅ **交易对配置**：`SymbolManager` 管理每个交易对的精度设置
4. ✅ **两种精度定义**：`decimals`（内部）vs `display_decimals`（显示）
