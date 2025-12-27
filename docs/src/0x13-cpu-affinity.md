# 0x11 CPU Affinity & Cache

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📅 Status**: 🚧 **Planned**
> **Core Objective**: Pin threads to CPU cores and optimize data layout for cache locality.

---

## 1. Overview

*   **CPU Affinity**: Bind matching threads to isolated cores to reduce context switching.
*   **Cache Locality**: Optimize `OrderBook` node layout to fit L1/L2 cache lines.
*   **False Sharing**: Padding atomic variables to prevent cache line contention.

*(Detailed content coming soon in Phase III)*

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📅 状态**: 🚧 **计划中**
> **核心目标**: 主要线程绑核与缓存友好性优化。

---

## 1. 概述

*   **CPU 亲和性 (Affinity)**: 将撮合线程绑定到隔离核心，减少上下文切换。
*   **缓存局部性 (Locality)**: 优化 `OrderBook` 节点布局以适应 L1/L2 缓存行。
*   **伪共享 (False Sharing)**: 通过 Padding 避免多线程竞争同一缓存行。

*(第三阶段详细内容敬请期待)*
