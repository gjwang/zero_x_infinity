# 0x07-a Testing Framework - Correctness

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📦 Code Changes**: [View Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.6-enforced-balance...v0.7-a-testing-framework)

> **Core Objective**: To establish a verifiable, repeatable, and traceable testing infrastructure for the matching engine.

This chapter is not just about "how to test", but importantly about understanding "why designed this way"—these design decisions stem directly from real-world exchange requirements.

### 1. Why a Testing Framework?

#### 1.1 The Uniqueness of Matching Engines

A matching engine is not a generic CRUD app. A single bug can lead to:

*   **Fund Errors**: Users' funds disappearing or inflating.
*   **Order Loss**: Orders executed but not recorded.
*   **Inconsistent States**: Contradictions between balances, orders, and ledgers.

Therefore, we need:

1.  **Deterministic Testing**: Same input must yield same output.
2.  **Complete Audit**: Every penny movement must be traceable.
3.  **Fast Verification**: Quickly confirm correctness after every code change.

#### 1.2 Golden File Testing Pattern

We adopt the **Golden File Pattern**:

```
fixtures/         # Input (Fixed)
    ├── orders.csv
    └── balances_init.csv

baseline/         # Golden Baseline (Result of first correct run, committed to git)
    ├── t1_balances_deposited.csv
    ├── t2_balances_final.csv
    ├── t2_ledger.csv
    └── t2_orderbook.csv

output/           # Current Run Result (gitignored)
    └── ...
```

**Why this pattern?**

1.  **Determinism**: Fixed seeds ensure identical random sequences.
2.  **Version Control**: Baselines are committed; any change triggers a git diff.
3.  **Fast Feedback**: Just `diff baseline/ output/`.
4.  **Auditable**: Baseline is the "contract"; deviations require explanation.

### 2. Precision Design: decimals vs display_decimals

#### 2.1 Why Two Precisions?

This is the most error-prone area in exchanges. Consider this real case:

```
User sees:      Buy 0.01 BTC @ $85,000.00
Internal store: qty=1000000 (satoshi), price=85000000000 (micro-cents)
```

If we confuse these layers:
*   User enters `0.01`, system treats as `0.01 satoshi` (= 0.00000001 BTC).
*   Or user account shows 100 BTC, but actually has 0.000001 BTC.

**Solution: Clearly distinguish two layers.**

#### 2.2 Precision Layers

```
┌─────────────────────────────────────────────────────────────┐
│ Layer 1: Client (display_decimals)                          │
│   - Numbers seen by users                                   │
│   - Can be adjusted based on business needs                 │
│   - E.g.: BTC displays 6 decimals (0.000001 BTC)            │
└─────────────────────────────────────────────────────────────┘
                              ↓
                    Auto Convert (× 10^decimals)
                              ↓
┌─────────────────────────────────────────────────────────────┐
│ Layer 2: Internal (decimals)                                │
│   - Precision for internal storage and calculation          │
│   - NEVER change once set                                   │
│   - E.g.: BTC stored with 8 decimals (satoshi)              │
└─────────────────────────────────────────────────────────────┘
```

#### 2.3 Configuration Design

**assets_config.csv** (Asset Precision Config):
```csv
asset_id,asset,decimals,display_decimals
1,BTC,8,6     # Min unit 0.000001 BTC ≈ $0.085
2,USDT,6,4    # Min unit 0.0001 USDT
3,ETH,8,4     # Min unit 0.0001 ETH ≈ $0.40
```

| Field | Mutability | Explanation |
|-------|------------|-------------|
| `decimals` | ⚠️ **Never Change** | Defines min unit; changing breaks all existing data. |
| `display_decimals` | ✅ Dynamic | Client-side precision for **Quantity (qty)**. |

**symbols_config.csv** (Trading Pair Config):
```csv
symbol_id,symbol,base_asset_id,quote_asset_id,price_decimal,price_display_decimal
0,BTC_USDT,1,2,6,2    # Price min unit $0.01
1,ETH_USDT,3,2,6,2
```

**Key Design: Precision Source**

