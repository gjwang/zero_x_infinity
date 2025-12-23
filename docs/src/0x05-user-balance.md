# 0x05 User Account & Balance Management

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📦 Code Changes**: [View Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.4-btree-orderbook...v0.5-user-balance)

In previous chapters, our matching engine could match orders correctly. But there's a key question: **User Funds?** In a real exchange, users must have sufficient funds before placing an order, and funds must be transferred upon matching.

This chapter implements the user account system, including:
*   Balance Management (Avail / Frozen)
*   Pre-trade Fund Validation
*   Post-trade Settlement

### 1. Dual State of Balance: Avail vs Frozen

In an exchange, a balance has two states:

| State | Meaning | Usage |
|-------|---------|-------|
| **Avail** | Can be used for trading or withdrawal | Daily operations |
| **Frozen** | Locked in open orders | Waiting for match or cancel |

**Why do we need Frozen?**

Suppose Alice has 10 BTC and she places two sell orders:
*   Order A: Sell 8 BTC
*   Order B: Sell 5 BTC

Without a freeze mechanism, these two orders require 13 BTC, but Alice only has 10! This is the **Over-Selling** problem.

**Correct Flow**:

```
1. Alice has 10 BTC (avail=10, frozen=0)
2. Place Order A (8 BTC) → freeze 8 BTC → (avail=2, frozen=8) ✅
3. Place Order B (5 BTC) → try freeze 5 BTC → Fail! avail only 2 ❌
```

### 2. Balance Structure

```rust
#[derive(Debug, Clone, Default)]
pub struct Balance {
    pub avail: u64,  // Available Balance
    pub frozen: u64, // Frozen Balance
}

impl Balance {
    /// Deposit (Increase avail)
    /// Returns false on overflow - Financial systems must detect this!
    pub fn deposit(&mut self, amount: u64) -> bool {
        match self.avail.checked_add(amount) {
            Some(new_avail) => {
                self.avail = new_avail;
                true
            }
            None => false, // Overflow! Alert and investigate.
        }
    }
```

> **Why `checked_add`?**
>
> | Method | Overflow Behavior (250u8 + 10u8) | Use Case |
> |--------|----------------------------------|----------|
> | `+` (Std) | Panic (Debug) or Wrap (Release) | General logic, overflow is bug |
> | `wrapping_add` | 4 (Wrap) | Hashing, Graphics |
> | `saturating_add` | 255 (Cap) | Quotas, Token buckets |
> | **`checked_add`** | **`None`** | ✅ **Finance**, Overflow must error! |
>
> ⚠️ In financial systems, "too much money causing overflow" is a severe bug. It must return an error for handling, not silently wrap or saturate.

```rust
    /// Freeze (avail → frozen)
    pub fn freeze(&mut self, amount: u64) -> bool {
        if self.avail >= amount {
            self.avail -= amount;
            self.frozen += amount;
            true
        } else {
            false
        }
    }

    /// Unfreeze (frozen → avail), for cancellations
    pub fn unfreeze(&mut self, amount: u64) -> bool {
        if self.frozen >= amount {
            self.frozen -= amount;
            self.avail += amount;
            true
        } else {
            false
        }
    }

    /// Consume Frozen (Fund leaves account after match)
    pub fn consume_frozen(&mut self, amount: u64) -> bool {
        if self.frozen >= amount {
            self.frozen -= amount;
            true
        } else {
            false
        }
    }

    /// Receive Funds (Fund enters account after match)
    pub fn receive(&mut self, amount: u64) {
        self.avail = self.avail.checked_add(amount);
    }
}
```

### 3. User Account Structure

Each user holds balances for multiple assets:

```rust
/// Use FxHashMap for O(1) asset lookup
/// FxHashMap is faster for integer keys
pub struct UserAccount {
    pub user_id: u64,
    balances: FxHashMap<u32, Balance>, // asset_id -> Balance
}

impl UserAccount {
    pub fn deposit(&mut self, asset_id: u32, amount: u64) {
        self.get_balance_mut(asset_id).deposit(amount);
    }

    pub fn avail(&self, asset_id: u32) -> u64 {
        self.balances.get(&asset_id).map(|b| b.avail).unwrap_or(0)
    }

    pub fn frozen(&self, asset_id: u32) -> u64 {
        self.balances.get(&asset_id).map(|b| b.frozen).unwrap_or(0)
    }
}
```

