//! HTTP API handlers.

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use rust_decimal::Decimal;
use serde::Serialize;
use std::sync::Arc;

use crate::arbitrage::ExecutorStats;

/// Application state shared with handlers.
#[derive(Debug, Clone)]
pub struct AppState {
    /// Whether the bot is ready to trade.
    pub ready: Arc<std::sync::atomic::AtomicBool>,
    /// Current market slug.
    pub market_slug: Arc<tokio::sync::RwLock<Option<String>>>,
    /// Executor stats.
    pub stats: Arc<tokio::sync::RwLock<ExecutorStats>>,
}

impl AppState {
    /// Create new app state.
    pub fn new() -> Self {
        Self {
            ready: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            market_slug: Arc::new(tokio::sync::RwLock::new(None)),
            stats: Arc::new(tokio::sync::RwLock::new(ExecutorStats {
                trades_executed: 0,
                opportunities_found: 0,
                total_invested: Decimal::ZERO,
                total_shares_bought: Decimal::ZERO,
                sim_balance: Decimal::ZERO,
                sim_start_balance: Decimal::ZERO,
            })),
        }
    }

    /// Set ready state.
    pub fn set_ready(&self, ready: bool) {
        self.ready
            .store(ready, std::sync::atomic::Ordering::SeqCst);
    }

    /// Check if ready.
    pub fn is_ready(&self) -> bool {
        self.ready.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Health check response.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    /// Status: "ok".
    pub status: &'static str,
}

/// Readiness check response.
#[derive(Debug, Serialize)]
pub struct ReadyResponse {
    /// Whether service is ready.
    pub ready: bool,
    /// Current market slug if available.
    pub market: Option<String>,
}

/// Status response.
#[derive(Debug, Serialize)]
pub struct StatusResponse {
    /// Service status.
    pub status: &'static str,
    /// Current market slug.
    pub market: Option<String>,
    /// Statistics.
    pub stats: StatsResponse,
}

/// Statistics in status response.
#[derive(Debug, Serialize)]
pub struct StatsResponse {
    /// Trades executed.
    pub trades_executed: u64,
    /// Opportunities found.
    pub opportunities_found: u64,
    /// Total invested.
    pub total_invested: String,
    /// Total shares bought.
    pub total_shares_bought: String,
}

/// Health check handler - always returns 200.
pub async fn health() -> impl IntoResponse {
    Json(HealthResponse { status: "ok" })
}

/// Readiness check handler - returns 200 if ready, 503 otherwise.
pub async fn ready(State(state): State<AppState>) -> impl IntoResponse {
    let is_ready = state.is_ready();
    let market = state.market_slug.read().await.clone();

    let response = ReadyResponse {
        ready: is_ready,
        market,
    };

    if is_ready {
        (StatusCode::OK, Json(response))
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(response))
    }
}

/// Status handler - returns bot status and statistics.
pub async fn status(State(state): State<AppState>) -> impl IntoResponse {
    let market = state.market_slug.read().await.clone();
    let stats = state.stats.read().await;

    let status = if state.is_ready() { "running" } else { "starting" };

    Json(StatusResponse {
        status,
        market,
        stats: StatsResponse {
            trades_executed: stats.trades_executed,
            opportunities_found: stats.opportunities_found,
            total_invested: stats.total_invested.to_string(),
            total_shares_bought: stats.total_shares_bought.to_string(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_state_ready_toggle() {
        let state = AppState::new();
        assert!(!state.is_ready());

        state.set_ready(true);
        assert!(state.is_ready());

        state.set_ready(false);
        assert!(!state.is_ready());
    }
}
