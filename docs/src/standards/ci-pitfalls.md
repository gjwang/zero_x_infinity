# CI 常见坑与解决方案

本文档汇总 GitHub Actions CI 中遇到的典型问题及解决方案。

---

## 🚨 0. 关键警告：禁止使用 `pkill -f`

### 问题描述
在 Antigravity IDE 中执行 `pkill -f "zero_x_infinity"` 会**导致 IDE 崩溃**。
因为 IDE 的 language_server 进程参数中包含项目路径，会被 `pkill -f` 误杀。

### 正确做法
**永远使用 PID 或精确匹配：**

```bash
# ✅ 方法 1: 启动时记录 PID (推荐)
./target/release/zero_x_infinity --gateway &
GW_PID=$!
# ...
kill "$GW_PID"

# ✅ 方法 2: 精确匹配进程名
pkill "^zero_x_infinity$"
```

## 1. 服务容器 (Service Containers)

### 1.1 禁止使用 `docker exec`

**问题描述**

GitHub Actions 的 `services:` 是托管服务容器，不是本地 Docker 容器。

```yaml
services:
  tdengine:
    image: tdengine/tdengine:latest
    ports:
      - 6041:6041
```

**典型报错**

```bash
docker exec tdengine taos -s "DROP DATABASE IF EXISTS trading"
# Error: No such container: tdengine
```

**解决方案**

使用 REST API 或网络协议连接，不用 `docker exec`：

```bash
# ❌ 错误
docker exec tdengine taos -s "DROP DATABASE IF EXISTS trading"

# ✅ TDengine REST API
curl -sf -u root:taosdata -d "DROP DATABASE IF EXISTS trading" http://localhost:6041/rest/sql

# ✅ PostgreSQL psql
PGPASSWORD=trading123 psql -h localhost -U trading -d exchange_info_db -c "..."
```

### 1.2 服务连接必须用 localhost

```yaml
# CI 中：
PG_HOST=localhost    # ✅ 正确
PG_HOST=postgres     # ❌ 只在 Docker Compose 中有效
```

---

## 2. 环境变量

### 2.1 测试脚本必须加载 db_env.sh

**问题描述**

测试脚本没有设置 `DATABASE_URL` 等环境变量，导致 PostgreSQL 连接超时。

**典型报错**

```
❌ Failed to connect to PostgreSQL: pool timed out while waiting for an open connection
```

**解决方案**

在脚本开头 source db_env.sh：

```bash
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib/db_env.sh"
```

### 2.2 CI 环境检测

```bash
if [ -n "$CI" ]; then
    # CI 专用逻辑
else
    # 本地环境逻辑
fi
```

---

## 3. workflow 步骤条件

### 3.1 正确的日志 Dump 模式

**问题描述**
如果不当使用 `continue-on-error: true`，会导致即使测试失败，Job 最终也被标记为成功（绿色），掩盖了错误。

**❌ 错误做法**：
```yaml
- name: Run Test
  run: ./test.sh
  continue-on-error: true  # 导致测试失败也被忽略

- name: Dump Logs
  run: cat logs/*.log
  # 结果：Job 变绿，错误被隐藏！
```

**✅ 正确做法**：
不要使用 `continue-on-error`。利用 `if: failure()` 条件在失败时运行日志打印步骤。

```yaml
- name: Run Test
  run: ./test.sh
  # 默认 behavior: 失败立即停止后续非 if: failure() 步骤

- name: Dump Logs
  if: failure()  # 仅在之前步骤失败时运行
  run: cat logs/*.log
  # 注意：此步骤本身会成功，但 Job 状态仍由 Run Test 决定（红色）
```

### 3.2 日志文件路径一致性

确保脚本写入的日志路径与 workflow 读取的路径一致：

```bash
# 脚本中
nohup ./gateway > /tmp/gateway_fee_e2e.log 2>&1 &

# workflow 中必须匹配
cat /tmp/gateway_fee_e2e.log   # ✅ 路径一致
cat /tmp/gw_test.log           # ❌ 路径不一致
```

---

## 4. 数据库初始化

### 4.1 PostgreSQL 健康检查

