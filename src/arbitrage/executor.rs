//! Arbitrage trade execution logic.

use std::time::Instant;

use rust_decimal::Decimal;
use tracing::{error, info, instrument, warn};

use super::calculator::ArbitrageOpportunity;
use crate::config::Config;
use crate::error::TradingError;
use crate::market::{Outcome, PolymarketClient};
use crate::trading::execution::{
    cancel_orders, submit_order, wait_for_terminal_order, DEFAULT_ORDER_TIMEOUT,
    DEFAULT_POLL_INTERVAL,
};
use crate::trading::order::{OrderParams, Side, TimeInForce};

/// Result of attempting to execute an arbitrage.
#[derive(Debug)]
pub enum ExecutionResult {
    /// Both legs filled successfully.
    BothFilled {
        /// UP order ID.
        up_order_id: String,
        /// DOWN order ID.
        down_order_id: String,
        /// UP filled size.
        up_filled_size: Decimal,
        /// DOWN filled size: Decimal,
        down_filled_size: Decimal,
        /// Actual investment.
        actual_investment: Decimal,
    },
    /// Only one leg filled; attempted unwind.
    PartialFill {
        /// Which leg filled.
        filled_leg: Outcome,
        /// Filled size.
        filled_size: Decimal,
        /// Whether unwind was attempted.
        unwind_attempted: bool,
        /// Unwind result.
        unwind_result: Option<String>,
    },
    /// Neither leg filled.
    NeitherFilled,
    /// Simulation mode - no real orders.
    Simulated {
        /// Would invest this amount.
        would_invest: Decimal,
        /// Would profit this amount.
        would_profit: Decimal,
    },
    /// Skipped due to cooldown.
    CooldownActive {
        /// Remaining seconds.
        remaining_seconds: u64,
    },
    /// Skipped due to insufficient balance.
    InsufficientBalance {
        /// Required balance.
        required: Decimal,
        /// Available balance.
        available: Decimal,
    },
}

/// Executor state for tracking cooldowns and stats.
#[derive(Debug)]
pub struct ArbitrageExecutor {
    /// Last execution timestamp.
    last_execution: Option<Instant>,
    /// Cooldown duration in seconds.
    cooldown_seconds: u64,
    /// Total trades executed.
    pub trades_executed: u64,
    /// Total opportunities found.
    pub opportunities_found: u64,
    /// Total investment.
    pub total_invested: Decimal,
    /// Total shares bought.
    pub total_shares_bought: Decimal,
    /// Simulation balance.
    pub sim_balance: Decimal,
    /// Starting simulation balance.
    pub sim_start_balance: Decimal,
}

impl ArbitrageExecutor {
    /// Create a new executor from config.
    pub fn new(config: &Config) -> Self {
        Self {
            last_execution: None,
            cooldown_seconds: config.cooldown_seconds,
            trades_executed: 0,
            opportunities_found: 0,
            total_invested: Decimal::ZERO,
            total_shares_bought: Decimal::ZERO,
            sim_balance: config.sim_balance,
            sim_start_balance: config.sim_balance,
        }
    }

    /// Check if cooldown is active.
    pub fn is_cooldown_active(&self) -> bool {
        if let Some(last) = self.last_execution {
            last.elapsed().as_secs() < self.cooldown_seconds
        } else {
            false
        }
    }

    /// Get remaining cooldown seconds.
    pub fn cooldown_remaining(&self) -> u64 {
        if let Some(last) = self.last_execution {
            let elapsed = last.elapsed().as_secs();
            if elapsed < self.cooldown_seconds {
                self.cooldown_seconds - elapsed
            } else {
                0
            }
        } else {
            0
        }
    }

    /// Execute an arbitrage opportunity.
    #[instrument(skip(self, client, config), fields(market = %opportunity.market.slug))]
    pub async fn execute(
        &mut self,
        client: &PolymarketClient,
        opportunity: &ArbitrageOpportunity,
        config: &Config,
    ) -> Result<ExecutionResult, TradingError> {
        // Track opportunity
        self.opportunities_found += 1;

        // Check cooldown
        if self.is_cooldown_active() {
            let remaining = self.cooldown_remaining();
            info!(remaining_seconds = remaining, "Cooldown active, skipping");
            return Ok(ExecutionResult::CooldownActive {
                remaining_seconds: remaining,
            });
        }

        // Update last execution time
        self.last_execution = Some(Instant::now());

        // Log opportunity details
        self.log_opportunity(opportunity);

        // Handle simulation mode
        if config.dry_run {
            return self.execute_simulated(opportunity);
        }

        // Check balance
        let required = opportunity.total_investment * config.balance_margin;
        let balance = client.get_balance().await?;

        if balance < required {
            warn!(
                required = %required,
                available = %balance,
                "Insufficient balance"
            );
            return Ok(ExecutionResult::InsufficientBalance {
                required,
                available: balance,
            });
        }

        // Execute real trades
        self.execute_real(client, opportunity, config).await
    }

