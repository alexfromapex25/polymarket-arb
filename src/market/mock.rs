//! Mock Polymarket client for unit testing.
//!
//! This module provides a mock client that can be used in tests
//! without making real network requests.

use rust_decimal::Decimal;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::error::{MarketError, TradingError};
use crate::market::client::PositionResponse;
use crate::orderbook::types::{OutcomeBook, PriceLevel};

use super::types::Outcome;

/// Mock order book data for testing.
#[derive(Debug, Clone)]
pub struct MockOrderBook {
    /// Token ID.
    pub token_id: String,
    /// Bid levels.
    pub bids: Vec<PriceLevel>,
    /// Ask levels.
    pub asks: Vec<PriceLevel>,
}

/// Mock position data.
#[derive(Debug, Clone)]
pub struct MockPosition {
    /// Token ID.
    pub token_id: String,
    /// Position size.
    pub size: Decimal,
    /// Average price.
    pub avg_price: Decimal,
}

/// Configuration for mock client behavior.
#[derive(Debug, Clone, Default)]
pub struct MockConfig {
    /// Balance to return.
    pub balance: Decimal,
    /// Whether to fail balance requests.
    pub fail_balance: bool,
    /// Whether to fail order book requests.
    pub fail_order_book: bool,
    /// Whether to fail position requests.
    pub fail_positions: bool,
    /// Simulated latency in milliseconds.
    pub latency_ms: u64,
}

/// Mock Polymarket client for testing.
#[derive(Debug, Clone)]
pub struct MockPolymarketClient {
    /// Mock configuration.
    config: MockConfig,
    /// Mock order books by token ID.
    order_books: Arc<Mutex<HashMap<String, MockOrderBook>>>,
    /// Mock positions.
    positions: Arc<Mutex<Vec<MockPosition>>>,
    /// Wallet address.
    address: String,
}

impl MockPolymarketClient {
    /// Create a new mock client with default configuration.
    pub fn new() -> Self {
        Self {
            config: MockConfig::default(),
            order_books: Arc::new(Mutex::new(HashMap::new())),
            positions: Arc::new(Mutex::new(Vec::new())),
            address: "0xMOCK000000000000000000000000000000000001".to_string(),
        }
    }

    /// Create a mock client with custom configuration.
    pub fn with_config(config: MockConfig) -> Self {
        Self {
            config,
            order_books: Arc::new(Mutex::new(HashMap::new())),
            positions: Arc::new(Mutex::new(Vec::new())),
            address: "0xMOCK000000000000000000000000000000000001".to_string(),
        }
    }

    /// Set the mock balance.
    pub fn set_balance(&mut self, balance: Decimal) {
        self.config.balance = balance;
    }

    /// Set a mock order book for a token.
    pub fn set_order_book(&self, book: MockOrderBook) {
        let mut books = self.order_books.lock().unwrap();
        books.insert(book.token_id.clone(), book);
    }

    /// Add a mock position.
    pub fn add_position(&self, position: MockPosition) {
        let mut positions = self.positions.lock().unwrap();
        positions.push(position);
    }

    /// Clear all mock data.
    pub fn clear(&self) {
        self.order_books.lock().unwrap().clear();
        self.positions.lock().unwrap().clear();
    }

    /// Get the mock wallet address.
    pub fn get_address(&self) -> Result<String, TradingError> {
        Ok(self.address.clone())
    }

    /// Get the mock balance.
    pub async fn get_balance(&self) -> Result<Decimal, TradingError> {
        if self.config.latency_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(self.config.latency_ms)).await;
        }

        if self.config.fail_balance {
            return Err(TradingError::SubmissionFailed(
                "Mock balance failure".to_string(),
            ));
        }

        Ok(self.config.balance)
    }

    /// Get a mock order book.
    pub async fn get_order_book(&self, token_id: &str) -> Result<OutcomeBook, MarketError> {
        if self.config.latency_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(self.config.latency_ms)).await;
        }

        if self.config.fail_order_book {
            return Err(MarketError::FetchFailed {
                slug: token_id.to_string(),
                reason: "Mock order book failure".to_string(),
            });
        }

        let books = self.order_books.lock().unwrap();
        if let Some(mock_book) = books.get(token_id) {
            Ok(OutcomeBook {
                token_id: mock_book.token_id.clone(),
                outcome: Outcome::Up,
                bids: mock_book.bids.clone(),
                asks: mock_book.asks.clone(),
                updated_at: time::OffsetDateTime::now_utc(),
            })
        } else {
            // Return an empty book if not configured
            Ok(OutcomeBook {
                token_id: token_id.to_string(),
                outcome: Outcome::Up,
                bids: Vec::new(),
                asks: Vec::new(),
                updated_at: time::OffsetDateTime::now_utc(),
            })
        }
    }

    /// Get mock positions.
    pub async fn get_positions(
        &self,
        token_ids: &[String],
    ) -> Result<Vec<PositionResponse>, TradingError> {
        if self.config.latency_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(self.config.latency_ms)).await;
        }

        if self.config.fail_positions {
            return Err(TradingError::SubmissionFailed(
                "Mock positions failure".to_string(),
            ));
        }

        let positions = self.positions.lock().unwrap();
        let result: Vec<PositionResponse> = positions
            .iter()
            .filter(|p| token_ids.is_empty() || token_ids.contains(&p.token_id))
            .map(|p| PositionResponse {
                token_id: Some(p.token_id.clone()),
                asset: None,
                size: Some(p.size.to_string()),
                avg_price: Some(p.avg_price.to_string()),
            })
            .collect();

        Ok(result)
    }
}