**问题**: 默认使用 root 用户，数据库没有 root 角色。

```yaml
services:
  postgres:
    options: >-
      --health-cmd "pg_isready -U trading -d exchange_info_db"  # 指定用户
```

### 4.2 TDengine 精度

**必须使用 `PRECISION 'us'`**：

```sql
CREATE DATABASE IF NOT EXISTS trading PRECISION 'us';
```

如果精度错误，微秒时间戳会报 "Timestamp data out of range"。

### 4.3 服务沉淀时间

```yaml
- name: Initialize TDengine
  run: ./scripts/db/init.sh td && sleep 5  # 等待元数据初始化
```

---

## 5. 二进制与启动

### 5.1 二进制新鲜度

本地测试前确保 release 二进制是最新的：

```bash
cargo build --release
```

CI 每次都是 fresh build，但本地开发可能运行旧版本。

### 5.2 Gateway 启动等待

```bash
for i in $(seq 1 60); do
    if curl -sf "http://localhost:8080/api/v1/health" > /dev/null 2>&1; then
        break
    fi
    sleep 1
done
```

**注意**：健康检查路径是 `/api/v1/health`，不是 `/health`。

---

---

## 6. 配置与端口对齐 (Config & Port Parity)

### 6.1 5433 vs 5432 端口陷阱

- **本地 (Dev)**: 默认端口 **5433** (`config/dev.yaml`)。
- **CI 环境**: 标准端口 **5432** (`config/ci.yaml`)。
- **解决方案**: 测试脚本必须检测 `CI=true` 并传递 `--env ci`。

```bash
if [ "$CI" = "true" ]; then
    GATEWAY_ARGS="--gateway --env ci"
fi
```

### 6.2 标准化脚本模板

请复用标准模板：`scripts/templates/test_integration_template.sh`。

---

## 7. Python 环境规范 (uv)

### 7.1 禁止裸跑 Python
CI 环境中直接运行 `python3` 可能找不到依赖。

### 7.2 解决方案
使用 `uv run` 显式管理依赖：

```bash
uv run --with requests --with pynacl python3 scripts/tests/my_test.py
```

---

## 8. 快速参考

| 场景 | 本地 | CI |
|------|------|-----|
| TDengine 操作 | `docker exec tdengine taos` | `curl localhost:6041/rest/sql` |
| PostgreSQL 连接 | 容器名或 localhost | `localhost` only |
| 环境变量 | 手动设置或 .env | `source db_env.sh` |
| 日志输出 | 终端 | 文件 + artifact 上传 |

---

## 9. 竞态条件与资源清理 (Race Conditions)

### 9.1 端口占用 ("Address already in use")

**问题描述**
在同一个 Job 中连续运行多个测试脚本（如 QA Suite + POC），前一个脚本可能未完全释放端口，导致后续脚本启动 Gateway 失败。

**解决方案**
在启动 Gateway 前，**必须**显式清理旧进程。在 CI 环境中（非本地 IDE），可以使用 `pkill`：

```bash
# Ensure clean slate
echo "Cleaning up any existing Gateway processes..."
pkill -9 -f "zero_x_infinity" || true
sleep 2 # 等待内核释放端口
```

**关键点**：使用 `kill -9` 确保立即终止，防止僵尸进程。

---

## 10. 错误处理规范

### 10.1 如果 Config 加载 Panic

**禁止**：
```rust
File::open("config.yaml").unwrap(); // ❌ 导致 crash，无详细日志
```

**必须**：
使用 `anyhow::Result` 并添加 Context：
```rust
File::open("config.yaml").with_context(|| "Failed to open config")?; // ✅
```

### 10.2 数据库唯一约束 (Duplicate Key)

**问题**：重复注册用户导致 500 Panic 并在日志中打印堆栈跟踪，干扰排查。

**解决方案**：捕获 "duplicate key" 错误，记录为 Warning，并返回 409 Conflict。

```rust
if err.to_string().contains("duplicate key") {
    tracing::warn!("User already exists: {}", err);
    return Err(StatusCode::CONFLICT);
}
```

---

*最后更新：2025-12-28*
