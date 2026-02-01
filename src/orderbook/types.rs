//! Order book types and data structures.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::market::Outcome;

/// Single price level in an order book.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct PriceLevel {
    /// Price at this level.
    pub price: Decimal,
    /// Total size available at this price.
    pub size: Decimal,
}

impl PriceLevel {
    /// Create a new price level.
    pub fn new(price: Decimal, size: Decimal) -> Self {
        Self { price, size }
    }
}

/// L2 order book for one outcome.
#[derive(Debug, Clone)]
pub struct OutcomeBook {
    /// Token ID this book represents.
    pub token_id: String,
    /// Which outcome (Up or Down).
    pub outcome: Outcome,
    /// Bid levels sorted by price descending.
    pub bids: Vec<PriceLevel>,
    /// Ask levels sorted by price ascending.
    pub asks: Vec<PriceLevel>,
    /// When this book was last updated.
    pub updated_at: OffsetDateTime,
}

impl Default for OutcomeBook {
    fn default() -> Self {
        Self {
            token_id: String::new(),
            outcome: Outcome::default(),
            bids: Vec::new(),
            asks: Vec::new(),
            updated_at: OffsetDateTime::now_utc(),
        }
    }
}

impl OutcomeBook {
    /// Get the best bid price.
    pub fn best_bid(&self) -> Option<Decimal> {
        self.bids.first().map(|l| l.price)
    }

    /// Get the best ask price.
    pub fn best_ask(&self) -> Option<Decimal> {
        self.asks.first().map(|l| l.price)
    }

    /// Get the spread between best bid and ask.
    pub fn spread(&self) -> Option<Decimal> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some(ask - bid),
            _ => None,
        }
    }

    /// Get size available at best bid.
    pub fn bid_size(&self) -> Decimal {
        self.bids.first().map(|l| l.size).unwrap_or(Decimal::ZERO)
    }

    /// Get size available at best ask.
    pub fn ask_size(&self) -> Decimal {
        self.asks.first().map(|l| l.size).unwrap_or(Decimal::ZERO)
    }

    /// Check if the book is inverted (best_ask < best_bid).
    pub fn is_inverted(&self) -> bool {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => ask < bid,
            _ => false,
        }
    }

    /// Get total liquidity on the bid side.
    pub fn total_bid_liquidity(&self) -> Decimal {
        self.bids.iter().map(|l| l.size).sum()
    }

    /// Get total liquidity on the ask side.
    pub fn total_ask_liquidity(&self) -> Decimal {
        self.asks.iter().map(|l| l.size).sum()
    }
}

/// Result of calculating fill price by walking the book.
#[derive(Debug, Clone)]
pub struct FillInfo {
    /// Total size that can be filled.
    pub filled_size: Decimal,
    /// Total cost to fill.
    pub total_cost: Decimal,
    /// Volume-weighted average price.
    pub vwap: Decimal,
    /// Worst price encountered (highest for buys).
    pub worst_price: Decimal,
    /// Best price available.
    pub best_price: Option<Decimal>,
}

/// WebSocket event types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WsEventType {
    /// Full book snapshot.
    Book,
    /// Incremental price change.
    PriceChange,
}

/// Book update notification.
#[derive(Debug, Clone)]
pub struct BookUpdate {
    /// Asset ID that was updated.
    pub asset_id: String,
    /// Type of event.
    pub event_type: WsEventType,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn price_level_creation() {
        let level = PriceLevel::new(dec!(0.50), dec!(100));
        assert_eq!(level.price, dec!(0.50));
        assert_eq!(level.size, dec!(100));
    }

    #[test]
    fn outcome_book_best_prices() {
        let book = OutcomeBook {
            token_id: "test".to_string(),
            outcome: Outcome::Up,
            bids: vec![
                PriceLevel::new(dec!(0.48), dec!(50)),
                PriceLevel::new(dec!(0.47), dec!(100)),
            ],
            asks: vec![
                PriceLevel::new(dec!(0.50), dec!(50)),
                PriceLevel::new(dec!(0.51), dec!(100)),
            ],
            updated_at: OffsetDateTime::now_utc(),
        };

        assert_eq!(book.best_bid(), Some(dec!(0.48)));
        assert_eq!(book.best_ask(), Some(dec!(0.50)));
        assert_eq!(book.spread(), Some(dec!(0.02)));
    }

    #[test]
    fn outcome_book_detects_inverted() {
        let inverted_book = OutcomeBook {
            token_id: "test".to_string(),
            outcome: Outcome::Up,
            bids: vec![PriceLevel::new(dec!(0.52), dec!(50))],
            asks: vec![PriceLevel::new(dec!(0.50), dec!(50))],
            updated_at: OffsetDateTime::now_utc(),
        };

        assert!(inverted_book.is_inverted());

        let normal_book = OutcomeBook {
            token_id: "test".to_string(),
            outcome: Outcome::Up,
            bids: vec![PriceLevel::new(dec!(0.48), dec!(50))],
            asks: vec![PriceLevel::new(dec!(0.50), dec!(50))],
            updated_at: OffsetDateTime::now_utc(),
        };

        assert!(!normal_book.is_inverted());
    }

    #[test]
    fn total_liquidity_calculation() {
        let book = OutcomeBook {
            token_id: "test".to_string(),
            outcome: Outcome::Up,
            bids: vec![
                PriceLevel::new(dec!(0.48), dec!(50)),
                PriceLevel::new(dec!(0.47), dec!(100)),
            ],
            asks: vec![
                PriceLevel::new(dec!(0.50), dec!(50)),
                PriceLevel::new(dec!(0.51), dec!(100)),
            ],
            updated_at: OffsetDateTime::now_utc(),
        };

        assert_eq!(book.total_bid_liquidity(), dec!(150));
        assert_eq!(book.total_ask_liquidity(), dec!(150));
    }
}