    /// Execute in simulation mode.
    fn execute_simulated(
        &mut self,
        opportunity: &ArbitrageOpportunity,
    ) -> Result<ExecutionResult, TradingError> {
        info!("SIMULATION MODE - No real orders will be executed");

        // Check simulated balance
        if self.sim_balance < opportunity.total_investment {
            error!(
                required = %opportunity.total_investment,
                available = %self.sim_balance,
                "Insufficient simulated balance"
            );
            return Ok(ExecutionResult::InsufficientBalance {
                required: opportunity.total_investment,
                available: self.sim_balance,
            });
        }

        // Deduct from simulated balance
        self.sim_balance -= opportunity.total_investment;
        self.total_invested += opportunity.total_investment;
        self.total_shares_bought += opportunity.order_size * Decimal::TWO;
        self.trades_executed += 1;

        info!(
            sim_balance = %self.sim_balance,
            deducted = %opportunity.total_investment,
            "Simulated trade executed"
        );

        Ok(ExecutionResult::Simulated {
            would_invest: opportunity.total_investment,
            would_profit: opportunity.expected_profit,
        })
    }

    /// Execute real trades with concurrent order submission.
    async fn execute_real(
        &mut self,
        client: &PolymarketClient,
        opportunity: &ArbitrageOpportunity,
        config: &Config,
    ) -> Result<ExecutionResult, TradingError> {
        info!("Executing REAL arbitrage trade");

        // Parse time-in-force from config
        let tif = match config.order_type.to_uppercase().as_str() {
            "FOK" => TimeInForce::FOK,
            "FAK" => TimeInForce::FAK,
            "GTC" => TimeInForce::GTC,
            _ => TimeInForce::FOK,
        };

        // Create UP order parameters
        let up_params = OrderParams {
            token_id: opportunity.market.up_token_id.clone(),
            side: Side::Buy,
            price: opportunity.up_price,
            size: opportunity.order_size,
            tif,
        };

        // Create DOWN order parameters
        let down_params = OrderParams {
            token_id: opportunity.market.down_token_id.clone(),
            side: Side::Buy,
            price: opportunity.down_price,
            size: opportunity.order_size,
            tif,
        };

        info!(
            up_token = %opportunity.market.up_token_id,
            down_token = %opportunity.market.down_token_id,
            up_price = %opportunity.up_price,
            down_price = %opportunity.down_price,
            size = %opportunity.order_size,
            "Submitting paired orders"
        );

        // Submit both orders concurrently
        let (up_result, down_result) = tokio::join!(
            submit_order(client, &up_params),
            submit_order(client, &down_params),
        );

        // Analyze results
        match (up_result, down_result) {
            // Both orders submitted successfully
            (Ok(up_order_id), Ok(down_order_id)) => {
                info!(
                    up_order_id = %up_order_id,
                    down_order_id = %down_order_id,
                    "Both orders submitted, waiting for fills"
                );

                // Wait for both orders to reach terminal state
                let (up_state, down_state) = tokio::join!(
                    wait_for_terminal_order(
                        client,
                        &up_order_id,
                        opportunity.order_size,
                        DEFAULT_ORDER_TIMEOUT,
                        DEFAULT_POLL_INTERVAL,
                    ),
                    wait_for_terminal_order(
                        client,
                        &down_order_id,
                        opportunity.order_size,
                        DEFAULT_ORDER_TIMEOUT,
                        DEFAULT_POLL_INTERVAL,
                    ),
                );

                // Check fill status
                let up_filled = up_state.is_filled;
                let down_filled = down_state.is_filled;

                match (up_filled, down_filled) {
                    // Both filled - success!
                    (true, true) => {
                        let up_filled_size = up_state.filled_size.unwrap_or(opportunity.order_size);
                        let down_filled_size =
                            down_state.filled_size.unwrap_or(opportunity.order_size);

                        // Calculate actual investment
                        let actual_investment =
                            (up_filled_size * opportunity.up_price) +
                            (down_filled_size * opportunity.down_price);

                        self.trades_executed += 1;
                        self.total_invested += actual_investment;
                        self.total_shares_bought += up_filled_size + down_filled_size;

                        info!(
                            up_filled = %up_filled_size,
                            down_filled = %down_filled_size,
                            investment = %actual_investment,
                            "ARBITRAGE EXECUTED SUCCESSFULLY"
                        );

                        Ok(ExecutionResult::BothFilled {
                            up_order_id,
                            down_order_id,
                            up_filled_size,
                            down_filled_size,
                            actual_investment,
                        })
                    }
                    // Only UP filled - need to handle partial
                    (true, false) => {
                        let filled_size = up_state.filled_size.unwrap_or(opportunity.order_size);
                        warn!(
                            up_filled = %filled_size,
                            "Partial fill: only UP leg filled"
                        );

                        // Cancel the DOWN order if still open
                        let _ = cancel_orders(client, &[down_order_id]).await;

                        // Attempt to unwind by selling the UP position
                        let unwind_result =
                            self.attempt_unwind(client, Outcome::Up, &opportunity.market.up_token_id, filled_size, config)
                                .await;

                        Ok(ExecutionResult::PartialFill {
                            filled_leg: Outcome::Up,
                            filled_size,
                            unwind_attempted: true,
                            unwind_result,
                        })
                    }
                    // Only DOWN filled - need to handle partial
                    (false, true) => {
                        let filled_size = down_state.filled_size.unwrap_or(opportunity.order_size);
                        warn!(
                            down_filled = %filled_size,
                            "Partial fill: only DOWN leg filled"
                        );

                        // Cancel the UP order if still open
                        let _ = cancel_orders(client, &[up_order_id]).await;

                        // Attempt to unwind by selling the DOWN position
                        let unwind_result = self
                            .attempt_unwind(
                                client,
                                Outcome::Down,
                                &opportunity.market.down_token_id,
                                filled_size,
                                config,
                            )
                            .await;

                        Ok(ExecutionResult::PartialFill {
                            filled_leg: Outcome::Down,
                            filled_size,
                            unwind_attempted: true,
                            unwind_result,
                        })
                    }
                    // Neither filled
                    (false, false) => {
                        warn!("Neither order filled");

                        // Cancel any remaining orders
                        let _ = cancel_orders(client, &[up_order_id, down_order_id]).await;

                        Ok(ExecutionResult::NeitherFilled)
                    }
                }
            }
            // Only UP order submitted
            (Ok(up_order_id), Err(down_err)) => {
                error!(error = %down_err, "DOWN order submission failed");

                // Cancel the UP order
                let _ = cancel_orders(client, &[up_order_id]).await;

                Ok(ExecutionResult::NeitherFilled)
            }
            // Only DOWN order submitted
            (Err(up_err), Ok(down_order_id)) => {
                error!(error = %up_err, "UP order submission failed");

                // Cancel the DOWN order
                let _ = cancel_orders(client, &[down_order_id]).await;

                Ok(ExecutionResult::NeitherFilled)
            }
            // Both failed
            (Err(up_err), Err(down_err)) => {
                error!(
                    up_error = %up_err,
                    down_error = %down_err,
                    "Both orders failed to submit"
                );

                Ok(ExecutionResult::NeitherFilled)
            }
        }
    }

