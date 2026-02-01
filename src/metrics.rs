//! Prometheus metrics for latency tracking and monitoring.
//!
//! This module provides comprehensive metrics for:
//! - Order submission latency
//! - WebSocket message processing latency
//! - Opportunity detection latency
//! - HTTP request latency
//! - Signing operation latency

use std::time::Instant;

use metrics::{counter, describe_counter, describe_histogram, histogram};
use tracing::debug;

/// Initialize all metric descriptions.
/// Call this once at startup to register metrics with descriptions.
pub fn init_metrics() {
    // Latency histograms
    describe_histogram!(
        "order_submit_latency_ms",
        "Order submission latency in milliseconds"
    );
    describe_histogram!(
        "ws_message_latency_ms",
        "WebSocket message processing latency in milliseconds"
    );
    describe_histogram!(
        "opportunity_detection_latency_ms",
        "Time to detect arbitrage opportunity in milliseconds"
    );
    describe_histogram!(
        "http_request_latency_ms",
        "HTTP request latency in milliseconds"
    );
    describe_histogram!(
        "signing_latency_ms",
        "Cryptographic signing latency in milliseconds"
    );
    describe_histogram!(
        "orderbook_fetch_latency_ms",
        "Order book fetch latency in milliseconds"
    );

    // Counters
    describe_counter!(
        "orders_submitted_total",
        "Total number of orders submitted"
    );
    describe_counter!(
        "orders_filled_total",
        "Total number of orders filled"
    );
    describe_counter!(
        "orders_failed_total",
        "Total number of orders that failed"
    );
    describe_counter!(
        "opportunities_detected_total",
        "Total number of arbitrage opportunities detected"
    );
    describe_counter!(
        "opportunities_executed_total",
        "Total number of arbitrage opportunities executed"
    );
    describe_counter!(
        "ws_messages_received_total",
        "Total number of WebSocket messages received"
    );
    describe_counter!(
        "ws_reconnects_total",
        "Total number of WebSocket reconnections"
    );

    debug!("Metrics initialized");
}

/// Record order submission latency.
pub fn record_order_submit_latency(start: Instant) {
    let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
    histogram!("order_submit_latency_ms").record(latency_ms);
}

/// Record WebSocket message processing latency.
pub fn record_ws_message_latency(start: Instant) {
    let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
    histogram!("ws_message_latency_ms").record(latency_ms);
}

/// Record opportunity detection latency.
pub fn record_opportunity_detection_latency(start: Instant) {
    let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
    histogram!("opportunity_detection_latency_ms").record(latency_ms);
}

/// Record HTTP request latency.
pub fn record_http_latency(start: Instant, endpoint: &str) {
    let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
    histogram!("http_request_latency_ms", "endpoint" => endpoint.to_string()).record(latency_ms);
}

/// Record signing operation latency.
pub fn record_signing_latency(start: Instant) {
    let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
    histogram!("signing_latency_ms").record(latency_ms);
}

/// Record order book fetch latency.
pub fn record_orderbook_fetch_latency(start: Instant, token_id: &str) {
    let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
    histogram!("orderbook_fetch_latency_ms", "token_id" => token_id.to_string()).record(latency_ms);
}

/// Increment order submitted counter.
pub fn inc_orders_submitted() {
    counter!("orders_submitted_total").increment(1);
}

/// Increment orders filled counter.
pub fn inc_orders_filled() {
    counter!("orders_filled_total").increment(1);
}

/// Increment orders failed counter.
pub fn inc_orders_failed() {
    counter!("orders_failed_total").increment(1);
}

/// Increment opportunities detected counter.
pub fn inc_opportunities_detected() {
    counter!("opportunities_detected_total").increment(1);
}

/// Increment opportunities executed counter.
pub fn inc_opportunities_executed() {
    counter!("opportunities_executed_total").increment(1);
}

/// Increment WebSocket messages received counter.
pub fn inc_ws_messages_received() {
    counter!("ws_messages_received_total").increment(1);
}

/// Increment WebSocket reconnects counter.
pub fn inc_ws_reconnects() {
    counter!("ws_reconnects_total").increment(1);
}

/// RAII guard for timing operations.
/// Automatically records latency when dropped.
pub struct LatencyTimer {
    start: Instant,
    metric_name: &'static str,
}

impl LatencyTimer {
    /// Create a new latency timer for the given metric.
    pub fn new(metric_name: &'static str) -> Self {
        Self {
            start: Instant::now(),
            metric_name,
        }
    }

    /// Get elapsed time in milliseconds (without recording).
    pub fn elapsed_ms(&self) -> f64 {
        self.start.elapsed().as_secs_f64() * 1000.0
    }
}

impl Drop for LatencyTimer {
    fn drop(&mut self) {
        let latency_ms = self.start.elapsed().as_secs_f64() * 1000.0;
        histogram!(self.metric_name).record(latency_ms);
    }
}

/// Create a latency timer for order submission.
pub fn timer_order_submit() -> LatencyTimer {
    LatencyTimer::new("order_submit_latency_ms")
}

/// Create a latency timer for WebSocket message processing.
pub fn timer_ws_message() -> LatencyTimer {
    LatencyTimer::new("ws_message_latency_ms")
}

/// Create a latency timer for opportunity detection.
pub fn timer_opportunity_detection() -> LatencyTimer {
    LatencyTimer::new("opportunity_detection_latency_ms")
}

/// Create a latency timer for signing operations.
pub fn timer_signing() -> LatencyTimer {
    LatencyTimer::new("signing_latency_ms")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn latency_timer_measures_time() {
        let timer = LatencyTimer::new("test_metric");
        sleep(Duration::from_millis(10));
        let elapsed = timer.elapsed_ms();
        assert!(elapsed >= 9.0); // Allow some tolerance
        // Timer will record on drop
    }
}
