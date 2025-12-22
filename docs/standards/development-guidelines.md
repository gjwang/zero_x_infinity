# 开发基本要求规范

## 1. 提交规范（Commit Guidelines）

### 1.1 原子化提交（Atomic Commits）

每个提交必须是**最小原子化**的，遵循以下原则：

- **单一职责**：一个提交只做一件事
- **可编译**：每个提交后代码必须能够编译通过
- **可测试**：相关测试必须通过
- **可回滚**：可以安全地 revert 而不影响其他功能

#### 示例：

✅ **好的提交**：
```
feat: add PostgreSQL connection pool to Database module

- Implement Database::connect() with connection pooling
- Add health_check() method
- Include unit tests for connection and health check
```

❌ **不好的提交**：
```
feat: add account system

- Add database, models, repository, handlers
- Update config, main.rs, gateway
- Add tests and documentation
```

### 1.2 提交前检查清单

在提交前必须确保：

- [ ] `cargo build` 成功
- [ ] `cargo test` 通过（至少相关模块）
- [ ] `cargo clippy` 无错误
- [ ] `cargo fmt` 已格式化
- [ ] 相关文档已更新

### 1.3 提交信息格式

使用 [Conventional Commits](https://www.conventionalcommits.org/) 格式：

```
<type>(<scope>): <subject>

<body>

<footer>
```

**Type 类型**：
- `feat`: 新功能
- `fix`: Bug 修复
- `docs`: 文档更新
- `test`: 测试相关
- `refactor`: 重构
- `perf`: 性能优化
- `chore`: 构建/工具配置

**示例**：
```
feat(account): add PostgreSQL repository layer

- Implement UserRepository with CRUD operations
- Add AssetManager and SymbolManager
- Convert sqlx macros to runtime queries

Closes #123
```

---

## 2. 模块化开发（Modular Development）

### 2.1 模块设计原则

- **高内聚，低耦合**：模块内部功能紧密相关，模块间依赖最小化
- **单一职责**：每个模块只负责一个明确的功能领域
- **接口清晰**：通过 `pub` 明确导出公共 API
- **依赖注入**：避免硬编码依赖，使用参数传递

#### 模块结构示例：

```
src/account/
├── mod.rs           # 模块入口，导出公共 API
├── db.rs            # 数据库连接管理
├── models.rs        # 数据模型定义
├── repository.rs    # 数据访问层
└── tests/           # 模块测试（可选）
    ├── db_tests.rs
    └── repository_tests.rs
```

### 2.2 单元测试要求

每个模块必须包含**完善的单元测试**：

#### 测试覆盖要求：

- **核心功能**：100% 覆盖
- **边界条件**：错误处理、空值、极值
- **公共 API**：所有 `pub` 函数/方法

#### 测试组织：

```rust
// 方式 1：内联测试（推荐用于访问私有成员）
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_name() {
        // Arrange
        let input = ...;
        
        // Act
        let result = function(input);
        
        // Assert
        assert_eq!(result, expected);
    }
}

// 方式 2：独立测试文件（用于集成测试）
// tests/integration_test.rs
```

#### 测试标记：

```rust
#[test]                    // 普通单元测试
#[tokio::test]             // 异步测试
#[ignore]                  // 需要外部依赖（如数据库）
#[should_panic]            // 预期 panic
```

---

## 3. 交付清单（Delivery Checklist）

每个开发任务完成时，必须交付以下内容：

### 3.1 文档更新

#### 必须更新的文档：

- [ ] **README.md**：如果影响使用方式或配置
- [ ] **API 文档**：新增或修改的 API 端点
- [ ] **架构文档**：`docs/src/` 中的相关章节
- [ ] **代码注释**：复杂逻辑必须有注释

#### 文档质量要求：

- 清晰的功能描述
- 完整的使用示例
- 配置说明（如有）
- 故障排查指南（如有）

### 3.2 测试脚本

#### 必须提供：

1. **单元测试**
   ```bash
   # 运行模块单元测试
   cargo test --lib module_name::
   ```

2. **集成测试脚本**
   ```bash
   # 示例：scripts/test_account_integration.sh
   #!/bin/bash
   # 1. 启动依赖服务（Docker）
   # 2. 运行 Gateway
   # 3. 测试 API 端点
   # 4. 清理资源
   ```

3. **E2E 测试方法**
   - 完整的测试步骤文档
   - 预期输出示例
   - 验收标准

#### 测试脚本要求：

- **可执行**：`chmod +x` 并可直接运行
- **幂等性**：多次运行结果一致
- **自动化**：无需手动干预
- **清理资源**：测试后恢复环境

### 3.3 自测验证

交付前必须**自行测试通过**：

#### 验证清单：

- [ ] **编译通过**：`cargo build --release`
- [ ] **单元测试通过**：`cargo test --lib`
- [ ] **集成测试通过**：运行测试脚本
- [ ] **代码质量**：`cargo clippy` 无警告
- [ ] **格式规范**：`cargo fmt --check`
- [ ] **文档构建**：`mdbook build` 成功（如有）
- [ ] **CI/CD 通过**：GitHub Actions 验证

#### 功能验证：

- [ ] 核心功能正常工作
- [ ] 边界条件处理正确
- [ ] 错误处理符合预期
- [ ] 性能满足要求（如有基准）

### 3.4 CI/CD 配置

必须确保 CI/CD 流水线正常工作：

#### 现有 CI/CD 配置（`.github/workflows/`）：

| 文件 | 用途 |
|------|------|
| `ci.yml` | 基础 CI（build, test, clippy, fmt） |
| `integration-tests.yml` | 集成测试 |
| `mdbook.yml` | 文档构建 |

#### CI 触发条件：

- Push 到 main/feature 分支
- Pull Request 到 main 分支

#### CI 验证：

```bash
# 本地模拟 CI 验证
cargo build --release
cargo test --lib
cargo clippy -- -D warnings
cargo fmt --check
```

---

## 4. 代码质量标准

### 4.1 代码风格

- 遵循 [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- 使用 `cargo fmt` 自动格式化
- 使用 `cargo clippy` 检查代码质量

### 4.2 错误处理

- 使用 `Result<T, E>` 而非 `panic!`
- 提供有意义的错误信息
- 适当使用 `?` 操作符传播错误

```rust
// ✅ 好的错误处理
pub fn connect(url: &str) -> Result<Database, sqlx::Error> {
    let pool = PgPoolOptions::new()
        .connect(url)
        .await?;
    Ok(Database { pool })
}

// ❌ 不好的错误处理
pub fn connect(url: &str) -> Database {
    let pool = PgPoolOptions::new()
        .connect(url)
        .await
        .unwrap();  // 可能 panic
    Database { pool }
}
```

### 4.3 性能考虑

- 避免不必要的克隆（`clone()`）
- 使用引用传递大对象
- 异步操作使用 `async/await`
- 考虑使用 `Arc` 共享只读数据

---

## 5. 持续集成（CI）要求

### 5.1 CI 流程

每个 PR 必须通过：

1. **编译检查**：`cargo build`
2. **测试**：`cargo test`
3. **代码质量**：`cargo clippy`
4. **格式检查**：`cargo fmt --check`

### 5.2 分支策略

- `main`：稳定分支，只接受经过测试的代码
- `feature/*`：功能分支，开发新功能
- `fix/*`：修复分支，修复 bug

---

## 6. 新功能开发检查清单 (New Feature Checklist)

每次开发新功能时，**必须同时完成**以下所有项目：

### 6.1 代码层面

- [ ] 功能代码实现完成
- [ ] 单元测试编写完成 (`cargo test`)
- [ ] 相关模块的 `mod.rs` 导出更新
- [ ] 错误处理完善，无 `unwrap()` 裸调用

### 6.2 测试层面

- [ ] 创建/更新集成测试脚本 (`scripts/test_*.sh`)
- [ ] 更新 `scripts/test_ci.sh` 添加新测试 Phase
- [ ] 测试脚本可独立运行且幂等

### 6.3 CI/CD 层面

- [ ] 检查 CI workflow 是否有必要的服务依赖 (PostgreSQL, TDengine)
- [ ] 确认 `integration-tests.yml` 包含新功能测试
- [ ] 本地运行 `./scripts/test_ci.sh --quick` 验证

### 6.4 文档层面

- [ ] 更新 `docs/src/*.md` 相关章节
- [ ] 更新 `docs/src/SUMMARY.md` 目录结构
- [ ] 运行 `mdbook build` 验证文档构建

### 6.5 提交前最终检查

```bash
# ✅ 必须全部通过才能提交
cargo check
cargo test
cargo clippy -- -D warnings
cargo fmt --check
mdbook build docs
./scripts/test_ci.sh --quick
```

---

## 7. 示例：完整的开发流程

### 场景：添加新的 API 端点

#### Step 1: 创建功能分支
```bash
git checkout -b feat/add-user-endpoint
```

#### Step 2: 开发功能
```rust
// src/gateway/handlers.rs
pub async fn get_user(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<u64>,
) -> Result<Json<ApiResponse<User>>, StatusCode> {
    // 实现逻辑
}

// 添加单元测试
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_get_user() {
        // 测试代码
    }
}
```

#### Step 3: 注册路由
```rust
// src/gateway/mod.rs
.route("/api/v1/user/:user_id", get(handlers::get_user))
```

#### Step 4: 编写测试脚本
```bash
# scripts/test_user_endpoint.sh
curl http://localhost:8080/api/v1/user/1001 | jq .
```

#### Step 5: 更新文档
```markdown
# README.md
- `GET /api/v1/user/:user_id` - 查询用户信息 ✅
```

#### Step 6: 自测验证
```bash
cargo build
cargo test --lib gateway::handlers::tests::test_get_user
./scripts/test_user_endpoint.sh
```

#### Step 7: 提交代码
```bash
git add .
git commit -m "feat(gateway): add GET /api/v1/user/:user_id endpoint

- Implement get_user handler
- Add unit tests for user endpoint
- Update README with new API documentation
- Add integration test script

Closes #456"
```

#### Step 8: 创建 PR
- 填写 PR 描述
- 附上测试截图
- 等待 CI 通过
- 请求 Code Review

---

## 8. 常见问题（FAQ）

### Q: 什么时候需要创建新的测试脚本？

A: 当添加新的功能模块或 API 端点时，应该创建对应的集成测试脚本。

### Q: 如何处理需要外部依赖的测试？

A: 使用 `#[ignore]` 标记，并在 CI 中配置环境后运行：
```bash
cargo test -- --ignored
```

### Q: 提交前忘记运行测试怎么办？

A: 使用 Git hooks 自动化：
```bash
# .git/hooks/pre-commit
#!/bin/bash
cargo test || exit 1
```

---

## 9. 参考资源

- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Conventional Commits](https://www.conventionalcommits.org/)
- [Git Best Practices](https://git-scm.com/book/en/v2)
- [Rust Testing Guide](https://doc.rust-lang.org/book/ch11-00-testing.html)

---

**最后更新**: 2025-12-22  
**版本**: 1.0
