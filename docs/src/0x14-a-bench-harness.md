# 0x14-a Benchmark Harness: Test Data Generation

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **Phase V, Step 1**
> **Objective**: Re-implement the Exchange-Core test data generation algorithm in Rust and verify correctness against golden data.

---

### 1. Chapter Objectives

| # | Goal | Deliverable |
|---|------|-------------|
| 1 | **Implement LCG PRNG** | `src/bench/java_random.rs` - Java-compatible random generator |
| 2 | **Implement Order Generator** | `src/bench/order_generator.rs` - Deterministic order sequence |
| 3 | **Verify Correctness** | Unit tests that compare generated data with `golden_*.csv` |

**Success Criteria**: Generated data matches golden CSV byte-for-byte (same `order_id`, `price`, `size`, `uid` for each row).

---

### 2. Reference Algorithm: LCG PRNG

The Exchange-Core project uses Java's `java.util.Random` as its PRNG. We must implement a bit-exact replica.

#### 2.1 Java Random Implementation

```rust
/// Java-compatible Linear Congruential Generator
pub struct JavaRandom {
    seed: u64,
}

impl JavaRandom {
    const MULTIPLIER: u64 = 0x5DEECE66D;
    const ADDEND: u64 = 0xB;
    const MASK: u64 = (1 << 48) - 1;

    pub fn new(seed: i64) -> Self {
        Self {
            seed: (seed as u64 ^ Self::MULTIPLIER) & Self::MASK,
        }
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.seed = self.seed
            .wrapping_mul(Self::MULTIPLIER)
            .wrapping_add(Self::ADDEND) & Self::MASK;
        (self.seed >> (48 - bits)) as i32
    }

    pub fn next_int(&mut self, bound: i32) -> i32 {
        assert!(bound > 0);
        let bound = bound as u32;
        if (bound & bound.wrapping_sub(1)) == 0 {
            // Power of two
            return ((bound as u64 * self.next(31) as u64) >> 31) as i32;
        }
        loop {
            let bits = self.next(31) as u32;
            let val = bits % bound;
            if bits.wrapping_sub(val).wrapping_add(bound.wrapping_sub(1)) >= bits {
                return val as i32;
            }
        }
    }

    pub fn next_long(&mut self) -> i64 {
        ((self.next(32) as i64) << 32) + self.next(32) as i64
    }

    pub fn next_double(&mut self) -> f64 {
        let a = (self.next(26) as u64) << 27;
        let b = self.next(27) as u64;
        (a + b) as f64 / ((1u64 << 53) as f64)
    }
}
```

#### 2.2 Seed Derivation

Each test session derives its seed from `symbol_id` and `benchmark_seed`:

```rust
fn derive_session_seed(symbol_id: i32, benchmark_seed: i64) -> i64 {
    let mut hash: i64 = 1;
    hash = 31 * hash + (symbol_id as i64 * -177277);
    hash = 31 * hash + (benchmark_seed * 10037 + 198267);
    hash
}
```

---

### 3. Golden Data Reference

**Location**: `docs/exchange_core_verification_kit/golden_data/`

| File | Records | Seed | Description |
|------|---------|------|-------------|
| `golden_single_pair_margin.csv` | 11,000 | 1 | Margin (futures) contract |
| `golden_single_pair_exchange.csv` | 11,000 | 1 | Spot exchange |

**CSV Format**:
```csv
phase,command,order_id,symbol,price,size,action,order_type,uid
```

---

### 4. Implementation Checklist

- [x] **Step 1**: Create `src/bench/mod.rs`
- [x] **Step 2**: Implement `JavaRandom` in `src/bench/java_random.rs`
    - [x] Unit test: verify first 100 random numbers match Java output
- [x] **Step 3**: Implement `TestOrdersGenerator` in `src/bench/order_generator.rs`
    - [x] Pareto distribution for symbol/user weights
    - [x] Order generation logic (GTC orders for FILL phase)
    - [x] Seed derivation using `Objects.hash` formula
- [x] **Step 4**: Load and compare with golden CSV
    - [x] `#[test] fn test_golden_single_pair_margin()`
    - [x] `#[test] fn test_golden_single_pair_exchange()`

---

### 5. Implementation Results

> [!NOTE]
> **✅ 100% BIT-EXACT MATCH ACHIEVED** - All fields now match the Java reference implementation exactly.

| Field | Match Status | Formula |
|:-----:|:------------:|:--------|
| **Price** | ✅ 100% | `pow(r,2)*deviation` + 4-value averaging |
| **Size** | ✅ 100% | `1 + rand(6)*rand(6)*rand(6)` |
| **Action** | ✅ 100% | `(rand(4)+priceDir>=2) ? BID : ASK` |
| **UID** | ✅ 100% | Pareto user account generation |

**Key Implementation Details**:
1. `JavaRandom` - Bit-exact `java.util.Random` LCG
2. Seed derivation: `Objects.hash(symbol*-177277, seed*10037+198267)`
3. User accounts: `1 + (int)paretoSample` formula
4. Currency order: `[978, 840]` based on HashMap bucket index
5. User selection: `min(users.size, max(2, symbolMessages/5))`

