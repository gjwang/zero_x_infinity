# 0x08-h Performance Monitoring & Observability

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📦 Code Changes**: [View Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.8-f-ring-buffer-pipeline...v0.8-h-performance-monitoring) | [Key File: pipeline_services.rs](https://github.com/gjwang/zero_x_infinity/blob/main/src/pipeline_services.rs)

"If you can't measure it, you can't improve it." This chapter focuses on introducing production-grade performance monitoring and observability for our multi-threaded pipeline.

## Monitoring Dimensions

### 1. Latency Metrics
In HFT, averages are misleading. We care about **Tail Latency**.
- **P50 (Median)**: General performance.
- **P99 / P99.9**: Stability in extreme cases.
- **Max**: Jitter, GC, or system calls.

### 2. Throughput
- **Orders/sec**: Processing capacity.
- **Trades/sec**: Matching capacity.

### 3. Queue Depth & Backpressure
Monitoring Ring Buffer occupancy reveals downstream bottlenecks and jitter.

### 4. Architectural Breakdown
Knowing where time is spent (Pre-Trade vs Matching vs Settlement).

---

## Test Execution

**Dataset**: 1.3M orders (30% cancel) from `fixtures/test_with_cancel_highbal/`.

**Single-Thread Run**:
```bash
cargo run --release -- --pipeline --input fixtures/test_with_cancel_highbal
```

**Multi-Thread Run**:
```bash
cargo run --release -- --pipeline-mt --input fixtures/test_with_cancel_highbal
```

**Compare Script**:
```bash
./scripts/test_pipeline_compare.sh highbal
```

---

## Analysis Results (1.3M Dataset)

### 1. Single-Thread Pipeline
*   **Throughput**: 210,000 orders/sec (P50 Latency: 1.25 µs)
*   **Breakdown**:
    *   Matching Engine: **91.5%** (The bottleneck)
    *   UBSCore Lock: 5.6%
    *   Persistence: 2.7%

### 2. Multi-Thread Pipeline (After Service Refactor)
*   **Throughput**: ~64,450 orders/sec
*   **E2E Latency (P50)**: ~113 ms
*   **E2E Latency (P99)**: ~188 ms

### Conclusion
1.  **Parallelism Works**: Total task CPU time (~34s) > Wall time (17.5s).
2.  **Bottleneck**: **Matching Engine** remains the serial bottleneck (~52k ops/s limit).
3.  **Latency Cost**: Multi-threading introduces significant message passing latency (µs → ms).

---

## Logging & Observability

We introduced a production-grade asynchronous logging system using `tracing`.

### 1. Non-blocking I/O
Using `tracing-appender` with a dedicated worker thread and memory buffer to prevent I/O blocking.

### 2. Environment-driven Config
*   **Dev**: Detailed, human-readable.
*   **Prod**: JSON format, high-frequency tracing disabled (`0XINFI=off`).

### 3. Standardized Targets
All pipeline logs use the **`0XINFI`** namespace (e.g., `0XINFI::ME`, `0XINFI::UBSC`) for precise filtering.

---

## Intent-Based Design: From Functions to Services

> "Good architecture is not designed upfront, but evolved through refactoring."

We refactored tightly coupled `spawn_*` functions into decoupled **Service Structs**.

### Problem: Coupled Functions

```rust
// ❌ Business logic buried in thread spawning
fn spawn_me_stage(...) -> JoinHandle<OrderBook> {
    thread::spawn(move || {
        // Logic locked inside closure
    })
}
```

*   **Untestable**: Cannot unit test logic without spawning threads.
*   **Not Reusable**: Cannot be used in single-thread mode.

### Solution: Service Structs

```rust
// ✅ Intent is clear and decoupled
pub struct MatchingService {
    book: OrderBook,
    // ...
}

impl MatchingService {
    pub fn run(&mut self, shutdown: &ShutdownSignal) { ... }
}
```

### Benefits
*   **Testability**: Services can be instantiated and tested in isolation.
*   **Reusability**: Core logic is decoupled from threading model.
*   **Clarity**: Code expresses "what" (Service), not just "how" (Thread).

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📦 代码变更**: [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/v0.8-f-ring-buffer-pipeline...v0.8-h-performance-monitoring) | [关键文件: pipeline_services.rs](https://github.com/gjwang/zero_x_infinity/blob/main/src/pipeline_services.rs)

在构建高性能低延迟交易系统时，"如果你无法测量它，你就无法优化它"。本章重点在于为我们的多线程 Pipeline 引入生产级的性能监控和延迟指标分析。

## 监控维度

### 1. 延迟指标 (Latency Metrics)
对于 HFT 系统，平均延迟往往是误导性的，我们更关心**长尾延迟 (Tail Latency)**。
- **P50 (Median)**: 中位数延迟，反映平均水平。
- **P99 / P99.9**: 长尾延迟，反映系统在极端情况下的稳定性。
- **Max**: 峰值延迟，通常由系统抖动 (Jitter) 或 GC/系统调用引起。

### 2. 吞吐量 (Throughput)
- **Orders/sec**: 每秒处理订单数。
- **Trades/sec**: 每秒撮合成交数。

### 3. 队列深度与背压 (Queue Depth & Backpressure)
监控 Ring Buffer 的占用情况，识别下游瓶颈。

### 4. 架构内部阶段耗时 (Architectural Breakdown)
清晰地知道时间花在了哪里：Pre-Trade / Matching / Settlement / Logging。

## 测试执行方法

**数据集**: 130 万订单（含 30% 撤单） `fixtures/test_with_cancel_highbal/`。

**运行单线程**:
```bash
cargo run --release -- --pipeline --input fixtures/test_with_cancel_highbal
```

**运行多线程**:
```bash
cargo run --release -- --pipeline-mt --input fixtures/test_with_cancel_highbal
```

**对比脚本**:
```bash
./scripts/test_pipeline_compare.sh highbal
```

## 执行结果与分析 (1.3M 数据集)

### 1. 单线程流水线
*   **性能**: 210,000 orders/sec (P50: 1.25 µs)
*   **瓶颈**: **Matching Engine** 耗时 91.5%，是最大瓶颈。

### 2. 多线程流水线 (重构后)
*   **吞吐量**: ~64,450 orders/sec
*   **端到端延迟 (P50)**: ~113 ms
*   **端到端延迟 (P99)**: ~188 ms

### 结论
1.  **并行有效**: CPU 总耗时远大于执行时间。
2.  **瓶颈**: **Matching Engine** 依然是最大的串行瓶颈 (吞吐上限 ~52k)。
3.  **延迟**: 多线程引入的消息传递开销导致端到端延迟从微秒级退化到毫秒级。

## 日志与可观测性

引入基于 `tracing` 的生产级异步日志体系。

### 1. 异步非阻塞架构
使用 `tracing-appender` 独立线程写入日志，不阻塞业务线程。

### 2. 环境驱动配置
Dev 开启详细日志，Prod 使用 JSON 并关闭高频追踪。

### 3. 标准化日志目标
使用 **`0XINFI`** 命名空间 (如 `0XINFI::ME`) 实现精细过滤。

## 意图编码：从函数到服务

> "好的架构不是一开始就设计出来的，而是通过不断重构演进出来的。"

我们将紧耦合的 `spawn_*` 函数重构为解耦的 **Service 结构体**。

### 问题：紧耦合

```rust
// ❌ 业务逻辑埋在线程创建中
fn spawn_me_stage(...) {
    thread::spawn(move || { ... })
}
```
无法单元测试，无法复用。

### 解决方案：Service 结构体

```rust
// ✅ 意图清晰，解耦
pub struct MatchingService { ... }

impl MatchingService {
    pub fn run(&mut self, shutdown: &ShutdownSignal) { ... }
}
```

### 收益
*   **可测试性**: 服务可独立实例化测试。
*   **可复用性**: 核心逻辑与线程模型解耦。
*   **清晰度**: 代码表达"做什么" (Service)，而非"怎么做" (Thread)。
