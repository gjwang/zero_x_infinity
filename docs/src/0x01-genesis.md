# 0x01 Genesis: Basic Engine

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📦 Code Changes**: [View Diff](https://github.com/gjwang/zero_x_infinity/compare/8046049...v0.1-genesis)

This is the first version of **0xInfinity**.
In this stage, we have built a minimal prototype of a **Central Limit Order Book (CLOB)**. Our goal is to intuitively demonstrate real-world trading logic using standard data structures to manage orders.

### 1. Visualizing the Orderbook

An Orderbook is essentially a list of orders arranged by price. We place **Sells (Asks)** at the top and **Buys (Bids)** at the bottom. The gap in the middle is called the "**Spread**".

We maintain two lists in memory:
*   **Sells**: Sorted by price **Low to High** (Buyers want the cheapest price).
*   **Buys**: Sorted by price **High to Low** (Sellers want the most expensive price).

```text
===========================================================
               ORDER BOOK SNAPSHOT
===========================================================

    Side   |   Price (f64)   |   Qty    |   Orders (FIFO)
-----------------------------------------------------------
    SELL   |     102.00      |   5.0    |   [Order #2]
    SELL   |     101.00      |   5.0    |   [Order #3]     ^
                                                           | Best Ask (Lowest)
-----------------------------------------------------------
             $$$  MARKET SPREAD  $$$
-----------------------------------------------------------
                                                           | Best Bid (Highest)
    BUY    |     100.00      |   10.0   |   [Order #1]     v
    BUY    |      99.00      |   10.0   |   [Order #5]

===========================================================
```

### 2. Program Output

After executing `cargo run`, we can observe the actual output of the engine:

```text
--- 0xInfinity: Stage 1 (Genesis) ---

[1] Makers coming in...

[2] Taker eats liquidity...
MATCH: Buy 4 eats Sell 1 @ Price 100 (Qty: 10)
MATCH: Buy 4 eats Sell 3 @ Price 101 (Qty: 2)

[3] More makers...

--- End of Simulation ---
```

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📦 代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/8046049...v0.1-genesis)

这是 **0xInfinity** 的第一个版本。
在这一阶段，我们构建了一个最简单的**中央限价订单簿（CLOB）**雏形。我们的目标是直观地展示现实世界的交易逻辑，使用标准的数据结构来管理订单。

### 1. 订单簿布局 (Visualizing the Orderbook)

订单簿本质上是一个按价格排列的列表。我们将**卖单（Sells）**放在上方，**买单（Buys）**放在下方。中间的空隙被称为“**价差（Spread）**”。

我们在内存中维护了两个列表：
*   **Sells**: 按价格 **从低到高** 排列（买家希望买到最便宜的）。
*   **Buys**: 按价格 **从高到低** 排列（卖家希望卖给最贵的）。

```text
===========================================================
               ORDER BOOK SNAPSHOT
===========================================================

    Side   |   Price (f64)   |   Qty    |   Orders (FIFO)
-----------------------------------------------------------
    SELL   |     102.00      |   5.0    |   [Order #2]
    SELL   |     101.00      |   5.0    |   [Order #3]     ^
                                                           | Best Ask (Lowest)
-----------------------------------------------------------
             $$$  MARKET SPREAD  $$$
-----------------------------------------------------------
                                                           | Best Bid (Highest)
    BUY    |     100.00      |   10.0   |   [Order #1]     v
    BUY    |      99.00      |   10.0   |   [Order #5]

===========================================================
```

### 2. 运行结果 (Program Output)

执行 `cargo run` 后，我们可以看到引擎的实际运行结果：

```text
--- 0xInfinity: Stage 1 (Genesis) ---

[1] Makers coming in...

[2] Taker eats liquidity...
MATCH: Buy 4 eats Sell 1 @ Price 100 (Qty: 10)
MATCH: Buy 4 eats Sell 3 @ Price 101 (Qty: 2)

[3] More makers...

--- End of Simulation ---
```
