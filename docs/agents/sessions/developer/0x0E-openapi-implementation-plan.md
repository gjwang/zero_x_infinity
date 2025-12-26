# 0x0E OpenAPI Implementation Plan

> **Status**: READY FOR DEVELOPMENT  
> **Author**: Architect (AI Agent)  
> **Date**: 2025-12-26

---

## 🎯 Goal (ONE SENTENCE)

Integrate `utoipa` to generate OpenAPI 3.0 documentation, enabling interactive API docs at `/docs` and client SDK auto-generation.

---

## 📦 Design Reference

| Document | Path |
|----------|------|
| Architecture Design | [`docs/src/0x0E-openapi-integration.md`](../../src/0x0E-openapi-integration.md) |
| API Conventions | [`docs/standards/api-conventions.md`](../../standards/api-conventions.md) |
| Auth Specification | [`docs/src/0x0A-c-api-auth.md`](../../src/0x0A-c-api-auth.md) |

---

## 📋 Phase Breakdown

| Phase | Description | Priority | Timeline |
|-------|-------------|----------|----------|
| **Phase 1** | Foundation (utoipa + Swagger UI) | P0 | ~1 day |
| **Phase 2** | Public Endpoints Documentation | P0 | ~1 day |
| **Phase 3** | Private Endpoints + Auth Docs | P1 | ~2 days |
| **Phase 4** | Client SDK Generation | P2 | ~1 day |

---

## 🔧 Phase 1: Foundation (P0)

### Task 1.1: Add Dependencies

**File**: `Cargo.toml`

```toml
# Add to [dependencies]
utoipa = { version = "5.3", features = ["axum_extras", "chrono", "uuid"] }
utoipa-swagger-ui = { version = "8.0", features = ["axum"] }
```

### Task 1.2: Create OpenAPI Module

**New File**: `src/gateway/openapi.rs`

```rust
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Zero X Infinity Exchange API",
        version = "1.0.0",
        description = "High-performance cryptocurrency exchange API"
    ),
    paths(),  // Will be populated in Phase 2
    components(schemas())
)]
pub struct ApiDoc;
```

### Task 1.3: Integrate Swagger UI

**File**: `src/gateway/mod.rs`

Add at router setup:
```rust
use utoipa_swagger_ui::SwaggerUi;
use crate::gateway::openapi::ApiDoc;

// Add to router
.merge(
    SwaggerUi::new("/docs")
        .url("/api-docs/openapi.json", ApiDoc::openapi())
)
```

### ✅ Phase 1 Acceptance Criteria

- [ ] `cargo build` succeeds with new dependencies
- [ ] Swagger UI accessible at `http://localhost:8080/docs`
- [ ] OpenAPI JSON at `http://localhost:8080/api-docs/openapi.json`
- [ ] All existing 289 tests still pass

---

## 🔧 Phase 2: Public Endpoints (P0)

### Task 2.1: Annotate Public Handlers

**File**: `src/gateway/handlers.rs`

Add `#[utoipa::path]` to each public handler:

| Handler | Endpoint | Tag |
|---------|----------|-----|
| `get_depth` | GET /api/v1/public/depth | Market Data |
| `get_klines` | GET /api/v1/public/klines | Market Data |
| `get_assets` | GET /api/v1/public/assets | Market Data |
| `get_symbols` | GET /api/v1/public/symbols | Market Data |
| `get_exchange_info` | GET /api/v1/public/exchange_info | Market Data |
| `health_check` | GET /api/v1/health | System |

### Task 2.2: Add ToSchema to Response Types

**File**: `src/gateway/types.rs` (or equivalent)

Add `#[derive(ToSchema)]` to:
- `ApiResponse<T>`
- `DepthApiData`
- `KLineApiData`
- `AssetInfo`
- `SymbolInfo`

### ✅ Phase 2 Acceptance Criteria

- [ ] 6 public endpoints visible in Swagger UI
- [ ] "Try it out" works for `/depth` endpoint
- [ ] Response schemas have example values

---

## 🔧 Phase 3: Private Endpoints (P1)

