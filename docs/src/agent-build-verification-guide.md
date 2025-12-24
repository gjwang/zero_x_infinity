# 编译与验证注意事项 (Build & Verification Guide)

本文档总结了在本地进行 Gateway 开发和 E2E 测试时，关于“修改未生效”和“端口冲突”的常见坑点及解决方案。

---

## 1. 源码修改未生效 (The Stale Binary Trap)

当你执行了 `cargo build --release` 但发现测试结果仍然运行旧代码逻辑时：

### 常见原因
*   **指纹失效 (Fingerprint)**：Cargo 错误认为二进制已是最新的，跳过了重编或重新链接。
*   **增量缓存损坏**：`target/release/incremental` 中的缓存导致逻辑未刷新。
*   **时间戳分辨率**：源码修改时间与上次构建时间太接近（APFS 精度问题）。

### 解决方案 (由轻到重)
1.  **最常用 (强制重链)**:
    ```bash
    touch src/main.rs && cargo build --release
    ```
2.  **清理增量缓存 (非全量 clean)**:
    ```bash
    rm -rf target/release/incremental
    ```
3.  **强制重编核心模块**:
    ```bash
    cargo clean -p zero_x_infinity && cargo build --release
    ```

---

## 2. 端口冲突与僵尸进程 (Port Conflict)

### 现象
Gateway 启动失败并报错：`❌ FATAL: Failed to bind to 0.0.0.0:8080: Address already in use`。
这通常是因为后台残留了一个旧的 Gateway 进程。

### 诊断与解决
1.  **查杀残留进程 (安全方式)**:
    不要使用 `pkill -f` (会杀掉 IDE)。请使用：
    ```bash
    # 查找并杀掉占用 8080 的进程
    lsof -i :8080
    kill -9 <PID>
    ```
2.  **检查脚本冲突**:
    确认你没有在终端手动运行 Gateway 的同时，又在另一个窗口跑 `test_transfer_e2e.sh`。

---

## 3. E2E 测试最佳实践

### 确认二进制新鲜度
在运行测试前，手动核对时间戳或观察 E2E 脚本的警告：
```bash
ls -lh src/funding/service.rs target/release/zero_x_infinity
```

### 数据库一致性
如果逻辑看起来对了但 API 报错 `Missing column`：
1.  确认 PostgreSQL 迁移已手动应用（如果 `init.sh` 因为存在旧数据跳过了）。
2.  确认 `balances_tb` 的 `account_type` 和 `status` 是 `SMALLINT`，在 Rust 中必须对应 `i16`。

### 运行 Gateway 时常备参数
手动调试时请务必带上环境变量参数：
```bash
./target/release/zero_x_infinity --gateway --env dev
```

---
*最后更新: 2025-12-24*
