# 0x08-h 性能监控与架构可观测性 (Performance Monitoring & Observability)

在构建高性能低延迟交易系统时，"如果你无法测量它，你就无法优化它"。本章重点在于为我们的多线程 Pipeline 引入生产级的性能监控和延迟指标分析。

## 监控维度

我们将从以下四个关键维度构建监控体系：

### 1. 延迟指标 (Latency Metrics)
对于 HFT (High Frequency Trading) 系统，平均延迟往往是误导性的，我们更关心**长尾延迟 (Tail Latency)**。
- **P50 (Median)**: 中位数延迟，反映平均水平。
- **P99 / P99.9**: 长尾延迟，反映系统在极端情况下的稳定性。
- **Max**: 峰值延迟，通常由系统抖动 (Jitter) 或 GC/系统调用引起。

### 2. 吞吐量 (Throughput)
系统在单位时间内处理能力的上限。
- **Orders/sec**: 每秒处理订单数。
- **Trades/sec**: 每秒撮合成交数。
- **Events/sec**: 每秒产生的事件流数量。

### 3. 队列深度与背压 (Queue Depth & Backpressure)
在多线程 Pipeline 中，监控 Ring Buffer 的占用情况至关重要。
- 如果队列长期接近满载，说明下游处理能力不足，正在产生背压。
- 队列抖动情况可以帮助我们优化 CPU 亲和性 (Affinity) 和线程调度。

### 4. 架构内部阶段耗时 (Architectural Breakdown)
正如 `perf.rs` 已经实现的，我们需要清晰地知道时间花在了哪里：
- Pre-Trade (WAL + Lock)
- Matching Engine
- Settlement
- Event Logging

## 测试执行方法 (Test Execution)

为了重现测试结果，请确保在 **Release 模式**下运行，并指定相应的数据集路径。

### 1. 数据集准备
默认使用 130 万量级的订单数据集（包含 30% 撤单）：
- **Path**: `fixtures/test_with_cancel_highbal/`
- **内容**: `orders.csv`, `balances_init.csv`

### 2. 运行单线程流水线 (Single-Thread)
```bash
cargo run --release -- --pipeline --input fixtures/test_with_cancel_highbal
```

### 3. 运行多线程流水线 (Multi-Thread)
```bash
cargo run --release -- --pipeline-mt --input fixtures/test_with_cancel_highbal
```

### 4. 自动化对比脚本
我们提供了一个脚本可以一次性运行两种模式并进行结果对等性校验：
```bash
./scripts/test_pipeline_compare.sh highbal
```

---

## 执行结果与分析 (1.3M 数据集)

针对 130 万订单数据集（包含 30% 撤单，高余额模式），我们对单线程和多线程流水线进行了对比测试。

### 1. 单线程流水线 (Single-Thread Pipeline)

*   **吞吐量**: ~210,000 orders/sec
*   **端到端延迟 (P50)**: 1.25 µs
*   **端到端延迟 (P99)**: 196 µs
*   **架构耗时占比**:
    *   Pre-Trade: 5.6% (0.64 µs/order)
    *   Matching: 91.5% (13.70 µs/order)
    *   Settlement: 0.2%
    *   Event Log: 2.7%

### 2. 多线程流水线 (Multi-Thread Pipeline)

*   **吞吐量**: ~74,000 orders/sec
*   **端到端延迟 (P50)**: 122,853,375 ns (~122 ms)
*   **端到端延迟 (P99)**: 188 ms
*   **架构耗时占比 (并行任务量)**:
    1. Pre-Trade: 14.69s (43.0%) [11.30 µs/order]
    2. Matching: 15.96s (46.8%) [15.96 µs/order]
    3. Settlement: 0.77s (2.3%)
    4. Event Log: 2.70s (7.9%)

### 分析结论

1.  **并行能力的体现**: 多线程模式下，总任务量 (Total Tracked ~34s) 远大于执行时间 (17.5s)，证明四个核心正在高效协作。
2.  **瓶颈识别**: 
    - **ME (Matching Engine)** 依然是最大的串行瓶颈，耗时占比最高。
    - **UBSCore (Pre-Trade)** 在多线程下因处理大量异步结算回调，从 0.6µs 衰减至 11µs，成为第二大性能杀手。
3.  **延迟的代价**: 多线程引入了显著的消息传递开销和排队效应，导致端到端延迟从微秒级回退到了毫秒级。对于极低延迟交易，单线程自旋架构依然具有不可替代的优势。
