//! Arbitrage opportunity detection.

use rust_decimal::Decimal;
use tracing::{debug, info, instrument, warn};

use super::calculator::{calculate_opportunity, ArbitrageOpportunity};
use crate::config::Config;
use crate::error::ArbitrageError;
use crate::market::Market;
use crate::orderbook::OutcomeBook;

/// Check for arbitrage opportunity given order books.
#[instrument(skip(up_book, down_book, config), fields(market = %market.slug))]
pub fn check_arbitrage(
    market: &Market,
    up_book: &OutcomeBook,
    down_book: &OutcomeBook,
    config: &Config,
) -> Result<Option<ArbitrageOpportunity>, ArbitrageError> {
    // Validate books are not inverted
    if up_book.is_inverted() {
        let best_ask = up_book.best_ask().unwrap_or_default();
        let best_bid = up_book.best_bid().unwrap_or_default();
        warn!(
            side = "UP",
            best_ask = %best_ask,
            best_bid = %best_bid,
            "Order book inverted"
        );
        return Err(ArbitrageError::BookInverted {
            side: "UP".to_string(),
            best_ask,
            best_bid,
        });
    }

    if down_book.is_inverted() {
        let best_ask = down_book.best_ask().unwrap_or_default();
        let best_bid = down_book.best_bid().unwrap_or_default();
        warn!(
            side = "DOWN",
            best_ask = %best_ask,
            best_bid = %best_bid,
            "Order book inverted"
        );
        return Err(ArbitrageError::BookInverted {
            side: "DOWN".to_string(),
            best_ask,
            best_bid,
        });
    }

    // Check if asks are available
    if up_book.asks.is_empty() || down_book.asks.is_empty() {
        debug!("No asks available in order book");
        return Ok(None);
    }

    // Calculate opportunity
    let opportunity = calculate_opportunity(
        market,
        up_book,
        down_book,
        config.order_size,
        config.target_pair_cost,
    );

    if let Some(ref opp) = opportunity {
        info!(
            total_cost = %opp.total_cost,
            profit_per_share = %opp.profit_per_share,
            profit_pct = %opp.profit_pct,
            "Arbitrage opportunity detected"
        );
    } else {
        let best_total = up_book.best_ask().unwrap_or_default()
            + down_book.best_ask().unwrap_or_default();
        debug!(
            best_total = %best_total,
            threshold = %config.target_pair_cost,
            "No arbitrage opportunity"
        );
    }

    Ok(opportunity)
}

/// Quick check if books might have an opportunity (without full calculation).
pub fn quick_opportunity_check(
    up_book: &OutcomeBook,
    down_book: &OutcomeBook,
    threshold: Decimal,
) -> bool {
    match (up_book.best_ask(), down_book.best_ask()) {
        (Some(up), Some(down)) => up + down <= threshold,
        _ => false,
    }
}

/// Get diagnostic information about why there's no opportunity.
pub fn diagnose_no_opportunity(
    up_book: &OutcomeBook,
    down_book: &OutcomeBook,
    target_size: Decimal,
    threshold: Decimal,
) -> NoOpportunityDiagnosis {
    let best_ask_up = up_book.best_ask();
    let best_ask_down = down_book.best_ask();

    let best_total = match (best_ask_up, best_ask_down) {
        (Some(up), Some(down)) => Some(up + down),
        _ => None,
    };

    let up_liquidity = up_book.total_ask_liquidity();
    let down_liquidity = down_book.total_ask_liquidity();
    let has_sufficient_liquidity = up_liquidity >= target_size && down_liquidity >= target_size;

    // Calculate fill-based total if possible
    let fill_total = {
        use crate::orderbook::calculate_fill_price;
        let up_fill = calculate_fill_price(&up_book.asks, target_size).ok();
        let down_fill = calculate_fill_price(&down_book.asks, target_size).ok();
        match (up_fill, down_fill) {
            (Some(u), Some(d)) => Some(u.worst_price + d.worst_price),
            _ => None,
        }
    };

    NoOpportunityDiagnosis {
        best_ask_up,
        best_ask_down,
        best_total,
        fill_total,
        threshold,
        up_liquidity,
        down_liquidity,
        has_sufficient_liquidity,
    }
}

