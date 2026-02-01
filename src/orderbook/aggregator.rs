//! Order book aggregation and fill price calculations.

use rust_decimal::Decimal;
use tracing::instrument;

use super::types::{FillInfo, OutcomeBook, PriceLevel};
use crate::error::ArbitrageError;

/// Calculate worst-case fill price by walking the ask book.
///
/// Returns VWAP and worst price needed to fill target_size.
#[instrument(skip(asks), fields(target_size = %target_size))]
pub fn calculate_fill_price(
    asks: &[PriceLevel],
    target_size: Decimal,
) -> Result<FillInfo, ArbitrageError> {
    if target_size <= Decimal::ZERO {
        return Err(ArbitrageError::InvalidSize(target_size));
    }

    if asks.is_empty() {
        return Err(ArbitrageError::InsufficientLiquidity {
            required: target_size,
            available: Decimal::ZERO,
        });
    }

    let mut remaining = target_size;
    let mut total_cost = Decimal::ZERO;
    let mut worst_price = Decimal::ZERO;
    let best_price = asks.first().map(|l| l.price);

    for level in asks {
        if remaining.is_zero() {
            break;
        }

        let fill_size = remaining.min(level.size);
        total_cost += fill_size * level.price;
        remaining -= fill_size;
        worst_price = level.price;
    }

    if !remaining.is_zero() {
        return Err(ArbitrageError::InsufficientLiquidity {
            available: target_size - remaining,
            required: target_size,
        });
    }

    let vwap = total_cost / target_size;

    Ok(FillInfo {
        filled_size: target_size,
        total_cost,
        vwap,
        worst_price,
        best_price,
    })
}

/// Calculate total cost to buy a given size from the ask book.
pub fn calculate_buy_cost(asks: &[PriceLevel], size: Decimal) -> Option<Decimal> {
    calculate_fill_price(asks, size).ok().map(|f| f.total_cost)
}

/// Calculate total revenue from selling a given size into the bid book.
pub fn calculate_sell_revenue(bids: &[PriceLevel], size: Decimal) -> Option<Decimal> {
    if size <= Decimal::ZERO || bids.is_empty() {
        return None;
    }

    let mut remaining = size;
    let mut total_revenue = Decimal::ZERO;

    for level in bids {
        if remaining.is_zero() {
            break;
        }

        let fill_size = remaining.min(level.size);
        total_revenue += fill_size * level.price;
        remaining -= fill_size;
    }

    if remaining.is_zero() {
        Some(total_revenue)
    } else {
        None
    }
}

/// Get depth at a specific price level.
pub fn depth_at_price(levels: &[PriceLevel], price: Decimal) -> Decimal {
    levels
        .iter()
        .filter(|l| l.price == price)
        .map(|l| l.size)
        .sum()
}

/// Get cumulative depth up to a price level (for asks: prices <= target).
pub fn cumulative_depth_up_to(asks: &[PriceLevel], target_price: Decimal) -> Decimal {
    asks.iter()
        .filter(|l| l.price <= target_price)
        .map(|l| l.size)
        .sum()
}

/// Merge two order books (used for combining snapshots with deltas).
pub fn merge_levels(existing: &[PriceLevel], updates: &[PriceLevel]) -> Vec<PriceLevel> {
    use std::collections::HashMap;

    let mut price_map: HashMap<Decimal, Decimal> = existing
        .iter()
        .map(|l| (l.price, l.size))
        .collect();

    for update in updates {
        if update.size.is_zero() {
            price_map.remove(&update.price);
        } else {
            price_map.insert(update.price, update.size);
        }
    }

    price_map
        .into_iter()
        .filter(|(_, size)| *size > Decimal::ZERO)
        .map(|(price, size)| PriceLevel { price, size })
        .collect()
}

/// Calculate the mid price from best bid and ask.
pub fn mid_price(book: &OutcomeBook) -> Option<Decimal> {
    match (book.best_bid(), book.best_ask()) {
        (Some(bid), Some(ask)) => Some((bid + ask) / Decimal::TWO),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn calculate_fill_price_single_level() {
        let asks = vec![PriceLevel::new(dec!(0.50), dec!(100))];
        let result = calculate_fill_price(&asks, dec!(10)).unwrap();

        assert_eq!(result.vwap, dec!(0.50));
        assert_eq!(result.worst_price, dec!(0.50));
        assert_eq!(result.total_cost, dec!(5.0));
        assert_eq!(result.filled_size, dec!(10));
    }

    #[test]
    fn calculate_fill_price_walks_multiple_levels() {
        let asks = vec![
            PriceLevel::new(dec!(0.48), dec!(5)),
            PriceLevel::new(dec!(0.49), dec!(5)),
            PriceLevel::new(dec!(0.50), dec!(10)),
        ];
        let result = calculate_fill_price(&asks, dec!(10)).unwrap();

        // 5 @ 0.48 + 5 @ 0.49 = 2.40 + 2.45 = 4.85
        assert_eq!(result.total_cost, dec!(4.85));
        assert_eq!(result.vwap, dec!(0.485));
        assert_eq!(result.worst_price, dec!(0.49));
    }

    #[test]
    fn calculate_fill_price_insufficient_liquidity() {
        let asks = vec![PriceLevel::new(dec!(0.50), dec!(5))];
        let result = calculate_fill_price(&asks, dec!(10));

        assert!(matches!(
            result,
            Err(ArbitrageError::InsufficientLiquidity { .. })
        ));
    }

    #[test]
    fn calculate_fill_price_invalid_size() {
        let asks = vec![PriceLevel::new(dec!(0.50), dec!(100))];
        let result = calculate_fill_price(&asks, dec!(0));

        assert!(matches!(result, Err(ArbitrageError::InvalidSize(_))));
    }

    #[test]
    fn calculate_sell_revenue_works() {
        let bids = vec![
            PriceLevel::new(dec!(0.48), dec!(50)),
            PriceLevel::new(dec!(0.47), dec!(50)),
        ];
        let revenue = calculate_sell_revenue(&bids, dec!(75)).unwrap();

        // 50 @ 0.48 + 25 @ 0.47 = 24 + 11.75 = 35.75
        assert_eq!(revenue, dec!(35.75));
    }

    #[test]
    fn merge_levels_adds_new() {
        let existing = vec![PriceLevel::new(dec!(0.50), dec!(100))];
        let updates = vec![PriceLevel::new(dec!(0.51), dec!(50))];
        let merged = merge_levels(&existing, &updates);

        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn merge_levels_removes_zero_size() {
        let existing = vec![
            PriceLevel::new(dec!(0.50), dec!(100)),
            PriceLevel::new(dec!(0.51), dec!(50)),
        ];
        let updates = vec![PriceLevel::new(dec!(0.50), dec!(0))]; // Remove
        let merged = merge_levels(&existing, &updates);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].price, dec!(0.51));
    }

    #[test]
    fn mid_price_calculation() {
        use time::OffsetDateTime;

        let book = OutcomeBook {
            token_id: "test".to_string(),
            outcome: crate::market::Outcome::Up,
            bids: vec![PriceLevel::new(dec!(0.48), dec!(50))],
            asks: vec![PriceLevel::new(dec!(0.52), dec!(50))],
            updated_at: OffsetDateTime::now_utc(),
        };

        assert_eq!(mid_price(&book), Some(dec!(0.50)));
    }
}
