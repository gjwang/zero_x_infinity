# Agent 测试注意事项

本文档记录了在本项目中进行测试时需要注意的关键事项，供未来的 AI Agent 参考。

---

## ⚠️ 关键警告：禁止使用 `pkill -f "zero_x_infinity"`

### 问题描述

在 Antigravity IDE 中执行 `pkill -f "zero_x_infinity"` 会导致 **IDE 崩溃**。

### 根因

`pkill -f` 会匹配进程的**完整命令行参数**。Antigravity IDE 的 `language_server` 进程参数中包含项目路径：

```
/Applications/Antigravity.app/.../language_server_macos_arm \
  --workspace_id file_Users_gjwang_eclipse_workspace_rust_source_zero_x_infinity
```

当你执行 `pkill -f "zero_x_infinity"` 时，它会同时杀死：
1. ✅ 你想要终止的 Gateway 进程
2. ❌ IDE 的 language_server 进程（误杀！）

### 症状

- IDE 弹出 "Antigravity server crashed unexpectedly"
- 错误日志显示 `reactive component ... not found`
- 连接错误 `ECONNREFUSED 127.0.0.1:49xxx`

### 正确做法

使用更精确的方式终止进程：

```bash
# ❌ 错误：会杀死 IDE
pkill -f "zero_x_infinity"

# ✅ 正确：方法 1 - 用 pgrep + kill
GW_PID=$(pgrep -f "./target/release/zero_x_infinity" | head -1)
if [ -n "$GW_PID" ]; then
    kill "$GW_PID"
fi

# ✅ 正确：方法 2 - 启动时记录 PID
./target/release/zero_x_infinity --gateway &
GW_PID=$!
# ... 需要停止时
kill "$GW_PID"

# ✅ 正确：方法 3 - 匹配进程名而非命令行
pkill "^zero_x_infinity$"
```

---

## CI/CD 注意事项 (GitHub Actions)

### 1. PostgreSQL 健康检查角色错误

#### 问题描述
在 GitHub Actions 服务配置中，如果不显式指定用户，`pg_isready` 之类的健康检查命令会默认使用 `root` 用户。如果数据库中没有 `root` 角色，会导致 Postgres 日志中出现大量的 `FATAL: role "root" does not exist` 报错。

#### 解决方案
在 `health-cmd` 中显式指定拥有超级权限的数据库用户：
```yaml
services:
  postgres:
    options: >-
      --health-cmd "pg_isready -U trading"
```

### 2. 隐含的服务依赖 (Service Dependencies)

#### 问题描述
在运行某些特定的集成测试（如 Account Integration）时，虽然测试目标是 PostgreSQL，但如果启动的是完整的 Gateway 二进制文件（`zero_x_infinity`），由于 Gateway 启动时会强制初始化 TDengine 连接，背景环境中必须提供 TDengine 服务。

#### 解决方案
即使当前测试不直接涉及 TDengine，只要启动了 Gateway 进程，就必须在 CI Job 的 `services` 中包含所有必需的中间件：
- 显式包含 `tdengine` 和 `postgres`。
- 执行对应的初始化脚本（`init.sh td` 和 `init.sh pg`）。

### 3. Gateway 启动时序与健康检查

#### 问题描述
- **默认超时过短**：在 CI 环境下，二进制文件启动并建立数据库连接的速度远慢于本地。原本 10s 的等待时间经常导致测试在 Gateway 准备好前就超时退出。
- **健康检查路径错误**：Gateway 的健康检查接口在 `/api/v1/health`。如果测试脚本请求 `/health`，会得到 404，导致脚本误判 Gateway 启动失败。

#### 解决方案
- **增加等待时间**：将 CI 测试脚本中的等待循环增加到 60s，并每 5s 输出一次进度日志。
- **校验路径**：统一使用完整的 `/api/v1/health` 路径。
- **服务沉淀**：在 `init.sh td` 后增加 `sleep 5`，让 TDengine 的元数据初始化有足够的“沉淀时间”，避免出现 `WAL ERROR` 或 `Table does not exist` 的瞬时报错。

---

*最后更新：2025-12-23*
