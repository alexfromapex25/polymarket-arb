//! Polymarket API client wrapper.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument, warn};

use crate::config::Config;
use crate::error::{MarketError, TradingError};
use crate::orderbook::types::OutcomeBook;
use crate::signing;

use super::types::Outcome;

/// Polymarket CLOB API client.
#[derive(Debug, Clone)]
pub struct PolymarketClient {
    /// HTTP client for API requests.
    http: reqwest::Client,
    /// Base URL for CLOB API.
    clob_url: String,
    /// Wallet private key.
    private_key: String,
    /// Signature type (0=EOA, 1=Magic.link, 2=Gnosis).
    signature_type: u8,
    /// Funder address (for Magic.link).
    funder: Option<String>,
    /// Chain ID (137 for Polygon).
    chain_id: u64,
}

/// Order book response from API.
#[derive(Debug, Clone, Deserialize)]
pub struct OrderBookResponse {
    /// Bid levels.
    pub bids: Option<Vec<OrderLevel>>,
    /// Ask levels.
    pub asks: Option<Vec<OrderLevel>>,
    /// Market ID.
    pub market: Option<String>,
    /// Asset ID.
    pub asset_id: Option<String>,
}

/// Single price level in order book.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OrderLevel {
    /// Price at this level.
    pub price: String,
    /// Size available at this level.
    pub size: String,
}

/// Balance allowance response from API.
#[derive(Debug, Clone, Deserialize)]
pub struct BalanceAllowanceResponse {
    /// Balance in wei.
    pub balance: Option<String>,
    /// Allowance in wei.
    pub allowance: Option<String>,
}

/// Position response from API.
#[derive(Debug, Clone, Deserialize)]
pub struct PositionResponse {
    /// Token ID.
    pub token_id: Option<String>,
    /// Asset info.
    pub asset: Option<AssetInfo>,
    /// Position size.
    pub size: Option<String>,
    /// Average entry price.
    pub avg_price: Option<String>,
}

/// Asset info in position.
#[derive(Debug, Clone, Deserialize)]
pub struct AssetInfo {
    /// Token ID.
    pub token_id: Option<String>,
}

impl PolymarketClient {
    /// Create a new Polymarket client from config with optimized HTTP settings.
    pub fn new(config: &Config) -> Self {
        let http = reqwest::Client::builder()
            // Configurable timeout (default 2s, down from 30s)
            .timeout(std::time::Duration::from_millis(config.http_timeout_ms))
            // Fast connection establishment
            .connect_timeout(std::time::Duration::from_millis(500))
            // TCP_NODELAY for low-latency (disable Nagle's algorithm)
            .tcp_nodelay(true)
            // Keep connections alive for reuse
            .tcp_keepalive(std::time::Duration::from_secs(30))
            // Connection pool per host (default 10)
            .pool_max_idle_per_host(config.http_pool_size)
            // Keep idle connections for 90 seconds
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .build()
            .expect("failed to create HTTP client");

        Self {
            http,
            clob_url: config.polymarket_clob_url.clone(),
            private_key: config.polymarket_private_key.clone(),
            signature_type: config.polymarket_signature_type,
            funder: config.polymarket_funder.clone(),
            chain_id: 137, // Polygon mainnet
        }
    }

    /// Get the HTTP client reference.
    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    /// Get the private key (for direct signing operations).
    pub fn private_key(&self) -> &str {
        &self.private_key
    }

    /// Get the signature type.
    pub fn signature_type(&self) -> u8 {
        self.signature_type
    }

    /// Get funder address.
    pub fn funder(&self) -> Option<&str> {
        self.funder.as_deref()
    }

    /// Get order book for a token.
    #[instrument(skip(self), fields(token_id = %token_id))]
    pub async fn get_order_book(&self, token_id: &str) -> Result<OutcomeBook, MarketError> {
        let url = format!("{}/book", self.clob_url);

        let response = self
            .http
            .get(&url)
            .query(&[("token_id", token_id)])
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(MarketError::FetchFailed {
                slug: token_id.to_string(),
                reason: format!("HTTP {}", response.status()),
            });
        }

        let book: OrderBookResponse = response.json().await.map_err(|e| {
            MarketError::ParseError(format!("Failed to parse order book: {}", e))
        })?;

