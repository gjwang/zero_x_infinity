# 0x10 Zero-Copy Optimization

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📅 Status**: 🚧 **Planned**
> **Core Objective**: Optimize deserialization and memory usage using Zero-Copy techniques (rkyv/capnproto).

---

## 1. Overview

*   **Goal**: Reduce CPU usage during object creation and cloning.
*   **Technique**: Use `rkyv` or `zerocopy` to cast bytes directly to structs.
*   **Target**: High-frequency data paths (Gateway -> Sequence -> Matching).

*(Detailed content coming soon in Phase III)*

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📅 状态**: 🚧 **计划中**
> **核心目标**: 使用零拷贝技术 (Zero-Copy) 优化反序列化与内存使用。

---

## 1. 概述

*   **目标**: 降低对象创建与克隆的 CPU 开销。
*   **技术**: 使用 `rkyv` 或 `zerocopy` 直接将字节映射为结构体。
*   **场景**: 高频数据路径 (Gateway -> Sequence -> Matching)。

*(第三阶段详细内容敬请期待)*
