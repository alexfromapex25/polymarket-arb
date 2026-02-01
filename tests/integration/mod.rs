//! Integration tests for the Polymarket arbitrage bot.
//!
//! These tests require a valid POLYMARKET_PRIVATE_KEY environment variable.
//! Run with: cargo test --test integration -- --ignored
//!
//! Note: These tests interact with the real Polymarket API.

use polymarket_arb::config::Config;
use polymarket_arb::market::PolymarketClient;
use rust_decimal::Decimal;

/// Get a test config from environment.
fn test_config() -> Option<Config> {
    // Try to load from environment
    dotenvy::dotenv().ok();

    let private_key = std::env::var("POLYMARKET_PRIVATE_KEY").ok()?;

    // Skip if using placeholder key
    if private_key.starts_with("0x1234") || private_key.len() < 64 {
        return None;
    }

    Some(Config {
        polymarket_private_key: private_key,
        polymarket_api_key: std::env::var("POLYMARKET_API_KEY").ok(),
        polymarket_api_secret: std::env::var("POLYMARKET_API_SECRET").ok(),
        polymarket_api_passphrase: std::env::var("POLYMARKET_API_PASSPHRASE").ok(),
        polymarket_signature_type: std::env::var("POLYMARKET_SIGNATURE_TYPE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0),
        polymarket_funder: std::env::var("POLYMARKET_FUNDER").ok(),
        target_pair_cost: Decimal::new(991, 3),
        order_size: Decimal::new(5, 0),
        order_type: "FOK".to_string(),
        balance_margin: Decimal::new(12, 1),
        dry_run: true,
        sim_balance: Decimal::new(100, 0),
        cooldown_seconds: 10,
        polymarket_market_slug: None,
        use_wss: false,
        polymarket_ws_url: "wss://ws-subscriptions-clob.polymarket.com".to_string(),
        polymarket_clob_url: "https://clob.polymarket.com".to_string(),
        port: 8080,
        rust_log: "info".to_string(),
        verbose: false,
    })
}

/// Test that we can derive the wallet address.
#[tokio::test]
#[ignore = "requires POLYMARKET_PRIVATE_KEY"]
async fn test_get_address() {
    let config = match test_config() {
        Some(c) => c,
        None => {
            println!("Skipping: POLYMARKET_PRIVATE_KEY not set or invalid");
            return;
        }
    };

    let client = PolymarketClient::new(&config);

    let result = client.get_address();
    assert!(result.is_ok(), "Failed to get address: {:?}", result.err());

    let address = result.unwrap();
    assert!(address.starts_with("0x"), "Invalid address format");
    assert_eq!(address.len(), 42, "Address should be 42 characters");

    println!("Wallet address: {}", address);
}

/// Test that we can query the balance.
#[tokio::test]
#[ignore = "requires POLYMARKET_PRIVATE_KEY"]
async fn test_get_balance() {
    let config = match test_config() {
        Some(c) => c,
        None => {
            println!("Skipping: POLYMARKET_PRIVATE_KEY not set or invalid");
            return;
        }
    };

    let client = PolymarketClient::new(&config);

    let result = client.get_balance().await;
    assert!(result.is_ok(), "Failed to get balance: {:?}", result.err());

    let balance = result.unwrap();
    assert!(balance >= Decimal::ZERO, "Balance should be non-negative");

    println!("USDC Balance: ${}", balance);
}

/// Test that we can query positions.
#[tokio::test]
#[ignore = "requires POLYMARKET_PRIVATE_KEY"]
async fn test_get_positions() {
    let config = match test_config() {
        Some(c) => c,
        None => {
            println!("Skipping: POLYMARKET_PRIVATE_KEY not set or invalid");
            return;
        }
    };

    let client = PolymarketClient::new(&config);

    let result = client.get_positions(&[]).await;
    assert!(
        result.is_ok(),
        "Failed to get positions: {:?}",
        result.err()
    );

    let positions = result.unwrap();
    println!("Found {} positions", positions.len());

    for pos in &positions {
        if let Some(token_id) = &pos.token_id {
            println!(
                "  Token: {} Size: {:?}",
                &token_id[..20.min(token_id.len())],
                pos.size
            );
        }
    }
}

/// Test that we can fetch an order book.
#[tokio::test]
#[ignore = "requires network access"]
async fn test_get_order_book() {
    let config = test_config().unwrap_or_else(|| Config {
        polymarket_private_key: "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        polymarket_api_key: None,
        polymarket_api_secret: None,
        polymarket_api_passphrase: None,
        polymarket_signature_type: 0,
        polymarket_funder: None,
        target_pair_cost: Decimal::new(991, 3),
        order_size: Decimal::new(5, 0),
        order_type: "FOK".to_string(),
        balance_margin: Decimal::new(12, 1),
        dry_run: true,
        sim_balance: Decimal::new(100, 0),
        cooldown_seconds: 10,
        polymarket_market_slug: None,
        use_wss: false,
        polymarket_ws_url: "wss://ws-subscriptions-clob.polymarket.com".to_string(),
        polymarket_clob_url: "https://clob.polymarket.com".to_string(),
        port: 8080,
        rust_log: "info".to_string(),
        verbose: false,
    });

    let client = PolymarketClient::new(&config);

    // Use a known active token ID (you may need to update this)
    // This is just testing the API call works, not a specific market
    let result = client.get_order_book("21742633143463906290569050155826241533067272736897614950488156847949938836455").await;

    // The call may fail if the token doesn't exist, but it should at least
    // complete without a connection error
    match result {
        Ok(book) => {
            println!("Order book for token:");
            println!("  Bids: {} levels", book.bids.len());
            println!("  Asks: {} levels", book.asks.len());
            if let Some(best_bid) = book.best_bid() {
                println!("  Best bid: ${}", best_bid);
            }
            if let Some(best_ask) = book.best_ask() {
                println!("  Best ask: ${}", best_ask);
            }
        }
        Err(e) => {
            // Token might not exist, but connection should work
            println!("Order book fetch returned error (expected for unknown token): {}", e);
        }
    }
}

/// Test market discovery.
#[tokio::test]
#[ignore = "requires network access"]
async fn test_market_discovery() {
    use polymarket_arb::market::discover_active_market;

    let config = test_config().unwrap_or_else(|| Config {
        polymarket_private_key: "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        polymarket_api_key: None,
        polymarket_api_secret: None,
        polymarket_api_passphrase: None,
        polymarket_signature_type: 0,
        polymarket_funder: None,
        target_pair_cost: Decimal::new(991, 3),
        order_size: Decimal::new(5, 0),
        order_type: "FOK".to_string(),
        balance_margin: Decimal::new(12, 1),
        dry_run: true,
        sim_balance: Decimal::new(100, 0),
        cooldown_seconds: 10,
        polymarket_market_slug: None,
        use_wss: false,
        polymarket_ws_url: "wss://ws-subscriptions-clob.polymarket.com".to_string(),
        polymarket_clob_url: "https://clob.polymarket.com".to_string(),
        port: 8080,
        rust_log: "info".to_string(),
        verbose: false,
    });

    let client = PolymarketClient::new(&config);

    let result = discover_active_market(&client).await;

    match result {
        Ok(market) => {
            println!("Found active market:");
            println!("  Slug: {}", market.slug);
            println!("  ID: {}", market.id);
            println!("  UP token: {}", &market.up_token_id[..20.min(market.up_token_id.len())]);
            println!("  DOWN token: {}", &market.down_token_id[..20.min(market.down_token_id.len())]);
        }
        Err(e) => {
            println!("No active market found: {}", e);
        }
    }
}

/// Test signing module functions.
#[test]
fn test_signing_utilities() {
    use polymarket_arb::signing::{address_from_private_key, create_signer, signature_type_from_u8};
    use polymarket_client_sdk::clob::types::SignatureType;

    // Test signature type conversion
    assert!(matches!(signature_type_from_u8(0), SignatureType::Eoa));
    assert!(matches!(signature_type_from_u8(1), SignatureType::Proxy));
    assert!(matches!(signature_type_from_u8(2), SignatureType::GnosisSafe));

    // Test signer creation
    let key = "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let signer = create_signer(key);
    assert!(signer.is_ok());

    // Test address derivation
    let address = address_from_private_key(key);
    assert!(address.is_ok());
    let addr = address.unwrap();
    assert!(addr.starts_with("0x"));
    assert_eq!(addr.len(), 42);
}
