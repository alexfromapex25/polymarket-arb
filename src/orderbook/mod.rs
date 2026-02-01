//! Order book module for managing market data.
//!
//! This module handles:
//! - Order book types and data structures
//! - Fill price calculations and book aggregation
//! - WebSocket connection for real-time updates

pub mod aggregator;
pub mod types;
pub mod websocket;

pub use aggregator::{calculate_fill_price, mid_price};
pub use types::{BookUpdate, FillInfo, OutcomeBook, PriceLevel, WsEventType};
pub use websocket::{L2BookState, MarketWebSocket};