| Order Field | Precision Source | Config File |
|-------------|------------------|-------------|
| `qty` | `base_asset.display_decimals` | assets_config.csv |
| `price` | `symbol.price_display_decimal` | symbols_config.csv |

> ⚠️ Note: **Price precision comes from Symbol config, NOT Quote Asset!**
> This is because the same quote asset (e.g., USDT) may have different price precisions in different pairs.

**Why `decimals` cannot change?**

Suppose BTC decimals change from 8 to 6:
*   Original balance 100,000,000 (= 1 BTC with 8 decimals).
*   New interpretation 100,000,000 / 10^6 = 100 BTC.
*   User gains 99 BTC out of thin air!

**Why `display_decimals` can change?**

This is just the display layer:
*   Original display: 0.12345678 BTC.
*   New display (6 decimals): 0.123456 BTC.
*   Internal storage remains 12,345,678 satoshis.

### 3. Balance Format: Row vs Column

#### 3.1 Problem: Storing Multi-Asset Balances

**Option A: Columnar (One column per asset)**
```csv
user_id,btc_avail,btc_frozen,usdt_avail,usdt_frozen
1,10000000000,0,10000000000000,0
```

**Option B: Row-based (One row per asset)**
```csv
user_id,asset_id,avail,frozen,version
1,1,10000000000,0,0
1,2,10000000000000,0,0
```

#### 3.2 Why Row-based?

| Dimension | Columnar | Row-based |
|-----------|----------|-----------|
| Extensibility | ❌ Alter table to add asset | ✅ Just add a row |
| Sparse Data | ❌ Many nulls/zeros | ✅ Store only non-zero assets |
| DB Compat | ❌ Non-standard | ✅ Standard normalization |
| Genericity | ❌ Asset names hardcoded | ✅ `asset_id` is generic |

**Real Scenario**: An exchange supports 500+ assets, but users avg 3-5 holdings. Row-based design saves 99% storage space.

### 4. Timeline Snapshot Design

#### 4.1 Why Multiple Snapshots?

Matching is a multi-stage process:

```
T0: Initial State (fixtures/balances_init.csv)
    ↓ deposit()
T1: Deposit Done (baseline/t1_balances_deposited.csv)
    ↓ execute orders
T2: Trading Done (baseline/t2_balances_final.csv)
```

**Errors can occur at any stage**:
*   T0→T1: Is deposit logic correct?
*   T1→T2: Is trade settlement correct?

Snapshots pinpoint issues:
```bash
# Verify Deposit
diff balances_init.csv t1_balances_deposited.csv

# Verify Settlement
diff t1_balances_deposited.csv t2_balances_final.csv
```

#### 4.2 Naming Convention

```
t1_balances_deposited.csv   # t1 stage, balances type, deposited state
t2_balances_final.csv       # t2 stage, balances type, final state
t2_ledger.csv               # t2 stage, ledger type
t2_orderbook.csv            # t2 stage, orderbook type
```

**Principle**: `{Time}_{Type}_{State}.csv`

Benefits:
1.  Natural sort order by time.
2.  Clear content identification.
3.  Avoids ambiguity.

### 5. Settlement Ledger Design

#### 5.1 Why Ledger?

`t2_ledger.csv` is the system's **Audit Log**. Every penny movement is recorded here.

**Without Ledger**:
*   User complaint: "Where did my money go?"
*   Support: "Your balance is X."
*   Unanswerable: "When did it change? Why?"

**With Ledger**:
```csv
trade_id,user_id,asset_id,op,delta,balance_after
1,96,2,debit,849700700,9999150299300
1,96,1,credit,1000000,10001000000
```

Traceability:
*   Trade #1 caused User #96's USDT to decrease by 849,700,700.
*   Simultaneously BTC increased by 1,000,000.
*   What is the balance after change.

#### 5.2 Why `delta + after` instead of `before + after`?

**Option A: before + after**
```csv
delta,balance_before,balance_after
849700700,10000000000,9999150299300
```

**Option B: delta + after**
```csv
delta,balance_after
849700700,9999150299300
```

