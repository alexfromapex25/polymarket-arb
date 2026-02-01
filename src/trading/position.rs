//! Position tracking.

use rust_decimal::Decimal;
use serde::Deserialize;

/// Position in a single token.
#[derive(Debug, Clone, Default)]
pub struct Position {
    /// Token ID.
    pub token_id: String,
    /// Number of shares owned.
    pub size: Decimal,
    /// Average entry price.
    pub avg_price: Decimal,
    /// Unrealized P&L.
    pub unrealized_pnl: Option<Decimal>,
}

impl Position {
    /// Calculate the cost basis.
    pub fn cost_basis(&self) -> Decimal {
        self.size * self.avg_price
    }

    /// Calculate current value at a given price.
    pub fn current_value(&self, current_price: Decimal) -> Decimal {
        self.size * current_price
    }

    /// Calculate P&L at a given price.
    pub fn pnl(&self, current_price: Decimal) -> Decimal {
        self.current_value(current_price) - self.cost_basis()
    }
}

/// Positions for both outcomes in a market.
#[derive(Debug, Clone, Default)]
pub struct MarketPositions {
    /// UP (YES) position.
    pub up: Position,
    /// DOWN (NO) position.
    pub down: Position,
}

impl MarketPositions {
    /// Check if positions are balanced (equal size on both sides).
    pub fn is_balanced(&self) -> bool {
        self.up.size == self.down.size
    }

    /// Get the imbalance (positive = more UP, negative = more DOWN).
    pub fn imbalance(&self) -> Decimal {
        self.up.size - self.down.size
    }

    /// Get total cost basis.
    pub fn total_cost_basis(&self) -> Decimal {
        self.up.cost_basis() + self.down.cost_basis()
    }

    /// Calculate expected payout at settlement.
    /// For balanced positions, payout = size (one side wins $1 each).
    pub fn expected_payout(&self) -> Decimal {
        // Each pair of (UP, DOWN) pays exactly $1 at settlement
        let pairs = self.up.size.min(self.down.size);

        // Imbalanced portion depends on outcome (unknown)
        // For simplicity, return just the guaranteed payout from pairs
        pairs
    }

    /// Calculate expected profit for balanced positions.
    pub fn expected_profit(&self) -> Decimal {
        let pairs = self.up.size.min(self.down.size);
        let cost = self.up.avg_price * pairs + self.down.avg_price * pairs;
        pairs - cost
    }
}

/// Position response from API.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiPosition {
    /// Token ID.
    #[serde(alias = "token_id")]
    pub asset_id: Option<String>,
    /// Nested asset info.
    pub asset: Option<ApiAsset>,
    /// Position size.
    pub size: Option<String>,
    /// Average entry price.
    pub avg_price: Option<String>,
}

/// Asset info in API response.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiAsset {
    /// Token ID.
    pub token_id: Option<String>,
}

impl ApiPosition {
    /// Get the token ID from this position.
    pub fn token_id(&self) -> Option<&str> {
        self.asset_id
            .as_deref()
            .or_else(|| self.asset.as_ref()?.token_id.as_deref())
    }

    /// Convert to Position struct.
    pub fn to_position(&self) -> Option<Position> {
        let token_id = self.token_id()?.to_string();
        let size: Decimal = self.size.as_ref()?.parse().ok()?;
        let avg_price: Decimal = self.avg_price.as_ref()?.parse().ok()?;

        Some(Position {
            token_id,
            size,
            avg_price,
            unrealized_pnl: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn position_calculations() {
        let pos = Position {
            token_id: "token".to_string(),
            size: dec!(10),
            avg_price: dec!(0.50),
            unrealized_pnl: None,
        };

        assert_eq!(pos.cost_basis(), dec!(5));
        assert_eq!(pos.current_value(dec!(0.60)), dec!(6));
        assert_eq!(pos.pnl(dec!(0.60)), dec!(1));
    }

    #[test]
    fn market_positions_balanced() {
        let positions = MarketPositions {
            up: Position {
                token_id: "up".to_string(),
                size: dec!(10),
                avg_price: dec!(0.48),
                unrealized_pnl: None,
            },
            down: Position {
                token_id: "down".to_string(),
                size: dec!(10),
                avg_price: dec!(0.51),
                unrealized_pnl: None,
            },
        };

        assert!(positions.is_balanced());
        assert_eq!(positions.imbalance(), dec!(0));
        assert_eq!(positions.total_cost_basis(), dec!(9.9));
        assert_eq!(positions.expected_payout(), dec!(10));
        assert_eq!(positions.expected_profit(), dec!(0.1));
    }

    #[test]
    fn market_positions_imbalanced() {
        let positions = MarketPositions {
            up: Position {
                token_id: "up".to_string(),
                size: dec!(15),
                avg_price: dec!(0.50),
                unrealized_pnl: None,
            },
            down: Position {
                token_id: "down".to_string(),
                size: dec!(10),
                avg_price: dec!(0.50),
                unrealized_pnl: None,
            },
        };

        assert!(!positions.is_balanced());
        assert_eq!(positions.imbalance(), dec!(5));
        // Expected payout only counts balanced pairs
        assert_eq!(positions.expected_payout(), dec!(10));
    }
}
