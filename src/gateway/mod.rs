pub mod cache;
pub mod handlers;
pub mod openapi;
pub mod state;
pub mod types;

use axum::{
    Router,
    body::Body,
    extract::State,
    http::Request,
    middleware::{Next, from_fn_with_state},
    response::Response,
    routing::{get, post},
};
use std::sync::Arc;
use tokio::net::TcpListener;

// Phase 0x0E: OpenAPI / Swagger UI
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::market::depth_service::DepthService;
use crate::symbol_manager::SymbolManager;
use crate::websocket::{ConnectionManager, ws_handler};
use crossbeam_queue::ArrayQueue;

use crate::persistence::TDengineClient;
use crate::pipeline::OrderAction;
use state::AppState;

// Phase 0x0A: Account management types
use crate::account::{Asset, Database, Symbol};

// Phase 0x0A-b: Authentication
use crate::api_auth::{
    ApiKeyRepository, AuthError, AuthState, AuthenticatedUser, TsStore, extract_auth_header,
    parse_authorization, validate_ts_nonce, verify_signature,
};

// Phase 0x0B-a: Internal Transfer FSM
use crate::internal_transfer::{
    adapters::{FundingAdapter, TradingAdapter},
    channel::TransferSender,
    coordinator::TransferCoordinator,
    db::TransferDb,
};

/// Axum middleware for API authentication in Gateway.
async fn gateway_auth_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request<Body>,
    next: Next,
) -> Result<Response, AuthError> {
    use crate::api_auth::AuthErrorCode;

    // Step 1: Extract Authorization header
    let auth_header = extract_auth_header(request.headers())?;

    // Step 2: Parse header components
    let (_version, api_key, ts_nonce_str, signature) = parse_authorization(auth_header)?;
    let ts_nonce: i64 = ts_nonce_str
        .parse()
        .map_err(|_| AuthError::from_code(AuthErrorCode::TsNonceRejected))?;

    // Step 3: Get API Key from database
    let db = state
        .pg_db
        .as_ref()
        .ok_or_else(|| AuthError::new(AuthErrorCode::InternalError, "Database not configured"))?;
    let repo = ApiKeyRepository::new(db.clone());
    let api_key_record = repo
        .get_active_by_key(api_key)
        .await
        .map_err(|e| AuthError::new(AuthErrorCode::InternalError, format!("DB error: {}", e)))?
        .ok_or_else(|| AuthError::from_code(AuthErrorCode::InvalidApiKey))?;

    // Step 4: Check API Key status
    if api_key_record.status != 1 {
        return Err(AuthError::from_code(AuthErrorCode::ApiKeyDisabled));
    }

    // Step 5: Validate ts_nonce (time window + monotonic)
    validate_ts_nonce(
        &state.auth_state.ts_store,
        api_key,
        ts_nonce,
        state.auth_state.time_window_ms,
    )?;

    // Step 6: Build signature payload
    let method = request.method().as_str();
    // Use OriginalUri to get the full path + query before route matching strips nested prefixes
    let original_uri = request
        .extensions()
        .get::<axum::extract::OriginalUri>()
        .map(|uri| {
            uri.0
                .path_and_query()
                .map(|pq| pq.as_str())
                .unwrap_or(uri.0.path())
        })
        .unwrap_or_else(|| {
            request
                .uri()
                .path_and_query()
                .map(|pq| pq.as_str())
                .unwrap_or(request.uri().path())
        });
    let body = "";

    // Debug: log signature verification payload
    eprintln!(
        "[DEBUG] Auth payload: api_key={}, ts_nonce={}, method={}, uri={}",
        api_key, ts_nonce_str, method, original_uri
    );

    // Step 7: Verify signature
    verify_signature(
        &api_key_record,
        api_key,
        ts_nonce_str,
        method,
        original_uri,
        body,
        signature,
    )?;

    // Step 8: Create authenticated user and inject
    let auth_user = AuthenticatedUser {
        user_id: api_key_record.user_id,
        api_key: api_key_record.api_key.clone(),
        permissions: api_key_record.permissions,
    };
    request.extensions_mut().insert(auth_user);

    // Step 9: Continue to handler
    Ok(next.run(request).await)
}

