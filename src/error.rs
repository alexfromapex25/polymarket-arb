//! Unified error types for the arbitrage bot.

use rust_decimal::Decimal;
use thiserror::Error;

use crate::market::Outcome;

/// Unified error type for the arbitrage bot.
#[derive(Error, Debug)]
pub enum BotError {
    /// Configuration loading error.
    #[error("configuration error: {0}")]
    Config(#[from] envy::Error),

    /// Market-related error.
    #[error("market error: {0}")]
    Market(#[from] MarketError),

    /// Arbitrage detection/execution error.
    #[error("arbitrage error: {0}")]
    Arbitrage(#[from] ArbitrageError),

    /// Trading/order error.
    #[error("trading error: {0}")]
    Trading(#[from] TradingError),

    /// WebSocket error.
    #[error("websocket error: {0}")]
    WebSocket(#[from] WsError),

    /// HTTP request error.
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON parsing error.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// IO error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Market discovery and management errors.
#[derive(Error, Debug)]
pub enum MarketError {
    /// No active BTC 15-minute market could be found.
    #[error("no active BTC 15min market found")]
    NoActiveMarketFound,

    /// Failed to fetch market information.
    #[error("failed to fetch market {slug}: {reason}")]
    FetchFailed {
        /// The market slug that failed.
        slug: String,
        /// Reason for failure.
        reason: String,
    },

    /// Market is closed.
    #[error("market {slug} is closed")]
    MarketClosed {
        /// The closed market slug.
        slug: String,
    },

    /// Failed to parse market data.
    #[error("failed to parse market data: {0}")]
    ParseError(String),

    /// HTTP request failed.
    #[error("http request failed: {0}")]
    HttpError(#[from] reqwest::Error),
}

/// Arbitrage detection and calculation errors.
#[derive(Error, Debug)]
pub enum ArbitrageError {
    /// Not enough liquidity in the order book.
    #[error("insufficient liquidity: need {required}, available {available}")]
    InsufficientLiquidity {
        /// Required size.
        required: Decimal,
        /// Available size.
        available: Decimal,
    },

    /// Account balance too low.
    #[error("insufficient balance: need {required}, have {available}")]
    InsufficientBalance {
        /// Required balance.
        required: Decimal,
        /// Available balance.
        available: Decimal,
    },

    /// Invalid order size.
    #[error("invalid order size: {0}")]
    InvalidSize(Decimal),

    /// Cooldown period active.
    #[error("cooldown active: {remaining_seconds}s remaining")]
    CooldownActive {
        /// Seconds remaining in cooldown.
        remaining_seconds: u64,
    },

    /// No arbitrage opportunity exists.
    #[error("no arbitrage opportunity: total cost {total_cost} >= threshold {threshold}")]
    NoOpportunity {
        /// Actual total cost.
        total_cost: Decimal,
        /// Required threshold.
        threshold: Decimal,
    },

    /// Order book is inverted (asks < bids).
    #[error("order book inverted for {side}: best_ask={best_ask} < best_bid={best_bid}")]
    BookInverted {
        /// Which side is inverted.
        side: String,
        /// Best ask price.
        best_ask: Decimal,
        /// Best bid price.
        best_bid: Decimal,
    },
}

/// Trading and order execution errors.
#[derive(Error, Debug)]
pub enum TradingError {
    /// Order submission failed.
    #[error("order submission failed: {0}")]
    SubmissionFailed(String),

    /// Order not filled within timeout.
    #[error("order {order_id} not filled within timeout")]
    FillTimeout {
        /// The order ID that timed out.
        order_id: String,
    },

    /// Only one leg of the arbitrage filled.
    #[error("paired execution failed: only {filled_leg:?} filled")]
    PartialExecution {
        /// Which leg filled.
        filled_leg: Outcome,
    },

    /// Failed to cancel order.
    #[error("failed to cancel order {order_id}: {reason}")]
    CancelFailed {
        /// Order ID that failed to cancel.
        order_id: String,
        /// Reason for failure.
        reason: String,
    },

    /// Failed to get order status.
    #[error("failed to get order status for {order_id}: {reason}")]
    StatusFailed {
        /// Order ID.
        order_id: String,
        /// Reason for failure.
        reason: String,
    },

    /// Invalid order parameters.
    #[error("invalid order parameters: {0}")]
    InvalidParams(String),

    /// Signing error.
    #[error("signing error: {0}")]
    SigningError(String),

    /// Authentication failed.
    #[error("authentication failed: {0}")]
    AuthenticationFailed(String),

    /// Rate limited by the API.
    #[error("rate limited: retry after {retry_after_seconds}s")]
    RateLimited {
        /// Seconds to wait before retrying.
        retry_after_seconds: u64,
    },

    /// Order rejected by the exchange.
    #[error("order rejected: {reason}")]
    OrderRejected {
        /// Rejection reason from the exchange.
        reason: String,
    },

    /// Insufficient funds for the order.
    #[error("insufficient funds: need {required}, have {available}")]
    InsufficientFunds {
        /// Required amount.
        required: rust_decimal::Decimal,
        /// Available amount.
        available: rust_decimal::Decimal,
    },
}

/// WebSocket connection and message errors.
#[derive(Error, Debug)]
pub enum WsError {
    /// Connection failed.
    #[error("websocket connection failed: {0}")]
    ConnectionFailed(String),

    /// Connection closed unexpectedly.
    #[error("websocket connection closed: code={code:?}, reason={reason}")]
    ConnectionClosed {
        /// Close code.
        code: Option<u16>,
        /// Close reason.
        reason: String,
    },

    /// Message parsing failed.
    #[error("failed to parse websocket message: {0}")]
    ParseError(String),

    /// Send failed.
    #[error("failed to send websocket message: {0}")]
    SendFailed(String),

    /// Tungstenite error.
    #[error("tungstenite error: {0}")]
    Tungstenite(#[from] tokio_tungstenite::tungstenite::Error),
}

/// Convenient Result type alias.
pub type Result<T> = std::result::Result<T, BotError>;