### Task 3.1: Annotate Private Handlers

Add `#[utoipa::path]` with `security = [("Ed25519Auth" = [])]`:

| Handler | Endpoint |
|---------|----------|
| `create_order` | POST /api/v1/private/order |
| `cancel_order` | POST /api/v1/private/cancel |
| `get_orders` | GET /api/v1/private/orders |
| `get_trades` | GET /api/v1/private/trades |
| `get_balances` | GET /api/v1/private/balances |
| `get_all_balances` | GET /api/v1/private/balances/all |
| `create_transfer` | POST /api/v1/private/transfer |
| `get_transfer` | GET /api/v1/private/transfer/{req_id} |

### Task 3.2: Implement SecurityAddon

**File**: `src/gateway/openapi.rs`

Implement `utoipa::Modify` trait to add Ed25519 auth documentation:
```rust
struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        // Add Ed25519Auth security scheme
    }
}
```

### ✅ Phase 3 Acceptance Criteria

- [ ] 9 private endpoints visible with lock icon
- [ ] "Authorize" button shows Ed25519 auth instructions
- [ ] 401 error response documented

---

## 🔧 Phase 4: Client SDK Generation (P2)

### Task 4.1: Export OpenAPI JSON

```bash
# During build or startup, export to docs/
curl http://localhost:8080/api-docs/openapi.json > docs/openapi.json
```

### Task 4.2: Create SDK Generation Script

**New File**: `scripts/generate_clients.sh`

```bash
#!/bin/bash
npx @openapitools/openapi-generator-cli generate \
  -i docs/openapi.json \
  -g python \
  -o clients/python

npx @openapitools/openapi-generator-cli generate \
  -i docs/openapi.json \
  -g typescript-fetch \
  -o clients/typescript
```

### ✅ Phase 4 Acceptance Criteria

- [ ] `docs/openapi.json` committed to repo
- [ ] Python client in `clients/python/`
- [ ] TypeScript client in `clients/typescript/`

---

## ⚠️ Implementation Notes

### DO (必须)

- [x] Use `#[utoipa::path]` on every handler
- [x] Add `#[schema(example = ...)]` to schema fields
- [x] Document error responses (400, 401, 503)
- [x] Keep existing handler signatures unchanged

### DON'T (禁止)

- [x] Don't modify existing API response formats
- [x] Don't remove existing routes
- [x] Don't change handler function names

---

## 🧪 Verification

### Unit Test

```bash
cargo test --lib gateway::openapi
```

### Manual Verification

1. Start Gateway: `cargo run --release -- --gateway --port 8080`
2. Open browser: `http://localhost:8080/docs`
3. Verify "Market Data" tag shows 6 endpoints
4. Click "GET /depth" → "Try it out" → "Execute"
5. Verify response matches schema

### CI Integration

Add to `.github/workflows/ci.yml`:
```yaml
- name: Validate OpenAPI
  run: |
    cargo run --release -- --gateway &
    sleep 5
    curl -s http://localhost:8080/api-docs/openapi.json | npx swagger-cli validate -
```

---

## 🔑 Key Design Decisions (from Architect)

| Decision | Rationale | Alternatives Rejected |
|----------|-----------|----------------------|
| **utoipa v5.3** | Axum 0.6+ support, active maintenance | paperclip (OpenAPI 2.0 only) |
| **Code-First** | Single source of truth | Spec-First YAML (drift risk) |
| **Swagger UI** | Interactive, industry standard | ReDoc (less interactive) |

---

## 📊 Metrics

| Metric | Target |
|--------|--------|
| Public endpoints documented | 6/6 |
| Private endpoints documented | 9/9 |
| Schema accuracy | 100% (no drift from code) |
| Build time increase | < 5 seconds |

---

## 📞 Ready for Development

**Architect Sign-off**: @Architect AI  
**Date**: 2025-12-26  
**Confidence**: HIGH (design doc complete, dependencies verified)

---

*Start with Phase 1, verify Swagger UI works, then proceed incrementally.*