    /// Attempt to unwind a partial fill by selling the filled position.
    async fn attempt_unwind(
        &self,
        client: &PolymarketClient,
        outcome: Outcome,
        token_id: &str,
        size: Decimal,
        _config: &Config,
    ) -> Option<String> {
        info!(
            outcome = ?outcome,
            token_id = %token_id,
            size = %size,
            "Attempting to unwind partial fill"
        );

        // Get the current order book to find a sell price
        match client.get_order_book(token_id).await {
            Ok(book) => {
                // Use best bid as sell price (minus small buffer for fill probability)
                if let Some(best_bid) = book.best_bid() {
                    let sell_price = best_bid - Decimal::new(1, 2); // $0.01 below best bid

                    let sell_params = OrderParams {
                        token_id: token_id.to_string(),
                        side: Side::Sell,
                        price: sell_price,
                        size,
                        tif: TimeInForce::GTC, // Use GTC for unwind
                    };

                    match submit_order(client, &sell_params).await {
                        Ok(order_id) => {
                            info!(
                                order_id = %order_id,
                                price = %sell_price,
                                "Unwind sell order submitted"
                            );
                            Some(format!("Unwind order submitted: {}", order_id))
                        }
                        Err(e) => {
                            error!(error = %e, "Failed to submit unwind order");
                            Some(format!("Unwind failed: {}", e))
                        }
                    }
                } else {
                    warn!("No bids available for unwind");
                    Some("No bids available for unwind".to_string())
                }
            }
            Err(e) => {
                error!(error = %e, "Failed to get order book for unwind");
                Some(format!("Failed to get order book: {}", e))
            }
        }
    }

