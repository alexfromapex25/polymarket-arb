//! BTC 15-minute Polymarket arbitrage bot entry point.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::{Parser, Subcommand};
use futures::StreamExt;
use tokio::net::TcpListener;
use tracing::{error, info, warn};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use polymarket_arb::api::{create_router, AppState};
use polymarket_arb::arbitrage::{check_arbitrage, ArbitrageExecutor};
use polymarket_arb::config::Config;
use polymarket_arb::market::{discover_active_market, PolymarketClient};
use polymarket_arb::metrics;
use polymarket_arb::orderbook::types::OutcomeBook;
use polymarket_arb::orderbook::websocket::{MarketWebSocket, ReconnectConfig};
use polymarket_arb::signing::address_from_private_key;
use polymarket_arb::utils::shutdown_signal;

/// BTC 15-minute Polymarket arbitrage bot.
#[derive(Parser, Debug)]
#[command(name = "polymarket-arb")]
#[command(about = "Automated arbitrage bot for BTC 15-minute markets on Polymarket")]
#[command(version)]
struct Args {
    /// Enable verbose logging.
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Option<Command>,

    /// Run in dry-run mode (no real orders).
    #[arg(long)]
    dry_run: Option<bool>,

    /// HTTP server port for health/metrics.
    #[arg(short, long, default_value = "8080")]
    port: u16,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Run the main arbitrage bot loop (default).
    Run {
        /// Run in dry-run mode (no real orders).
        #[arg(long)]
        dry_run: Option<bool>,

        /// HTTP server port for health/metrics.
        #[arg(short, long, default_value = "8080")]
        port: u16,

        /// Use WebSocket for market data (lower latency).
        #[arg(long)]
        websocket: bool,
    },

    /// Check configuration validity.
    CheckConfig,

    /// Check wallet balance and connection.
    CheckBalance,

    /// Discover the current active BTC 15min market.
    DiscoverMarket,

    /// Test WebSocket connection (diagnostic).
    WsTest,

    /// Run latency benchmark.
    Benchmark,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse CLI arguments
    let args = Args::parse();

    // Initialize logging
    let filter = if args.verbose {
        EnvFilter::new("polymarket_arb=debug,info")
    } else {
        EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info"))
    };

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter)
        .init();

    // Initialize metrics
    metrics::init_metrics();

    // Handle subcommands
    match args.command {
        Some(Command::CheckConfig) => cmd_check_config().await,
        Some(Command::CheckBalance) => cmd_check_balance().await,
        Some(Command::DiscoverMarket) => cmd_discover_market().await,
        Some(Command::Run { dry_run, port, websocket }) => {
            if websocket {
                cmd_run_websocket(dry_run, port).await
            } else {
                cmd_run(dry_run, port).await
            }
        }
        Some(Command::WsTest) => cmd_ws_test().await,
        Some(Command::Benchmark) => cmd_benchmark().await,
        None => cmd_run(args.dry_run, args.port).await,
    }
}

