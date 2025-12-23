# Summary

## 🛠️ 第一阶段：核心匹配引擎 (Core Engine)

- [0x01 Genesis | 创世纪](./0x01-genesis.md)
- [0x02 Float Curse | 浮点数的诅咒](./0x02-the-curse-of-float.md)
- [0x03 Decimal World | 十进制世界](./0x03-decimal-world.md)
- [0x04 BTree OrderBook | 重构 OrderBook](./0x04-btree-orderbook.md)
- [0x05 User Balance | 余额管理](./0x05-user-balance.md)
- [0x06 Enforced Balance | 强制余额](./0x06-enforced-balance.md)
- [0x07 Testing Framework | 测试框架](./0x07-a-testing-framework.md)
    - [0x07-b Perf Baseline | 性能基线](./0x07-b-perf-baseline.md)
- [0x08 Trading Pipeline | 交易流水线](./0x08-a-trading-pipeline-design.md)
    - [0x08-b UBScore Implementation | UBScore 实现](./0x08-b-ubscore-implementation.md)
    - [0x08-c Complete Event Flow | 完整事件流](./0x08-c-ring-buffer-pipeline.md)
    - [0x08-d Complete Order Lifecycle | 完整订单生命周期](./0x08-d-complete-order-lifecycle.md)
    - [0x08-e Performance Profiling | 性能优化](./0x08-e-cancel-optimization.md)
    - [0x08-f Ring Buffer Pipeline | Pipeline 实现](./0x08-f-ring-buffer-pipeline.md)
    - [0x08-g Multi-Thread Pipeline | 多线程 Pipeline](./0x08-g-multi-thread-pipeline.md)
    - [0x08-h Performance Monitoring | 性能监控](./0x08-h-performance-monitoring.md)
- [0x09 接入层集成与持久化校验](./0x09-a-gateway.md)
    - [0x09-b Settlement Persistence | Settlement 持久化](./0x09-b-settlement-persistence.md)
    - [0x09-c WebSocket Push | WebSocket 推送](./0x09-c-websocket-push.md)
    - [0x09-d K-Line Aggregation | K-Line 聚合](./0x09-d-kline-aggregation.md)
    - [0x09-e OrderBook Depth | 盘口深度](./0x09-e-orderbook-depth.md)
    - [0x09-f Integration Test | 集成测试](./0x09-f-integration-test.md)

---

## 🚀 第二阶段：产品化与业务闭环 (Productization)

- [Part II: Productization | 第二部分：产品化](./0x0A-part-ii-introduction.md)
    - [0x0A-a Account System | 账户体系](./0x0A-a-account-system.md)
    - [0x0A-b ID Specification | ID 规范](./0x0A-a-id-specification.md)
    - [0x0A-c Authentication | 安全鉴权](./0x0A-b-api-auth.md)
- [0x0B Funding & Transfer | 资金体系: 充提与划转](./0x0B-funding.md)
- [0x0C Fee System | 经济模型: 手续费](./0x0C-fee-system.md)
- [0x0D Snapshot & Recovery | 鲁棒性: 快照与恢复](./0x0D-snapshot-recovery.md)

---

## ⚡ 第三阶段：极致单点性能优化 (Extreme Optimization)

- [0x10 Zero-Copy Optimization | Zero-Copy 反序列化优化](./0x10-zero-copy.md)
- [0x11 CPU Affinity & Cache | 缓存友好性与 CPU 亲和性](./0x11-cpu-affinity.md)
- [0x12 SIMD 矢量化撮合加速](./0x12-simd-matching.md)

---

- [Performance Report (Latest)](./perf-report.md)
- [Performance History](./perf-history/README.md)
    - [2025-12-18-0x08h](./perf-history/2025-12-18-0x08h.md)
    - [2025-12-16-0x07b](./perf-history/2025-12-16-0x07b.md)

---

# Reference

- [开发规范 (Development Guidelines)](../standards/development-guidelines.md)
- [API 规范 (API Conventions)](../standards/api-conventions.md)
- [ID 规范 (ID Specification)](../standards/id-specification.md)
- [命名规范 (Naming Convention)](../standards/naming-convention.md)
- [数据库选型: TDengine (Database Selection)](./database-selection-tdengine.md)