---

### 6. Verification Commands

**One-Click Verification**:
```bash
# Run all golden data verification tests
cargo test golden_ -- --nocapture
```

**Detailed Comparison Test**:
```bash
# Compare first 20 orders against golden CSV with full output
cargo test test_generator_vs_golden_detailed -- --nocapture
```

**All Benchmark Tests**:
```bash
# Run all tests in the bench module
cargo test bench:: -- --nocapture
```

**Expected Output**:
```
[  1] ✅ | Golden: id=1, price=34386, size=  1, action=BID, uid=377
[  2] ✅ | Golden: id=2, price=34135, size=  1, action=BID, uid=110
[  3] ✅ | Golden: id=3, price=34347, size=  2, action=BID, uid=459
...
[20] ✅ | Golden: id=20, price=34297, size=  1, action=BID, uid=491
```

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **Phase V, 步骤 1**
> **目标**: 用 Rust 重新实现 Exchange-Core 测试数据生成算法，并对比黄金数据验证正确性。

---

### 1. 章节目标

| # | 目标 | 交付物 |
|---|------|--------|
| 1 | **实现 LCG PRNG** | `src/bench/java_random.rs` - Java 兼容随机数生成器 |
| 2 | **实现订单生成器** | `src/bench/order_generator.rs` - 确定性订单序列 |
| 3 | **验证正确性** | 单元测试对比生成数据与 `golden_*.csv` |

**成功标准**: 生成的数据与黄金 CSV 逐字节匹配（每行的 `order_id`, `price`, `size`, `uid` 完全一致）。

---

### 2. 参考算法: LCG PRNG

Exchange-Core 项目使用 Java 的 `java.util.Random` 作为 PRNG。我们必须实现一个比特级精确的副本。

---

### 3. 黄金数据参考

**位置**: `docs/exchange_core_verification_kit/golden_data/`

| 文件 | 记录数 | Seed | 描述 |
|------|--------|------|------|
| `golden_single_pair_margin.csv` | 11,000 | 1 | 保证金（期货）合约 |
| `golden_single_pair_exchange.csv` | 11,000 | 1 | 现货交易 |

---

### 4. 实施清单

- [x] **步骤 1**: 创建 `src/bench/mod.rs`
- [x] **步骤 2**: 在 `src/bench/java_random.rs` 中实现 `JavaRandom`
    - [x] 单元测试: 验证前 100 个随机数与 Java 输出匹配
- [x] **步骤 3**: 在 `src/bench/order_generator.rs` 中实现 `TestOrdersGenerator`
    - [x] Pareto 分布用于用户权重
    - [x] 订单生成逻辑 (GTC 阶段)
    - [x] 使用 `Objects.hash` 公式进行种子派生
- [x] **步骤 4**: 加载并对比黄金 CSV
    - [x] `#[test] fn test_golden_single_pair_margin()`
    - [x] `#[test] fn test_golden_single_pair_exchange()`

---

### 5. 实现结果

> [!NOTE]
> **✅ 100% 比特级精确匹配已达成** - 所有字段现在与 Java 参考实现完全匹配。

| 字段 | 匹配状态 | 公式 |
|:----:|:--------:|:-----|
| **Price** | ✅ 100% | `pow(r,2)*deviation` + 4 值平均 |
| **Size** | ✅ 100% | `1 + rand(6)*rand(6)*rand(6)` |
| **Action** | ✅ 100% | `(rand(4)+priceDir>=2) ? BID : ASK` |
| **UID** | ✅ 100% | Pareto 用户账户生成 |

**关键实现细节**:
1. `JavaRandom` - 比特级精确的 `java.util.Random` LCG
2. 种子派生: `Objects.hash(symbol*-177277, seed*10037+198267)`
3. 用户账户: `1 + (int)paretoSample` 公式
4. 货币顺序: `[978, 840]` 基于 HashMap bucket 索引
5. 用户选择: `min(users.size, max(2, symbolMessages/5))`

---

### 6. 验证命令

**一键验证**:
```bash
# 运行所有黄金数据验证测试
cargo test golden_ -- --nocapture
```

**详细对比测试**:
```bash
# 逐行对比前 20 个订单与黄金 CSV
cargo test test_generator_vs_golden_detailed -- --nocapture
```

**所有 Benchmark 测试**:
```bash
# 运行 bench 模块的所有测试
cargo test bench:: -- --nocapture
```

**预期输出**:
```
[  1] ✅ | Golden: id=1, price=34386, size=  1, action=BID, uid=377
[  2] ✅ | Golden: id=2, price=34135, size=  1, action=BID, uid=110
[  3] ✅ | Golden: id=3, price=34347, size=  2, action=BID, uid=459
...
[20] ✅ | Golden: id=20, price=34297, size=  1, action=BID, uid=491
```
