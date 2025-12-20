//! 0xInfinity - High-Frequency Trading Engine
//!
//! Chapter 8b: UBSCore Integration
//!
//! This is the main entry point. Architecture:
//!
//! ```text
//! ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐
//! │  Config  │───▶│ UBSCore  │───▶│  Engine  │───▶│  Output  │
//! │  (CSV)   │    │(WAL+Lock)│    │ (Match)  │    │  (CSV)   │
//! └──────────┘    └──────────┘    └──────────┘    └──────────┘
//!
//! UBSCore responsibilities:
//! - InternalOrder WAL (persistence first!)
//! - Balance Lock/Unlock/Settle
//! - Single-threaded atomic operations
//! ```

use std::fs::File;
use std::io::Write;
use std::time::Instant;

use zero_x_infinity::csv_io::{
    chrono_lite_now, dump_balances, dump_orderbook_snapshot, load_balances_and_deposit,
    load_orders, load_symbol_manager,
};
use zero_x_infinity::ledger::LedgerWriter;
use zero_x_infinity::messages::BalanceEvent;
use zero_x_infinity::orderbook::OrderBook;
use zero_x_infinity::perf::PerfMetrics;
use zero_x_infinity::pipeline_mt::run_pipeline_multi_thread;
use zero_x_infinity::pipeline_runner::run_pipeline_single_thread;
use zero_x_infinity::ubscore::UBSCore;
use zero_x_infinity::wal::WalConfig;

// ============================================================
// OUTPUT DIRECTORY
// ============================================================

fn get_output_dir() -> &'static str {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--baseline") {
        "baseline/default"
    } else {
        "output"
    }
}

fn get_input_dir() -> String {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--input" && i + 1 < args.len() {
            return args[i + 1].clone();
        }
    }
    "fixtures".to_string()
}

fn get_env() -> String {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() {
        if (args[i] == "--env" || args[i] == "-e") && i + 1 < args.len() {
            return args[i + 1].clone();
        }
    }
    "dev".to_string()
}

// ============================================================
// ORDER EXECUTION
// ============================================================

// Note: execute_orders functions removed in favor of PipelineRunner
// Note: execute_orders functions removed in favor of PipelineRunner

// ============================================================
// MAIN
// ============================================================

fn use_ubscore_mode() -> bool {
    std::env::args().any(|a| a == "--ubscore")
}

fn use_pipeline_mode() -> bool {
    std::env::args().any(|a| a == "--pipeline")
}

fn use_pipeline_mt_mode() -> bool {
    std::env::args().any(|a| a == "--pipeline-mt")
}

fn use_gateway_mode() -> bool {
    std::env::args().any(|a| a == "--gateway")
}

/// Get port override from command line (--port argument)
fn get_port_override() -> Option<u16> {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--port" && i + 1 < args.len() {
            return args[i + 1].parse().ok();
        }
    }
    None
}

