# 0x06 强制余额管理 (Enforced Balance)

在上一章中，我们实现了用户账户的余额管理。但在金融系统中，资金操作是**最核心、最关键**的操作，必须确保万无一失。本章我们将余额管理升级为**类型系统强制**的安全版本。

---

## 1. 为什么需要"强制"版本？

上一章的实现存在几个隐患：

```rust
// ❌ 旧版问题1：字段是公开的，容易被无意修改
pub struct Balance {
    pub avail: u64,   // 开发者可能不小心直接赋值，绕过业务逻辑校验
    pub frozen: u64,
}

// ❌ 旧版问题2：返回 bool，错误信息不明确
fn freeze(&mut self, amount: u64) -> bool {
    // 失败了？为什么失败？不知道
}

// ❌ 旧版问题3：无审计追踪
// 余额变了，但没有版本号，无法追溯
```

这些问题可能导致：
- **开发者无意中绕过校验**：在复杂的业务代码中，可能不小心直接修改公开字段
- **错误难以排查**：只知道操作失败，不知道具体原因
- **审计困难**：没有变更追踪，难以定位问题发生的时间点

> **注意**：这不是防止恶意攻击（这是内部系统），而是**防止开发者无意挖坑**。
> 就像 Rust 的所有权系统一样——我们用类型系统来减少挖坑的机会。

---

## 2. 强制余额设计 (Enforced Balance)

新版本通过 **Rust 类型系统** 强制安全：

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Balance {
    avail: u64,      // ← 私有！只能通过方法访问
    frozen: u64,     // ← 私有！
    version: u64,    // ← 私有！每次变更自动递增
}
```

### 核心原则

| 原则 | 实现方式 |
|------|---------|
| **封装** | 所有字段私有，提供只读 getter |
| **显式错误** | 所有变更返回 `Result<(), &'static str>` |
| **审计追踪** | `version` 在每次变更时自动递增 |
| **溢出保护** | 使用 `checked_add/sub`，溢出返回错误 |

### 方法命名变更

| 旧版 (v0.5) | 新版 (v0.6) | 说明 |
|-------------|-------------|------|
| `freeze()` | `lock()` | 更准确：锁定资金用于订单 |
| `unfreeze()` | `unlock()` | 解锁（取消订单时） |
| `consume_frozen()` | `spend_frozen()` | 消费冻结资金（成交后） |
| `receive()` | `deposit()` | 统一为存款语义 |

---

## 3. Balance API 详解

### 只读方法 (Safe Getters)

```rust
impl Balance {
    /// 获取可用余额 (只读)
    pub const fn avail(&self) -> u64 { self.avail }
    
    /// 获取冻结余额 (只读)
    pub const fn frozen(&self) -> u64 { self.frozen }
    
    /// 获取总余额 (avail + frozen)
    /// 返回 None 表示溢出（数据损坏）
    pub const fn total(&self) -> Option<u64> {
        self.avail.checked_add(self.frozen)
    }
    
    /// 获取版本号 (只读)
    pub const fn version(&self) -> u64 { self.version }
}
```

> **为什么用 `const fn`？** 编译器保证永远不会修改状态，提供最强的安全保证。

### 变更方法 (Validated Mutations)

每个变更方法都：
1. 验证前置条件
2. 使用 checked 算术
3. 返回 `Result`
4. 自动递增 `version`

```rust
/// 存款：增加可用余额
pub fn deposit(&mut self, amount: u64) -> Result<(), &'static str> {
    self.avail = self.avail.checked_add(amount)
        .ok_or("Deposit overflow")?;  // ← 溢出返回错误
    self.version = self.version.wrapping_add(1);  // ← 自动递增
    Ok(())
}

/// 锁定：可用 → 冻结
pub fn lock(&mut self, amount: u64) -> Result<(), &'static str> {
    if self.avail < amount {
        return Err("Insufficient funds to lock");  // ← 明确错误信息
    }
    self.avail = self.avail.checked_sub(amount)
        .ok_or("Lock avail underflow")?;
    self.frozen = self.frozen.checked_add(amount)
        .ok_or("Lock frozen overflow")?;
    self.version = self.version.wrapping_add(1);
    Ok(())
}

/// 解锁：冻结 → 可用
pub fn unlock(&mut self, amount: u64) -> Result<(), &'static str> {
    if self.frozen < amount {
        return Err("Insufficient frozen funds");
    }
    self.frozen = self.frozen.checked_sub(amount)
        .ok_or("Unlock frozen underflow")?;
    self.avail = self.avail.checked_add(amount)
        .ok_or("Unlock avail overflow")?;
    self.version = self.version.wrapping_add(1);
    Ok(())
}

/// 消费冻结资金：成交后资金离开账户
pub fn spend_frozen(&mut self, amount: u64) -> Result<(), &'static str> {
    if self.frozen < amount {
        return Err("Insufficient frozen funds");
    }
    self.frozen = self.frozen.checked_sub(amount)
        .ok_or("Spend frozen underflow")?;
    self.version = self.version.wrapping_add(1);
    Ok(())
}
```

