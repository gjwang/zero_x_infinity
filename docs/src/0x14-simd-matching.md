# 0x14 SIMD Matching Acceleration

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **📅 Status**: 🚧 **Planned**
> **Core Objective**: Use SIMD (AVX2/AVX-512) instructions to accelerate order matching.

---

## 1. Overview

*   **Vectorization**: Process multiple price levels in parallel.
*   **Intrinsics**: Direct use of Rust `std::arch` intrinsics.
*   **Benchmark**: Aiming for > 5M TPS.

*(Detailed content coming soon in Phase III)*

<br>
<div align="right"><a href="#-english">↑ Back to Top</a></div>
<br>

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **📅 状态**: 🚧 **计划中**
> **核心目标**: 使用 SIMD (AVX2/AVX-512) 指令集加速订单撮合。

---

## 1. 概述

*   **向量化 (Vectorization)**: 并行处理多个价格档位。
*   **Intrinsics**: 直接使用 Rust `std::arch` 内联汇编/指令。
*   **基准目标**: 目标吞吐量 > 500万 TPS。

*(第三阶段详细内容敬请期待)*
