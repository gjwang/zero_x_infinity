# 0x14 Extreme Optimization: The Metal Mode

<h3>
  <a href="#-english">🇺🇸 English</a>
  &nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
  <a href="#-chinese">🇨🇳 中文</a>
</h3>

<div id="-english"></div>

## 🇺🇸 English

> **Phase V Keynote**
> **Codename**: "Metal Mode"
> **Philosophy**: "Safe Abstractions must incur Zero Cost. If they do, strip them away."

### 1. The HFT Performance Ceiling

In the previous chapters, we built a highly reliable exchange core (Phase I-IV). We achieved **1.3M TPS** on a single thread using the Ring Buffer architecture. This is "fast enough" for 99% of crypto exchanges.

但对于顶级的 HFT 引擎，"Fast Enough" is not enough. We want to hit the physical limits of the CPU and Memory.

#### 1.1 The "Invisible" Wall

When we profile our engine at microsecond scales, we see the CPU spending significant time in `memcpy` and `malloc`.

*   **The Problem**: Handling an incoming Order involves:
    1.  Reading bytes from network.
    2.  **Allocating** a new `Order` struct on Heap.
    3.  **Parsing** JSON/Bincode and **Copying** data fields.
    4.  Processing.
    5.  **Deallocating** the struct.
*   **The Impact**:
    *   **Memory Bandwidth**: Wasted on copying data that already exists in the inputs.
    *   **Cache Pollution**: New allocations evict hot cache lines.
    *   **Allocator Jitter**: The Allocator is a complex global resource; lock contention or fragmentation causes latency spikes.

### 2. The Metal Mode Strategy

**Metal Mode** is our strategy to break this ceiling. It is defined by three pillars:

1.  **Zero-Copy (0x14)**: Never move data. View it where it lands.
2.  **CPU Affinity (0x15)**: Bind execution to specific silicon to minimize context switches.
3.  **SIMD (0x16)**: Process multiple data points in a single CPU cycle.

### 3. Deep Dive: Zero-Copy Architecture

#### 3.1 The "View" Paradigm

In a standard Rust program (using `serde`), deserialization is a transformation:
`Socket Buffer (Bytes) -> Transformer -> Rust Struct (Heap Objects)`

In a Zero-Copy architecture (using `rkyv`), deserialization is merely a "cast":
`Socket Buffer (Bytes) -> Trusted View (Pointer)`

We do not "read" the data. We **overlay** our data structure template onto the raw bytes in memory.

#### 3.2 `rkyv`: Relative Pointers

Standard C-structs use absolute pointers (`*const T`), which makes them impossible to send over network (memory addresses differ between machines).

`rkyv` solves this with **Relative Pointers**. Instead of storing `0x12345678`, it stores "The data is 16 bytes immediately after this field". This makes the serialized data **Position Independent** and directly mappable.

### 4. The Implementation Strategy: Parallel Engine

Moving to Zero-Copy is a "Brain Transplant" for the engine. It is high risk. To mitigate this, we adopt the **Parallel Engine Strategy**:

#### Step 1: Tier 2 Pipeline Benchmarks
We cannot optimize what we cannot measure.
*   **Tier 1**: Unit tests.
*   **Tier 2 (The Metal Harness)**: We will build a pure-memory benchmark harness. It isolates the engine from Network/OS noise, feeding it pre-loaded data at RAM speeds. This gives us a microscope to see nanosecond-level improvements.

#### Step 2: Layout Hardening
We must strictly define the memory layout of our types.
*   `#[repr(C)]` for predictable alignment.
*   Replacing dynamic `String` and `Vec` with fixed-size arrays or `Archived` variants.

#### Step 3: The ZeroCopyPipeline
We will not modify the existing `TradingPipeline` immediately. Instead, we build a `ZeroCopyPipeline` next to it.
*   It accepts raw bytes `&[u8]`.
*   It uses `rkyv` to "view" orders.
*   It shares the same core business logic (UBSCore).

Only when `ZeroCopyPipeline` proves to be significantly faster (>50%) and equally correct (Golden Set verification) will we perform "The Switch".

---

<div id="-chinese"></div>

## 🇨🇳 中文

> **Phase V 基调 (Keynote)**
> **内部代号**: "Metal Mode"
> **核心哲学**: "抽象必须是零成本的。如果不是，就剥离它。"

