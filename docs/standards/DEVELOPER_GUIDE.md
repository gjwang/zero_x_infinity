# 开发者必读 (Developer Guide)

本文档是项目规范的索引入口，整合所有代码规范、架构原则和交付流程。

---

## 📋 核心规范文档

| 文档 | 说明 | 链接 |
|------|------|------|
| **API 规范** | HTTP 响应格式、数字格式、命名规范 | [api-conventions.md](api-conventions.md) |
| **Gateway API** | HTTP 端点使用、请求/响应示例 | [gateway-api.md](gateway-api.md) |
| **命名规范** | 数据库字段、Rust 代码、章节编号 | [naming-convention.md](naming-convention.md) |
| **交付检查清单** | 发布前必查项 (分支、文档、代码) | [checklist.md](checklist.md) |
| **验证工作流** | 测试策略和验证流程 | [verification-workflow.md](verification-workflow.md) |


---

## 🏗️ 架构设计文档

| 文档 | 说明 | 链接 |
|------|------|------|
| **Part II 导读** | 产品化阶段概览 (0x0A-0x0D) | [../src/Part-II-Introduction.md](../src/Part-II-Introduction.md) |
| **ID 规范** | User/Asset/Symbol/Order ID 生成规则 | [../src/0x0A-a-id-specification.md](../src/0x0A-a-id-specification.md) |

---

## 🎯 快速开始

### 新开发者入门流程

1. **阅读核心规范** (30分钟)
   - [naming-convention.md](naming-convention.md) - 命名规则
   - [api-conventions.md](api-conventions.md) - API 设计规范
   - [checklist.md](checklist.md) - 交付标准

2. **环境搭建** (10分钟)
   ```bash
   # 启动数据库
   docker-compose up -d
   
   # 编译项目
   cargo build
   
   # 运行测试
   cargo test
   ```

3. **运行 Gateway** (5分钟)
   ```bash
   # 启动 Gateway
   cargo run -- --gateway --env dev
   
   # 测试 API
   curl http://localhost:8080/api/v1/health
   ```

4. **阅读 API 文档**
   - [gateway-api.md](gateway-api.md) - 了解端点使用

---

## 🔧 常用命令

### 开发环境
```bash
docker-compose up -d              # 启动数据库
docker-compose down               # 停止数据库
docker-compose logs -f postgres   # 查看 PostgreSQL 日志
```

### 编译和测试
```bash
cargo build                       # 开发编译
cargo build --release             # 生产编译
cargo test                        # 运行测试
cargo fmt                         # 代码格式化
cargo clippy -- -W clippy::all    # 代码检查
```

### 运行模式
```bash
cargo run -- --gateway --env dev  # Gateway 模式
cargo run -- --pipeline           # 单线程 Pipeline
cargo run -- --pipeline-mt        # 多线程 Pipeline
```

### 文档
```bash
mdbook build docs                 # 构建文档
mdbook serve docs                 # 本地预览 (http://localhost:3000)
```

---

## ✅ 提交前检查清单

**必须完成以下所有项**:

```bash
# 1. 代码格式化
cargo fmt

# 2. 编译检查
cargo check

# 3. 运行测试
cargo test

# 4. 代码检查
cargo clippy -- -W clippy::all

# 5. 文档构建
mdbook build docs
```

**详细检查清单**: 参见 [checklist.md](checklist.md)

---

## 🚨 关键规范速查

### API 规范 (详见 [api-conventions.md](api-conventions.md))
- **枚举值**: 使用 SCREAMING_CASE (`NEW`, `FILLED`, `BUY`, `SELL`)
- **数字格式**: 所有数字必须转换为字符串，使用 `display_decimals` 精度
- **资产表示**: 使用名称而非 ID (`"BTC"` 而非 `1`)
- **响应格式**: 统一使用 `{code, msg, data}` 结构

### Emoji 使用原则
- 避免过多使用,滥用Emoji
- 只在关键位置用 ✅ / ❌
- 数据行保持干净

### 命名规范
- 跨表字段使用表名前缀: `user_flags`, `asset_flags`
- Rust struct 字段与数据库列名一致
- 章节编号: Part I (0x01-0x09), Part II (0x0A-0x0D), Part III (0x10-0x12)

### 架构原则
- **最小外部依赖** - 逻辑内聚
- **可审计性** - 完整事件流水
- **渐进式增强** - 保持系统可运行
- **向后兼容** - 复用核心类型

---

## 🐛 故障排查

### 数据库连接失败
```bash
docker ps | grep postgres                                    # 检查状态
docker exec -it postgres psql -U trading -d trading -c "SELECT 1;"  # 测试连接
docker-compose restart postgres                              # 重启
```

### 编译错误
```bash
cargo clean      # 清理缓存
cargo update     # 更新依赖
cargo build      # 重新编译
```

### 测试失败
```bash
RUST_LOG=debug cargo test -- --nocapture    # 详细日志
cargo test test_name -- --nocapture         # 单个测试
```

---

## 📚 完整文档索引

### 规范文档
- [api-conventions.md](api-conventions.md) - **API 规范** (重要)
- [gateway-api.md](gateway-api.md) - Gateway API 使用指南
- [naming-convention.md](naming-convention.md) - 命名规范
- [checklist.md](checklist.md) - 交付检查清单
- [verification-workflow.md](verification-workflow.md) - 验证工作流

### 架构设计
- [../src/Part-II-Introduction.md](../src/Part-II-Introduction.md) - Part II 产品化导读
- [../src/0x0A-a-id-specification.md](../src/0x0A-a-id-specification.md) - ID 规范与账户结构

### 交接文档
- `.agent/handover/2025-12-22-0x0A-account-system.md` - 账户系统交接
- `.agent/handover/2025-12-16-chapter7b-final.md` - CI/CD 交接

---

**最后更新**: 2025-12-22
