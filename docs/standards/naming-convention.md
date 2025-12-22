# 命名规范 (Naming Convention)

本文档定义项目中数据库和代码的命名规则。

---

## 1. 核心原则

| 原则 | 说明 |
|------|------|
| **避免通用命名** | 不要使用 `flags` 等在多表重复的名称 |
| **表名前缀** | 跨表重复的字段应加表名前缀：`user_flags`, `asset_flags` |
| **可搜索性** | 命名应方便全局搜索，减少歧义 |

---

## 2. 数据库表名

### 规则：所有表名使用 `_tb` 后缀

**目的**：
- 方便全局搜索时区分表名和其他标识符
- 避免与保留字冲突
- 提高 SQL 代码可读性

### ❌ 错误示例
```sql
CREATE TABLE users (...);
CREATE TABLE assets (...);
CREATE TABLE symbols (...);
```

### ✅ 正确示例
```sql
CREATE TABLE users_tb (...);
CREATE TABLE assets_tb (...);
CREATE TABLE symbols_tb (...);
```

**查询示例**：
```sql
SELECT * FROM users_tb WHERE user_id = 1001;
SELECT a.asset, s.symbol 
FROM assets_tb a 
JOIN symbols_tb s ON s.base_asset_id = a.asset_id;
```

---

## 3. 数据库字段

### 规则：跨表重复字段使用表名前缀

### ❌ 错误示例
```sql
-- 三个表都叫 flags，难以区分
CREATE TABLE users_tb (... flags INT ...);
CREATE TABLE assets_tb (... flags INT ...);
CREATE TABLE symbols_tb (... flags INT ...);
```

### ✅ 正确示例
```sql
-- 各表使用带前缀的字段名
CREATE TABLE users_tb (... user_flags INT ...);
CREATE TABLE assets_tb (... asset_flags INT ...);
CREATE TABLE symbols_tb (... symbol_flags INT ...);
```

---

## 4. 章节编号

| 阶段 | 范围 | 说明 |
|------|------|------|
| Part I | 0x01-0x09 | 核心引擎 |
| Part II | 0x0A-0x0D | 产品化 |
| Part III | 0x10-0x12 | 极致优化 |

---

## 5. Rust 命名

| 类型 | 规则 | 示例 |
|------|------|------|
| struct 字段 | 与数据库列名一致 | `user_flags: i32` |
| 常量模块 | 小写 + 下划线 | `user_flags::CAN_LOGIN` |
| 枚举 | PascalCase | `UserStatus::Active` |
| 方法参数 | 与数据库字段名一致 | `get_by_asset(asset: &str)` |

---

## 6. 命名一致性检查清单

在提交代码前，请确认：

- [ ] 所有数据库表名使用 `_tb` 后缀
- [ ] 跨表重复字段使用表名前缀（如 `user_flags`, `asset_flags`）
- [ ] Rust struct 字段名与数据库列名完全一致
- [ ] 方法参数名与数据库字段名一致（如 `get_by_asset(asset)`）
- [ ] SQL 查询中表名使用 `_tb` 后缀

---

**最后更新**: 2025-12-22  
**版本**: 1.1