### 1. HFT 的性能天花板

在前几个阶段（Phase I-IV），我们构建了一个高可靠的交易所核心。利用 Ring Buffer 架构，我们在单线程上实现了 **130万 TPS**。对于 99% 的加密货币交易所来说，这已经"足够快"了。

但对于顶级的 HFT 引擎，"足够快"是不够的。我们要触达 CPU 和内存的物理极限。

#### 1.1 "隐形"的墙

当我们以微秒级精度分析引擎性能时，会发现 CPU 将大量时间消耗在 `memcpy`（内存拷贝）和 `malloc`（内存分配）上。

*   **问题所在**: 处理一个传入订单涉及以下步骤：
    1.  从网络读取字节。
    2.  在堆(Heap)上**分配**一个新的 `Order` 结构体。
    3.  **解析** JSON/Bincode 并将数据字段**拷贝**过去。
    4.  处理业务逻辑。
    5.  **释放**结构体内存。
*   **影响**:
    1.  **内存带宽**: 浪费在搬运那些本就已经存在于输入缓冲区的数据上。
    2.  **缓存污染**: 新的分配会驱逐 L1/L2 缓存中的热数据。
    3.  **分配器抖动**: 内存分配器是一个复杂的全局资源；锁竞争或碎片化会导致不可预测的延迟尖峰。

### 2. Metal Mode 战略

**Metal Mode** 是我们要打破这一天花板的战略代号。它由三大支柱定义：

1.  **Zero-Copy (0x14)**: 绝不移动数据。原地观测。
2.  **CPU Affinity (0x15)**: 将执行流绑定到特定硅片核心，消除上下文切换。
3.  **SIMD (0x16)**: 单指令多数据，一个 CPU 周期处理多个数据点。

### 3. 深度解析：Zero-Copy 架构

#### 3.1 "视图 (View)" 范式

在标准的 Rust 程序中（使用 `serde`），反序列化是一个转换过程：
`Socket 缓冲区 (字节) -> 转换器 -> Rust 结构体 (堆对象)`

在 Zero-Copy 架构中（使用 `rkyv`），反序列化仅仅是一个"类型转换 (Cast)"：
`Socket 缓冲区 (字节) -> 可信视图 (指针)`

我们不"读取"数据。我们将数据结构的模板直接**覆盖 (Overlay)** 在内存的原始字节上。

#### 3.2 `rkyv`：相对指针 (Relative Pointers)

标准的 C 结构体使用绝对指针 (`*const T`)，这使得它们无法在网络间传输（不同机器的内存地址不同）。

`rkyv`通过**相对指针**解决了这个问题。它不存储 `0x12345678`，而是存储"数据位于此字段之后 16 字节处"。这使得序列化后的数据是**位置无关 (Position Independent)** 的，可以直接映射使用。

### 4. 实施策略：并行引擎 (Parallel Engine)

转向 Zero-Copy 对引擎来说是一次"大脑移植"手术，风险极高。为了降低风险，我们采用 **并行引擎战略**：

#### 步骤 1: Tier 2 流水线基准测试 (Pipeline Benchmarks)
我们无法优化我们无法测量的东西。
*   **Tier 1**: 单元测试（太微观）。
*   **Tier 2 (Metal Harness)**: 我们将构建一个纯内存基准测试脚手架。它将引擎与网络/操作系统噪声隔离，以内存速度向其灌入预加载数据。这给了我们要给显微镜，去观察纳秒级的改进。

#### 步骤 2: 布局硬化 (Layout Hardening)
我们需要严格定义数据类型的内存布局。
*   使用 `#[repr(C)]` 确保可预测的内存对齐。
*   将动态的 `String` 和 `Vec` 替换为定长数组或 `Archived` 变体。

#### 步骤 3: ZeroCopyPipeline
我们不会立即修改现有的 `TradingPipeline`。相反，我们在它旁边构建一个 `ZeroCopyPipeline`。
*   它接受原始字节 `&[u8]`。
*   它使用 `rkyv` 来"透视"订单。
*   它共享相同的核心业务逻辑 (UBSCore)。

只有当 `ZeroCopyPipeline` 证明显著更快（>50%）且同样正确（通过 Golden Set 验证）时，我们才会执行"切换 (The Switch)"。
