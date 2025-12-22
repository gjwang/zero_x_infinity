# 命名规范 (Naming Convention)

本文档定义项目中数据库和代码的命名规则。

---

## 1. 核心原则

| 原则 | 说明 |
|------|------|
| **避免通用命名** | 不要使用 `flags`, `status`, `data` 等在多表重复的名称 |
| **表名前缀** | 跨表重复的字段应加表名前缀：`user_flags`, `asset_flags` |
| **可搜索性** | 命名应方便全局搜索，减少歧义 |

---

## 2. 数据库字段

### ❌ 错误示例
```sql
-- 三个表都叫 flags，难以区分
CREATE TABLE users (... flags INT ...);
CREATE TABLE assets (... flags INT ...);
CREATE TABLE symbols (... flags INT ...);
```

### ✅ 正确示例
```sql
-- 各表使用带前缀的字段名
CREATE TABLE users (... user_flags INT ...);
CREATE TABLE assets (... asset_flags INT ...);
CREATE TABLE symbols (... symbol_flags INT ...);
```

---

## 3. 章节编号

| 阶段 | 范围 | 说明 |
|------|------|------|
| Part I | 0x01-0x09 | 核心引擎 |
| Part II | 0x0A-0x0D | 产品化 |
| Part III | 0x10-0x12 | 极致优化 |

---

## 4. Rust 命名

| 类型 | 规则 | 示例 |
|------|------|------|
| struct 字段 | 与数据库列名一致 | `user_flags: i32` |
| 常量模块 | 小写 + 下划线 | `user_flags::CAN_LOGIN` |
| 枚举 | PascalCase | `UserStatus::Active` |
