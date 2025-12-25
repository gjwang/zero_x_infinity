# 0x0D WAL & Snapshot 开发交付清单

> **Target**: Developer, QA, DevOps 团队  
> **Branch**: `0x0D-wal-snapshot-design`  
> **Status**: ✅ 设计完成，准备实施

---

## 📦 交付物概览

| 类型 | 数量 | 说明 |
|------|------|------|
| 架构设计文档 | 2 份 | 顶层设计 + 服务概览 |
| 详细设计文档 | 3 份 | UBSCore + Matching + Settlement |
| 实施计划 | 1 份 | 4 阶段开发路线图 |
| 测试清单 | 1 份 | 完整测试计划 |
| Walkthrough | 1 份 | 团队设计总览 |
| 参考实现 | 3 份 | WAL v2 + 验证工具 |
| **总计** | **11 份** | 约 38,000 字 + 500 行代码 |

---

## 📚 1. 架构设计文档

### 1.1 WAL Rotation 设计
**路径**: [`docs/agents/sessions/architect/0x0D-wal-rotation-design.md`](../architect/0x0D-wal-rotation-design.md)

**内容**:
- Producer-Consumer WAL 模型
- 服务隔离存储架构
- WAL rotation 机制
- EPOCH 概念

**阅读对象**: 全员

---

### 1.2 服务级 WAL & Snapshot 设计 ⭐
**路径**: [`docs/agents/sessions/architect/0x0D-service-wal-snapshot-design.md`](../architect/0x0D-service-wal-snapshot-design.md)

**内容**:
- 三大服务概览 (UBSCore, Matching, Settlement)
- 重放协议设计
- 恢复失败场景 (4 种)
- WAL Rotation 协调策略

**阅读对象**: 全员，开始前必读

---

## 🔍 2. 详细设计文档

### 2.1 UBSCore WAL & Snapshot
**路径**: [`docs/agents/sessions/architect/0x0D-ubscore-wal-snapshot.md`](../architect/0x0D-ubscore-wal-snapshot.md)

**内容**:
- Order WAL 设计 (Order/Cancel/Deposit/Withdraw)
- Accounts Snapshot 格式
- 验证 → WAL → 内存 → 输出 流程
- 恢复流程 (Snapshot + WAL 重放)
- 配置参数

**实施**: Phase 1 (P0, 3-5 天)

---

### 2.2 Matching WAL & Snapshot
**路径**: [`docs/agents/sessions/architect/0x0D-matching-wal-snapshot.md`](../architect/0x0D-matching-wal-snapshot.md)

**内容**:
- Trade WAL 设计
- OrderBook Snapshot 格式 (多文件)
- 接收 → 撮合 → WAL → OrderBook 流程
- 恢复流程 (Snapshot + UBSCore 重放)
- 重放输出 API (给 Settlement)

**实施**: Phase 2 (P0, 3-5 天)

---

### 2.3 Settlement WAL & Snapshot
**路径**: [`docs/agents/sessions/architect/0x0D-settlement-wal-snapshot.md`](../architect/0x0D-settlement-wal-snapshot.md)

**内容**:
- Checkpoint WAL 设计 (轻量)
- 进度 Snapshot (极小)
- 无状态设计 (幂等性保证)
- 恢复流程 (Snapshot + ME 重放)

**实施**: Phase 3 (P1, 2-3 天)

---

## 🔧 3. 实施计划

### 3.1 Implementation Plan
**路径**: [`docs/agents/sessions/developer/0x0D-implementation-plan.md`](./0x0D-implementation-plan.md)

**内容**:
- 实施原则 (Write-Ahead Logging, 服务隔离)
- 4 阶段任务拆解
  - Phase 1: UBSCore WAL + Snapshot
  - Phase 2: Matching WAL + Snapshot
  - Phase 3: Settlement WAL + Snapshot
  - Phase 4: Replay Protocol
- 每个任务的代码示例
- 验收标准
- 测试策略
- 风险与缓解

**时间估算**: 13-18 天

**阅读对象**: Developer

---

## ✅ 4. 测试清单

### 4.1 QA Test Checklist
**路径**: [`docs/agents/sessions/qa/0x0D-test-checklist.md`](../qa/0x0D-test-checklist.md)

**内容**:
- Phase 1-4 测试计划
- 单元测试 (WAL, Snapshot, Recovery)
- 集成测试 (跨服务重放)
- E2E 测试 (全链路恢复)
- 性能基准测试
- 安全测试 (Checksum 篡改)
- 回归测试 (长时间运行)

**时间估算**: 14-19 天

**阅读对象**: QA

