# Summary

## 🛠️ 第一阶段：核心匹配引擎 (Core Engine)

- [0x01 创世纪: 基础引擎 (Genesis)](./0x01-genesis.md)
- [0x02 浮点数的诅咒 (The Curse of Float)](./0x02-the-curse-of-float.md)
- [0x03 十进制世界 (Decimal World)](./0x03-decimal-world.md)
- [0x04 Orderbook数据结构重构](./0x04-btree-orderbook.md)
- [0x05 用户账户与余额管理 (User Balance)](./0x05-user-balance.md)
- [0x06 强制余额管理 (Enforced Balance)](./0x06-enforced-balance.md)
- [0x07 测试框架与性能基线](./0x07-a-testing-framework.md)
    - [0x07-b 性能基线](./0x07-b-perf-baseline.md)
- [0x08 交易流水线与多线程优化](./0x08-a-trading-pipeline-design.md)
    - [0x08-b UBScore 实现](./0x08-b-ubscore-implementation.md)
    - [0x08-c Ring Buffer Pipeline](./0x08-c-ring-buffer-pipeline.md)
    - [0x08-d 完整订单生命周期](./0x08-d-complete-order-lifecycle.md)
    - [0x08-e Cancel 优化](./0x08-e-cancel-optimization.md)
    - [0x08-f Ring Buffer 优化](./0x08-f-ring-buffer-pipeline.md)
    - [0x08-g 多线程 Pipeline](./0x08-g-multi-thread-pipeline.md)
    - [0x08-h 性能监控](./0x08-h-performance-monitoring.md)
- [0x09 接入层集成与持久化校验](./0x09-a-gateway.md)
    - [0x09-b Settlement 持久化](./0x09-b-settlement-persistence.md)
    - [0x09-c WebSocket 推送](./0x09-c-websocket-push.md)
    - [0x09-d K-Line 聚合](./0x09-d-kline-aggregation.md)
    - [0x09-e OrderBook Depth](./0x09-e-orderbook-depth.md)
    - [0x09-f 集成测试](./0x09-f-integration-test.md)

---

## 🚀 第二阶段：产品化与业务闭环 (Productization)

- [0x0A 第二部分导读 (Part II Introduction)](./0x0A-part-ii-introduction.md)
    - [0x0A-a 账户体系 (Account System)](./0x0A-a-account-system.md)
    - [0x0A-b ID 规范 (ID Specification)](./0x0A-b-id-specification.md)
    - [0x0A-c 安全鉴权 (Auth)](./0x0A-c-auth.md)
- [0x0B 资金体系: 充提与划转 (Funding & Transfer)](./0x0B-funding.md)
- [0x0C 经济模型: 手续费 (Fee System)](./0x0C-fee-system.md)
- [0x0D 鲁棒性: 快照与恢复 (Snapshot & Recovery)](./0x0D-snapshot-recovery.md)

---

## ⚡ 第三阶段：极致单点性能优化 (Extreme Optimization)

- [0x10 Zero-Copy 反序列化优化](./0x10-zero-copy.md)
- [0x11 缓存友好性与 CPU 亲和性](./0x11-cpu-affinity.md)
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
- [命名规范 (Naming Convention)](../standards/naming-convention.md)
- [数据库选型: TDengine (Database Selection)](./database-selection-tdengine.md)
