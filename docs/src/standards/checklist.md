---
description: 交付前检查清单 - 确保文档和代码的一致性
---

# 交付检查清单 (Delivery Checklist)

每次完成功能开发或文档变更后，必须按以下清单逐项检查：

## 1. 分支管理
- [ ] 确认在正确的 feature 分支上工作，而不是 main
- [ ] 检查 `git branch --show-current`

## 2. 文档一致性
- [ ] SUMMARY.md 中的链接与实际文件名一致
- [ ] 文档标题与文件名中的章节号一致 (如 0x0A-auth.md → # 0x0A ...)
- [ ] 所有新增的 .md 文件都已添加到 SUMMARY.md
- [ ] 子章节正确缩进 (4个空格)

## 3. mdbook 验证
- [ ] 运行 `mdbook build docs` 无错误
- [ ] 运行 `mdbook serve docs` 并在浏览器中验证目录结构
- [ ] 检查所有章节链接可点击

## 4. README.md 更新 (⚠️ 易遗漏)
- [ ] **英文章节索引表**更新（L71-99）
- [ ] **中文章节索引表**更新（L235-263）
- [ ] 移除已完成功能的 "(WIP)" 标记
- [ ] API 列表与当前路由一致（公开/私有）
- [ ] 架构图（Mermaid）与实际实现一致

## 5. 代码一致性
- [ ] SQL Schema 字段名与 Rust struct 字段名一致
- [ ] Repository 中的 SQL 查询与 Schema 匹配
- [ ] 运行 `cargo check` 无错误
- [ ] 运行 `cargo test` 通过

## 6. Git 提交
- [ ] 所有变更已 `git add`
- [ ] Commit message 符合规范 (feat/fix/docs: 描述)
- [ ] 提交前运行 pre-commit hooks 通过

## 7. 命名规范
- [ ] Part II 章节使用 0x0A-0x0D 格式
- [ ] Part III 章节使用 0x12-0x14 格式
- [ ] assets 表字段: `asset` (不是 symbol)
- [ ] symbols 表字段: `symbol` (不是 name)

## 8. 工件清理 (Artifacts)
- [ ] 删除临时测试日志 (`/tmp/*.log`)
- [ ] 清理 `task.md`, `walkthrough.md` 中的过时信息
- [ ] 确认 `implementation_plan.md` 状态为 COMPLETE
