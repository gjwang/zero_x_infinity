# 0x08-h 性能监控与架构可观测性 (Performance Monitoring & Observability)

> **📦 代码变更**: [查看分支](https://github.com/gjwang/zero_x_infinity/tree/0x08-h-performance-monitoring) | [查看 Diff](https://github.com/gjwang/zero_x_infinity/compare/0x08-f-ring-buffer-pipeline...0x08-h-performance-monitoring) | [关键文件: pipeline_services.rs](https://github.com/gjwang/zero_x_infinity/blob/main/src/pipeline_services.rs)

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

*   **性能概览**: 210,000 orders/sec (P50: 1.25 µs)

| 服务 (Service) | 模块 / 关键 Span | 任务总耗时 | 耗时占比 | 单笔延迟 (Latency) | 理论吞吐上限 (Throughput) |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **UBSCore** | `ORDER` (Lock) | 0.83s | 5.6% | 0.64 µs | 1.56M ops/s |
| **Matching Engine** | `ORDER` (Match) | 17.81s | 91.5% | 13.70 µs | 73.0k ops/s |
| **UBSCore** | `SETTLE` (Upd) | 0.04s | 0.2% | 0.03 µs | 33.3M ops/s |
| **Persistence** | `TRADE` (Ledger) | 0.52s | 2.7% | 0.40 µs | 2.50M ops/s |

### 2. 多线程流水线 (Multi-Thread Pipeline) - 服务化重构后

*   **吞吐量**: ~64,450 orders/sec
*   **端到端延迟 (P50)**: 112,862,875 ns (~113 ms)
*   **端到端延迟 (P99)**: 188 ms
*   **架构耗时与吞吐量 (模块级分析)**:

| 服务 (Service) | 模块 / 关键 Span | 任务总耗时 | 耗时占比 | 单笔延迟 (Latency) | 理论吞吐上限 (Throughput) |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **UBSCore** | `ORDER` (Lock) | 0.00s | 0.0% | 0.00 µs | N/A |
| **Matching Engine** | `ORDER` (Match) | 19.23s | 76.6% | 19.23 µs | 52.0k ops/s |
| **UBSCore** | `SETTLE` (Upd) | 0.51s | 2.0% | 0.76 µs | 1.31M ops/s |
| **Persistence** | `TRADE` (Ledger) | 5.35s | 21.3% | 4.12 µs | 242.9k ops/s |

### 分析结论

1.  **并行能力的体现**: 多线程模式下，总任务量 (Total Tracked ~34s) 远大于实际执行时间 (17.5s)，证明四个核心正在高效并行。
2.  **瓶颈识别**: 
    - **ME (Matching Engine)** 依然是最大的串行瓶颈，吞吐上限最低（约 81k），直接限制了系统的整体吞吐量（~74k）。
    - **UBSCore (Pre-Trade)** 在多线程下因处理大量异步结算回调，从单线程的 0.6µs 衰减至 11µs，成为第二大瓶颈。
3.  **延迟的代价**: 多线程引入了显著的消息传递开销和排队效应，导致端到端延迟从微秒级回退到了毫秒级。

---

## 日志与可观测性 (Logging & Observability)

在高并发的交易系统中，传统的日志记录往往会成为瓶颈。我们引入了基于 `tracing` 框架的生产级异步日志体系。

### 1. 异步非阻塞架构 (Non-blocking I/O)
为了不让磁盘 I/O 阻塞关键的撮合路径，我们使用了 `tracing-appender` 的非阻塞组件：
- **Worker Thread**: 日志写入操作被分发到独立线程执行。
- **Memory Buffer**: 流水线线程通过内存缓冲区传递日志，实现极速返回。

### 2. 多环境配置驱动 (Environment-driven Config)
系统支持通过 `config/{env}.yaml` 灵活调整观测深度：
- **Dev**: 开启详细追踪，人类可读格式，每日轮转。
- **Test**: 中等采样率，专注于正确性校验。
- **Prod**: 开启 **JSON 格式** 日志，每小时轮转，默认关闭高频生命周期追踪以换取性能。

### 3. 标准化日志目标与全链路追踪 (Standardized Targets & Tracing)
我们在代码中定义了具名常量（如 `TARGET_ME`, `TARGET_UBSC`）来统一日志目标。所有的流水线日志都归属于分层命名空间 **`0XINFI`**：
- `0XINFI::UBSC`: UBSCore 服务（包含 `ORDER`, `SETTLE` 动作）。
- `0XINFI::ME`: 撮合引擎服务（包含 `ORDER`, `CANCEL` 动作）。
- `0XINFI::PERS`: 持久化服务（包含 `TRADE` 动作）。

这种分层设计使得我们可以针对特定服务进行精细化的流量染色与追踪。

### 4. 动态追踪开关与 Target 过滤 (Target-based Filtering)
结合重构后的常量定义，我们通过 `EnvFilter` 实现了精细的动态控制。例如，在生产环境中，只需要设置 `log_level: "info,0XINFI=off"`，即可在保留基础系统日志的同时，完全关闭高频的流水线追踪。

在 1.3M 测试中，关闭 `0XINFI` 追踪能将系统吞吐量进一步提升约 **8%-12%**。

```yaml
# config/prod.yaml 示例
enable_tracing: false  # 全局流水线追踪开关
log_level: "info,0XINFI=off"      # 系统基础日志级别
use_json: true         # 开启 JSON 以支持 ELK 集成
```

这种设计确保了我们在排查线上复杂异常时有记录可查，而在正常运行时又能保持极致的性能。

---

## 结论：可观测性驱动开发

通过本章的重构，我们建立了一套**数据驱动**的优化闭环：
1.  **自动化报表**：引擎不再只是输出杂乱的日志，而是直接在控制台生成可以直接用于 Markdown 文档的性能表格（见 `src/perf.rs` 中的 `markdown_report` 实现）。
2.  **瓶颈导向**：通过“理论吞吐上限”这一指标，开发人员可以瞬间识别出当前系统的最弱环节（如 ME），从而避免无效的优化。
3.  **零损耗采样**：通过配置化的采样率，我们在不牺牲性能的前提下获得了 99 分位延迟的真实画像。

在下一阶段的优化中，我们将深入 ME 内部，针对本章识别出的单笔执行耗时进行针对性的数据结构优化。

---

## 意图编码：从函数到服务的演进 (Intent-Based Design)

> "好的架构不是一开始就设计出来的，而是通过不断重构演进出来的。"

本节记录了我们如何将紧耦合的函数重构为解耦的服务结构，这是**意图编码 (Intent-Based Design)** 的核心实践。

### 问题：紧耦合的 Spawn 函数

最初的 `pipeline_mt.rs` 包含 4 个 `spawn_*_stage` 函数，**将线程管理与业务逻辑紧密耦合**：

```rust
// ❌ 问题代码：业务逻辑被埋在线程创建中
fn spawn_me_stage(...) -> JoinHandle<OrderBook> {
    thread::spawn(move || {
        // 175 行撮合逻辑被"锁死"在这里
        // 无法单独测试
        // 无法在单线程模式复用
    })
}
```

这种设计的问题：

| 问题 | 影响 |
|------|------|
| **无法单元测试** | 测试业务逻辑必须启动线程 |
| **无法复用** | 单线程模式无法使用同一份逻辑 |
| **意图不清晰** | 调用者无法知道"启动 ME 服务"vs"执行撮合逻辑" |

### 解决方案：意图编码

**意图编码的核心思想：代码应该表达"做什么"，而不是"怎么做"。**

我们将每个阶段的**业务意图**提取为独立的 Service 结构体：

```rust
// ✅ 改进后：意图清晰，职责单一
pub struct MatchingService {
    book: OrderBook,
    queues: Arc<MultiThreadQueues>,
    stats: Arc<PipelineStats>,
    market: MarketContext,
}

impl MatchingService {
    /// 意图：运行撮合服务直到收到关闭信号
    pub fn run(&mut self, shutdown: &ShutdownSignal) { ... }
    
    /// 意图：取出内部组件（所有权转移）
    pub fn into_inner(self) -> OrderBook { ... }
}
```

调用者现在可以**清晰表达意图**：

```rust
// 意图：创建服务 → 启动线程 → 运行服务 → 返回结果
let t3_me = {
    let mut service = MatchingService::new(book, queues, stats, market);
    let s = shutdown.clone();
    thread::spawn(move || {
        service.run(&s);        // 意图：运行撮合
        service.into_inner()    // 意图：返回 OrderBook
    })
};
```

### 增量迁移策略

> ⚠️ **教训**: 第一次尝试一次性迁移所有服务，导致丢失约 2000 笔交易。

成功的策略是**增量迁移**：

1. **一次只迁移一个服务**
2. **每次迁移后运行完整的 `test_pipeline_compare.sh`**
3. **测试通过才提交，失败则回滚**
4. **保留原函数直到所有阶段完成**

| Phase | Service | 验证结果 |
|-------|---------|----------|
| 1 | IngestionService | ✅ 667,567 trades |
| 2 | UBSCoreService | ✅ 667,567 trades |
| 3 | MatchingService | ✅ 667,567 trades |
| 4 | SettlementService | ✅ 667,567 trades |
| Cleanup | 删除旧函数 | ✅ -467 lines |

### 收益总结

| 指标 | Before | After |
|------|--------|-------|
| `pipeline_mt.rs` 行数 | 720 | ~250 |
| 服务可测试性 | ❌ | ✅ |
| ST/MT 复用 | ❌ | ✅ (future) |
| 代码意图清晰度 | 模糊 | 清晰 |

### 设计模式：Service 结构体

我们建立了标准化的 Service 模式：

```rust
pub struct XxxService {
    // 拥有的核心组件
    component: Component,
    // 共享的基础设施
    queues: Arc<MultiThreadQueues>,
    stats: Arc<PipelineStats>,
}

impl XxxService {
    pub fn new(...) -> Self { ... }
    pub fn run(&mut self, shutdown: &ShutdownSignal) { ... }
    pub fn into_inner(self) -> Component { ... }
}
```

这种模式的优势：
- **所有权明确**：Service 拥有核心组件，调用结束后可取回
- **生命周期清晰**：`run()` 阻塞直到 shutdown，然后 `into_inner()` 返回
- **为服务拆分做好准备**：每个 Service 结构体都是独立的处理单元，未来可轻松拆分为独立进程或微服务

---

## 未来工作

- [ ] 添加 Service 级别的单元测试