        Ok(self.convert_order_book(token_id, book))
    }

    /// Convert API response to OutcomeBook.
    fn convert_order_book(&self, token_id: &str, response: OrderBookResponse) -> OutcomeBook {
        use crate::orderbook::types::PriceLevel;
        use time::OffsetDateTime;

        let parse_levels = |levels: Option<Vec<OrderLevel>>| -> Vec<PriceLevel> {
            levels
                .unwrap_or_default()
                .into_iter()
                .filter_map(|level| {
                    let price: Decimal = level.price.parse().ok()?;
                    let size: Decimal = level.size.parse().ok()?;
                    if size > Decimal::ZERO {
                        Some(PriceLevel { price, size })
                    } else {
                        None
                    }
                })
                .collect()
        };

        let mut bids = parse_levels(response.bids);
        let mut asks = parse_levels(response.asks);

        // Sort bids descending by price
        bids.sort_by(|a, b| b.price.cmp(&a.price));
        // Sort asks ascending by price
        asks.sort_by(|a, b| a.price.cmp(&b.price));

        OutcomeBook {
            token_id: token_id.to_string(),
            outcome: Outcome::Up, // Will be set by caller
            bids,
            asks,
            updated_at: OffsetDateTime::now_utc(),
        }
    }

    /// Get USDC balance using authenticated API call.
    #[instrument(skip(self))]
    pub async fn get_balance(&self) -> Result<Decimal, TradingError> {
        debug!("Getting balance from Polymarket API");

        let url = format!("{}/balance-allowance", self.clob_url);
        let auth_headers = signing::generate_auth_headers(&self.private_key, self.signature_type).await?;

        let mut request = self.http.get(&url);
        for (key, value) in auth_headers {
            request = request.header(&key, &value);
        }

        let response = request.send().await.map_err(|e| {
            TradingError::SubmissionFailed(format!("Failed to get balance: {}", e))
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(TradingError::SubmissionFailed(format!(
                "Balance request failed: HTTP {} - {}",
                status, body
            )));
        }

        let balance_response: BalanceAllowanceResponse = response.json().await.map_err(|e| {
            TradingError::SubmissionFailed(format!("Failed to parse balance: {}", e))
        })?;

        // Parse balance from response (in wei, 6 decimals for USDC)
        let balance_wei: Decimal = balance_response
            .balance
            .as_deref()
            .unwrap_or("0")
            .parse()
            .unwrap_or(Decimal::ZERO);

        // Convert from wei (6 decimals) to USDC
        let balance = balance_wei / Decimal::new(1_000_000, 0);

        debug!(balance = %balance, "Retrieved USDC balance");

        Ok(balance)
    }

    /// Get positions for specified token IDs.
    #[instrument(skip(self))]
    pub async fn get_positions(
        &self,
        token_ids: &[String],
    ) -> Result<Vec<PositionResponse>, TradingError> {
        debug!("Getting positions from Polymarket API");

        // Use the REST API for positions
        let url = format!("{}/positions", self.clob_url);

        // Get the wallet address for the query
        let address = self.get_address()?;

        let response = self
            .http
            .get(&url)
            .query(&[("address", &address)])
            .send()
            .await
            .map_err(|e| TradingError::SubmissionFailed(format!("Failed to get positions: {}", e)))?;

        if !response.status().is_success() {
            warn!(status = %response.status(), "Failed to get positions");
            return Ok(Vec::new());
        }

        let positions: Vec<PositionResponse> = response
            .json()
            .await
            .map_err(|e| TradingError::SubmissionFailed(format!("Failed to parse positions: {}", e)))?;

        // Filter to only the requested token IDs if specified
        if token_ids.is_empty() {
            return Ok(positions);
        }

        let filtered: Vec<_> = positions
            .into_iter()
            .filter(|p| {
                p.token_id
                    .as_ref()
                    .map(|id| token_ids.contains(id))
                    .unwrap_or(false)
                    || p.asset
                        .as_ref()
                        .and_then(|a| a.token_id.as_ref())
                        .map(|id| token_ids.contains(id))
                        .unwrap_or(false)
            })
            .collect();

        debug!(count = filtered.len(), "Retrieved filtered positions");

        Ok(filtered)
    }

    /// Get the wallet address derived from the private key.
    pub fn get_address(&self) -> Result<String, TradingError> {
        signing::address_from_private_key(&self.private_key)
    }

    /// Get the CLOB base URL.
    pub fn clob_url(&self) -> &str {
        &self.clob_url
    }

    /// Get the chain ID.
    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Config {
        Config {
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
            polymarket_ws_url: "wss://test".to_string(),
            polymarket_clob_url: "https://clob.polymarket.com".to_string(),
            port: 8080,
            rust_log: "info".to_string(),
            verbose: false,
            http_timeout_ms: 2000,
            http_pool_size: 10,
            order_timeout_ms: 500,
            order_poll_interval_ms: 50,
            ws_reconnect_max_delay_s: 30,
            ws_heartbeat_interval_s: 30,
            metrics_enabled: true,
            metrics_port: 9090,
        }
    }

    #[test]
    fn client_creation_works() {
        let config = test_config();
        let client = PolymarketClient::new(&config);
        assert_eq!(client.chain_id(), 137);
        assert_eq!(client.clob_url(), "https://clob.polymarket.com");
    }

    #[test]
    fn get_address_works() {
        let config = test_config();
        let client = PolymarketClient::new(&config);
        let address = client.get_address();
        assert!(address.is_ok());
        let addr = address.unwrap();
        assert!(addr.starts_with("0x"));
        assert_eq!(addr.len(), 42); // 0x + 40 hex chars
    }
}
