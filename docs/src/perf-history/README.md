# Performance History

性能报告历史存档。每个重要章节完成后生成一份报告。

## 报告列表

| 日期 | 章节 | 关键变化 |
|------|------|----------|
| [2025-12-18](./2025-12-18-0x08h.md) | 0x08h | 服务化重构，ME占76.6%，1.3M数据集 |
| [2025-12-16](./2025-12-16-0x07b.md) | 0x07b | 性能基线建立，Ledger I/O 占 98.5% |

## 命名规范

```
YYYY-MM-DD-章节.md
```

例如：`2025-12-16-0x07b.md`

## 如何生成报告

```bash
# 1. 运行性能测试
cargo run --release

# 2. 生成报告
python3 scripts/generate_perf_report.py > docs/src/perf-report.md

# 3. 存档历史
cp docs/src/perf-report.md docs/src/perf-history/$(date +%Y-%m-%d)-章节.md

# 4. 更新此索引文件，添加新条目

# 5. 提交
git add docs/src/perf-report.md docs/src/perf-history/
git commit -m "docs: Update perf report"
```
