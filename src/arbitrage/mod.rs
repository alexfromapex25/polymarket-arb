//! Arbitrage module for detecting and executing opportunities.
//!
//! This module handles:
//! - Opportunity detection from order books
//! - Profit/cost calculations
//! - Trade execution with verification

pub mod calculator;
pub mod detector;
pub mod executor;

pub use calculator::{calculate_opportunity, ArbitrageOpportunity};
pub use detector::{check_arbitrage, diagnose_no_opportunity, quick_opportunity_check};
pub use executor::{ArbitrageExecutor, ExecutionResult, ExecutorStats};