**Why B?**
1.  **Less Redundancy**: `before = after - delta`.
2.  **Usefulness**: We mostly verify "Is the final state correct?".
3.  **Clarity**: Delta directly explains the change.

### 6. ME Orderbook Snapshot

#### 6.1 Why Orderbook Snapshot?

After trading, the Orderbook still holds **unfilled orders**. These orders:
*   Reside in RAM.
*   Are lost if system restarts.

`t2_orderbook.csv` is a **Full Snapshot of ME State**:

```csv
order_id,user_id,side,order_type,price,qty,filled_qty,status
6,907,sell,limit,85330350000,2000000,0,New
```

**Uses**:
1.  **Recovery**: Revert Orderbook state after restart.
2.  **Verification**: Compare against theoretical expectations.
3.  **Debugging**: Check stuck orders.

#### 6.2 Why Record All Fields?

The goal is **Full Recovery**. Rebuilding `Order` struct requires:

```rust
struct Order {
    id, user_id, price, qty, filled_qty, side, order_type, status
}
```

Missing any field prevents recovery.

### 7. Test Script Design

#### 7.1 Modular Scripts

```
scripts/
├── test_01_generate.sh     # Step 1: Generate Data
├── test_02_baseline.sh     # Step 2: Generate Baseline
├── test_03_verify.sh       # Step 3: Run & Verify
└── test_e2e.sh             # Combo: Full E2E Flow
```

**Why Modular?**
1.  **Isolated Debugging**: Run only relevant steps.
2.  **Flexible Composition**: CI can verify without regenerating.
3.  **Readability**: One script, one job.

#### 7.2 Usage

```bash
# Daily Test (Use existing baseline)
./scripts/test_e2e.sh

# Regenerate Baseline & Test
./scripts/test_e2e.sh --regenerate
```

### 8. CLI Design: `--baseline` Switch

#### 8.1 Why Switch?

Default behavior:
*   Output to `output/`
*   Never overwrite baseline

Update baseline:
*   Add `--baseline` arg
*   Output to `baseline/`

**Why not auto-overwrite?**
1.  **Safety**: Prevent accidental baseline corruption.
2.  **Intent**: Updating baseline is a conscious decision.
3.  **Git Friendly**: Changes trigger diff.

#### 8.2 Implementation

```rust
fn get_output_dir() -> &'static str {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--baseline") {
        "baseline"
    } else {
        "output"
    }
}
```

### 9. Execution Example

#### 9.1 Full Flow

```bash
# 1. Generate Data
python3 scripts/generate_orders.py --orders 100000 --seed 42

# 2. Generate Baseline (First run or update)
cargo run --release -- --baseline

# 3. Daily Test
./scripts/test_e2e.sh
```

#### 9.2 Verification Output

```
╔════════════════════════════════════════════════════════════╗
║     0xInfinity Testing Framework - E2E Test                ║
╚════════════════════════════════════════════════════════════╝

  t1_balances_deposited.csv: ✅ MATCH
  t2_balances_final.csv: ✅ MATCH
  t2_ledger.csv: ✅ MATCH
  t2_orderbook.csv: ✅ MATCH

✅ All tests passed!
```

### 10. Summary

This chapter established a complete testing infrastructure:

| Design Point | Problem Solved | Solution |
|--------------|----------------|----------|
| Precision Confusion | User vs Internal precision | decimals + display_decimals |
| Asset Extension | Support N assets | Row-based balance format |
| Traceability | Where failed? | Timeline Snapshots (T0→T1→T2) |
| Fund Audit | Where funds go? | Settlement Ledger |
| State Recovery | Restart recovery | Orderbook Snapshot |
| Regression | Breaking changes? | Golden File Pattern |
| Efficiency | Fast feedback | Modular scripts |

**Core Philosophy**:

> Testing is not an afterthought, but part of the design. A good testing framework gives you confidence when changing code.

Next section (0x07-b) will add performance benchmarks on top of this.

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📦 代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.6-enforced-balance...v0.7-a-testing-framework)

> **核心目的**：为撮合引擎建立可验证、可重复、可追溯的测试基础设施。