/// Start HTTP Gateway server
#[allow(clippy::too_many_arguments)]
pub async fn run_server(
    port: u16,
    order_queue: Arc<ArrayQueue<OrderAction>>,
    symbol_mgr: Arc<SymbolManager>,
    active_symbol_id: u32,
    db_client: Option<Arc<TDengineClient>>,
    push_event_queue: Arc<ArrayQueue<crate::websocket::PushEvent>>,
    depth_service: Arc<DepthService>,
    pg_db: Option<Arc<Database>>,
    pg_assets: Arc<Vec<Asset>>,
    pg_symbols: Arc<Vec<Symbol>>,
    // Phase 0x0B-a: Optional channel to UBSCore for TradingAdapter
    transfer_sender: Option<TransferSender>,
    // WebSocket manager for broadcasting
    ws_manager: Arc<ConnectionManager>,
    // JWT Secret for User Auth (Phase 0x10.6)
    jwt_secret: String,
) {
    // WebSocket connection manager is passed in

    // Start WebSocket push service
    let ws_service =
        crate::websocket::WsService::new(ws_manager.clone(), push_event_queue, symbol_mgr.clone());
    tokio::spawn(async move {
        ws_service.run().await;
    });
    println!("📡 WebSocket push service started");

    // Create Auth state (Phase 0x0A-b)
    let auth_state = Arc::new(AuthState {
        ts_store: Arc::new(TsStore::new()),
        time_window_ms: 30_000, // 30 seconds
    });

    // Create User Auth Service (Phase 0x10.6)
    let user_auth = if let Some(ref db) = pg_db {
        Some(Arc::new(crate::user_auth::UserAuthService::new(
            db.pool().clone(),
            jwt_secret,
        )))
    } else {
        println!("⚠️  User Auth Service disabled (PostgreSQL required)");
        None
    };

    // ==========================================================================
    // Phase 0x0B-a: Initialize Internal Transfer FSM
    // ==========================================================================
    let transfer_coordinator = if let Some(ref db) = pg_db {
        // Create TransferDb using same connection pool as account DB
        let transfer_db = Arc::new(TransferDb::new(db.pool().clone()));

        // Create adapters
        // FundingAdapter uses PostgreSQL directly
        let funding_adapter = Arc::new(FundingAdapter::new(db.pool().clone()));
        // TradingAdapter uses UBSCore via TransferChannel (if provided)
        let trading_adapter = if let Some(sender) = transfer_sender {
            println!("  → TradingAdapter connected to UBSCore via TransferChannel");
            Arc::new(TradingAdapter::with_channel(sender))
        } else {
            println!("  → TradingAdapter in simulation mode (no UBSCore connection)");
            Arc::new(TradingAdapter::new())
        };

        // Create coordinator
        let coordinator = Arc::new(TransferCoordinator::new(
            transfer_db,
            funding_adapter,
            trading_adapter,
        ));
        println!("🔄 Transfer FSM coordinator initialized");
        Some(coordinator)
    } else {
        println!("⚠️  Transfer FSM disabled (no PostgreSQL connection)");
        None
    };

    // Create shared state
    let mut state = AppState::new(
        order_queue,
        symbol_mgr,
        active_symbol_id,
        db_client,
        ws_manager.clone(),
        depth_service,
        pg_db.clone(),
        pg_assets,
        pg_symbols,
        auth_state,
        user_auth.clone(),
    );

    // Wire TransferCoordinator to state
    if let Some(coordinator) = transfer_coordinator {
        state = state.with_transfer_coordinator(coordinator);
    }

    let state = Arc::new(state);

    // ==========================================================================
    // User Auth Routes (Phase 0x10.6)
    // ==========================================================================
    let auth_routes = Router::new()
        .route("/register", post(crate::user_auth::handlers::register))
        .route("/login", post(crate::user_auth::handlers::login));

    // ==========================================================================
    // User Routes (Phase 0x10.6) - Protected by JWT
    // ==========================================================================
    let user_routes = Router::new()
        .route("/apikeys", post(crate::user_auth::handlers::create_api_key))
        .route("/apikeys", get(crate::user_auth::handlers::list_api_keys))
        .route(
            "/apikeys/{api_key}",
            axum::routing::delete(crate::user_auth::handlers::delete_api_key),
        )
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::user_auth::middleware::jwt_auth_middleware,
        ));

    // ==========================================================================
    // Public Routes (no auth required)
    // ==========================================================================
    let public_routes = Router::new()
        // Market data
        .route("/exchange_info", get(handlers::get_exchange_info))
        .route("/assets", get(handlers::get_assets))
        .route("/symbols", get(handlers::get_symbols))
        .route("/depth", get(handlers::get_depth))
        .route("/klines", get(handlers::get_klines))
        .route("/trades", get(handlers::get_public_trades));

    // ==========================================================================
    // Private Routes (auth required)
    // ==========================================================================
    let private_routes = Router::new()
        // Account queries
        .route("/orders", get(handlers::get_orders))
        .route("/order/{order_id}", get(handlers::get_order))
        .route("/trades", get(handlers::get_trades))
        .route("/balances", get(handlers::get_balances))
        .route("/balances/all", get(handlers::get_all_balances))
        .route("/account", get(handlers::get_account))
        // Trading operations
        .route("/order", post(handlers::create_order))
        .route("/cancel", post(handlers::cancel_order))
        .route(
            "/order/{order_id}",
            axum::routing::delete(handlers::cancel_order_by_id),
        )
        .route("/order/reduce", post(handlers::reduce_order))
        .route("/order/move", post(handlers::move_order))
        // Internal transfers (Phase 0x0B-a)
        .route("/transfer", post(handlers::create_transfer))
        .route("/transfer/{req_id}", get(handlers::get_transfer))
        // Apply auth middleware
        .layer(from_fn_with_state(state.clone(), gateway_auth_middleware));

    // Build complete router
    let app = Router::new()
        // WebSocket endpoint
        .route("/ws", get(ws_handler))
        // Health check
        .route("/api/v1/health", get(handlers::health_check))
        // API Routes
        .nest("/api/v1/auth", auth_routes)
        .nest("/api/v1/user", user_routes) // Phase 0x10.6 User Center
        .nest("/api/v1/public", public_routes)
        .nest("/api/v1/private", private_routes)
        .nest(
            "/api/v1/capital",
            Router::new()
                .route(
                    "/deposit/address",
                    get(crate::funding::handlers::get_deposit_address),
                )
                .route(
                    "/deposit/history",
                    get(crate::funding::handlers::get_deposit_history),
                )
                .route(
                    "/withdraw/apply",
                    post(crate::funding::handlers::apply_withdraw),
                )
                .route(
                    "/withdraw/history",
                    get(crate::funding::handlers::get_withdraw_history),
                )
                .route("/account", get(handlers::get_account_jwt))
                .route("/balances", get(handlers::get_balance_jwt))
                .route("/balances/all", get(handlers::get_account_jwt)) // Use account_jwt for all balances
                .route("/transfer", post(handlers::create_transfer_jwt))
                .route("/transfer/{req_id}", get(handlers::get_transfer))
                .route("/order", post(handlers::create_order_jwt))
                .route("/cancel", post(handlers::cancel_order_jwt))
                .route("/orders", get(handlers::get_orders_jwt))
                .route("/order/{order_id}", get(handlers::get_order))
                .route("/trades", get(handlers::get_trades))
                .layer(from_fn_with_state(
                    state.clone(),
                    crate::user_auth::middleware::jwt_auth_middleware,
                )),
        );

    // [SECURITY] Mock API routes - only compiled when 'mock-api' feature is enabled.
    // Production builds MUST be compiled with `--no-default-features` to exclude this.
    #[cfg(feature = "mock-api")]
    let app = app.nest(
        "/internal/mock",
        Router::new().route("/deposit", post(crate::funding::handlers::mock_deposit)),
    );

    let app = app
        .with_state(state)
        // Phase 0x0E: OpenAPI / Swagger UI (stateless, added after with_state)
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", openapi::ApiDoc::openapi()));

    // Bind address
    let addr = format!("0.0.0.0:{}", port);
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("❌ FATAL: Failed to bind to {}: {}", addr, e);
            eprintln!(
                "   Hint: Port {} may already be in use. Check with: lsof -i :{}",
                port, port
            );
            std::process::exit(1);
        }
    };

    println!("🚀 Gateway listening on http://{}", addr);
    println!("📡 WebSocket endpoint: ws://{}/ws", addr);
    println!("📖 API Docs: http://{}/docs", addr);
    println!("📂 Public API:  /api/v1/public/*");
    println!("🔒 Private API: /api/v1/private/* (auth required)");

    // Start server
    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("❌ FATAL: Server error: {}", e);
        std::process::exit(1);
    }
}