impl Default for MockPolymarketClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating mock order books with common patterns.
pub struct MockOrderBookBuilder {
    token_id: String,
    bids: Vec<PriceLevel>,
    asks: Vec<PriceLevel>,
}

impl MockOrderBookBuilder {
    /// Create a new builder for the given token.
    pub fn new(token_id: impl Into<String>) -> Self {
        Self {
            token_id: token_id.into(),
            bids: Vec::new(),
            asks: Vec::new(),
        }
    }

    /// Add a bid level.
    pub fn bid(mut self, price: Decimal, size: Decimal) -> Self {
        self.bids.push(PriceLevel { price, size });
        self
    }

    /// Add an ask level.
    pub fn ask(mut self, price: Decimal, size: Decimal) -> Self {
        self.asks.push(PriceLevel { price, size });
        self
    }

    /// Create a typical order book with spread.
    pub fn with_spread(mut self, best_bid: Decimal, best_ask: Decimal, depth: Decimal) -> Self {
        self.bids = vec![
            PriceLevel {
                price: best_bid,
                size: depth,
            },
            PriceLevel {
                price: best_bid - Decimal::new(1, 2),
                size: depth * Decimal::TWO,
            },
        ];
        self.asks = vec![
            PriceLevel {
                price: best_ask,
                size: depth,
            },
            PriceLevel {
                price: best_ask + Decimal::new(1, 2),
                size: depth * Decimal::TWO,
            },
        ];
        self
    }

    /// Build the mock order book.
    pub fn build(mut self) -> MockOrderBook {
        // Sort bids descending
        self.bids.sort_by(|a, b| b.price.cmp(&a.price));
        // Sort asks ascending
        self.asks.sort_by(|a, b| a.price.cmp(&b.price));

        MockOrderBook {
            token_id: self.token_id,
            bids: self.bids,
            asks: self.asks,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[tokio::test]
    async fn mock_client_balance() {
        let mut client = MockPolymarketClient::new();
        client.set_balance(dec!(100.50));

        let balance = client.get_balance().await.unwrap();
        assert_eq!(balance, dec!(100.50));
    }

    #[tokio::test]
    async fn mock_client_order_book() {
        let client = MockPolymarketClient::new();

        let book = MockOrderBookBuilder::new("token-123")
            .with_spread(dec!(0.48), dec!(0.52), dec!(100))
            .build();
        client.set_order_book(book);

        let result = client.get_order_book("token-123").await.unwrap();
        assert_eq!(result.token_id, "token-123");
        assert!(!result.bids.is_empty());
        assert!(!result.asks.is_empty());
    }

    #[tokio::test]
    async fn mock_client_positions() {
        let client = MockPolymarketClient::new();
        client.add_position(MockPosition {
            token_id: "token-123".to_string(),
            size: dec!(10),
            avg_price: dec!(0.50),
        });

        let positions = client.get_positions(&[]).await.unwrap();
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].token_id, Some("token-123".to_string()));
    }

    #[tokio::test]
    async fn mock_client_failure_modes() {
        let config = MockConfig {
            fail_balance: true,
            ..Default::default()
        };
        let client = MockPolymarketClient::with_config(config);

        let result = client.get_balance().await;
        assert!(result.is_err());
    }

    #[test]
    fn mock_order_book_builder() {
        let book = MockOrderBookBuilder::new("test-token")
            .bid(dec!(0.50), dec!(100))
            .bid(dec!(0.49), dec!(200))
            .ask(dec!(0.52), dec!(100))
            .ask(dec!(0.53), dec!(200))
            .build();

        assert_eq!(book.bids.len(), 2);
        assert_eq!(book.asks.len(), 2);
        // Verify sorted correctly
        assert_eq!(book.bids[0].price, dec!(0.50)); // Highest bid first
        assert_eq!(book.asks[0].price, dec!(0.52)); // Lowest ask first
    }
}