本章不仅是"如何测试"，更重要的是理解"为什么这样设计"——这些设计决策直接源于真实交易所的需求。

### 1. 为什么需要测试框架？

#### 1.1 撮合引擎的特殊性

撮合引擎不是普通的 CRUD 应用。一个 bug：

- **资金错误**：用户资金凭空消失或增加
- **订单丢失**：订单被执行但没有记录
- **状态不一致**：余额、订单、成交记录互相矛盾

因此，我们需要：

1. **确定性测试**：相同的输入必须产生相同的输出
2. **完整审计**：每一分钱的变动都可追溯
3. **快速验证**：每次修改代码后能快速确认没有破坏正确性

#### 1.2 Golden File 测试模式

我们采用 **Golden File 模式**：

```
fixtures/         # 输入（固定）
    ├── orders.csv
    └── balances_init.csv

baseline/         # 黄金基准（第一次正确运行的结果，git 提交）
    ├── t1_balances_deposited.csv
    ├── t2_balances_final.csv
    ├── t2_ledger.csv
    └── t2_orderbook.csv

output/           # 当前运行结果（gitignored）
    └── ...
```

**为什么选择这种模式？**

1. **确定性**：固定的 seed 保证相同的随机数序列
2. **版本控制**：baseline 提交到 git，任何变化都能被 diff 检测
3. **快速反馈**：只需 `diff baseline/ output/`
4. **可审计**：baseline 是"合约"，任何偏离都需要解释

### 2. 精度设计：decimals vs display_decimals

#### 2.1 为什么需要两种精度？

这是交易所最容易出错的地方。看这个真实案例：

```
用户看到：买入 0.01 BTC @ $85,000.00
内部存储：qty=1000000 (satoshi), price=85000000000 (微美分)
```

如果混淆这两层，会发生什么？

- 用户输入 `0.01`，系统理解为 `0.01 satoshi` = 实际 0.0000001 BTC
- 或者用户账户显示有 100 BTC，实际只有 0.000001 BTC

**解决方案：明确区分两层精度**

#### 2.2 精度层次

```
┌─────────────────────────────────────────────────────────────┐
│ Layer 1: Client (display_decimals)                          │
│   - 用户看到的数字                                            │
│   - 可以根据业务需求调整                                        │
│   - 例如：BTC 数量显示 6 位小数 (0.000001 BTC)              │
└─────────────────────────────────────────────────────────────┘
                              ↓
                    自动转换 (× 10^decimals)
                              ↓
┌─────────────────────────────────────────────────────────────┐
│ Layer 2: Internal (decimals)                                │
│   - 内部存储和计算的精度                                        │
│   - 一旦设定永不改变                                            │
│   - 例如：BTC 存储 8 位精度 (satoshi)                          │
└─────────────────────────────────────────────────────────────┘
```

#### 2.3 配置文件设计

**assets_config.csv**（资产精度配置）：
```csv
asset_id,asset,decimals,display_decimals
1,BTC,8,6     # 最小单位 0.000001 BTC ≈ $0.085
2,USDT,6,4   # 最小单位 0.0001 USDT
3,ETH,8,4    # 最小单位 0.0001 ETH ≈ $0.40
```

| 字段 | 可变性 | 说明 |
|------|--------|------|
| `decimals` | ⚠️ **永不改变** | 定义最小单位，改变会破坏所有现有数据 |
| `display_decimals` | ✅ 可动态调整 | 用于**数量 (qty)** 的客户端精度 |

**symbols_config.csv**（交易对配置）：
```csv
symbol_id,symbol,base_asset_id,quote_asset_id,price_decimal,price_display_decimal
0,BTC_USDT,1,2,6,2    # 价格最小单位 $0.01
1,ETH_USDT,3,2,6,2
```

**关键设计：精度来源**

| 订单字段 | 精度来源 | 配置位置 |
|----------|----------|----------|
| `qty` (数量) | `base_asset.display_decimals` | assets_config.csv |
| `price` (价格) | `symbol.price_display_decimal` | symbols_config.csv |