### 4. Order Placing: Freezing Funds

When placing an order, we freeze specific assets based on order side:

| Order Side | Asset to Freeze | Amount |
|------------|-----------------|--------|
| Buy | Quote Asset (e.g. USDT) | price × quantity / qty_unit |
| Sell | Base Asset (e.g. BTC) | quantity |

#### Using SymbolManager for Precision

Each pair has its own precision config:

```rust
let symbol_info = manager.get_symbol_info("BTC_USDT").unwrap();
let price_decimal = symbol_info.price_decimal;  // 2
let base_asset = manager.assets.get(&symbol_info.base_asset_id).unwrap();
let qty_decimal = base_asset.decimals;  // 8
let qty_unit = 10u64.pow(qty_decimal);  // 100_000_000

// price = 100 USDT (Internal: 100 * price_unit)
// qty = 10 BTC (Internal: 10 * qty_unit)
// cost = price * qty / qty_unit (Prevent overflow)
let cost = price * qty / qty_unit;

if accounts.freeze(user_id, USDT, cost) {
    let result = book.add_order(Order::new(id, user_id, price, qty, Side::Buy));
} else {
    println!("REJECTED: Insufficient balance");
}

// Sell Order: Freeze BTC
if accounts.freeze(user_id, BTC, qty) {
    let result = book.add_order(Order::new(id, user_id, price, qty, Side::Sell));
}
```

### 5. Settlement: Fund Transfer

When orders match, funds transfer between buyer and seller:

```
Trade: Alice sells 1 BTC to Bob @ $100

Before:
  Alice: BTC(frozen=1), USDT(avail=0)
  Bob:   BTC(avail=0), USDT(frozen=100)

Settlement:
  Alice: consume_frozen(BTC, 1) + receive(USDT, 100)
  Bob:   consume_frozen(USDT, 100) + receive(BTC, 1)

After:
  Alice: BTC(frozen=0), USDT(avail=100)
  Bob:   BTC(avail=1), USDT(frozen=0)
```

Code Implementation:

```rust
pub fn settle_trade(
    &mut self,
    buyer_id: u64,
    seller_id: u64,
    base_asset_id: u32,
    quote_asset_id: u32,
    base_amount: u64,    // Trade Qty
    quote_amount: u64,   // Trade Amount (price × qty)
) {
    // Buyer: Use USDT, Get BTC
    self.get_account_mut(buyer_id)
        .get_balance_mut(quote_asset_id)
        .consume_frozen(quote_amount);
    self.get_account_mut(buyer_id)
        .get_balance_mut(base_asset_id)
        .receive(base_amount);

    // Seller: Use BTC, Get USDT
    self.get_account_mut(seller_id)
        .get_balance_mut(base_asset_id)
        .consume_frozen(base_amount);
    self.get_account_mut(seller_id)
        .get_balance_mut(quote_asset_id)
        .receive(quote_amount);
}
```

### 6. Refined Trade Structure

To support settlement, `Trade` needs user IDs:

```rust
pub struct Trade {
    pub id: u64,
    pub buyer_order_id: u64,
    pub seller_order_id: u64,
    pub buyer_user_id: u64,   // New
    pub seller_user_id: u64,  // New
    pub price: u64,
    pub qty: u64,
}
```

### 7. Execution Results

