# 0x02: 浮点数的诅咒 (The Curse of Float)

> **📦 代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.1-genesis...v0.2-the-curse-of-float)

## 1. 新手常犯的错误 (The Rookie Mistake)

有经验的老手，应该马上看到 `price` 的类型是 `f64`，这是有问题的。因为我们在 `models.rs` 里有这行代码：

```rust
pub price: f64, // The root of all evil
```

在大多数不要求计算结果绝对精确的场合，使用浮点数是没问题的。如果单精度不够，那就使用双精度，一般都不会有什么问题。但是在金融领域，使用浮点数存储金额，属于**工程事故**。

使用浮点数存储金额，稍微长一点时间，都不可能做到账本的完全精确、分毫不差。即使通过频繁的对账校验，最后也只能接受"大差不差，差不多就行"的结果。

而且使用浮点数存储金额，会带来**累积误差**。在常年累月的交易后，这些微小的误差会越来越多。使用各种不同的误差舍入模式，如果做对了，可以减少累积误差。

如果说累积误差在一定范围内是可以接受的，那么误差本身一般不是问题。最大的问题是：如果不能从根本上检验结算的正确性，就可能因此而隐藏了真正的 bug。

---

## 2. 精度陷阱 (The Precision Trap)

跑一下这段极其简单的代码（你可以在本项目中运行 `cargo run --example the_curse_of_float`）：

```rust
fn main() {
    let a: f64 = 0.1;
    let b: f64 = 0.2;
    let sum = a + b;

    // You expect this to pass, right?
    if sum == 0.3 {
        println!("Math works!");
    } else {
        println!("PANIC: Math is broken! Sum is {:.20}", sum);
    }
}
```

输出结果会让人惊讶：

```text
PANIC: Math is broken! Sum is 0.30000000000000004441
```

看到了吗？那个多出来的 `0.00000000000000004441`。这是什么鬼？为什么会这样？

主要的问题不仅仅是浮点数精度够不够的问题，而是**计算机根本无法精确表示某些数字**的问题。

计算机是二进制的，而人类的常用数字是十进制的。就像十进制里 `1/3 = 0.3333...` 永远写不完一样，在二进制里，`0.1` 也是一个用二进制永远无法完全精确表达的数。

在撮合引擎里，如果你的 OrderBook 里的 Ask 是 `0.3`，而用户的 Bid 是 `0.1 + 0.2`，由于浮点误差，这两个本来应该成交的单子，**永远不会匹配**。

---

## 3. 区块链的零容忍 (Why Blockchain Hates Floats)

如果了解过以太坊的智能合约语言就知道，在合约里面是没有任何浮点数的。很多人不知道为什么。

原因只有一个：区块链的核心是要求同样的输入必须 100% 确定的输出。无论你在什么时间、什么地方，都必须在不同的硬件、不同的操作系统、不同的 CPU 架构上，运行同一段代码，并得到**完全一致**的结果。只有**完全一致**，一个 bit 的误差都没有，才能确定全球所有人共享的都是同一个账本、同一种"比特币"。

具体而言，浮点数计算遵循 IEEE 754 标准，但在极端边缘情况下，不同的 CPU 对浮点数的处理可能会有极其微小的差异：

```text
Node A (Intel) 算出结果：100.00000000000001
Node B (ARM) 算出结果：100.00000000000000
```

一旦发生这种情况，Hash 就会不同，共识就会破裂，链就会分叉。

---

## 4. Decimal 的诱惑与陷阱 (The Decimal Temptation)

有人意识到 `f64` 的问题时，会寻找一种**精确的小数类型**，比如 `rust_decimal`。

但即使是 Decimal，在不同的硬件、不同编程语言，甚至同一种语言的不同版本、编译器的实现上，都可能有细微的差别，都不可能做到区块链要求的 100% 确定性。

能做到 100% 确定性的，只有整数。如果全部是整数计算结果也不一致的话，可以 100% 确定是有 bug。

### Decimal 的问题

**Decimal (Software Struct):**
- Decimal 是软件模拟的
- Decimal 的一致性依赖于库的实现
- 如果你的后端用 Rust (`rust_decimal`)，风控用 Python (`decimal`)，前端用 JS (`BigInt`)，不同的库对"舍入模式 (Rounding Mode)"和"溢出处理"可能有不同的"方言"
- 这种微小的差异会导致长时间之后系统对不上账

---

## 5. 性能之争: f64 vs u64 (Need for Speed)

除了 100% 确定性，我们不使用 `Decimal` 的另一个核心理由是：**性能**。

**u64 (Native Integer):**
- 当你执行 `a + b` 时，CPU 内部有专门的 ALU 电路直接处理 64 位整数加法
- 它最快只需要 **1 个时钟周期** 就完成了计算

**Decimal (Software Struct):**
- 当你执行加法时，CPU 实际上是在运行一段复杂的代码：检查 Scale、调整对齐、处理溢出、最后计算
- 这需要多 **上百倍甚至几千倍** 的指令周期

大多数情况下，CPU 时钟周期都过剩，因此一般应用无需过多考虑。而且大多数现代 CPU 都有浮点计算单元，也会很快。但我们要写的是 HFT 引擎，纳秒必争。

还有就是 **Cache Efficiency（缓存效率）**：
- `u64` 占 8 字节
- `Decimal` 通常占 16 字节 (128-bit)
- 使用 `u64` 意味着你的 CPU 缓存能多存一倍的价格数据，这直接意味着**吞吐量翻倍**

关于 Cache 的问题，后面再详细讨论。

---

## Summary

不能使用浮点数的两个理由：

1. **不能保证 100% 确定性** — 无法满足区块链共识和精确对账的要求
2. **Decimal 有性能问题** — 对于 HFT 引擎来说，整数是唯一的选择

---

## 重构后的运行结果

我们已经把 `models.rs` 中的 `f64` 全部重构为 `u64`：

```rust
pub struct Order {
    pub id: u64,
    pub price: u64,  // 使用整数表示价格
    pub qty: u64,    // 使用整数表示数量
    pub side: Side,
}
```

运行 `cargo run` 后的输出：

```text
--- 0xInfinity: Stage 2 (Integer) ---

[1] Makers coming in...

[2] Taker eats liquidity...
MATCH: Buy 4 eats Sell 1 @ Price 100 (Qty: 10)
MATCH: Buy 4 eats Sell 3 @ Price 101 (Qty: 2)

[3] More makers...

--- End of Simulation ---
```

现在所有的价格比较都是精确的整数比较，不再有浮点数误差的问题。
