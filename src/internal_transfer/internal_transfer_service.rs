//! Internal Transfer Service
//!
//! Combined HTTP API + Background Worker for internal transfers.
//!
//! This service can run in two modes:
//! 1. HTTP + Worker (default): Provides HTTP API + background processing
//! 2. Worker only: Set WORKER_ONLY=1 for background processing only
//!
//! In production, the Gateway handles HTTP, and this runs as worker only.
//! For testing, this can be the standalone HTTP+Worker service.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    extract::{Extension, Json, Path},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use tower_http::cors::CorsLayer;

use fetcher::transfer::{
    TransferCoordinator, TransferDb, TransferQueue, TransferRequest, TransferState,
    TransferWorker, WorkerConfig,
    adapters::{TbFundingAdapter, TbTradingAdapter},
};

/// Application state
struct AppState {
    coordinator: Arc<TransferCoordinator>,
    worker: Arc<TransferWorker>,
    queue: Arc<TransferQueue>,
}

#[derive(Debug, serde::Deserialize)]
struct TransferReq {
    from: String,
    to: String,
    user_id: u64,
    asset_id: u32,
    amount: u64,
}

#[derive(Debug, serde::Serialize)]
struct TransferResp {
    req_id: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// POST /api/v1/transfer
async fn handle_transfer(
    Extension(state): Extension<Arc<AppState>>,
    Json(payload): Json<TransferReq>,
) -> impl IntoResponse {
    use fetcher::transfer::ServiceId;

    // Validate
    if payload.amount == 0 {
        return Json(TransferResp {
            req_id: "".to_string(),
            status: "failed".to_string(),
            message: None,
            error: Some("Amount must be greater than 0".to_string()),
        }).into_response();
    }

    if payload.from == payload.to {
        return Json(TransferResp {
            req_id: "".to_string(),
            status: "failed".to_string(),
            message: None,
            error: Some("Source and target cannot be the same".to_string()),
        }).into_response();
    }

    // Parse service IDs
    let from: ServiceId = match payload.from.parse() {
        Ok(s) => s,
        Err(_) => {
            return Json(TransferResp {
                req_id: "".to_string(),
                status: "failed".to_string(),
                message: None,
                error: Some(format!("Invalid source: {}", payload.from)),
            }).into_response();
        }
    };

    let to: ServiceId = match payload.to.parse() {
        Ok(s) => s,
        Err(_) => {
            return Json(TransferResp {
                req_id: "".to_string(),
                status: "failed".to_string(),
                message: None,
                error: Some(format!("Invalid target: {}", payload.to)),
            }).into_response();
        }
    };

    let req = TransferRequest {
        from,
        to,
        user_id: payload.user_id,
        asset_id: payload.asset_id,
        amount: payload.amount,
    };

    let req_id = match state.coordinator.create(req).await {
        Ok(id) => id,
        Err(e) => {
            return Json(TransferResp {
                req_id: "".to_string(),
                status: "failed".to_string(),
                message: None,
                error: Some(e.to_string()),
            }).into_response();
        }
    };

    // Process using FSM through ServiceAdapter abstraction
    let result = state.worker.process_now(req_id).await;

    let (status, message) = match result {
        TransferState::Committed => ("committed", None),
        TransferState::RolledBack => ("rolled_back", Some("Transfer cancelled".to_string())),
        TransferState::Failed => ("failed", Some("Transfer failed".to_string())),
        _ => {
            let _ = state.queue.try_push(req_id);
            ("pending", Some("Processing in background".to_string()))
        }
    };

    Json(TransferResp {
        req_id: req_id.to_string(),
        status: status.to_string(),
        message,
        error: None,
    }).into_response()
}

/// GET /api/v1/transfer/:req_id
async fn get_transfer(
    Extension(state): Extension<Arc<AppState>>,
    Path(req_id): Path<String>,
) -> impl IntoResponse {
    use fetcher::transfer::RequestId;

    let request_id = match RequestId::from_str(&req_id) {
        Ok(id) => id,
        Err(_) => {
            return Json(serde_json::json!({
                "error": "Invalid req_id format"
            })).into_response();
        }
    };

    match state.coordinator.get(request_id).await {
        Ok(Some(record)) => {
            Json(serde_json::json!({
                "req_id": record.req_id.to_string(),
                "state": record.state.as_ref(),
                "source": record.source.as_ref(),
                "target": record.target.as_ref(),
                "user_id": record.user_id,
                "asset_id": record.asset_id,
                "amount": record.amount,
                "created_at": record.created_at,
                "updated_at": record.updated_at,
                "error": record.error,
                "retry_count": record.retry_count,
            })).into_response()
        }
        Ok(None) => {
            Json(serde_json::json!({
                "error": "Transfer not found"
            })).into_response()
        }
        Err(e) => {
            Json(serde_json::json!({
                "error": e.to_string()
            })).into_response()
        }
    }
}

async fn run_http_server(state: Arc<AppState>) {
    let app = Router::new()
        .route("/api/v1/transfer", post(handle_transfer))
        .route("/api/v1/transfer/:req_id", get(get_transfer))
        .layer(Extension(state))
        .layer(CorsLayer::permissive());

    let port = 8080;
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("🚀 Internal Transfer Service on http://127.0.0.1:{}", port);
    println!("");
    println!("📤 Endpoints:");
    println!("  POST /api/v1/transfer       - Create internal transfer");
    println!("  GET  /api/v1/transfer/:id   - Query transfer status");

    axum::serve(
        tokio::net::TcpListener::bind(&addr).await.unwrap(),
        app.into_make_service(),
    )
    .await
    .unwrap();
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let worker_only = std::env::var("WORKER_ONLY").is_ok();

    if worker_only {
        println!("🔄 Internal Transfer Service (Worker Only Mode)");
        println!("================================================");
    } else {
        println!("🧪 Internal Transfer Service (HTTP + Worker Mode)");
        println!("==================================================");
    }

    // Connect to ScyllaDB with retry
    println!("📦 Connecting to ScyllaDB (with retry)...");
    let max_retries = 10;
    let mut retry_delay_ms = 1000u64;
    let mut session_opt = None;

    for attempt in 1..=max_retries {
        match scylla::SessionBuilder::new()
            .known_node("127.0.0.1:9042")
            .build()
            .await
        {
            Ok(s) => {
                println!("✅ Connected to ScyllaDB (attempt {})", attempt);
                session_opt = Some(Arc::new(s));
                break;
            }
            Err(e) => {
                if attempt < max_retries {
                    eprintln!("⚠️ ScyllaDB connection attempt {} failed: {}. Retrying in {}ms...", attempt, e, retry_delay_ms);
                    tokio::time::sleep(tokio::time::Duration::from_millis(retry_delay_ms)).await;
                    retry_delay_ms = (retry_delay_ms * 2).min(30000); // Max 30 seconds
                } else {
                    eprintln!("❌ Failed to connect to ScyllaDB after {} attempts: {}", max_retries, e);
                    std::process::exit(1);
                }
            }
        }
    }
    let session = session_opt.expect("Session should be set");

    // Initialize schema
    println!("📋 Setting up schema...");
    let schema = r#"
        CREATE TABLE IF NOT EXISTS trading.transfers (
            req_id bigint,
            source text,
            target text,
            user_id bigint,
            asset_id int,
            amount bigint,
            state text,
            created_at bigint,
            updated_at bigint,
            error text,
            retry_count int,
            PRIMARY KEY (req_id)
        )
    "#;
    if let Err(e) = session.query(schema, &[]).await {
        eprintln!("⚠️ Schema setup warning: {}", e);
    }

    // Create DB layer
    let db = Arc::new(TransferDb::new(session));

    // Connect to TigerBeetle with retry
    println!("🐯 Connecting to TigerBeetle (with retry)...");
    let tb_address = std::env::var("TIGERBEETLE_ADDRESS").unwrap_or_else(|_| "3000".to_string());
    let mut tb_retry_delay_ms = 1000u64;
    let mut tb_client_opt = None;

    for attempt in 1..=max_retries {
        match tigerbeetle_unofficial::Client::new(0, &tb_address) {
            Ok(client) => {
                println!("✅ Connected to TigerBeetle at {} (attempt {})", tb_address, attempt);
                tb_client_opt = Some(Arc::new(client));
                break;
            }
            Err(e) => {
                if attempt < max_retries {
                    eprintln!("⚠️ TigerBeetle connection attempt {} failed: {:?}. Retrying in {}ms...", attempt, e, tb_retry_delay_ms);
                    tokio::time::sleep(tokio::time::Duration::from_millis(tb_retry_delay_ms)).await;
                    tb_retry_delay_ms = (tb_retry_delay_ms * 2).min(30000);
                } else {
                    eprintln!("❌ Failed to connect to TigerBeetle after {} attempts: {:?}", max_retries, e);
                    eprintln!("   Make sure TigerBeetle is running on port {}", tb_address);
                    std::process::exit(1);
                }
            }
        }
    }
    let tb_client: Arc<tigerbeetle_unofficial::Client> = tb_client_opt.expect("TB client should be set");

    // Create adapters
    // Funding: TigerBeetle direct
    let funding: Arc<dyn fetcher::transfer::adapters::ServiceAdapter + Send + Sync> =
        Arc::new(TbFundingAdapter::new(tb_client.clone()));

    // Trading: Use UBSCore via Aeron in production, TigerBeetle in test
    #[cfg(feature = "aeron")]
    let trading: Arc<dyn fetcher::transfer::adapters::ServiceAdapter + Send + Sync> = {
        use fetcher::transfer::adapters::UbsTradingAdapter;
        match UbsTradingAdapter::new() {
            Ok(adapter) => {
                println!("✅ Trading adapter: UBSCore (via Aeron)");
                Arc::new(adapter)
            }
            Err(e) => {
                eprintln!("⚠️ Failed to connect to UBSCore: {}", e);
                eprintln!("   Using TigerBeetle direct adapter");
                Arc::new(TbTradingAdapter::new(tb_client.clone()))
            }
        }
    };

    #[cfg(not(feature = "aeron"))]
    let trading: Arc<dyn fetcher::transfer::adapters::ServiceAdapter + Send + Sync> = {
        println!("✅ Trading adapter: TigerBeetle (direct)");
        Arc::new(TbTradingAdapter::new(tb_client.clone()))
    };

    // Create coordinator
    let coordinator = Arc::new(TransferCoordinator::new(
        db.clone(),
        funding,
        trading,
    ));

    // Create queue and worker
    let queue = Arc::new(TransferQueue::new(10000));
    let config = WorkerConfig::default();

    let worker = Arc::new(TransferWorker::new(
        coordinator.clone(),
        db.clone(),
        queue.clone(),
        config,
    ));

    // Spawn background worker
    let worker_clone = worker.clone();
    tokio::spawn(async move {
        worker_clone.run().await;
    });

    if worker_only {
        println!("");
        println!("🔄 Worker running in background...");
        println!("   Press Ctrl+C to stop.");

        // Just wait forever
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
        }
    } else {
        let state = Arc::new(AppState {
            coordinator,
            worker,
            queue,
        });

        run_http_server(state).await;
    }
}
