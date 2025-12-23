# Pre-Merge to Main Checklist

合并到 main 分支之前，**必须完成所有检查项**。

---

## 1. 代码质量 ✓

- [ ] `cargo check` 通过
- [ ] `cargo test` 通过
- [ ] `cargo clippy` 无 error
- [ ] `cargo fmt --check` 通过

## 2. 文档更新 ✓

- [ ] `docs/src/*.md` 相关章节已更新
- [ ] `docs/src/SUMMARY.md` 目录结构正确
- [ ] `mdbook build` 构建成功
- [ ] **README.md** 已添加新章节链接

## 3. CI/CD ✓

### 3.1 本地验证（必须）
- [ ] `./scripts/test_ci.sh --quick` 通过
- [ ] **模拟 CI 单独运行**（关键！本地全跑可能掩盖问题）：
  ```bash
  CI=true ./scripts/test_ci.sh --test-gateway-e2e
  CI=true ./scripts/test_ci.sh --test-kline
  CI=true ./scripts/test_ci.sh --test-depth
  CI=true ./scripts/test_ci.sh --test-account
  ```

### 3.2 CI 环境检查
- [ ] 不使用 `docker exec` (CI service container 不支持)
- [ ] 数据库连接使用 `localhost` 而非容器名
- [ ] 所有 helper 函数定义在全局作用域（不在 `if` 块内）

### 3.3 CI 失败时
1. **立即下载日志**：`gh run view <run-id> --log-failed`
2. **搜索错误**：`grep -i "error\|fail\|fatal" logs/*.txt`
3. **根据日志修复**，不要瞎猜

## 4. Git 操作 ✓

- [ ] 所有更改已 commit
- [ ] `git status` 显示 clean
- [ ] 分支已 rebase/merge 到最新 main (无冲突)

## 5. 发布 ✓

- [ ] **合并后** 创建 Git Tag: `git tag v{版本号}`
- [ ] 推送 Tag: `git push origin --tags`

> [!CAUTION]
> **⚠️ 严禁删除 feature 分支！分支是项目历史的重要组成部分，必须永久保留。**

---

## 执行命令

```bash
# 1. 最终检查
cargo check && cargo test && cargo clippy && cargo fmt --check

# 2. 文档构建
cd docs && mdbook build && cd ..

# 3. 合并
git checkout main
git merge <feature-branch> --no-ff -m "Merge branch '<feature-branch>'"

# 4. 打 Tag
git tag v0.10-a-account-system
git push origin main --tags

# 5. 完成
echo "✅ Merge complete!"
```
