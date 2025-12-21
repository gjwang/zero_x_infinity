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

## 测试脚本参考

已修复的脚本：
- `scripts/test_gateway_e2e_full.sh` - 使用 pgrep + kill 方式

---

## 其他注意事项

### TDengine 数据库名称

本项目使用 `trading` 作为数据库名，不是 `exchange`。

```bash
# 正确
docker exec tdengine taos -s "USE trading; ..."

# 错误
docker exec tdengine taos -s "USE exchange; ..."
```

### P1 已知问题 (已修复)

在 Phase 0x09-f 中，已解决了以下关键持久化问题：
1. **Orders 持久化** - `SettlementService` 已实现订单状态同步。
2. **Balances 持久化** - 已实现。
3. **Pipeline MT 写入 TDengine** - 已实现海量流水的高效写入。

详见：`docs/src/0x09-f-integration-test.md`

---

*最后更新：2025-12-20*