---

## 4. UserAccount 重构

新版 `UserAccount` 也进行了重构：

### 数据结构变更

```rust
// 旧版：使用 FxHashMap
pub struct UserAccount {
    pub user_id: u64,
    balances: FxHashMap<u32, Balance>,
}

// 新版：O(1) 直接数组索引
pub struct UserAccount {
    user_id: UserId,      // 私有
    assets: Vec<Balance>, // 私有，asset_id 作为下标
}
```

> **O(1) 直接数组索引**
>
> ```rust
> // deposit() 自动创建资产槽位（唯一入口）
> pub fn deposit(&mut self, asset_id: AssetId, amount: u64) -> Result<(), &'static str> {
>     let idx = asset_id as usize;
>     if idx >= self.assets.len() {
>         self.assets.resize(idx + 1, Balance::default());
>     }
>     self.assets[idx].deposit(amount)
> }
>
> // get_balance_mut() 不创建槽位，返回 Result
> pub fn get_balance_mut(&mut self, asset_id: AssetId) -> Result<&mut Balance, &'static str> {
>     self.assets.get_mut(asset_id as usize).ok_or("Asset not found")
> }
> ```

> ⚠️ **Asset ID 分配约束**
>
> | 约束 | 说明 |
> |------|------|
> | **不可变** | 一旦分配，永远不能改变 |
> | **小值** | 必须是小整数（用作数组下标） |
> | **连续** | 按顺序分配（1,2,3... 或 0,1,2...） |

> 📊 **性能对比**
>
> | 方法 | 查找 | 内存 | 缓存 |
> |------|------|------|------|
> | HashMap | O(1)* | 有开销 | 差 |
> | Vec + 线性查找 | O(n) | 紧凑 | 好 |
> | **直接索引** | **O(1)** | 紧凑 | **最佳** |
>
> *HashMap 的 O(1) 有隐藏的常数开销（哈希计算、桶查找）

> 🚀 **为什么 `Vec<Balance>` 直接索引是最高效选择？**
>
> **1. 极佳的缓存友好性 (Cache-Friendly)**
>
> `Vec<Balance>` 是连续内存布局，相邻资产的 Balance 在内存中也相邻。
> 当 CPU 读取一个 Balance 时，整个缓存行（通常 64 字节）会被加载，
> 相邻的 Balance 数据也一并进入 L1/L2 缓存，后续访问几乎零延迟。
>
> ```text
> 内存布局:
> ┌──────────┬──────────┬──────────┬──────────┐
> │Balance[0]│Balance[1]│Balance[2]│Balance[3]│ ← 连续内存，缓存友好
> └──────────┴──────────┴──────────┴──────────┘
>
> HashMap 内存布局:
> ┌─────┐     ┌─────┐         ┌─────┐
> │ B0  │ ... │ B2  │ ....... │ B1  │  ← 分散内存，缓存不友好
> └─────┘     └─────┘         └─────┘
> ```
>
> **2. `get_balance()` 是高频调用函数**
>
> 在撮合引擎中，每笔订单都需要多次调用 `get_balance()`：
> - 下单前检查余额
> - 冻结资金
> - 每笔成交结算（买方 + 卖方各 2-3 次）
> - 退款未使用资金
>
> 一笔订单可能产生 5-10 次 `get_balance()` 调用。
> 在高频交易场景（每秒万笔订单），这意味着每秒 5-10 万次调用。
> **O(1) + 缓存友好** 对性能至关重要。

### 结算方法

新增专门的结算方法，一次性处理买方或卖方的所有结算：

