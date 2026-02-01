//! Order types and creation.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};

/// Order side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "UPPERCASE")]
pub enum Side {
    /// Buy order.
    #[strum(serialize = "BUY", serialize = "buy")]
    Buy,
    /// Sell order.
    #[strum(serialize = "SELL", serialize = "sell")]
    Sell,
}

/// Order time-in-force.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString, Default,
)]
#[serde(rename_all = "UPPERCASE")]
pub enum TimeInForce {
    /// Fill-or-kill: must fill entirely or cancel.
    #[default]
    #[strum(serialize = "FOK", serialize = "fok")]
    FOK,
    /// Fill-and-kill: fill what's available, cancel rest.
    #[strum(serialize = "FAK", serialize = "fak")]
    FAK,
    /// Good-till-cancelled: stays on book until filled or cancelled.
    #[strum(serialize = "GTC", serialize = "gtc")]
    GTC,
}

/// Order parameters for submission.
#[derive(Debug, Clone)]
pub struct OrderParams {
    /// Token ID to trade.
    pub token_id: String,
    /// Order side (buy/sell).
    pub side: Side,
    /// Limit price.
    pub price: Decimal,
    /// Order size.
    pub size: Decimal,
    /// Time-in-force.
    pub tif: TimeInForce,
}

impl OrderParams {
    /// Create a new buy order.
    pub fn buy(token_id: impl Into<String>, price: Decimal, size: Decimal) -> Self {
        Self {
            token_id: token_id.into(),
            side: Side::Buy,
            price,
            size,
            tif: TimeInForce::FOK,
        }
    }

    /// Create a new sell order.
    pub fn sell(token_id: impl Into<String>, price: Decimal, size: Decimal) -> Self {
        Self {
            token_id: token_id.into(),
            side: Side::Sell,
            price,
            size,
            tif: TimeInForce::FOK,
        }
    }

    /// Set time-in-force.
    pub fn with_tif(mut self, tif: TimeInForce) -> Self {
        self.tif = tif;
        self
    }

    /// Validate order parameters.
    pub fn validate(&self) -> Result<(), String> {
        if self.token_id.is_empty() {
            return Err("token_id is required".to_string());
        }
        if self.price <= Decimal::ZERO {
            return Err("price must be positive".to_string());
        }
        if self.size <= Decimal::ZERO {
            return Err("size must be positive".to_string());
        }
        Ok(())
    }
}

/// Order status from API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Display, EnumString)]
#[serde(rename_all = "lowercase")]
pub enum OrderStatus {
    /// Order is pending.
    #[strum(serialize = "pending", serialize = "PENDING")]
    Pending,
    /// Order is live on the book.
    #[strum(serialize = "live", serialize = "LIVE")]
    Live,
    /// Order is fully filled.
    #[strum(serialize = "filled", serialize = "FILLED")]
    Filled,
    /// Order was cancelled.
    #[strum(serialize = "canceled", serialize = "cancelled", serialize = "CANCELED", serialize = "CANCELLED")]
    Canceled,
    /// Order was rejected.
    #[strum(serialize = "rejected", serialize = "REJECTED")]
    Rejected,
    /// Order expired.
    #[strum(serialize = "expired", serialize = "EXPIRED")]
    Expired,
}

impl OrderStatus {
    /// Check if status is terminal (won't change).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            OrderStatus::Filled | OrderStatus::Canceled | OrderStatus::Rejected | OrderStatus::Expired
        )
    }

    /// Check if order was filled.
    pub fn is_filled(&self) -> bool {
        matches!(self, OrderStatus::Filled)
    }
}

/// Order state summary.
#[derive(Debug, Clone)]
pub struct OrderState {
    /// Order ID.
    pub order_id: String,
    /// Current status.
    pub status: Option<OrderStatus>,
    /// Filled size.
    pub filled_size: Option<Decimal>,
    /// Remaining size.
    pub remaining_size: Option<Decimal>,
    /// Original size.
    pub original_size: Option<Decimal>,
    /// Whether order is in terminal state.
    pub is_terminal: bool,
    /// Whether order was fully filled.
    pub is_filled: bool,
}

impl Default for OrderState {
    fn default() -> Self {
        Self {
            order_id: String::new(),
            status: None,
            filled_size: None,
            remaining_size: None,
            original_size: None,
            is_terminal: false,
            is_filled: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn order_params_creation() {
        let buy = OrderParams::buy("token-123", dec!(0.50), dec!(10));
        assert_eq!(buy.side, Side::Buy);
        assert_eq!(buy.price, dec!(0.50));
        assert_eq!(buy.size, dec!(10));
        assert_eq!(buy.tif, TimeInForce::FOK);

        let sell = OrderParams::sell("token-456", dec!(0.60), dec!(5)).with_tif(TimeInForce::GTC);
        assert_eq!(sell.side, Side::Sell);
        assert_eq!(sell.tif, TimeInForce::GTC);
    }

    #[test]
    fn order_params_validation() {
        let valid = OrderParams::buy("token", dec!(0.50), dec!(10));
        assert!(valid.validate().is_ok());

        let no_token = OrderParams::buy("", dec!(0.50), dec!(10));
        assert!(no_token.validate().is_err());

        let zero_price = OrderParams::buy("token", dec!(0), dec!(10));
        assert!(zero_price.validate().is_err());

        let negative_size = OrderParams::buy("token", dec!(0.50), dec!(-10));
        assert!(negative_size.validate().is_err());
    }

    #[test]
    fn order_status_terminal() {
        assert!(OrderStatus::Filled.is_terminal());
        assert!(OrderStatus::Canceled.is_terminal());
        assert!(OrderStatus::Rejected.is_terminal());
        assert!(OrderStatus::Expired.is_terminal());
        assert!(!OrderStatus::Pending.is_terminal());
        assert!(!OrderStatus::Live.is_terminal());
    }

    #[test]
    fn time_in_force_from_string() {
        use std::str::FromStr;
        assert_eq!(TimeInForce::from_str("FOK").unwrap(), TimeInForce::FOK);
        assert_eq!(TimeInForce::from_str("fok").unwrap(), TimeInForce::FOK);
        assert_eq!(TimeInForce::from_str("GTC").unwrap(), TimeInForce::GTC);
    }
}
