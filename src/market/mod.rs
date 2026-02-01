//! Market module for BTC 15-minute prediction markets.
//!
//! This module handles:
//! - Market types and data structures
//! - Market discovery (finding active markets)
//! - Polymarket API client
//! - Mock client for testing

pub mod client;
pub mod discovery;
pub mod mock;
pub mod types;

pub use client::PolymarketClient;
pub use discovery::{discover_active_market, fetch_market_from_slug};
pub use mock::{MockOrderBook, MockOrderBookBuilder, MockPolymarketClient, MockPosition};
pub use types::{Market, Outcome};