    /// Log opportunity details.
    fn log_opportunity(&self, opportunity: &ArbitrageOpportunity) {
        info!("========================================");
        info!("ARBITRAGE OPPORTUNITY DETECTED");
        info!("========================================");
        info!("UP limit price:       ${}", opportunity.up_price);
        info!("DOWN limit price:     ${}", opportunity.down_price);
        if let (Some(vwap_up), Some(vwap_down)) = (opportunity.vwap_up, opportunity.vwap_down) {
            info!("UP VWAP (est):        ${}", vwap_up);
            info!("DOWN VWAP (est):      ${}", vwap_down);
        }
        info!("Total cost:           ${}", opportunity.total_cost);
        info!("Profit per share:     ${}", opportunity.profit_per_share);
        info!("Profit %:             {}%", opportunity.profit_pct);
        info!("----------------------------------------");
        info!("Order size:           {} shares each side", opportunity.order_size);
        info!("Total investment:     ${}", opportunity.total_investment);
        info!("Expected payout:      ${}", opportunity.expected_payout);
        info!("EXPECTED PROFIT:      ${}", opportunity.expected_profit);
        info!("========================================");
    }

    /// Get statistics summary.
    pub fn stats(&self) -> ExecutorStats {
        ExecutorStats {
            trades_executed: self.trades_executed,
            opportunities_found: self.opportunities_found,
            total_invested: self.total_invested,
            total_shares_bought: self.total_shares_bought,
            sim_balance: self.sim_balance,
            sim_start_balance: self.sim_start_balance,
        }
    }
}

/// Executor statistics.
#[derive(Debug, Clone)]
pub struct ExecutorStats {
    /// Total trades executed.
    pub trades_executed: u64,
    /// Total opportunities found.
    pub opportunities_found: u64,
    /// Total investment.
    pub total_invested: Decimal,
    /// Total shares bought.
    pub total_shares_bought: Decimal,
    /// Current simulation balance.
    pub sim_balance: Decimal,
    /// Starting simulation balance.
    pub sim_start_balance: Decimal,
}

impl ExecutorStats {
    /// Calculate expected profit at settlement.
    pub fn expected_profit(&self) -> Decimal {
        // Each pair of shares (UP + DOWN) pays $1.00 at settlement
        let pairs = self.total_shares_bought / Decimal::TWO;
        pairs - self.total_invested
    }

    /// Calculate simulation ending balance (after claiming).
    pub fn sim_ending_balance(&self) -> Decimal {
        let pairs = self.total_shares_bought / Decimal::TWO;
        self.sim_balance + pairs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::Market;
    use rust_decimal_macros::dec;
    use time::OffsetDateTime;

    fn test_config() -> Config {
        Config {
            polymarket_private_key: "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
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

    fn test_opportunity() -> ArbitrageOpportunity {
        ArbitrageOpportunity {
            market: Market {
                slug: "btc-updown-15m-123".to_string(),
                id: "market-id".to_string(),
                up_token_id: "up-token".to_string(),
                down_token_id: "down-token".to_string(),
                start_timestamp: 0,
                end_timestamp: 900,
                question: None,
            },
            up_price: dec!(0.48),
            down_price: dec!(0.51),
            total_cost: dec!(0.99),
            profit_per_share: dec!(0.01),
            profit_pct: dec!(1.0101),
            order_size: dec!(10),
            total_investment: dec!(9.9),
            expected_payout: dec!(10),
            expected_profit: dec!(0.1),
            best_ask_up: Some(dec!(0.48)),
            best_ask_down: Some(dec!(0.51)),
            vwap_up: Some(dec!(0.48)),
            vwap_down: Some(dec!(0.51)),
            detected_at: OffsetDateTime::now_utc(),
        }
    }

    #[test]
    fn executor_creation() {
        let config = test_config();
        let executor = ArbitrageExecutor::new(&config);

        assert_eq!(executor.trades_executed, 0);
        assert_eq!(executor.opportunities_found, 0);
        assert_eq!(executor.sim_balance, dec!(100));
    }

    #[test]
    fn cooldown_tracking() {
        let config = test_config();
        let mut executor = ArbitrageExecutor::new(&config);

        assert!(!executor.is_cooldown_active());

        executor.last_execution = Some(Instant::now());
        assert!(executor.is_cooldown_active());
    }

    #[test]
    fn stats_expected_profit() {
        let stats = ExecutorStats {
            trades_executed: 3,
            opportunities_found: 5,
            total_invested: dec!(29.7), // 3 trades * 9.9
            total_shares_bought: dec!(60), // 3 trades * 20 shares
            sim_balance: dec!(70.3),
            sim_start_balance: dec!(100),
        };

        // 30 pairs * $1 - $29.7 = $0.3 profit
        assert_eq!(stats.expected_profit(), dec!(0.3));
    }
}
