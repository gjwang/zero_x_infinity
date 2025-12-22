# 0x0A 账户管理系统 - 开发者交接

> **日期**: 2025-12-22  
> **分支**: `0x10-productization-core`  
> **状态**: 架构设计完成，待 Gateway 集成

---

## 1. 已完成模块

| 模块 | 文件 | 说明 |
|------|------|------|
| Docker | `docker-compose.yml` | PostgreSQL 18 + TDengine |
| Migration | `migrations/001_init_schema.sql` | users, assets, symbols 三表 |
| 连接池 | `src/account/db.rs` | sqlx PgPool 封装 |
| 模型 | `src/account/models.rs` | User, Asset, Symbol structs |
| Repository | `src/account/repository.rs` | CRUD 操作 |

---

## 2. 文档结构

```
docs/src/
├── 0x0A-part-ii-introduction.md  # Part II 导读 (主章节)
├── 0x0A-a-id-specification.md    # ID 规范
└── 0x0A-b-auth.md                # 安全鉴权
```

---

## 3. 数据库表

| 表 | 关键字段 | flags 位 |
|---|---|---|
| users | user_id, username, user_flags | 0x01=login, 0x02=trade, 0x04=withdraw |
| assets | asset_id, asset, asset_flags | 0x01=deposit, 0x02=withdraw, 0x04=trade |
| symbols | symbol_id, symbol, symbol_flags | 0x01=tradable, 0x02=visible |

---

## 4. 待开发者完成

- [ ] `src/lib.rs` 添加 `pub mod account;`
- [ ] Gateway 启动时加载 assets/symbols
- [ ] 配置文件添加 `postgres_url`
- [ ] 可选: `/api/v1/assets` 端点
- [ ] 可选: `/api/v1/symbols` 端点

---

## 5. 验收命令

```bash
docker-compose up -d
cargo build
cargo test
curl http://localhost:8080/api/v1/symbols
```