/// Check configuration validity.
async fn cmd_check_config() -> anyhow::Result<()> {
    println!("======================================================================");
    println!("BTC 15M ARB BOT - CONFIGURATION CHECK");
    println!("======================================================================");

    // Load configuration
    print!("Loading configuration... ");
    let config = match Config::load() {
        Ok(c) => {
            println!("OK");
            c
        }
        Err(e) => {
            println!("FAILED");
            println!("  Error: {}", e);
            return Err(anyhow::anyhow!("Configuration load failed"));
        }
    };

    // Validate configuration
    print!("Validating configuration... ");
    match config.validate() {
        Ok(()) => println!("OK"),
        Err(e) => {
            println!("FAILED");
            println!("  Error: {}", e);
            return Err(anyhow::anyhow!("Configuration validation failed"));
        }
    }

    // Check private key
    print!("Checking private key... ");
    match address_from_private_key(&config.polymarket_private_key) {
        Ok(addr) => {
            println!("OK");
            println!("  Wallet address: {}", addr);
        }
        Err(e) => {
            println!("FAILED");
            println!("  Error: {}", e);
            return Err(anyhow::anyhow!("Private key invalid"));
        }
    }

    // Show configuration summary
    println!("----------------------------------------------------------------------");
    println!("Configuration Summary:");
    println!("  Signature Type: {} ({})", config.polymarket_signature_type,
        match config.polymarket_signature_type {
            0 => "EOA - Standard wallet",
            1 => "Magic.link - Proxy wallet",
            2 => "Gnosis Safe - Multi-sig",
            _ => "Unknown",
        });

    if config.polymarket_signature_type == 1 {
        match &config.polymarket_funder {
            Some(funder) => println!("  Funder Address: {}", funder),
            None => println!("  WARNING: Magic.link requires POLYMARKET_FUNDER to be set!"),
        }
    }

    println!("  Target Pair Cost: ${}", config.target_pair_cost);
    println!("  Order Size: {} shares", config.order_size);
    println!("  Order Type: {}", config.order_type);
    println!("  Dry Run: {}", config.dry_run);
    println!("  Cooldown: {}s", config.cooldown_seconds);
    println!("  WebSocket: {}", if config.use_wss { "Enabled" } else { "Disabled" });
    println!("======================================================================");
    println!("CONFIGURATION CHECK PASSED");
    println!("======================================================================");

    Ok(())
}

/// Check wallet balance and connection.
async fn cmd_check_balance() -> anyhow::Result<()> {
    println!("======================================================================");
    println!("BTC 15M ARB BOT - BALANCE CHECK");
    println!("======================================================================");

    // Load configuration
    let config = Config::load()?;
    config.validate().map_err(|e| anyhow::anyhow!(e))?;

    println!("Host: {}", config.polymarket_clob_url);
    println!("Signature Type: {}", config.polymarket_signature_type);
    println!("Private Key: present");
    println!("======================================================================");

    // Create client
    print!("\n1. Creating client... ");
    let client = PolymarketClient::new(&config);
    println!("OK");

    // Get wallet address
    print!("\n2. Getting wallet address... ");
    let address = client.get_address()?;
    println!("OK");
    println!("   Address: {}", address);

    // Get balance
    print!("\n3. Getting USDC balance... ");
    match client.get_balance().await {
        Ok(balance) => {
            println!("OK");
            println!("   USDC Balance: ${:.6}", balance);
        }
        Err(e) => {
            println!("FAILED");
            println!("   Error: {}", e);
        }
    }

    // Get positions
    print!("\n4. Getting positions... ");
    match client.get_positions(&[]).await {
        Ok(positions) => {
            println!("OK");
            println!("   Total positions: {}", positions.len());
            for pos in positions.iter().take(5) {
                if let Some(token_id) = &pos.token_id {
                    let short_id = if token_id.len() > 20 {
                        format!("{}...", &token_id[..20])
                    } else {
                        token_id.clone()
                    };
                    println!("   - Token: {} Size: {:?}", short_id, pos.size);
                }
            }
            if positions.len() > 5 {
                println!("   ... and {} more", positions.len() - 5);
            }
        }
        Err(e) => {
            println!("FAILED");
            println!("   Error: {}", e);
        }
    }

    println!("\n======================================================================");
    println!("BALANCE CHECK COMPLETED");
    println!("======================================================================");

    Ok(())
}

/// Discover the current active BTC 15min market.
async fn cmd_discover_market() -> anyhow::Result<()> {
    println!("======================================================================");
    println!("BTC 15M ARB BOT - MARKET DISCOVERY");
    println!("======================================================================");

    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    println!("\nSearching for active BTC 15min market...\n");

    match discover_active_market(&http_client).await {
        Ok(market) => {
            println!("MARKET FOUND");
            println!("----------------------------------------------------------------------");
            println!("  Slug: {}", market.slug);
            println!("  ID: {}", market.id);
            println!("  UP Token: {}", market.up_token_id);
            println!("  DOWN Token: {}", market.down_token_id);
            println!("  Time Remaining: {}", market.time_remaining_str());
            if let Some(q) = &market.question {
                println!("  Question: {}", q);
            }
            println!("======================================================================");
        }
        Err(e) => {
            println!("NO ACTIVE MARKET FOUND");
            println!("  Error: {}", e);
            println!("\nMarkets open every 15 minutes. Try again shortly.");
            println!("======================================================================");
        }
    }

    Ok(())
}

