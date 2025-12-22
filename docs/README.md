# 0xInfinity 文档导航

欢迎来到 0xInfinity 交易系统文档中心。

---

## 📋 开发规范

**入口**: [开发者指南](standards/DEVELOPER_GUIDE.md) ⭐

核心规范文档（位于 `standards/` 目录）:

- [API 规范](standards/api-conventions.md) - HTTP 响应格式、数字格式、命名规范
- [Gateway API](standards/gateway-api.md) - 端点使用指南和示例
- [命名规范](standards/naming-convention.md) - 数据库和代码命名规则
- [交付检查清单](standards/checklist.md) - 发布前必查项
- [验证工作流](standards/verification-workflow.md) - 测试策略和验证流程

---

## 📚 技术文档

使用 mdbook 构建，查看 [在线文档](https://gjwang.github.io/zero_x_infinity/)

### Part I: 核心引擎 (0x01-0x09)
- [0x01 创世纪](src/0x01-genesis.md) - 最简撮合原型
- [0x02-03 浮点数与定点数](src/0x02-the-curse-of-float.md) - 金融级精度
- [0x04 BTree OrderBook](src/0x04-btree-orderbook.md) - O(log n) 撮合
- [0x05-06 用户余额](src/0x05-user-balance.md) - 锁定/解锁机制
- [0x07 测试框架](src/0x07-a-testing-framework.md) - 100K 订单基线
- [0x08 多线程 Pipeline](src/0x08-a-trading-pipeline-design.md) - 四线程并发架构
- [0x09 接入层 & 持久化](src/0x09-a-gateway.md) - Gateway, TDengine, WebSocket

### Part II: 产品化 (0x0A-0x0D)
- [Part II 导读](src/Part-II-Introduction.md) - 产品化阶段概览
- [0x0A 账户体系](src/0x0A-a-id-specification.md) - ID 规范与账户结构
- [0x0B 资金体系](src/0x0B-funding.md) - Funding/Spot 双账户
- [0x0C 经济模型](src/0x0C-fee-system.md) - 手续费计算
- [0x0D 快照与恢复](src/0x0D-snapshot-recovery.md) - 优雅停机

### Part III: 极致优化 (0x10-0x12)
- [0x10 零拷贝](src/0x10-zero-copy.md)
- [0x11 CPU 亲和性](src/0x11-cpu-affinity.md)
- [0x12 SIMD 撮合](src/0x12-simd-matching.md)

---

## 🚀 快速开始

### 新开发者入门 (45分钟)

1. **阅读核心规范** (30分钟)
   - [开发者指南](standards/DEVELOPER_GUIDE.md)
   - [API 规范](standards/api-conventions.md)
   - [命名规范](standards/naming-convention.md)

2. **环境搭建** (10分钟)
   ```bash
   docker-compose up -d
   cargo build
   cargo test
   ```

3. **运行 Gateway** (5分钟)
   ```bash
   cargo run -- --gateway --env dev
   curl http://localhost:8080/api/v1/health
   ```

---

## 🏗️ 目录结构

```
docs/
├── README.md                    # 本文件（导航入口）
├── standards/                   # 规范文档
│   ├── DEVELOPER_GUIDE.md       # 开发者指南 ⭐
│   ├── api-conventions.md       # API 规范
│   ├── naming-convention.md     # 命名规范
│   ├── checklist.md             # 交付检查清单
│   ├── gateway-api.md           # Gateway API
│   └── verification-workflow.md # 验证工作流
├── src/                         # mdbook 章节
│   ├── SUMMARY.md
│   ├── Part-II-Introduction.md
│   ├── 0x01-genesis.md
│   └── ...
├── archive/                     # 归档文档
└── book/                        # mdbook 构建输出
```

---

## 📖 本地预览文档

```bash
# 安装 mdbook (首次)
cargo install mdbook

# 构建文档
mdbook build docs

# 本地预览
mdbook serve docs

# 访问 http://localhost:3000
```

---

**最后更新**: 2025-12-22
