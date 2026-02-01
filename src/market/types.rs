//! Market-related types for BTC 15-minute prediction markets.

use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use time::OffsetDateTime;

/// Market outcome for BTC 15min binary markets.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString, Default,
)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    /// BTC goes up (YES token).
    #[strum(serialize = "up", serialize = "yes", serialize = "UP", serialize = "YES")]
    #[default]
    Up,
    /// BTC goes down (NO token).
    #[strum(serialize = "down", serialize = "no", serialize = "DOWN", serialize = "NO")]
    Down,
}

impl Outcome {
    /// Get the opposite outcome.
    pub fn opposite(&self) -> Self {
        match self {
            Outcome::Up => Outcome::Down,
            Outcome::Down => Outcome::Up,
        }
    }
}

/// Active BTC 15-minute market information.
#[derive(Debug, Clone)]
pub struct Market {
    /// Market slug (e.g., "btc-updown-15m-1765301400").
    pub slug: String,
    /// Unique market identifier.
    pub id: String,
    /// UP (YES) token ID for CLOB.
    pub up_token_id: String,
    /// DOWN (NO) token ID for CLOB.
    pub down_token_id: String,
    /// Unix timestamp when market opened.
    pub start_timestamp: i64,
    /// Unix timestamp when market closes (start + 900s).
    pub end_timestamp: i64,
    /// Market question text.
    pub question: Option<String>,
}

impl Market {
    /// Duration of a BTC 15-minute market in seconds.
    pub const WINDOW_SECONDS: i64 = 900;

    /// Get the token ID for a given outcome.
    pub fn token_id(&self, outcome: Outcome) -> &str {
        match outcome {
            Outcome::Up => &self.up_token_id,
            Outcome::Down => &self.down_token_id,
        }
    }

    /// Check if the market is closed.
    pub fn is_closed(&self) -> bool {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        now >= self.end_timestamp
    }

    /// Get remaining time until market closes.
    pub fn time_remaining(&self) -> Option<std::time::Duration> {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        let remaining = self.end_timestamp - now;
        if remaining <= 0 {
            None
        } else {
            Some(std::time::Duration::from_secs(remaining as u64))
        }
    }

    /// Format remaining time as "Xm Ys" string.
    pub fn time_remaining_str(&self) -> String {
        match self.time_remaining() {
            Some(duration) => {
                let secs = duration.as_secs();
                let minutes = secs / 60;
                let seconds = secs % 60;
                format!("{}m {}s", minutes, seconds)
            }
            None => "CLOSED".to_string(),
        }
    }
}

/// Parsed market data from Polymarket API.
#[derive(Debug, Clone, Deserialize)]
pub struct MarketData {
    /// Market slug.
    pub slug: Option<String>,
    /// Market ID.
    pub id: Option<String>,
    /// CLOB token IDs.
    #[serde(rename = "clobTokenIds")]
    pub clob_token_ids: Option<Vec<String>>,
    /// Market outcomes.
    pub outcomes: Option<Vec<String>>,
    /// Market question.
    pub question: Option<String>,
    /// Start date (ISO format).
    #[serde(rename = "startDate")]
    pub start_date: Option<String>,
    /// End date (ISO format).
    #[serde(rename = "endDate")]
    pub end_date: Option<String>,
}

/// Market info from Gamma API.
#[derive(Debug, Clone, Deserialize)]
pub struct GammaMarket {
    /// Market slug.
    pub slug: Option<String>,
    /// Whether market is closed.
    pub closed: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_opposite_works() {
        assert_eq!(Outcome::Up.opposite(), Outcome::Down);
        assert_eq!(Outcome::Down.opposite(), Outcome::Up);
    }

    #[test]
    fn outcome_from_string_works() {
        use std::str::FromStr;
        assert_eq!(Outcome::from_str("up").unwrap(), Outcome::Up);
        assert_eq!(Outcome::from_str("down").unwrap(), Outcome::Down);
        assert_eq!(Outcome::from_str("yes").unwrap(), Outcome::Up);
        assert_eq!(Outcome::from_str("no").unwrap(), Outcome::Down);
    }

    #[test]
    fn market_token_id_works() {
        let market = Market {
            slug: "btc-updown-15m-123".to_string(),
            id: "market-id".to_string(),
            up_token_id: "up-token".to_string(),
            down_token_id: "down-token".to_string(),
            start_timestamp: 0,
            end_timestamp: 900,
            question: None,
        };

        assert_eq!(market.token_id(Outcome::Up), "up-token");
        assert_eq!(market.token_id(Outcome::Down), "down-token");
    }
}