```text
=== 0xInfinity: Stage 5 (User Balance) ===
Symbol: BTC_USDT | Price: 2 decimals, Qty: 8 decimals
Cost formula: price * qty / 100000000

[0] Initial deposits...
    Alice: 100.00000000 BTC, 10000.00 USDT
    Bob:   5.00000000 BTC, 200000.00 USDT

[1] Alice places sell orders...
    Order 1: Sell 10.00000000 BTC @ $100.00 -> New
    Order 2: Sell 5.00000000 BTC @ $101.00 -> New
    Alice balance: avail=85.00000000 BTC, frozen=15.00000000 BTC

[2] Bob places buy order (taker)...
    Order 3: Buy 12.00000000 BTC @ $101.00 (cost: 1212.00 USDT)
    Trades:
      - Trade #1: 10.00000000 BTC @ $100.00
      - Trade #2: 2.00000000 BTC @ $101.00
    Order status: Filled

[3] Final balances:
    Alice: 85.00000000 BTC (frozen: 3.00000000), 11202.00 USDT
    Bob:   17.00000000 BTC, 198798.00 USDT (frozen: 0.00)

    Book: Best Bid=None, Best Ask=Some("101.00")
```

**Analysis**:
*   Alice initial 100 BTC. Sold 10+2=12. Remaining 85 avail + 3 frozen = 88 BTC ✓
*   Alice got 10×100 + 2×101 = 1202 USDT. Initial 10000 + 1202 = 11202 USDT ✓
*   Bob initial 5 BTC. Bought 12. Total 17 BTC ✓
*   Bob spent 1202 USDT. Initial 200000 - 1202 = 198798 USDT ✓

### Summary

This chapter accomplished:

1.  ✅ **Implemented Balance**: Dual-state (avail/frozen).
2.  ✅ **Implemented UserAccount**: Multi-asset support.
3.  ✅ **Implemented AccountManager**: Managing all users.
4.  ✅ **Pre-trade Freeze**: Prevent over-selling/buying.
5.  ✅ **Post-trade Settlement**: Correct fund transfer.
6.  ✅ **Refined Trade**: Included user_ids.

Now our engine not only matches orders but also ensures funding sufficiency and correct settlement!

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📦 代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.4-btree-orderbook...v0.5-user-balance)

在前几章中，我们的撮合引擎已经可以正确匹配订单并产生成交。但有一个关键问题：**钱从哪里来？** 在真实的交易所中，用户必须先有足够的资金才能下单，成交后资金才会转移。

本章我们将实现用户账户系统，包括：
- 余额管理（可用 / 冻结）
- 下单前资金校验
- 成交后资金结算

### 1. 余额的双重状态：Avail vs Frozen

在交易所中，用户的余额有两种状态：

| 状态 | 含义 | 使用场景 |
|------|------|---------|
| **Avail** (可用) | 可以用于下单或提现 | 日常操作 |
| **Frozen** (冻结) | 已锁定在挂单中 | 等待成交或取消 |

**为什么需要冻结？**

假设 Alice 有 10 BTC，她同时挂了两个卖单：
- 卖单 A：卖 8 BTC
- 卖单 B：卖 5 BTC

如果没有冻结机制，这两个订单共需要 13 BTC，但 Alice 只有 10 BTC！这就是**超卖**问题。

**正确的流程**：

```
1. Alice 有 10 BTC (avail=10, frozen=0)
2. 下卖单 A (8 BTC) → freeze 8 BTC → (avail=2, frozen=8) ✅
3. 下卖单 B (5 BTC) → 尝试 freeze 5 BTC → 失败！avail 只有 2 ❌
```

### 2. Balance 结构

```rust
#[derive(Debug, Clone, Default)]
pub struct Balance {
    pub avail: u64,  // 可用余额 (简短命名，JSON 输出更高效)
    pub frozen: u64, // 冻结余额
}

impl Balance {
    /// 存款 (增加 avail)
    /// 返回 false 表示溢出 - 金融系统必须检测此错误
    pub fn deposit(&mut self, amount: u64) -> bool {
        match self.avail.checked_add(amount) {
            Some(new_avail) => {
                self.avail = new_avail;
                true
            }
            None => false, // 溢出！需要报警和调查
        }
    }
```

> **为什么要用 `checked_add`？**
>
> | 方法 | 溢出行为 (250u8 + 10u8) | 适用场景 |
> |------|------------------------|---------|
> | `+` (标准) | Panic (Debug) 或 4 (Release回绕) | 常规逻辑，溢出是 Bug |
> | `wrapping_add` | 4 (回绕) | 哈希计算、图形算法 |
> | `saturating_add` | 255 (封顶) | 资源配额、令牌桶 |
> | **`checked_add`** | **`None`** | ✅ **金融余额**，溢出必须报错! |
>
> ⚠️ 金融系统中，"钱多到溢出"是严重的 Bug，必须返回错误让上层处理，而不是静默封顶或回绕。

