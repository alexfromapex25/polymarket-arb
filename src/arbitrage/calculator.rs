//! Profit and cost calculations for arbitrage opportunities.

use rust_decimal::Decimal;
use time::OffsetDateTime;

use crate::market::Market;
use crate::orderbook::{calculate_fill_price, OutcomeBook};

/// Detected arbitrage opportunity.
#[derive(Debug, Clone)]
pub struct ArbitrageOpportunity {
    /// Market being traded.
    pub market: Market,
    /// Limit price for UP leg (worst fill price).
    pub up_price: Decimal,
    /// Limit price for DOWN leg (worst fill price).
    pub down_price: Decimal,
    /// Total cost per share pair (up_price + down_price).
    pub total_cost: Decimal,
    /// Profit per share pair (1.0 - total_cost).
    pub profit_per_share: Decimal,
    /// Profit percentage ((profit / cost) * 100).
    pub profit_pct: Decimal,
    /// Number of shares to trade each side.
    pub order_size: Decimal,
    /// Total investment (total_cost * order_size).
    pub total_investment: Decimal,
    /// Expected payout at close (order_size * 1.0).
    pub expected_payout: Decimal,
    /// Expected profit (expected_payout - total_investment).
    pub expected_profit: Decimal,
    /// Best ask price for UP.
    pub best_ask_up: Option<Decimal>,
    /// Best ask price for DOWN.
    pub best_ask_down: Option<Decimal>,
    /// VWAP for UP fill.
    pub vwap_up: Option<Decimal>,
    /// VWAP for DOWN fill.
    pub vwap_down: Option<Decimal>,
    /// Timestamp when opportunity was detected.
    pub detected_at: OffsetDateTime,
}

impl ArbitrageOpportunity {
    /// Calculate expected return on investment.
    pub fn roi(&self) -> Decimal {
        if self.total_investment.is_zero() {
            Decimal::ZERO
        } else {
            (self.expected_profit / self.total_investment) * Decimal::ONE_HUNDRED
        }
    }
}

/// Calculate arbitrage metrics from order books.
pub fn calculate_opportunity(
    market: &Market,
    up_book: &OutcomeBook,
    down_book: &OutcomeBook,
    target_size: Decimal,
    threshold: Decimal,
) -> Option<ArbitrageOpportunity> {
    // Get fill prices for both sides
    let up_fill = calculate_fill_price(&up_book.asks, target_size).ok()?;
    let down_fill = calculate_fill_price(&down_book.asks, target_size).ok()?;

    // Use worst-case prices for guaranteed arbitrage
    let up_price = up_fill.worst_price;
    let down_price = down_fill.worst_price;
    let total_cost = up_price + down_price;

    // Check if profitable
    if total_cost > threshold {
        return None;
    }

    let profit_per_share = Decimal::ONE - total_cost;
    let profit_pct = if total_cost > Decimal::ZERO {
        (profit_per_share / total_cost) * Decimal::ONE_HUNDRED
    } else {
        Decimal::ZERO
    };

    let total_investment = total_cost * target_size;
    let expected_payout = target_size; // $1.00 per share at settlement
    let expected_profit = expected_payout - total_investment;

    Some(ArbitrageOpportunity {
        market: market.clone(),
        up_price,
        down_price,
        total_cost,
        profit_per_share,
        profit_pct,
        order_size: target_size,
        total_investment,
        expected_payout,
        expected_profit,
        best_ask_up: up_fill.best_price,
        best_ask_down: down_fill.best_price,
        vwap_up: Some(up_fill.vwap),
        vwap_down: Some(down_fill.vwap),
        detected_at: OffsetDateTime::now_utc(),
    })
}

/// Calculate the break-even threshold (cost at which profit = 0).
pub fn break_even_cost() -> Decimal {
    Decimal::ONE
}

/// Calculate required balance including safety margin.
pub fn calculate_required_balance(opportunity: &ArbitrageOpportunity, margin: Decimal) -> Decimal {
    opportunity.total_investment * margin
}

/// Calculate the effective spread between best asks.
pub fn effective_spread(up_book: &OutcomeBook, down_book: &OutcomeBook) -> Option<Decimal> {
    match (up_book.best_ask(), down_book.best_ask()) {
        (Some(up), Some(down)) => Some(up + down - Decimal::ONE),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::Outcome;
    use crate::orderbook::PriceLevel;
    use rust_decimal_macros::dec;
    use time::OffsetDateTime;

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

    fn test_book(outcome: Outcome, ask_price: Decimal, ask_size: Decimal) -> OutcomeBook {
        OutcomeBook {
            token_id: "token".to_string(),
            outcome,
            bids: vec![],
            asks: vec![PriceLevel::new(ask_price, ask_size)],
            updated_at: OffsetDateTime::now_utc(),
        }
    }

    #[test]
    fn detect_arbitrage_when_profitable() {
        let market = test_market();
        let up_book = test_book(Outcome::Up, dec!(0.48), dec!(100));
        let down_book = test_book(Outcome::Down, dec!(0.51), dec!(100));

        let opp = calculate_opportunity(&market, &up_book, &down_book, dec!(10), dec!(0.991));

        assert!(opp.is_some());
        let opp = opp.unwrap();
        assert_eq!(opp.total_cost, dec!(0.99));
        assert_eq!(opp.profit_per_share, dec!(0.01));
        assert_eq!(opp.total_investment, dec!(9.9));
        assert_eq!(opp.expected_payout, dec!(10));
        assert_eq!(opp.expected_profit, dec!(0.1));
    }

    #[test]
    fn no_arbitrage_when_cost_exceeds_threshold() {
        let market = test_market();
        let up_book = test_book(Outcome::Up, dec!(0.50), dec!(100));
        let down_book = test_book(Outcome::Down, dec!(0.51), dec!(100));

        let opp = calculate_opportunity(&market, &up_book, &down_book, dec!(10), dec!(0.99));

        assert!(opp.is_none()); // 0.50 + 0.51 = 1.01 > 0.99
    }

    #[test]
    fn opportunity_roi_calculation() {
        let opp = ArbitrageOpportunity {
            market: test_market(),
            up_price: dec!(0.48),
            down_price: dec!(0.51),
            total_cost: dec!(0.99),
            profit_per_share: dec!(0.01),
            profit_pct: dec!(1.0101),
            order_size: dec!(100),
            total_investment: dec!(99),
            expected_payout: dec!(100),
            expected_profit: dec!(1),
            best_ask_up: Some(dec!(0.48)),
            best_ask_down: Some(dec!(0.51)),
            vwap_up: Some(dec!(0.48)),
            vwap_down: Some(dec!(0.51)),
            detected_at: OffsetDateTime::now_utc(),
        };

        // ROI = (1 / 99) * 100 ≈ 1.0101%
        assert!(opp.roi() > dec!(1) && opp.roi() < dec!(1.02));
    }

    #[test]
    fn effective_spread_calculation() {
        let up_book = test_book(Outcome::Up, dec!(0.48), dec!(100));
        let down_book = test_book(Outcome::Down, dec!(0.51), dec!(100));

        let spread = effective_spread(&up_book, &down_book);

        // 0.48 + 0.51 - 1.0 = -0.01 (favorable)
        assert_eq!(spread, Some(dec!(-0.01)));
    }
}
