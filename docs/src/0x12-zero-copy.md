# 0x12 Zero-Copy Optimization

| Status | **DRAFT** |
| :--- | :--- |
| **Date** | 2025-12-28 |
| **Context** | Phase IV: Extreme Optimization |
| **Goal** | Eliminate memory copy overhead in the Hot Path. |

## 1. Problem Definition
Currently, an Order flows through these copy stages:
1.  **Network Read**: Socket -> Kernel Buffer -> User Space Buffer.
2.  **Deserialization**: JSON/Bincode -> `OrderRequest` struct (Heap Allocation).
3.  **Conversion**: `OrderRequest` -> `InternalOrder` (Copy).
4.  **Ring Buffer Push**: `InternalOrder` -> `OrderEvent` (Copy into slot).
5.  **WAL Write**: `InternalOrder` -> bincode bytes -> Disk Buffer.

Each copy consumes memory bandwidth and CPU cycles. At 1.3M TPS, these micro-latencies compound.

## 2. Optimization Strategy

### 2.1 Zero-Copy Deserialization (`rkyv`)
We will replace `bincode` with **`rkyv`** for internal persistence (WAL) and potentially IPC.
*   **Why**: Guaranteed Zero-Copy deserialization. Accessing a field is just pointer arithmetic.
*   **Target**: `src/wal_v2`, `src/ubscore_wal`.

### 2.2 Arena Allocation (Ring Buffer)
The `crossbeam_array::ArrayQueue` stores `OrderEvent`.
Instead of pushing *values*, we should investigate pushing *handles* to a pre-allocated Arena, OR ensure `OrderEvent` is `Copy` and small enough to fit in cache lines.
*   **Current `InternalOrder`**: ~60-80 bytes?
*   **Goal**: Ensure `InternalOrder` is aligned to 64 bytes (Cache Line).

### 2.3 Zero-Copy Network Handling (Future)
Investigate `io_uring` to read directly into pre-allocated Ring Buffer slots (Advanced).

## 3. Implementation Plan

1.  **Benchmark**: Create a benchmark (Criterion) for `Order` serialization/deserialization.
2.  **Prototype `rkyv`**: Implement `Archive` trait for `InternalOrder`. Compare speed.
3.  **WAL Refactor**: Switch WAL v2 to use `rkyv` aligned buffers.

## 4. Risks
*   **Complexity**: `rkyv` lifetimes (`Pin`) can be complex.
*   **Compatibility**: Breaking change for WAL format (requires migration tool).

## 5. Success Metric
*   **TPS**: Increase from 1.3M to 1.5M+.
*   **Latency**: P99 reduction by 10%.