/// Run the main arbitrage bot loop.
async fn cmd_run(dry_run_override: Option<bool>, port: u16) -> anyhow::Result<()> {
    // Load configuration
    info!("Loading configuration...");
    let mut config = Config::load().map_err(|e| {
        error!("Failed to load configuration: {}", e);
        e
    })?;

    // Override with CLI args if provided
    if let Some(dry_run) = dry_run_override {
        config.dry_run = dry_run;
    }

    // Validate configuration
    if let Err(e) = config.validate() {
        error!("Invalid configuration: {}", e);
        return Err(anyhow::anyhow!("Configuration validation failed: {}", e));
    }

    info!("Configuration loaded successfully");
    info!("Mode: {}", if config.dry_run { "SIMULATION" } else { "LIVE TRADING" });
    info!("Target pair cost: ${}", config.target_pair_cost);
    info!("Order size: {} shares", config.order_size);
    info!("Order type: {}", config.order_type);

    // Create app state
    let app_state = AppState::new();

    // Start HTTP server
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;
    info!("HTTP server listening on {}", addr);

    let router = create_router(app_state.clone());

    // Spawn HTTP server
    let _server_handle = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown_signal())
            .await
    });

    // Create Polymarket client
    let client = PolymarketClient::new(&config);
    let http_client = client.http().clone();

    // Create executor
    let mut executor = ArbitrageExecutor::new(&config);

    // Main bot loop
    info!("Starting arbitrage bot...");

    loop {
        // Discover active market
        info!("Searching for active BTC 15min market...");

        let market = match discover_active_market(&http_client).await {
            Ok(m) => {
                info!("Found market: {}", m.slug);
                info!("Time remaining: {}", m.time_remaining_str());

                // Update app state
                *app_state.market_slug.write().await = Some(m.slug.clone());
                app_state.set_ready(true);

                m
            }
            Err(e) => {
                warn!("No active market found: {}. Retrying in 30s...", e);
                app_state.set_ready(false);
                tokio::time::sleep(Duration::from_secs(30)).await;
                continue;
            }
        };

        info!("========================================");
        info!("BTC 15MIN ARBITRAGE BOT STARTED");
        info!("========================================");
        info!("Market: {}", market.slug);
        info!("UP Token: {}", market.up_token_id);
        info!("DOWN Token: {}", market.down_token_id);
        info!("Mode: {}", if config.dry_run { "SIMULATION" } else { "LIVE TRADING" });
        info!("========================================");

        // Market monitoring loop
        let mut scan_count = 0u64;

        while !market.is_closed() {
            scan_count += 1;

            // Fetch order books concurrently
            let (up_result, down_result) = tokio::join!(
                client.get_order_book(&market.up_token_id),
                client.get_order_book(&market.down_token_id),
            );

            let up_book = match up_result {
                Ok(book) => book,
                Err(e) => {
                    warn!("Failed to fetch UP order book: {}", e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }
            };

            let down_book = match down_result {
                Ok(book) => book,
                Err(e) => {
                    warn!("Failed to fetch DOWN order book: {}", e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }
            };

            // Check for arbitrage opportunity
            match check_arbitrage(&market, &up_book, &down_book, &config) {
                Ok(Some(opportunity)) => {
                    // Execute arbitrage
                    match executor.execute(&client, &opportunity, &config).await {
                        Ok(result) => {
                            info!("Execution result: {:?}", result);

                            // Update stats in app state
                            let stats = executor.stats();
                            *app_state.stats.write().await = stats;
                        }
                        Err(e) => {
                            error!("Execution failed: {}", e);
                        }
                    }
                }
                Ok(None) => {
                    // No opportunity
                    let diagnosis = polymarket_arb::arbitrage::diagnose_no_opportunity(
                        &up_book,
                        &down_book,
                        config.order_size,
                        config.target_pair_cost,
                    );
                    info!(
                        "[Scan #{}] No arbitrage: {} [Time: {}]",
                        scan_count,
                        diagnosis,
                        market.time_remaining_str()
                    );
                }
                Err(e) => {
                    warn!("Arbitrage check error: {}", e);
                }
            }

            // Brief pause between scans (configurable, 0 for continuous)
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Market closed
        info!("========================================");
        info!("MARKET CLOSED - FINAL SUMMARY");
        info!("========================================");
        info!("Market: {}", market.slug);
        info!("Mode: {}", if config.dry_run { "SIMULATION" } else { "LIVE TRADING" });
        info!("----------------------------------------");

        let stats = executor.stats();
        info!("Total opportunities detected: {}", stats.opportunities_found);
        info!("Total trades executed: {}", stats.trades_executed);
        info!("Total shares bought: {}", stats.total_shares_bought);
        info!("----------------------------------------");
        info!("Total invested: ${}", stats.total_invested);
        info!("Expected payout: ${}", stats.total_shares_bought / rust_decimal::Decimal::TWO);
        info!("Expected profit: ${}", stats.expected_profit());

        if config.dry_run {
            info!("----------------------------------------");
            info!("Sim start cash: ${}", stats.sim_start_balance);
            info!("Sim cash remaining: ${}", stats.sim_balance);
            info!("Sim ending balance: ${}", stats.sim_ending_balance());
        }

        info!("========================================");

        // Brief pause before searching for next market
        info!("Searching for next market in 10s...");
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

/// Run the bot with WebSocket-driven execution (lower latency).
async fn cmd_run_websocket(dry_run_override: Option<bool>, port: u16) -> anyhow::Result<()> {
    // Load configuration
    info!("Loading configuration...");
    let mut config = Config::load().map_err(|e| {
        error!("Failed to load configuration: {}", e);
        e
    })?;

    // Override with CLI args if provided
    if let Some(dry_run) = dry_run_override {
        config.dry_run = dry_run;
    }

    // Validate configuration
    if let Err(e) = config.validate() {
        error!("Invalid configuration: {}", e);
        return Err(anyhow::anyhow!("Configuration validation failed: {}", e));
    }

    info!("Configuration loaded successfully");
    info!("Mode: {} (WebSocket-driven)", if config.dry_run { "SIMULATION" } else { "LIVE TRADING" });
    info!("Target pair cost: ${}", config.target_pair_cost);
    info!("Order size: {} shares", config.order_size);

    // Create app state
    let app_state = AppState::new();

    // Start HTTP server
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;
    info!("HTTP server listening on {}", addr);

    let router = create_router(app_state.clone());

    // Spawn HTTP server
    let _server_handle = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown_signal())
            .await
    });

    // Create Polymarket client
    let client = PolymarketClient::new(&config);
    let http_client = client.http().clone();

    // Create executor
    let mut executor = ArbitrageExecutor::new(&config);

    // Main bot loop
    info!("Starting WebSocket-driven arbitrage bot...");

    loop {
        // Discover active market
        info!("Searching for active BTC 15min market...");

        let market = match discover_active_market(&http_client).await {
            Ok(m) => {
                info!("Found market: {}", m.slug);
                info!("Time remaining: {}", m.time_remaining_str());

                // Update app state
                *app_state.market_slug.write().await = Some(m.slug.clone());
                app_state.set_ready(true);

                m
            }
            Err(e) => {
                warn!("No active market found: {}. Retrying in 30s...", e);
                app_state.set_ready(false);
                tokio::time::sleep(Duration::from_secs(30)).await;
                continue;
            }
        };

        info!("========================================");
        info!("WEBSOCKET-DRIVEN ARBITRAGE BOT STARTED");
        info!("========================================");
        info!("Market: {}", market.slug);
        info!("UP Token: {}", market.up_token_id);
        info!("DOWN Token: {}", market.down_token_id);
        info!("Mode: {}", if config.dry_run { "SIMULATION" } else { "LIVE TRADING" });
        info!("========================================");

        // Create WebSocket client with reconnection config
        let reconnect_config = ReconnectConfig::from_config(
            config.ws_reconnect_max_delay_s,
            config.ws_heartbeat_interval_s,
        );
        let ws = Arc::new(MarketWebSocket::with_reconnect_config(
            config.polymarket_ws_url.clone(),
            reconnect_config,
        ));

        // Start WebSocket with auto-reconnect
        let asset_ids = vec![
            market.up_token_id.clone(),
            market.down_token_id.clone(),
        ];

        let mut ws_receiver = ws.clone().run_with_reconnect(asset_ids).await;

        info!("WebSocket connected, waiting for book updates...");

        // Process WebSocket updates until market closes
        while !market.is_closed() {
            tokio::select! {
                Some(_update) = ws_receiver.recv() => {
                    let detection_start = Instant::now();

                    // Get both books from WebSocket state
                    let up_book = ws.get_book(&market.up_token_id);
                    let down_book = ws.get_book(&market.down_token_id);

                    if let (Some(up_state), Some(down_state)) = (up_book, down_book) {
                        // Convert WebSocket state to OutcomeBook
                        let (up_bids, up_asks) = up_state.to_levels();
                        let (down_bids, down_asks) = down_state.to_levels();

                        let up_outcome_book = OutcomeBook {
                            token_id: market.up_token_id.clone(),
                            outcome: polymarket_arb::market::Outcome::Up,
                            bids: up_bids,
                            asks: up_asks,
                            updated_at: time::OffsetDateTime::now_utc(),
                        };

                        let down_outcome_book = OutcomeBook {
                            token_id: market.down_token_id.clone(),
                            outcome: polymarket_arb::market::Outcome::Down,
                            bids: down_bids,
                            asks: down_asks,
                            updated_at: time::OffsetDateTime::now_utc(),
                        };

                        // Check for arbitrage opportunity
                        match check_arbitrage(&market, &up_outcome_book, &down_outcome_book, &config) {
                            Ok(Some(opportunity)) => {
                                metrics::record_opportunity_detection_latency(detection_start);
                                metrics::inc_opportunities_detected();

                                // Execute arbitrage immediately
                                match executor.execute(&client, &opportunity, &config).await {
                                    Ok(result) => {
                                        info!("Execution result: {:?}", result);
                                        metrics::inc_opportunities_executed();

                                        // Update stats in app state
                                        let stats = executor.stats();
                                        *app_state.stats.write().await = stats;
                                    }
                                    Err(e) => {
                                        error!("Execution failed: {}", e);
                                    }
                                }
                            }
                            Ok(None) => {
                                // No opportunity - just continue listening
                            }
                            Err(e) => {
                                warn!("Arbitrage check error: {}", e);
                            }
                        }
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(1)) => {
                    // Periodic check if market is still open
                    if market.is_closed() {
                        break;
                    }
                }
            }
        }

        // Market closed - print summary
        info!("========================================");
        info!("MARKET CLOSED - FINAL SUMMARY");
        info!("========================================");
        info!("Market: {}", market.slug);

        let stats = executor.stats();
        info!("Total opportunities detected: {}", stats.opportunities_found);
        info!("Total trades executed: {}", stats.trades_executed);
        info!("Total shares bought: {}", stats.total_shares_bought);
        info!("Total invested: ${}", stats.total_invested);
        info!("Expected profit: ${}", stats.expected_profit());

        if config.dry_run {
            info!("Sim ending balance: ${}", stats.sim_ending_balance());
        }

        info!("========================================");
        info!("Searching for next market in 10s...");
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

/// Test WebSocket connection.
async fn cmd_ws_test() -> anyhow::Result<()> {
    println!("======================================================================");
    println!("BTC 15M ARB BOT - WEBSOCKET TEST");
    println!("======================================================================");

    let config = Config::load()?;
    config.validate().map_err(|e| anyhow::anyhow!(e))?;

    // First discover a market to get token IDs
    let http_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    println!("\n1. Discovering active market...");
    let market = discover_active_market(&http_client).await?;
    println!("   Found: {}", market.slug);
    println!("   UP Token: {}", market.up_token_id);
    println!("   DOWN Token: {}", market.down_token_id);

    println!("\n2. Connecting to WebSocket...");
    let ws = MarketWebSocket::new(config.polymarket_ws_url.clone());

    let asset_ids = vec![
        market.up_token_id.clone(),
        market.down_token_id.clone(),
    ];

    let stream = ws.run(asset_ids).await?;
    let mut stream = Box::pin(stream);
    println!("   Connected!");

    println!("\n3. Waiting for book updates (10 seconds)...");
    let start = Instant::now();
    let mut message_count = 0u32;

    while start.elapsed() < Duration::from_secs(10) {
        tokio::select! {
            Some(update) = stream.next() => {
                message_count += 1;
                println!("   [{:.1}s] Received: {:?}", start.elapsed().as_secs_f64(), update);

                // Show book state after a few messages
                if message_count == 3 {
                    if let Some(book) = ws.get_book(&market.up_token_id) {
                        let (bids, asks) = book.to_levels();
                        println!("   UP Book - Bids: {}, Asks: {}", bids.len(), asks.len());
                        if let Some(best_bid) = bids.first() {
                            println!("   Best Bid: ${} x {}", best_bid.price, best_bid.size);
                        }
                        if let Some(best_ask) = asks.first() {
                            println!("   Best Ask: ${} x {}", best_ask.price, best_ask.size);
                        }
                    }
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {}
        }
    }

    println!("\n======================================================================");
    println!("WEBSOCKET TEST COMPLETE");
    println!("  Messages received: {}", message_count);
    println!("  Connection status: {}", if ws.is_connected() { "Connected" } else { "Disconnected" });
    println!("======================================================================");

    Ok(())
}

/// Run latency benchmark.
async fn cmd_benchmark() -> anyhow::Result<()> {
    println!("======================================================================");
    println!("BTC 15M ARB BOT - LATENCY BENCHMARK");
    println!("======================================================================");

    let config = Config::load()?;
    config.validate().map_err(|e| anyhow::anyhow!(e))?;

    let client = PolymarketClient::new(&config);

    // Discover market
    println!("\n1. Discovering market...");
    let http_client = client.http().clone();
    let market = discover_active_market(&http_client).await?;
    println!("   Found: {}", market.slug);

    // Benchmark order book fetches
    println!("\n2. Benchmarking order book fetch latency (10 iterations)...");
    let mut latencies = Vec::with_capacity(10);

    for i in 0..10 {
        let start = Instant::now();
        let _ = client.get_order_book(&market.up_token_id).await;
        let latency = start.elapsed();
        latencies.push(latency.as_millis() as f64);
        println!("   Iteration {}: {:.1}ms", i + 1, latency.as_millis());
    }

    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let avg = latencies.iter().sum::<f64>() / latencies.len() as f64;
    let p50 = latencies[latencies.len() / 2];
    let p95 = latencies[(latencies.len() as f64 * 0.95) as usize];

    println!("\n   Results:");
    println!("   - Average: {:.1}ms", avg);
    println!("   - P50: {:.1}ms", p50);
    println!("   - P95: {:.1}ms", p95);
    println!("   - Min: {:.1}ms", latencies.first().unwrap());
    println!("   - Max: {:.1}ms", latencies.last().unwrap());

    // Benchmark signing
    println!("\n3. Benchmarking signing latency (10 iterations)...");
    let mut sign_latencies = Vec::with_capacity(10);

    for i in 0..10 {
        let start = Instant::now();
        let _ = polymarket_arb::signing::generate_auth_headers(
            client.private_key(),
            client.signature_type(),
        ).await;
        let latency = start.elapsed();
        sign_latencies.push(latency.as_millis() as f64);
        println!("   Iteration {}: {:.1}ms", i + 1, latency.as_millis());
    }

    let avg_sign = sign_latencies.iter().sum::<f64>() / sign_latencies.len() as f64;
    println!("\n   Average signing latency: {:.1}ms", avg_sign);

    println!("\n======================================================================");
    println!("BENCHMARK COMPLETE");
    println!("======================================================================");

    Ok(())
}