> ⚠️ 注意：**price 精度来自 symbol 配置，不是 quote_asset！**
> 这样设计是因为同一个 quote asset（如 USDT）在不同交易对中可能有不同的价格精度。

**为什么 decimals 不能改变？**

假设 BTC decimals 从 8 改为 6：
- 原来账户余额 100000000 (= 1 BTC)
- 现在变成 100000000 / 10^6 = 100 BTC
- 用户凭空获得 99 BTC！

**为什么 display_decimals 可以改变？**

这只是显示层，不影响存储：
- 原来显示 0.12345678 BTC
- 调整后显示 0.123456 BTC（6位）
- 内部存储仍然是 12345678 satoshi

### 3. 余额格式设计：行式 vs 列式

#### 3.1 问题：如何存储多资产余额？

**Option A：列式（每个资产一列）**
```csv
user_id,btc_avail,btc_frozen,usdt_avail,usdt_frozen
1,10000000000,0,10000000000000,0
```

**Option B：行式（每个资产一行）**
```csv
user_id,asset_id,avail,frozen,version
1,1,10000000000,0,0
1,2,10000000000000,0,0
```

#### 3.2 为什么选择行式？

| 对比维度 | 列式 | 行式 |
|----------|------|------|
| 扩展性 | ❌ 添加资产需改表结构 | ✅ 直接添加新行 |
| 稀疏数据 | ❌ 大量空值 | ✅ 只存有余额的资产 |
| 数据库兼容 | ❌ 非标准化 | ✅ 标准化范式 |
| 通用性 | ❌ 资产名硬编码 | ✅ asset_id 通用 |

**真实场景**：交易所支持 500+ 种资产，但用户平均只持有 3-5 种。行式设计节省 99% 的存储空间。

### 4. 时间线快照设计

#### 4.1 为什么需要多个快照？

撮合过程不是单一操作，而是多阶段流程：

```
T0: 初始状态 (fixtures/balances_init.csv)
    ↓ deposit()
T1: 充值完成 (baseline/t1_balances_deposited.csv)
    ↓ execute orders
T2: 交易完成 (baseline/t2_balances_final.csv)
```

**每个阶段都可能出错**：

- T0→T1：deposit 逻辑是否正确？
- T1→T2：交易结算是否正确？

有了快照，可以精确定位问题：

```bash
# 验证 deposit 正确性
diff balances_init.csv t1_balances_deposited.csv

# 验证交易结算正确性
diff t1_balances_deposited.csv t2_balances_final.csv
```

#### 4.2 文件命名设计

```
t1_balances_deposited.csv   # t1 阶段，balances 类型，deposited 状态
t2_balances_final.csv       # t2 阶段，balances 类型，final 状态
t2_ledger.csv               # t2 阶段，ledger 类型
t2_orderbook.csv            # t2 阶段，orderbook 类型
```

**命名原则**：`{时间点}_{数据类型}_{状态}.csv`

这样的命名：
1. 按时间排序时自然有序
2. 一眼看出数据是什么
3. 避免文件名歧义

### 5. Settlement Ledger 设计

#### 5.1 为什么需要 Ledger？

`t2_ledger.csv` 是整个系统的**审计日志**。每一分钱的变动都记录在这里。

**没有 Ledger 的问题**：

- 用户投诉：我的钱去哪了？
- 只能说：交易后余额是 X
- 无法回答：什么时候变的？为什么变？

**有了 Ledger**：

```csv
trade_id,user_id,asset_id,op,delta,balance_after
1,96,2,debit,849700700,9999150299300
1,96,1,credit,1000000,10001000000
```

可以完整追溯：
- Trade #1 导致 User #96 的 USDT 减少 849700700
- 同时 BTC 增加 1000000
- 变化后余额是多少

#### 5.2 为什么用 delta + after，而不是 before + after？

**Option A：before + after**
```csv
delta,balance_before,balance_after
849700700,10000000000,9999150299300
```

**Option B：delta + after**
```csv
delta,balance_after
849700700,9999150299300
```

**选择 Option B 的原因**：