fn main() {
    let output_dir = get_output_dir();
    let input_dir = get_input_dir();
    let ubscore_mode = use_ubscore_mode();
    let pipeline_mode = use_pipeline_mode();
    let pipeline_mt_mode = use_pipeline_mt_mode();
    let gateway_mode = use_gateway_mode();

    let env = get_env();
    let app_config = zero_x_infinity::config::AppConfig::load(&env);
    let _log_guard = zero_x_infinity::logging::init_logging(&app_config);

    tracing::info!("Starting 0xInfinity Engine in {} mode", env);

    // Gateway mode: HTTP + Trading Core
    if gateway_mode {
        println!("=== 0xInfinity: Chapter 9a - Gateway Mode ===");

        // Load configuration
        let (symbol_mgr, active_symbol_id) = load_symbol_manager();

        // Get Gateway config from YAML, allow --port override
        let gateway_config = &app_config.gateway;
        let port = if let Some(override_port) = get_port_override() {
            override_port
        } else {
            gateway_config.port
        };

        println!("Gateway will listen on {}:{}", gateway_config.host, port);
        println!(
            "Active symbol: {}",
            symbol_mgr
                .get_symbol_info_by_id(active_symbol_id)
                .unwrap()
                .symbol
        );
        println!("Order queue size: {}", gateway_config.queue_size);

        // Create shared queues
        let queues = std::sync::Arc::new(zero_x_infinity::MultiThreadQueues::new());
        let symbol_mgr = std::sync::Arc::new(symbol_mgr);

        // Create DepthService
        let depth_service = std::sync::Arc::new(
            zero_x_infinity::market::depth_service::DepthService::new(queues.clone()),
        );

        // Create tokio runtime and initialize TDengine in main thread
        // This allows sharing db_client with both Gateway and Pipeline
        let shared_rt = std::sync::Arc::new(tokio::runtime::Runtime::new().unwrap());
        let rt_handle = shared_rt.handle().clone();

        let persistence_config = app_config.persistence.clone();
        let db_client: Option<std::sync::Arc<zero_x_infinity::persistence::TDengineClient>> =
            if persistence_config.enabled {
                println!("\n[Persistence] Connecting to TDengine...");
                shared_rt.block_on(async {
                    match zero_x_infinity::persistence::TDengineClient::connect(
                        &persistence_config.tdengine_dsn,
                    )
                    .await
                    {
                        Ok(client) => match client.init_schema().await {
                            Ok(_) => {
                                println!("✅ TDengine connected and schema initialized");
                                // Create K-Line streams
                                if let Err(e) =
                                    zero_x_infinity::persistence::klines::create_kline_streams(
                                        client.taos(),
                                    )
                                    .await
                                {
                                    eprintln!("⚠️ Failed to create K-Line streams: {}", e);
                                } else {
                                    println!("✅ K-Line streams created");
                                }
                                Some(std::sync::Arc::new(client))
                            }
                            Err(e) => {
                                eprintln!("❌ Failed to initialize TDengine schema: {}", e);
                                None
                            }
                        },
                        Err(e) => {
                            eprintln!("❌ Failed to connect to TDengine: {}", e);
                            None
                        }
                    }
                })
            } else {
                println!("\n[Persistence] Disabled");
                None
            };

        let db_client_for_gateway = db_client.clone();
        let db_client_for_pipeline = db_client.clone();
        let rt_handle_for_pipeline = Some(rt_handle.clone());

        // Start HTTP Server in separate thread with tokio runtime
        let queues_clone = queues.clone();
        let symbol_mgr_clone = symbol_mgr.clone();
        let gateway_thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // Spawn DepthService task (inside tokio runtime)
                let depth_service_clone = depth_service.clone();
                tokio::spawn(async move {
                    depth_service_clone.run().await;
                });
                println!("📊 DepthService started");

                // Use shared db_client from main thread (no duplicate init needed)
                zero_x_infinity::gateway::run_server(
                    port,
                    queues_clone.order_queue.clone(),
                    symbol_mgr_clone,
                    active_symbol_id,
                    db_client_for_gateway,
                    queues_clone.push_event_queue.clone(),
                    depth_service,
                )
                .await;
            });
        });

        println!("🚀 Gateway thread started");

        // Prepare for Trading Core
        println!("\n[1] Initializing Trading Core...");

        // Load initial balances
        let accounts = load_balances_and_deposit(&format!("{}/balances_init.csv", input_dir));

        // Create output paths
        std::fs::create_dir_all(output_dir).unwrap();
        let ledger_path = format!("{}/t2_ledger.csv", output_dir);
        let events_path = format!("{}/t2_events.csv", output_dir);
        let wal_path = format!("{}/orders.wal", output_dir);

        // Initialize services (use the SHARED OrderBook)
        let mut ledger = LedgerWriter::new(&ledger_path);
        ledger.enable_event_logging(&events_path);

        let wal_config = WalConfig {
            path: wal_path,
            flush_interval_entries: 100,
            sync_on_flush: false,
        };

        let mut ubscore =
            UBSCore::new((*symbol_mgr).clone(), wal_config).expect("Failed to create UBSCore");

        // Transfer initial balances to UBSCore
        let symbol_info = symbol_mgr.get_symbol_info_by_id(active_symbol_id).unwrap();
        let base_id = symbol_info.base_asset_id;
        let quote_id = symbol_info.quote_asset_id;

        for (user_id, account) in &accounts {
            for asset_id in [base_id, quote_id] {
                if let Some(balance) = account.get_balance(asset_id) {
                    if balance.avail() > 0 {
                        ubscore
                            .deposit(*user_id, asset_id, balance.avail())
                            .unwrap();
                    }
                }
            }
        }

        println!("✅ Trading Core initialized");
        println!(
            "\n🎯 System ready! Send orders to http://localhost:{}/api/v1/create_order",
            port
        );
        println!("Press Ctrl+C to shutdown\n");

        // Create OrderBook for Trading Core
        let book = OrderBook::new();

        // Run Trading Core (this will block and process orders from gateway)
        let _result = run_pipeline_multi_thread(
            vec![], // No pre-loaded orders, will come from gateway
            zero_x_infinity::pipeline::PipelineServices {
                ubscore,
                book,
                ledger,
            },
            zero_x_infinity::pipeline::PipelineConfig {
                symbol_mgr: &symbol_mgr,
                active_symbol_id,
                sample_rate: app_config.sample_rate,
                continuous: true,
            },
            queues,
            rt_handle_for_pipeline, // rt_handle for SettlementService TDengine
            db_client_for_pipeline, // db_client for SettlementService TDengine
        );

        // Wait for gateway thread
        gateway_thread.join().unwrap();

        return;
    }

    if pipeline_mt_mode {
        println!("=== 0xInfinity: Chapter 8g - Multi-Thread Pipeline ===");
    } else if pipeline_mode {
        println!("=== 0xInfinity: Chapter 8f - Ring Buffer Pipeline ===");
    } else if ubscore_mode {
        println!("=== 0xInfinity: Chapter 8b - UBSCore Pipeline ===");
    } else {
        println!("=== 0xInfinity: Chapter 7 - Testing Framework ===");
    }
    println!("Output directory: {}/", output_dir);
    println!("Input directory: {}/\n", input_dir);

    let start_time = Instant::now();

    // Step 1: Load configuration
    println!("[1] Loading configuration...");
    let (symbol_mgr, active_symbol_id) = load_symbol_manager();

    // Generate output paths
    let balances_t1 = format!("{}/t1_balances_deposited.csv", output_dir);
    let balances_t2 = format!("{}/t2_balances_final.csv", output_dir);
    let orderbook_t2 = format!("{}/t2_orderbook.csv", output_dir);
    let ledger_path = format!("{}/t2_ledger.csv", output_dir);
    let events_path = format!("{}/t2_events.csv", output_dir); // New: BalanceEvent log
    let order_events_path = format!("{}/t2_order_events.csv", output_dir); // New: OrderEvent log
    let summary_path = format!("{}/t2_summary.txt", output_dir);
    let wal_path = format!("{}/orders.wal", output_dir);

    std::fs::create_dir_all(output_dir).unwrap();

    // Step 2: Load balances
    println!("\n[2] Loading balances and depositing...");
    let accounts = load_balances_and_deposit(&format!("{}/balances_init.csv", input_dir));

    // Step 3: Snapshot after deposit
    println!("\n[3] Dumping balance snapshot after deposit...");
    dump_balances(&accounts, &symbol_mgr, active_symbol_id, &balances_t1);

    // Step 4: Load orders
    println!("\n[4] Loading orders...");
    let orders = load_orders(
        &format!("{}/orders.csv", input_dir),
        &symbol_mgr,
        active_symbol_id,
    );

    let load_time = start_time.elapsed();
    println!("\n    Data loading completed in {:.2?}", load_time);

    // Step 5: Initialize engine
    println!("\n[5] Initializing matching engine...");
    let mut book = OrderBook::new();
    let mut ledger = LedgerWriter::new(&ledger_path);

    // Enable event logging for UBSCore mode (complete event sourcing)
    if ubscore_mode {
        ledger.enable_event_logging(&events_path);
        ledger.enable_order_logging(&order_events_path);
        println!(
            "    Event logging enabled: {}, {}",
            events_path, order_events_path
        );
    }

    // Step 6: Execute orders
    println!("\n[6] Executing orders...");
    let exec_start = Instant::now();

    let symbol_info = symbol_mgr
        .get_symbol_info_by_id(active_symbol_id)
        .expect("Active symbol not found");
    let base_id = symbol_info.base_asset_id;
    let quote_id = symbol_info.quote_asset_id;
    let (accepted, rejected, total_trades, perf, final_accounts, final_book) = if pipeline_mt_mode {
        println!("    Using Multi-Thread Pipeline (4 threads)...");

        // Enable event logging for multi-thread mode
        ledger.enable_event_logging(&events_path);
        println!("    Event logging enabled: {}", events_path);

        // Create UBSCore and initialize with deposits
        let wal_config = WalConfig {
            path: wal_path.clone(),
            flush_interval_entries: 100,
            sync_on_flush: false,
        };

        let mut ubscore =
            UBSCore::new(symbol_mgr.clone(), wal_config).expect("Failed to create UBSCore");

        // Transfer initial balances to UBSCore via deposit()
        let mut deposit_count = 0u64;
        for (user_id, account) in &accounts {
            for asset_id in [base_id, quote_id] {
                if let Some(balance) = account.get_balance(asset_id) {
                    if balance.avail() > 0 {
                        ubscore
                            .deposit(*user_id, asset_id, balance.avail())
                            .unwrap();

                        // Record Deposit event
                        if let Some(b) = ubscore.get_balance(*user_id, asset_id) {
                            deposit_count += 1;
                            let deposit_event = BalanceEvent::deposit(
                                *user_id,
                                asset_id,
                                deposit_count,
                                balance.avail(),
                                b.lock_version(),
                                b.avail(),
                                b.frozen(),
                            );
                            ledger.write_balance_event(&deposit_event);
                        }
                    }
                }
            }
        }
        println!("    Recorded {} deposit events", deposit_count);

        // Clone orders for multi-thread (takes ownership)
        let orders_owned: Vec<_> = orders.iter().cloned().collect();

        // Create queues
        let queues = std::sync::Arc::new(zero_x_infinity::MultiThreadQueues::new());

        // Run multi-threaded pipeline (consumes services)
        let result = run_pipeline_multi_thread(
            orders_owned,
            zero_x_infinity::pipeline::PipelineServices {
                ubscore,
                book,
                ledger,
            },
            zero_x_infinity::pipeline::PipelineConfig {
                symbol_mgr: &symbol_mgr,
                active_symbol_id,
                sample_rate: app_config.sample_rate,
                continuous: false,
            },
            queues,
            None, // rt_handle: Pipeline MT mode doesn't use TDengine
            None, // db_client
        );

        // Print stats (use snapshot to get place/cancel counts)
        let stats_snap = result.stats.snapshot();
        println!("Accepted:         {}", result.accepted);
        println!("Rejected:         {}", result.rejected);
        println!("Total Trades:     {}", result.total_trades);
        println!(
            "    Pipeline: ingested={} (place={}, cancel={}), accepted={}, rejected={}, trades={}",
            stats_snap.orders_ingested,
            stats_snap.places_count,
            stats_snap.cancels_count,
            result.accepted,
            result.rejected,
            result.total_trades
        );

        // Extract performance metrics
        let perf = if let Ok(perf) = result.stats.perf_samples.lock() {
            perf.clone()
        } else {
            PerfMetrics::default()
        };

        (
            result.accepted,
            result.rejected,
            result.total_trades,
            perf,
            result.final_accounts,
            None, // OrderBook consumed by threads
        )
    } else if pipeline_mode {
        println!("    Using Ring Buffer Pipeline (Single Thread)...");

        // Create UBSCore and initialize with deposits
        let wal_config = WalConfig {
            path: wal_path.clone(),
            flush_interval_entries: 100,
            sync_on_flush: false,
        };

        let mut ubscore =
            UBSCore::new(symbol_mgr.clone(), wal_config).expect("Failed to create UBSCore");

        // Transfer initial balances to UBSCore via deposit()
        let mut deposit_count = 0u64;
        for (user_id, account) in &accounts {
            for asset_id in [base_id, quote_id] {
                if let Some(balance) = account.get_balance(asset_id) {
                    if balance.avail() > 0 {
                        ubscore
                            .deposit(*user_id, asset_id, balance.avail())
                            .unwrap();

                        // Record Deposit event
                        if let Some(b) = ubscore.get_balance(*user_id, asset_id) {
                            deposit_count += 1;
                            let deposit_event = BalanceEvent::deposit(
                                *user_id,
                                asset_id,
                                deposit_count,
                                balance.avail(),
                                b.lock_version(),
                                b.avail(),
                                b.frozen(),
                            );
                            ledger.write_balance_event(&deposit_event);
                        }
                    }
                }
            }
        }
        println!("    Recorded {} deposit events", deposit_count);

        // Run pipeline
        let result = run_pipeline_single_thread(
            &orders,
            zero_x_infinity::pipeline::PipelineServices {
                ubscore: &mut ubscore,
                book: &mut book,
                ledger: &mut ledger,
            },
            zero_x_infinity::pipeline::PipelineConfig {
                symbol_mgr: &symbol_mgr,
                active_symbol_id,
                sample_rate: app_config.sample_rate,
                continuous: false,
            },
        );

        // Get final accounts from UBSCore
        let final_accs = ubscore.accounts().clone();

        // Print stats in a single standard format for script compatibility
        println!(
            "Accepted:         {}",
            result.pipeline_stats.orders_accepted
        );
        println!(
            "Rejected:         {}",
            result.pipeline_stats.orders_rejected
        );
        println!(
            "Total Trades:     {}",
            result.pipeline_stats.trades_generated
        );
        println!(
            "Pipeline Stats: ingested={} (place={}, cancel={}), accepted={}, rejected={}, trades={}",
            result.pipeline_stats.orders_ingested,
            result.pipeline_stats.places_count,
            result.pipeline_stats.cancels_count,
            result.pipeline_stats.orders_accepted,
            result.pipeline_stats.orders_rejected,
            result.pipeline_stats.trades_generated
        );

        (
            result.accepted,
            result.rejected,
            result.total_trades,
            result.perf,
            final_accs,
            Some(book),
        )
    } else if ubscore_mode {
        println!("    Using UBSCore pipeline (WAL + Balance Lock)...");

        // Create UBSCore and initialize with deposits
        let wal_config = WalConfig {
            path: wal_path.clone(),
            flush_interval_entries: 100, // Group commit every 100 orders
            sync_on_flush: false,        // Faster for benchmarks
        };

        let mut ubscore =
            UBSCore::new(symbol_mgr.clone(), wal_config).expect("Failed to create UBSCore");

        // Transfer initial balances to UBSCore via deposit()
        // Also record Deposit events for complete audit trail
        let mut deposit_count = 0u64;
        for (user_id, account) in &accounts {
            for asset_id in [base_id, quote_id] {
                if let Some(balance) = account.get_balance(asset_id) {
                    if balance.avail() > 0 {
                        ubscore
                            .deposit(*user_id, asset_id, balance.avail())
                            .unwrap();

                        // Record Deposit event
                        if let Some(b) = ubscore.get_balance(*user_id, asset_id) {
                            deposit_count += 1;
                            let deposit_event = BalanceEvent::deposit(
                                *user_id,
                                asset_id,
                                deposit_count, // ref_id = deposit sequence
                                balance.avail(),
                                b.lock_version(),
                                b.avail(),
                                b.frozen(),
                            );
                            ledger.write_balance_event(&deposit_event);
                        }
                    }
                }
            }
        }
        println!("    Recorded {} deposit events", deposit_count);

        // Run pipeline
        let result = run_pipeline_single_thread(
            &orders,
            zero_x_infinity::pipeline::PipelineServices {
                ubscore: &mut ubscore,
                book: &mut book,
                ledger: &mut ledger,
            },
            zero_x_infinity::pipeline::PipelineConfig {
                symbol_mgr: &symbol_mgr,
                active_symbol_id,
                sample_rate: app_config.sample_rate,
                continuous: false,
            },
        );

        // Get final accounts from UBSCore
        let final_accs = ubscore.accounts().clone();

        // Print stats
        println!(
            "Accepted:         {}",
            result.pipeline_stats.orders_accepted
        );
        println!(
            "Rejected:         {}",
            result.pipeline_stats.orders_rejected
        );
        println!(
            "Total Trades:     {}",
            result.pipeline_stats.trades_generated
        );
        println!(
            "Pipeline Stats: ingested={} (place={}, cancel={}), accepted={}, rejected={}, trades={}",
            result.pipeline_stats.orders_ingested,
            result.pipeline_stats.places_count,
            result.pipeline_stats.cancels_count,
            result.pipeline_stats.orders_accepted,
            result.pipeline_stats.orders_rejected,
            result.pipeline_stats.trades_generated
        );

        (
            result.accepted,
            result.rejected,
            result.total_trades,
            result.perf,
            final_accs,
            Some(book),
        )
    } else {
        println!("    Standard mode redirected to Ring Buffer Pipeline...");

        // Create UBSCore for standard mode
        let wal_config = WalConfig {
            path: wal_path.clone(),
            flush_interval_entries: 100,
            sync_on_flush: false,
        };

        let mut ubscore =
            UBSCore::new(symbol_mgr.clone(), wal_config).expect("Failed to create UBSCore");

        // Initial deposits
        for (user_id, account) in &accounts {
            for asset_id in [base_id, quote_id] {
                if let Some(balance) = account.get_balance(asset_id) {
                    if balance.avail() > 0 {
                        ubscore
                            .deposit(*user_id, asset_id, balance.avail())
                            .unwrap();
                    }
                }
            }
        }

        let result = run_pipeline_single_thread(
            &orders,
            zero_x_infinity::pipeline::PipelineServices {
                ubscore: &mut ubscore,
                book: &mut book,
                ledger: &mut ledger,
            },
            zero_x_infinity::pipeline::PipelineConfig {
                symbol_mgr: &symbol_mgr,
                active_symbol_id,
                sample_rate: app_config.sample_rate,
                continuous: false,
            },
        );

        (
            result.accepted,
            result.rejected,
            result.total_trades,
            result.perf,
            ubscore.accounts().clone(),
            Some(book),
        )
    };

    let exec_time = exec_start.elapsed();

    // Step 7: Dump final state
    println!("\n[7] Dumping final state...");
    dump_balances(&final_accounts, &symbol_mgr, active_symbol_id, &balances_t2);
    if let Some(ref book) = final_book {
        dump_orderbook_snapshot(book, &orderbook_t2);
    } else {
        println!("    (OrderBook not available in multi-thread mode)");
    }

    // Step 8: Summary
    let total_time = start_time.elapsed();
    let orders_per_sec = orders.len() as f64 / exec_time.as_secs_f64();
    let trades_per_sec = total_trades as f64 / exec_time.as_secs_f64();

    let summary = format!(
        r#"=== Execution Summary ===
Symbol: {}
Total Orders: {}
  Accepted: {}
  Rejected: {}
Total Trades: {}
Execution Time: {:.2?}
Throughput: {:.0} orders/sec | {:.0} trades/sec

Final Orderbook:
  Best Bid: {:?}
  Best Ask: {:?}
  Bid Depth: {} levels
  Ask Depth: {} levels

Total Time: {:.2?}

=== Performance Breakdown ===
{}
=== Latency Percentiles (sampled) ===
  Min:   {:>8} ns
  Avg:   {:>8} ns
  P50:   {:>8} ns
  P99:   {:>8} ns
  P99.9: {:>8} ns
  Max:   {:>8} ns
Samples: {}
"#,
        symbol_info.symbol,
        orders.len(),
        accepted,
        rejected,
        total_trades,
        exec_time,
        orders_per_sec,
        trades_per_sec,
        final_book.as_ref().map(|b| b.best_bid()).unwrap_or(None),
        final_book.as_ref().map(|b| b.best_ask()).unwrap_or(None),
        final_book.as_ref().map(|b| b.depth().0).unwrap_or(0),
        final_book.as_ref().map(|b| b.depth().1).unwrap_or(0),
        total_time,
        perf.breakdown(),
        perf.min_latency().unwrap_or(0),
        perf.avg_latency().unwrap_or(0),
        perf.percentile(50.0).unwrap_or(0),
        perf.percentile(99.0).unwrap_or(0),
        perf.percentile(99.9).unwrap_or(0),
        perf.max_latency().unwrap_or(0),
        perf.latency_samples.len(),
    );

    println!("\n{}", summary);

    // Write summary
    let mut summary_file = File::create(&summary_path).unwrap();
    summary_file.write_all(summary.as_bytes()).unwrap();
    println!("Summary written to {}", summary_path);

    // Write perf baseline
    let perf_path = format!("{}/t2_perf.txt", output_dir);
    let mut perf_file = File::create(&perf_path).unwrap();
    writeln!(perf_file, "# Performance Baseline - 0xInfinity").unwrap();
    writeln!(perf_file, "# Generated: {}", chrono_lite_now()).unwrap();
    writeln!(perf_file, "orders={}", orders.len()).unwrap();
    writeln!(perf_file, "trades={}", total_trades).unwrap();
    writeln!(
        perf_file,
        "exec_time_ms={:.2}",
        exec_time.as_secs_f64() * 1000.0
    )
    .unwrap();
    writeln!(perf_file, "throughput_ops={:.0}", orders_per_sec).unwrap();
    writeln!(perf_file, "throughput_tps={:.0}", trades_per_sec).unwrap();
    writeln!(perf_file, "pretrade_ns={}", perf.total_pretrade_ns).unwrap();
    writeln!(perf_file, "matching_ns={}", perf.total_matching_ns).unwrap();
    writeln!(perf_file, "settlement_ns={}", perf.total_settlement_ns).unwrap();
    writeln!(perf_file, "event_log_ns={}", perf.total_event_log_ns).unwrap();
    writeln!(
        perf_file,
        "latency_min_ns={}",
        perf.min_latency().unwrap_or(0)
    )
    .unwrap();
    writeln!(
        perf_file,
        "latency_avg_ns={}",
        perf.avg_latency().unwrap_or(0)
    )
    .unwrap();
    writeln!(
        perf_file,
        "latency_p50_ns={}",
        perf.percentile(50.0).unwrap_or(0)
    )
    .unwrap();
    writeln!(
        perf_file,
        "latency_p99_ns={}",
        perf.percentile(99.0).unwrap_or(0)
    )
    .unwrap();
    writeln!(
        perf_file,
        "latency_p999_ns={}",
        perf.percentile(99.9).unwrap_or(0)
    )
    .unwrap();
    writeln!(
        perf_file,
        "latency_max_ns={}",
        perf.max_latency().unwrap_or(0)
    )
    .unwrap();
    println!("Perf baseline written to {}", perf_path);

    println!("\n=== Done ===");
}