/// Diagnostic information for debugging.
#[derive(Debug, Clone)]
pub struct NoOpportunityDiagnosis {
    /// Best ask price for UP.
    pub best_ask_up: Option<Decimal>,
    /// Best ask price for DOWN.
    pub best_ask_down: Option<Decimal>,
    /// Sum of best asks.
    pub best_total: Option<Decimal>,
    /// Sum of worst fill prices for target size.
    pub fill_total: Option<Decimal>,
    /// Cost threshold.
    pub threshold: Decimal,
    /// Total ask liquidity for UP.
    pub up_liquidity: Decimal,
    /// Total ask liquidity for DOWN.
    pub down_liquidity: Decimal,
    /// Whether there's enough liquidity for target size.
    pub has_sufficient_liquidity: bool,
}

impl std::fmt::Display for NoOpportunityDiagnosis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "UP=${} + DOWN=${} = ${} (threshold=${}) | fill=${} | liq: UP={}, DOWN={}",
            self.best_ask_up.map(|d| d.to_string()).unwrap_or_else(|| "N/A".to_string()),
            self.best_ask_down.map(|d| d.to_string()).unwrap_or_else(|| "N/A".to_string()),
            self.best_total.map(|d| d.to_string()).unwrap_or_else(|| "N/A".to_string()),
            self.threshold,
            self.fill_total.map(|d| d.to_string()).unwrap_or_else(|| "N/A".to_string()),
            self.up_liquidity,
            self.down_liquidity,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::Outcome;
    use crate::orderbook::PriceLevel;
    use rust_decimal_macros::dec;
    use time::OffsetDateTime;

    fn test_config() -> Config {
        Config {
            polymarket_private_key: "0x1234".to_string(),
            polymarket_api_key: None,
            polymarket_api_secret: None,
            polymarket_api_passphrase: None,
            polymarket_signature_type: 0,
            polymarket_funder: None,
            target_pair_cost: dec!(0.991),
            order_size: dec!(10),
            order_type: "FOK".to_string(),
            balance_margin: dec!(1.2),
            dry_run: true,
            sim_balance: dec!(100),
            cooldown_seconds: 10,
            polymarket_market_slug: None,
            use_wss: false,
            polymarket_ws_url: "wss://test".to_string(),
            polymarket_clob_url: "https://test".to_string(),
            port: 8080,
            rust_log: "info".to_string(),
            verbose: false,
        }
    }

    fn test_market() -> Market {
        Market {
            slug: "btc-updown-15m-123".to_string(),
            id: "market-id".to_string(),
            up_token_id: "up-token".to_string(),
            down_token_id: "down-token".to_string(),
            start_timestamp: 0,
            end_timestamp: 900,
            question: None,
        }
    }

    fn test_book(outcome: Outcome, asks: Vec<(Decimal, Decimal)>) -> OutcomeBook {
        OutcomeBook {
            token_id: "token".to_string(),
            outcome,
            bids: vec![],
            asks: asks.into_iter().map(|(p, s)| PriceLevel::new(p, s)).collect(),
            updated_at: OffsetDateTime::now_utc(),
        }
    }

    #[test]
    fn check_arbitrage_finds_opportunity() {
        let market = test_market();
        let config = test_config();
        let up_book = test_book(Outcome::Up, vec![(dec!(0.48), dec!(100))]);
        let down_book = test_book(Outcome::Down, vec![(dec!(0.51), dec!(100))]);

        let result = check_arbitrage(&market, &up_book, &down_book, &config).unwrap();

        assert!(result.is_some());
    }

    #[test]
    fn check_arbitrage_returns_none_when_costly() {
        let market = test_market();
        let config = test_config();
        let up_book = test_book(Outcome::Up, vec![(dec!(0.55), dec!(100))]);
        let down_book = test_book(Outcome::Down, vec![(dec!(0.55), dec!(100))]);

        let result = check_arbitrage(&market, &up_book, &down_book, &config).unwrap();

        assert!(result.is_none());
    }

    #[test]
    fn quick_opportunity_check_works() {
        let up_book = test_book(Outcome::Up, vec![(dec!(0.48), dec!(100))]);
        let down_book = test_book(Outcome::Down, vec![(dec!(0.51), dec!(100))]);

        assert!(quick_opportunity_check(&up_book, &down_book, dec!(0.991)));
        assert!(!quick_opportunity_check(&up_book, &down_book, dec!(0.98)));
    }
}
