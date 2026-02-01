//! BTC 15-minute Polymarket arbitrage bot.
//!
//! This library implements Jeremy Whittaker's strategy: buying both UP and DOWN
//! outcomes when their combined cost is < $1.00, guaranteeing profit regardless
//! of outcome.
//!
//! # Strategy
//!
//! At market close, ONE side pays $1.00 per share. If total cost < $1.00,
//! profit is guaranteed:
//!
//! ```text
//! BTC UP price:   $0.48
//! BTC DOWN price: $0.51
//! ─────────────────────
//! Total:          $0.99 < $1.00 ✅
//! Profit:         $0.01 per share (1.01% guaranteed)
//! ```
//!
//! # Modules
//!
//! - [`config`]: Configuration loading from environment
//! - [`error`]: Unified error types
//! - [`market`]: Market discovery and Polymarket client
//! - [`orderbook`]: Order book management and calculations
//! - [`arbitrage`]: Opportunity detection and execution
//! - [`trading`]: Order types and position tracking
//! - [`api`]: HTTP API for health/metrics
//! - [`utils`]: Utility functions

pub mod api;
pub mod arbitrage;
pub mod config;
pub mod error;
pub mod market;
pub mod orderbook;
pub mod signing;
pub mod trading;
pub mod utils;

pub use config::Config;
pub use error::{BotError, Result};