1. **冗余更少**：before = after - delta，可计算得出
2. **after 更有用**：通常我们想验证的是"最终状态对不对"
3. **delta 直接说明变化**：不需要心算 before - after

### 6. ME Orderbook 快照

#### 6.1 为什么需要 Orderbook 快照？

交易完成后，Orderbook 里仍然有**未成交的挂单**。这些订单：

- 在内存中
- 如果系统重启，会丢失

`t2_orderbook.csv` 是 **ME 状态的完整快照**：

```csv
order_id,user_id,side,order_type,price,qty,filled_qty,status
6,907,sell,limit,85330350000,2000000,0,New
```

**用途**：

1. **状态恢复**：重启后可以从快照恢复 Orderbook
2. **正确性验证**：与理论预期对比
3. **调试**：哪些订单还在挂着？

#### 6.2 为什么记录所有字段？

快照目的是**完整恢复**。恢复时需要重建 `Order` 结构体：

```rust
struct Order {
    id,
    user_id,
    price,
    qty,
    filled_qty,
    side,
    order_type,
    status,
}
```

缺少任何字段都无法恢复。

### 7. 测试脚本设计

#### 7.1 模块化脚本

```
scripts/
├── test_01_generate.sh     # Step 1: 生成测试数据
├── test_02_baseline.sh     # Step 2: 生成基准
├── test_03_verify.sh       # Step 3: 运行并验证
└── test_e2e.sh             # 组合：完整 E2E 流程
```

**为什么模块化？**

1. **单独调试**：出问题时只运行相关步骤
2. **灵活组合**：CI 可以只运行 verify，不重新生成数据
3. **可读性**：每个脚本做一件事

#### 7.2 使用方式

```bash
# 日常测试（使用现有 baseline）
./scripts/test_e2e.sh

# 重新生成基准并测试
./scripts/test_e2e.sh --regenerate
```

### 8. 命令行设计：--baseline 开关

#### 8.1 为什么需要开关？

默认行为：
- 输出到 `output/`
- 不会覆盖 baseline

需要更新基准时：
- 加 `--baseline` 参数
- 输出到 `baseline/`

**为什么不自动覆盖？**

1. **安全**：防止意外覆盖基准
2. **意图明确**：更新基准是有意识的决定
3. **Git 友好**：baseline 变化会触发 git diff
4. **代码实现**：

```rust
fn get_output_dir() -> &'static str {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--baseline") {
        "baseline"
    } else {
        "output"
    }
}
```

### 9. 运行示例

#### 9.1 完整流程

```bash
# 1. 生成测试数据
python3 scripts/generate_orders.py --orders 100000 --seed 42

# 2. 生成基准（首次或需要更新时）
cargo run --release -- --baseline

# 3. 日常测试
./scripts/test_e2e.sh
```

#### 9.2 验证输出

```
╔════════════════════════════════════════════════════════════╗
║     0xInfinity Testing Framework - E2E Test                ║
╚════════════════════════════════════════════════════════════╝

  t1_balances_deposited.csv: ✅ MATCH
  t2_balances_final.csv: ✅ MATCH
  t2_ledger.csv: ✅ MATCH
  t2_orderbook.csv: ✅ MATCH

✅ All tests passed!
```

### 10. Summary

本章建立了完整的测试基础设施：

| 设计点 | 解决的问题 | 方案 |
|--------|------------|------|
| 精度混淆 | 用户精度 vs 内部精度 | decimals + display_decimals |
| 资产扩展 | 支持 N 种资产 | 行式余额格式 |
| 过程追溯 | 哪一步出错？ | 时间线快照 (T0→T1→T2) |
| 资金审计 | 每分钱去向 | Settlement Ledger |
| 状态恢复 | 重启后恢复 | Orderbook 快照 |
| 回归测试 | 代码改动是否破坏正确性 | Golden File 模式 |
| 测试效率 | 快速反馈 | 模块化脚本 |

**核心理念**：

> 测试不是事后补的，而是设计的一部分。好的测试框架能让你在改动代码时有信心。

下一节 (0x07-b) 将在此基础上添加性能测试和优化基准。
