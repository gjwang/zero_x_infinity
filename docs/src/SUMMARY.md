# Summary

- [📊 MVP Roadmap | MVP 路线图](./0x00-mvp-roadmap.md)

---

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
    - [0x0A-b ID Specification | ID 规范](./0x0A-b-id-specification.md)
    - [0x0A-c Authentication | 安全鉴权](./0x0A-c-api-auth.md)
- [0x0B Funding & Transfer | 资金体系: 充提与划转](./0x0B-funding.md)
    - [0x0B-a Internal Transfer | 内部转账架构](./0x0B-a-transfer.md)
        - [E2E Testing Guide | E2E 测试指南](./0x0B-a-transfer-testing.md)
        - [Build & Verification Guide | 编译与验证事项](./agent-build-verification-guide.md)
- [0x0C Trade Fee | 手续费系统](./0x0C-trade-fee.md)

---

## 🔶 第三阶段：韧性与资金 (Resilience & Funding)

- [0x0D Snapshot & Recovery | 鲁棒性: 快照与恢复](./0x0D-snapshot-recovery.md)
- [0x0E OpenAPI Integration | OpenAPI 集成](./0x0E-openapi-integration.md)
- [0x0F Admin Dashboard | 管理后台](./0x0F-admin-dashboard.md)
    - [Testing Guide | 测试指南](./0x0F-admin-testing.md)
    - [Token Listing SOP | 上币操作手册](./manuals/0x0F-token-listing-sop.md)
- [0x10 Web Frontend | 前端外包需求](./0x10-web-frontend.md)
- [0x11 Deposit & Withdraw | 充值与提现 (Mock Chain)](./0x11-deposit-withdraw.md)
- [0x11-a Real Chain Integration | 真实链集成 (Sentinel)](./0x11-a-real-chain.md)
- [0x11-b Sentinel Hardening | 哨兵强化 (SegWit & ETH)](./0x11-b-sentinel-hardening.md)

---

## 🔶 第四阶段：交易集成与验证 (Trading Integration)

- [0x12 Real Trading Verification | 全链路验证 (Mock Removal)](./0x12-real-trading.md)
- [0x13 Market Data Experience | 行情数据体验 (WS Verification)](./0x13-market-data.md)

---

## ⚡ 第五阶段：极致单点性能优化 (Extreme Optimization / Metal Mode)

- [0x14 Extreme Optimization | 极致优化方法论](./0x14-extreme-optimization.md)
    - [0x14-a Benchmark Harness | 基准测试脚手架](./0x14-a-bench-harness.md)
    - [0x14-b Order Commands | 订单命令扩展](./0x14-b-order-commands.md)
- [0x15 Zero-Copy | Zero-Copy 反序列化优化](./0x15-zero-copy.md)
- [0x16 CPU Affinity & Cache | 缓存友好性与 CPU 亲和性](./0x16-cpu-affinity.md)
- [0x17 SIMD Matching Acceleration | SIMD 矢量化撮合加速](./0x17-simd-matching.md)


---

- [Performance Report (Latest) | 性能报告](./perf-report.md)
- [Performance History](./perf-history/README.md)
    - [2025-12-18-0x08h](./perf-history/2025-12-18-0x08h.md)
    - [2025-12-16-0x07b](./perf-history/2025-12-16-0x07b.md)

---

# Reference

- [Development Guidelines](../standards/development-guidelines.md)
- [API Conventions](../standards/api-conventions.md)
- [ID Specification](../standards/id-specification.md)
- [Naming Convention](../standards/naming-convention.md)
- [Money Type Safety | 资金类型安全规范](../standards/money-type-safety.md)
  - [API Money Enforcement | API层资金强制规范](../standards/api-money-enforcement.md)
- [CI Pitfalls](./standards/ci-pitfalls.md)
- [Pre-merge Checklist](./standards/pre-merge-checklist.md)
- [Build Verification Guide](./build-verification-guide.md)
- [Database Selection: TDengine](./database-selection-tdengine.md)
- [ADR-001: WebSocket Security (Strict Auth)](./architecture/decisions/ADR-001-websocket-security-auth-enforcement.md)
- [ADR-005: Unified Chain-Asset Schema](./architecture/decisions/ADR-005-unified-asset-schema.md)
- [ADR-006: User Address Decoupling](./architecture/decisions/ADR-006-user-address-decoupling.md)
- [AR-001: Request for Auth Design](./architecture/requests/AR-001-websocket-auth-design.md)