---

## 📖 5. 设计 Walkthrough

### 5.1 Design Walkthrough ⭐
**路径**: 见 Artifact (`.gemini/antigravity/brain/.../walkthrough.md`)

**内容**:
- 设计目标与技术指标
- 核心架构原则 (Producer-Consumer)
- 系统全景 (数据流 + 恢复流程)
- 三大服务设计总结
- 关键设计决策解释
- 数据流与恢复详解
- 实施路线图

**阅读对象**: 全员，最佳起点

---

## 💻 6. 参考实现

### 6.1 WAL v2 实现
**路径**: `src/wal_v2.rs`

**内容**:
- 20-byte 对齐 header
- CRC32 checksum
- WalWriterV2 / WalReaderV2
- 8 个单元测试 (包括真实文件 I/O)

**用途**: 理解 WAL 格式和实现细节

---

### 6.2 Python WAL 验证工具
**路径**: `scripts/verify_wal.py`

**内容**:
- 独立读取 WAL 文件
- Header 解析
- CRC32 校验
- Entry type 识别

**用途**: 验证 WAL 文件格式正确性

---

### 6.3 E2E 测试脚本
**路径**: `scripts/test_wal_v2_e2e.sh`

**内容**:
- Rust 写 WAL → Python 验证
- 完整的端到端测试流程

**用途**: 快速验证实现

---

## 🚀 开发快速开始

### Step 1: 环境准备
```bash
# 切换到设计分支
git checkout 0x0D-wal-snapshot-design

# 确保依赖安装
cargo build

# 运行现有测试确认环境
cargo test wal_v2
./scripts/test_wal_v2_e2e.sh
```

### Step 2: 阅读 Walkthrough
```bash
# 理解整体设计（推荐最先阅读）
cat .gemini/antigravity/brain/.../walkthrough.md
```

### Step 3: 阅读 Implementation Plan
```bash
# 了解实施细节
cat docs/agents/sessions/developer/0x0D-implementation-plan.md
```

### Step 4: 选择 Phase 开始实施

#### Phase 1: UBSCore (P0, 优先)
```bash
# 阅读详细设计
cat docs/agents/sessions/architect/0x0D-ubscore-wal-snapshot.md

# 创建实施分支
git checkout -b 0x0D-phase1-ubscore

# 开始实施
cd src/ubscore/
```

#### Phase 2: Matching (P0)
```bash
cat docs/agents/sessions/architect/0x0D-matching-wal-snapshot.md
git checkout -b 0x0D-phase2-matching
```

#### Phase 3: Settlement (P1)
```bash
cat docs/agents/sessions/architect/0x0D-settlement-wal-snapshot.md
git checkout -b 0x0D-phase3-settlement
```

---

## 📊 实施时间线

```
Week 1
├── Phase 1: UBSCore WAL + Snapshot (3-5 天)
└── 单元测试 + 集成测试

Week 2
├── Phase 2: Matching WAL + Snapshot (3-5 天)
└── E2E 测试 (UBSCore → Matching)

Week 3
├── Phase 3: Settlement WAL + Snapshot (2-3 天)
├── Phase 4: Replay Protocol (2 天)
└── 全链路测试

Week 4 (QA)
└── 完整测试 + 性能基准 + 回归测试
```

**总计**: 3-4 周

---

## ✅ 开始前检查清单

在开始实施前，确认：
- [ ] 已阅读 Walkthrough
- [ ] 已阅读 Implementation Plan
- [ ] 已理解整体架构 (Producer-Consumer 模型)
- [ ] 已理解服务隔离原则 (SSOT, 数据所有权)
- [ ] 开发环境已准备
- [ ] `data/` 目录结构已了解
- [ ] WAL v2 参考实现已运行成功
- [ ] 已与 QA 沟通测试计划

---

## 🔗 相关资源

| 资源 | 路径 |
|------|------|
| 架构师角色定义 | `docs/agents/architect.md` |
| Developer 角色定义 | `docs/agents/developer.md` |
| QA 角色定义 | `docs/agents/qa-engineer.md` |
| 项目路线图 | `docs/src/0x00-mvp-roadmap.md` |

---

## 📞 问题求助

| 问题类型 | 参考文档 |
|----------|----------|
| 架构理解 | Walkthrough |
| 实施细节 | Implementation Plan + Detailed Design |
| 测试相关 | QA Test Checklist |
| WAL 格式 | `src/wal_v2.rs` + `docs/agents/sessions/architect/0x0D-wal-format-spec.md` |

---

*Deliverables prepared by Architect Team on 2024-12-25*