```rust
/// 买方结算：消费 Quote，获得 Base，退款未使用的 Quote
pub fn settle_as_buyer(
    &mut self,
    quote_asset_id: AssetId,
    base_asset_id: AssetId,
    spend_quote: u64,   // 消费的 USDT
    gain_base: u64,     // 获得的 BTC
    refund_quote: u64,  // 退款的 USDT
) -> Result<(), &'static str> {
    // 1. 消费 Quote (Frozen)
    self.get_balance_mut(quote_asset_id).spend_frozen(spend_quote)?;
    
    // 2. 获得 Base (Available)
    self.get_balance_mut(base_asset_id).deposit(gain_base)?;
    
    // 3. 退款 (Frozen → Available)
    if refund_quote > 0 {
        self.get_balance_mut(quote_asset_id).unlock(refund_quote)?;
    }
    Ok(())
}

/// 卖方结算：消费 Base，获得 Quote
pub fn settle_as_seller(
    &mut self,
    base_asset_id: AssetId,
    quote_asset_id: AssetId,
    spend_base: u64,    // 消费的 BTC
    gain_quote: u64,    // 获得的 USDT
    refund_base: u64,   // 退款的 BTC
) -> Result<(), &'static str> {
    // 1. 消费 Base (Frozen)
    self.get_balance_mut(base_asset_id).spend_frozen(spend_base)?;
    
    // 2. 获得 Quote (Available)
    self.get_balance_mut(quote_asset_id).deposit(gain_quote)?;
    
    // 3. 退款 (Frozen → Available)
    if refund_base > 0 {
        self.get_balance_mut(base_asset_id).unlock(refund_base)?;
    }
    Ok(())
}
```

---

## 5. 运行结果

```text
=== 0xInfinity: Stage 6 (Enforced Balance) ===
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

=== End of Simulation ===
```

结果与前一章一致，但现在所有余额操作都通过类型系统保护！

---

## 6. 单元测试

新增 8 个 `enforced_balance` 测试：

```bash
$ cargo test

running 16 tests
test enforced_balance::tests::test_deposit ... ok
test enforced_balance::tests::test_deposit_overflow ... ok
test enforced_balance::tests::test_lock_unlock ... ok
test enforced_balance::tests::test_spend_frozen ... ok
test enforced_balance::tests::test_total ... ok
test enforced_balance::tests::test_version_increments ... ok
test enforced_balance::tests::test_withdraw ... ok
test enforced_balance::tests::test_withdraw_insufficient ... ok
test engine::tests::test_add_resting_order ... ok
test engine::tests::test_cancel_order ... ok
test engine::tests::test_fifo_at_same_price ... ok
test engine::tests::test_full_match ... ok
test engine::tests::test_multiple_trades_single_order ... ok
test engine::tests::test_partial_match ... ok
test engine::tests::test_price_priority ... ok
test engine::tests::test_spread ... ok

test result: ok. 16 passed; 0 failed
```

---

## 7. 错误处理示例

使用新 API 时，必须处理 `Result`：

```rust
// ❌ 编译错误：未处理的 Result
balance.deposit(100);

// ✅ 正确：显式处理
balance.deposit(100)?;  // 使用 ? 传播错误

// ✅ 正确：使用 unwrap（仅在确定不会失败时）
balance.deposit(100).unwrap();

// ✅ 正确：使用 expect 添加上下文
balance.deposit(100).expect("Initial deposit should not overflow");

// ✅ 正确：匹配处理
match balance.lock(1000) {
    Ok(()) => println!("Locked successfully"),
    Err(e) => println!("Failed to lock: {}", e),
}
```

---

## Summary

本章完成了以下工作：

1. ✅ **私有字段封装**：所有余额字段私有化，防止无意修改
2. ✅ **Result 返回类型**：所有变更操作返回明确的错误信息
3. ✅ **版本追踪**：每次变更自动递增 `version`，支持审计
4. ✅ **Checked 算术**：所有运算使用 checked_add/sub，溢出返回错误
5. ✅ **方法重命名**：`lock/unlock/spend_frozen` 语义更清晰
6. ✅ **结算方法**：`settle_as_buyer/settle_as_seller` 一站式结算
7. ✅ **Asset ID 约束**：为未来 O(1) 直接索引优化做准备
8. ✅ **16 个测试通过**：包括 8 个新的 enforced_balance 测试

现在我们的余额管理是**类型安全**的——编译器本身就能防止大部分余额操作错误！