```rust

    /// 冻结 (avail → frozen)
    pub fn freeze(&mut self, amount: u64) -> bool {
        if self.avail >= amount {
            self.avail -= amount;
            self.frozen += amount;
            true
        } else {
            false
        }
    }

    /// 解冻 (frozen → avail)，用于取消订单
    pub fn unfreeze(&mut self, amount: u64) -> bool {
        if self.frozen >= amount {
            self.frozen -= amount;
            self.avail += amount;
            true
        } else {
            false
        }
    }

    /// 消耗冻结资金 (成交后，资金离开账户)
    pub fn consume_frozen(&mut self, amount: u64) -> bool {
        if self.frozen >= amount {
            self.frozen -= amount;
            true
        } else {
            false
        }
    }

    /// 接收资金 (成交后，资金进入账户)
    pub fn receive(&mut self, amount: u64) {
        self.avail = self.avail.checked_add(amount);
    }
}
```

### 3. 用户账户结构

每个用户持有多种资产的余额：

```rust
/// 使用 FxHashMap 实现 O(1) 资产查找
/// FxHashMap 使用更简单、更快的哈希函数，特别适合整数键
pub struct UserAccount {
    pub user_id: u64,
    balances: FxHashMap<u32, Balance>, // asset_id -> Balance
}

impl UserAccount {
    pub fn deposit(&mut self, asset_id: u32, amount: u64) {
        self.get_balance_mut(asset_id).deposit(amount);
    }

    pub fn avail(&self, asset_id: u32) -> u64 {
        self.balances.get(&asset_id).map(|b| b.avail).unwrap_or(0)
    }

    pub fn frozen(&self, asset_id: u32) -> u64 {
        self.balances.get(&asset_id).map(|b| b.frozen).unwrap_or(0)
    }
}
```

### 4. 下单流程：冻结资金

在下单时，我们需要根据订单类型冻结相应的资产：

| 订单类型 | 需要冻结的资产 | 冻结金额 |
|---------|--------------|---------|
| 买单 (Buy) | Quote 资产 (如 USDT) | price × quantity / qty_unit |
| 卖单 (Sell) | Base 资产 (如 BTC) | quantity |

#### 从 SymbolManager 获取精度配置

每个交易对有独立的精度配置：

```rust
let symbol_info = manager.get_symbol_info("BTC_USDT").unwrap();
let price_decimal = symbol_info.price_decimal;  // 2 (价格精度)

let base_asset = manager.assets.get(&symbol_info.base_asset_id).unwrap();
let qty_decimal = base_asset.decimals;  // 8 (数量精度)
let qty_unit = 10u64.pow(qty_decimal);  // 100_000_000

// price = 100 USDT (内部单位: 100 * price_unit)
// qty = 10 BTC (内部单位: 10 * qty_unit)
// cost = price * qty / qty_unit (确保不会溢出)
let cost = price * qty / qty_unit;

if accounts.freeze(user_id, USDT, cost) {
    let result = book.add_order(Order::new(id, user_id, price, qty, Side::Buy));
} else {
    println!("REJECTED: Insufficient balance");
}

// 卖单：冻结 BTC
if accounts.freeze(user_id, BTC, qty) {
    let result = book.add_order(Order::new(id, user_id, price, qty, Side::Sell));
}
```

这样，精度配置跟着 Symbol 走，`price * qty / qty_unit` 保证结果在合理范围内。

### 5. 成交结算：资金转移

当订单匹配成交后，需要在买卖双方之间转移资金：

```
Trade: Alice sells 1 BTC to Bob @ $100

Before:
  Alice: BTC(frozen=1), USDT(avail=0)
  Bob:   BTC(avail=0), USDT(frozen=100)

Settlement:
  Alice: consume_frozen(BTC, 1) + receive(USDT, 100)
  Bob:   consume_frozen(USDT, 100) + receive(BTC, 1)

After:
  Alice: BTC(frozen=0), USDT(avail=100)
  Bob:   BTC(avail=1), USDT(frozen=0)
```

