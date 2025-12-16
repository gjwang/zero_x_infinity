# 0x07a 测试框架 - 正确性验证 (Testing Framework - Correctness)

> **核心目的**：为撮合引擎建立可验证、可重复、可追溯的测试基础设施。

本章不仅是"如何测试"，更重要的是理解"为什么这样设计"——这些设计决策直接源于真实交易所的需求。

---

## 1. 为什么需要测试框架？

### 1.1 撮合引擎的特殊性

撮合引擎不是普通的 CRUD 应用。一个 bug：

- **资金错误**：用户资金凭空消失或增加
- **订单丢失**：订单被执行但没有记录
- **状态不一致**：余额、订单、成交记录互相矛盾

因此，我们需要：

1. **确定性测试**：相同的输入必须产生相同的输出
2. **完整审计**：每一分钱的变动都可追溯
3. **快速验证**：每次修改代码后能快速确认没有破坏正确性

### 1.2 Golden File 测试模式

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

---

## 2. 精度设计：decimals vs display_decimals

### 2.1 为什么需要两种精度？

这是交易所最容易出错的地方。看这个真实案例：

```
用户看到：买入 0.01 BTC @ $85,000.00
内部存储：qty=1000000 (satoshi), price=85000000000 (微美分)
```

如果混淆这两层，会发生什么？

- 用户输入 `0.01`，系统理解为 `0.01 satoshi` = 实际 0.0000001 BTC
- 或者用户账户显示有 100 BTC，实际只有 0.000001 BTC

**解决方案：明确区分两层精度**

### 2.2 精度层次

```
┌─────────────────────────────────────────────────────────────┐
│ Layer 1: Client (display_decimals)                          │
│   - 用户看到的数字                                            │
│   - 可以根据业务需求调整                                        │
│   - 例如：BTC 显示 2 位小数 (0.01 BTC)                         │
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

### 2.3 配置文件设计

**assets_config.csv**：
```csv
asset_id,asset,decimals,display_decimals
1,BTC,8,2
2,USDT,6,2
```

| 字段 | 可变性 | 说明 |
|------|--------|------|
| `decimals` | ⚠️ **永不改变** | 定义最小单位，改变会破坏所有现有数据 |
| `display_decimals` | ✅ 可动态调整 | 可根据市场情况随时调整 |

**为什么 decimals 不能改变？**

假设 BTC decimals 从 8 改为 6：
- 原来账户余额 100000000 (= 1 BTC)
- 现在变成 100000000 / 10^6 = 100 BTC
- 用户凭空获得 99 BTC！

**为什么 display_decimals 可以改变？**

这只是显示层，不影响存储：
- 原来显示 0.12345678 BTC
- 调整后显示 0.12 BTC
- 内部存储仍然是 12345678 satoshi

---

## 3. 余额格式设计：行式 vs 列式

### 3.1 问题：如何存储多资产余额？

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

### 3.2 为什么选择行式？

| 对比维度 | 列式 | 行式 |
|----------|------|------|
| 扩展性 | ❌ 添加资产需改表结构 | ✅ 直接添加新行 |
| 稀疏数据 | ❌ 大量空值 | ✅ 只存有余额的资产 |
| 数据库兼容 | ❌ 非标准化 | ✅ 标准化范式 |
| 通用性 | ❌ 资产名硬编码 | ✅ asset_id 通用 |

**真实场景**：交易所支持 500+ 种资产，但用户平均只持有 3-5 种。行式设计节省 99% 的存储空间。

---

## 4. 时间线快照设计

### 4.1 为什么需要多个快照？

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

### 4.2 文件命名设计

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

---

## 5. Settlement Ledger 设计

### 5.1 为什么需要 Ledger？

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

### 5.2 为什么用 delta + after，而不是 before + after？

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

---

## 6. ME Orderbook 快照

### 6.1 为什么需要 Orderbook 快照？

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

### 6.2 为什么记录所有字段？

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

---

## 7. 测试脚本设计

### 7.1 模块化脚本

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

### 7.2 使用方式

```bash
# 日常测试（使用现有 baseline）
./scripts/test_e2e.sh

# 重新生成基准并测试
./scripts/test_e2e.sh --regenerate

# 单独运行
./scripts/test_01_generate.sh 1000000 42  # 自定义参数
```

---

## 8. 命令行设计：--baseline 开关

### 8.1 为什么需要开关？

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

### 8.2 代码实现

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

---

## 9. 运行示例

### 9.1 完整流程

```bash
# 1. 生成测试数据
python3 scripts/generate_orders.py --orders 100000 --seed 42

# 2. 生成基准（首次或需要更新时）
cargo run --release -- --baseline

# 3. 日常测试
./scripts/test_e2e.sh
```

### 9.2 验证输出

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

---

## 10. Summary

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

下一节 (0x07b) 将在此基础上添加性能测试和优化基准。
