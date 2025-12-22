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

- [ ] 本地 `./scripts/test_ci.sh --quick` 通过
- [ ] CI workflow 包含新功能依赖 (PostgreSQL/TDengine)
- [ ] CI workflow 包含新测试脚本

## 4. Git 操作 ✓

- [ ] 所有更改已 commit
- [ ] `git status` 显示 clean
- [ ] 分支已 rebase/merge 到最新 main (无冲突)

## 5. 发布 ✓

- [ ] **合并后** 创建 Git Tag: `git tag v{版本号}`
- [ ] 推送 Tag: `git push origin --tags`
- [ ] 删除已合并的 feature 分支 (可选)

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

# 5. 清理
git branch -d <feature-branch>
```
