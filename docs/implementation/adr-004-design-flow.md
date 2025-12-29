# ADR-004 Implementation Flow: Chain Asset Binding & Hot Reload

## 1. 核心选型：In-Loop Internal Refresh (循环内自刷新)
为了避免复杂的锁机制 (`RwLock`) 和并发问题，我们采用 **In-Loop Refresh** 模式。由于 `EthScanner` 在 Sentinel 中是以 `async` 方式顺序执行的，我们可以在每次扫描循环的开始处检查是否需要刷新配置。

这种方式的优点：
*   **无锁 (Lock-free)**: 唯一的并发点是 DB I/O，状态更新是原子的（在 `&mut self` 上）。
*   **透明 (Transparent)**: `Worker` 不需要知道配置刷新的细节，只需调用 `scan_block`/`get_logs`。
*   **安全 (Safe)**: 避免了在扫描过程中配置发生变更导致的数据不一致。

## 2. 详细执行流程

### Phase 1: 初始化 (Initialization)
1.  **Sentinel 启动**:
2.  **ChainManager 初始化**: 连接 DB (`PgPool`)。
3.  **EthScanner 初始化**:
    *   调用 `ChainManager::get_assets_by_chain("ETH")` 进行首次加载。
    *   将加载的合约地址 (e.g., USDT `0xdac17...`) 存入 `self.watched_contracts`。
    *   设置 `self.last_refresh_time = Instant::now()`。

### Phase 2: 运行循环 (Runtime Loop)
在 `EthScanner::scan_block` (或底层 `get_logs`) 中：

```rust
// 伪代码流程
async fn scan(&mut self) {
    // 步骤 A: 检查热加载 (Hot Reload Check)
    if self.last_refresh_time.elapsed() > Duration::from_secs(60) {
        // 1. 从 DB 拉取最新配置
        let new_assets = self.chain_manager.get_assets_by_chain("ETH").await?;
        
        // 2. 提取有效合约地址
        let new_contracts: HashSet<String> = new_assets
            .iter()
            .filter_map(|a| a.contract_address.clone())
            .collect();

        // 3. Diff 更新 (Log变更)
        if new_contracts != self.watched_contracts {
            info!("Config Changed! Reloading contracts: {:?}", new_contracts);
            self.watched_contracts = new_contracts; // 原子更新
        }
        
        // 4. 重置计时器
        self.last_refresh_time = Instant::now();
    }

    // 步骤 B: 构建 Filter
    let filter = Filter::new()
        .address(self.watched_contracts.iter().cloned().collect()) // 使用最新合约列表
        .topic0(TRANSFER_TOPIC);

    // 步骤 C: 执行扫描
    let logs = self.rpc.get_logs(filter).await?;
    // ... 处理 logs ...
}
```

### Phase 3: 验证 (Verification)
1.  启动 Sentinel (只配置了 ETH Native)。
2.  发送 MockUSDT 交易 -> **预期**: Sentinel 忽略 (未监听)。
3.  **Action**: 在 `chain_assets_tb` 中插入 MockUSDT 记录。
4.  等待 60秒 (或手动触发测试模式下的刷新)。
5.  发送 MockUSDT 交易 -> **预期**: Sentinel 捕获并处理。
6.  **验证成功**。

## 3. 代码变更点
*   `src/sentinel/eth.rs`:
    *   Struct: 增加 `chain_manager`, `watched_contracts`, `last_refresh_time`。
    *   Method `refresh_config()`: 实现 DB 拉取和更新逻辑。
    *   Method `get_logs()`: 在调用 RPC 前插入刷新检查。

此方案无需引入 `RwLock`，结构最简单，稳定性最高。
