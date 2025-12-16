# 0x07a 测试框架 - 正确性测试 (Testing Framework - Correctness)

本节建立了撮合引擎的批量测试框架，为后续性能优化提供基准。

---

## 1. 测试数据生成

使用 Python 脚本生成真实分布的 BTC/USDT 订单数据：

```bash
python3 scripts/generate_orders.py \
  --orders 1000000 \
  --accounts 1000 \
  --price 85000 \
  --output-dir fixtures
```

### 数据特征

| 参数 | 值 |
|------|-----|
| 订单总数 | 1,000,000 |
| 账户数量 | 1,000 |
| 基准价格 | $85,000 |
| 价格精度 | 2 位小数 (分) |
| 数量精度 | 8 位小数 (聪) |
| 买卖比例 | 50% / 50% |

### 价格分布

- 使用**指数分布**模拟真实市场：大多数订单靠近盘口，少量远离
- 买单略低于中间价，卖单略高于中间价
- 价格范围：±5% 左右

### 数量分布

- 使用**对数正态分布**：大多数订单是小单，少量大单
- 典型订单：0.1 BTC
- 范围：0.0001 - 10 BTC

---

## 2. 配置文件（Single Source of Truth）

### assets_config.csv

```csv
asset_id,asset,decimals,display_decimals
1,BTC,8,2
2,USDT,6,2
3,ETH,8,2
```

#### Decimal Precision 设计原则

| 字段 | 可变性 | 用途 | 示例 |
|------|--------|------|------|
| `decimals` | ⚠️ **不可变** | 内部存储精度 | BTC=8 (satoshi) |
| `display_decimals` | ✅ 可动态调整 | 客户端显示精度 | BTC=2 (0.01 BTC) |

**关键规则：**

1. **`decimals`** - 设置后**永不改变**
   - 定义最小单位（如 1 satoshi = 10^-8 BTC）
   - 所有内部计算、存储使用此精度
   - 修改会破坏所有现有余额/订单数据

2. **`display_decimals`** - 可随时调整
   - 客户端看到的价格/数量精度
   - 可根据市场情况调整
   - 示例：显示 $84,907.12 而非 $84,907.123456

### symbols_config.csv
```csv
symbol_id,symbol,base_asset_id,quote_asset_id,price_decimal,price_display_decimal
1,BTC_USDT,1,2,6,2
2,ETH_USDT,3,2,6,2
```

---

## 3. 输入文件格式

### orders.csv（客户端格式，使用 display_decimals）
```csv
order_id,user_id,side,price,qty
1,655,buy,84907.12,0.39
2,559,buy,84735.71,1.01
```

> 注意：price 和 qty 使用 `display_decimals` 格式（字符串），
> 接收端自动转换为内部 `decimals` 精度

### balances_init.csv（行式格式，支持 N 个资产）
```csv
user_id,asset_id,avail,frozen,version
1,1,10000000000,0,0
1,2,10000000000000,0,0
```

---

## 4. 执行流程

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│ Load Config │ -> │ Load Accts  │ -> │ Load Orders │
└─────────────┘    └─────────────┘    └─────────────┘
                           │
                           v
                   ┌─────────────┐
                   │ Init Engine │
                   └─────────────┘
                           │
                           v
           ┌───────────────────────────────┐
           │     For each order:           │
           │  1. Lock funds (freeze)       │
           │  2. Submit to OrderBook       │
           │  3. Process trades (settle)   │
           │  4. Write output CSV          │
           └───────────────────────────────┘
                           │
                           v
           ┌───────────────────────────────┐
           │     Write final state:        │
           │  - final_balances.csv         │
           │  - final_orderbook.csv        │
           │  - summary.txt                │
           └───────────────────────────────┘
```

---

## 5. 输出文件

| 文件 | 内容 |
|------|------|
| `baseline/trades.csv` | 所有成交记录 |
| `baseline/order_results.csv` | 每笔订单的执行结果 |
| `baseline/final_balances.csv` | 最终账户余额快照 |
| `baseline/final_orderbook.csv` | 最终订单簿状态 |
| `baseline/summary.txt` | 执行摘要 |

### trades.csv
```csv
trade_id,price,quantity,buyer_user_id,seller_user_id,buyer_order_id,seller_order_id
```

### order_results.csv
```csv
order_id,user_id,side,price,quantity,status,filled_qty,num_trades
```

### final_balances.csv
```csv
user_id,btc_avail,btc_frozen,usdt_avail,usdt_frozen
```

---

## 6. 执行结果

```
=== Execution Summary ===
Total Orders: 1,000,000
  Accepted: 771,943
  Rejected: 228,057
Total Trades: 21,215
Execution Time: 21.39s
Throughput: 46,750 orders/sec

Final Orderbook:
  Best Bid: $84,999.99
  Best Ask: $85,002.31
  Bid Depth: 124,906 levels
  Ask Depth: 118,806 levels
```

### 关键指标

| 指标 | 值 | 说明 |
|------|-----|------|
| 接受率 | 77.2% | 有足够余额的订单 |
| 成交率 | 2.7% | 成交订单占接受订单的比例 |
| 吞吐量 | 46,750/s | 单线程 Release 模式 |
| 订单簿深度 | 243,712 | bid + ask levels |

---

## 7. 运行方式

```bash
# 1. 生成测试数据
python3 scripts/generate_orders.py

# 2. 运行测试
cargo run --release

# 3. 查看结果
cat baseline/summary.txt
```

---

## Summary

本节完成了以下工作：

1. ✅ **Python 订单生成器**：真实分布的 100 万订单
2. ✅ **重构 main.rs**：批量 CSV 读取与执行
3. ✅ **完整输出记录**：trades、orders、balances CSV
4. ✅ **最终状态快照**：余额 + 订单簿状态
5. ✅ **性能基准**：46,750 orders/sec (单线程)

下一节 (7b) 将添加性能测试和优化基准。
