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

## 2. 数据库表结构

### users
| 列 | 类型 | 说明 |
|---|---|---|
| user_id | BIGSERIAL | 主键 |
| username | VARCHAR(64) | 唯一用户名 |
| user_flags | INT | 权限位 (0x01=login, 0x02=trade, 0x04=withdraw) |

### assets  
| 列 | 类型 | 说明 |
|---|---|---|
| asset_id | SERIAL | 主键 |
| asset | VARCHAR(16) | 资产代码 (BTC, USDT) |
| asset_flags | INT | 权限位 (0x01=deposit, 0x02=withdraw, 0x04=trade) |

### symbols
| 列 | 类型 | 说明 |
|---|---|---|
| symbol_id | SERIAL | 主键 |
| symbol | VARCHAR(32) | 交易对 (BTC_USDT) |
| symbol_flags | INT | 权限位 (0x01=tradable, 0x02=visible) |

---

## 3. 待开发者完成

- [ ] 在 `src/lib.rs` 添加 `pub mod account;`
- [ ] Gateway 启动时加载 assets/symbols 到内存
- [ ] 配置文件添加 postgres URL
- [ ] 编写单元测试

---

## 4. 验收标准

```bash
# 1. 启动服务
docker-compose up -d

# 2. 编译
cargo build

# 3. 测试
cargo test

# 4. 验证 Gateway 加载配置
curl http://localhost:8080/api/v1/symbols
```

---

## 5. 相关文档

- `docs/src/Part-II-Introduction.md` - Part II 导读
- `docs/src/0x0A-auth.md` - 账户鉴权设计
- `docs/src/0x0A-a-id-specification.md` - ID 规范
- `docs/NAMING_CONVENTION.md` - 命名规范
- `docs/CHECKLIST_ARCHITECTURE_DESIGN.md` - 架构检查清单