代码实现：

```rust
pub fn settle_trade(
    &mut self,
    buyer_id: u64,
    seller_id: u64,
    base_asset_id: u32,  // 如 BTC
    quote_asset_id: u32, // 如 USDT
    base_amount: u64,    // 成交数量
    quote_amount: u64,   // 成交金额 (price × qty)
) {
    // Buyer: 消耗 USDT，获得 BTC
    self.get_account_mut(buyer_id)
        .get_balance_mut(quote_asset_id)
        .consume_frozen(quote_amount);
    self.get_account_mut(buyer_id)
        .get_balance_mut(base_asset_id)
        .receive(base_amount);

    // Seller: 消耗 BTC，获得 USDT
    self.get_account_mut(seller_id)
        .get_balance_mut(base_asset_id)
        .consume_frozen(base_amount);
    self.get_account_mut(seller_id)
        .get_balance_mut(quote_asset_id)
        .receive(quote_amount);
}
```

### 6. Trade 结构的完善

为了正确结算，`Trade` 结构需要包含买卖双方的用户 ID：

```rust
pub struct Trade {
    pub id: u64,
    pub buyer_order_id: u64,
    pub seller_order_id: u64,
    pub buyer_user_id: u64,   // 新增
    pub seller_user_id: u64,  // 新增
    pub price: u64,
    pub qty: u64,
}
```

在撮合时，从 Order 中提取 user_id 并写入 Trade：

```rust
trades.push(Trade::new(
    self.trade_id_counter,
    buy_order.id,
    sell_order.id,
    buy_order.user_id,   // 从订单获取用户 ID
    sell_order.user_id,
    price,
    trade_qty,
));
```

### 7. 运行结果

```text
=== 0xInfinity: Stage 5 (User Balance) ===
Symbol: BTC_USDT | Price: 2 decimals, Qty: 8 decimals
Cost formula: price * qty / 100000000

[0] Initial deposits...
    Alice: 100.00000000 BTC, 10000.00 USDT
    Bob:   5.00000000 BTC, 200000.00 USDT

[1] Alice places sell orders...
    Order 1: Sell 10.00000000 BTC @ $100.00 -> New
    Order 2: Sell 5.00000000 BTC @ $101.00 -> New
    Alice balance: avail=85.00000000 BTC, frozen=15.00000000 BTC

[2] Bob places buy order (taker)...
    Order 3: Buy 12.00000000 BTC @ $101.00 (cost: 1212.00 USDT)
    Trades:
      - Trade #1: 10.00000000 BTC @ $100.00
      - Trade #2: 2.00000000 BTC @ $101.00
    Order status: Filled

[3] Final balances:
    Alice: 85.00000000 BTC (frozen: 3.00000000), 11202.00 USDT
    Bob:   17.00000000 BTC, 198798.00 USDT (frozen: 0.00)

    Book: Best Bid=None, Best Ask=Some("101.00")
```

**分析**：
- Alice 初始有 100 BTC，卖出 10+2=12 BTC，还剩 85 + 3(frozen) = 88 BTC ✓
- Alice 收到 10×100 + 2×101 = 1202 USDT，加上初始 10000 = 11202 USDT ✓
- Bob 初始有 5 BTC，买入 12 BTC = 17 BTC ✓
- Bob 花费 1202 USDT，初始 200000 - 1202 = 198798 USDT ✓

### Summary

本章完成了以下工作：

1. ✅ **实现 Balance 结构**：avail/frozen 双状态余额管理
2. ✅ **实现 UserAccount**：一个用户持有多种资产余额
3. ✅ **实现 AccountManager**：管理所有用户账户
4. ✅ **下单前资金冻结**：防止超卖/超买
5. ✅ **成交后资金结算**：在买卖双方间正确转移资金
6. ✅ **完善 Trade 结构**：包含买卖双方 user_id
7. ✅ **添加单元测试**：4 个新测试覆盖余额管理

现在我们的撮合引擎不仅能正确匹配订单，还能确保用户有足够的资金，并在成交后正确结算！
