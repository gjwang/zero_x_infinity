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
| `golden_single_pair_margin.csv` | 1,100 | 1 | Margin (futures) contract |
| `golden_single_pair_exchange.csv` | 1,100 | 1 | Spot exchange |

**CSV Format**:
```csv
phase,command,order_id,symbol,price,size,action,order_type,uid
```

---

### 4. Implementation Checklist

- [ ] **Step 1**: Create `src/bench/mod.rs`
- [ ] **Step 2**: Implement `JavaRandom` in `src/bench/java_random.rs`
    - [ ] Unit test: verify first 100 random numbers match Java output
- [ ] **Step 3**: Implement `TestOrdersGenerator` in `src/bench/order_generator.rs`
    - [ ] Pareto distribution for symbol/user weights
    - [ ] Order generation logic (GTC, IOC, Cancel, Move, Reduce)
- [ ] **Step 4**: Load and compare with golden CSV
    - [ ] `#[test] fn test_golden_single_pair_margin()`
    - [ ] `#[test] fn test_golden_single_pair_exchange()`

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
| `golden_single_pair_margin.csv` | 1,100 | 1 | 保证金（期货）合约 |
| `golden_single_pair_exchange.csv` | 1,100 | 1 | 现货交易 |

---

### 4. 实施清单

- [ ] **步骤 1**: 创建 `src/bench/mod.rs`
- [ ] **步骤 2**: 在 `src/bench/java_random.rs` 中实现 `JavaRandom`
    - [ ] 单元测试: 验证前 100 个随机数与 Java 输出匹配
- [ ] **步骤 3**: 在 `src/bench/order_generator.rs` 中实现 `TestOrdersGenerator`
    - [ ] Pareto 分布用于交易对/用户权重
    - [ ] 订单生成逻辑 (GTC, IOC, Cancel, Move, Reduce)
- [ ] **步骤 4**: 加载并对比黄金 CSV
    - [ ] `#[test] fn test_golden_single_pair_margin()`
    - [ ] `#[test] fn test_golden_single_pair_exchange()`
