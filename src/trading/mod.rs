//! Trading module for order management and execution.
//!
//! This module handles:
//! - Order types and creation
//! - Order submission and execution
//! - Position tracking

pub mod execution;
pub mod order;
pub mod position;

pub use execution::{cancel_orders, submit_order, submit_orders_fast, wait_for_terminal_order};
pub use order::{OrderParams, OrderState, OrderStatus, Side, TimeInForce};
pub use position::{MarketPositions, Position};
