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

// === Metric Name Constants ===

/// Order submission latency metric name.
pub const METRIC_ORDER_SUBMIT_LATENCY: &str = "order_submit_latency_ms";
/// WebSocket message latency metric name.
pub const METRIC_WS_MESSAGE_LATENCY: &str = "ws_message_latency_ms";
/// Opportunity detection latency metric name.
pub const METRIC_OPPORTUNITY_DETECTION_LATENCY: &str = "opportunity_detection_latency_ms";
/// HTTP request latency metric name.
pub const METRIC_HTTP_REQUEST_LATENCY: &str = "http_request_latency_ms";
/// Signing latency metric name.
pub const METRIC_SIGNING_LATENCY: &str = "signing_latency_ms";
/// Order book fetch latency metric name.
pub const METRIC_ORDERBOOK_FETCH_LATENCY: &str = "orderbook_fetch_latency_ms";
/// Orders submitted counter metric name.
pub const METRIC_ORDERS_SUBMITTED: &str = "orders_submitted_total";
/// Orders filled counter metric name.
pub const METRIC_ORDERS_FILLED: &str = "orders_filled_total";
/// Orders failed counter metric name.
pub const METRIC_ORDERS_FAILED: &str = "orders_failed_total";
/// Opportunities detected counter metric name.
pub const METRIC_OPPORTUNITIES_DETECTED: &str = "opportunities_detected_total";
/// Opportunities executed counter metric name.
pub const METRIC_OPPORTUNITIES_EXECUTED: &str = "opportunities_executed_total";
/// WebSocket messages received counter metric name.
pub const METRIC_WS_MESSAGES_RECEIVED: &str = "ws_messages_received_total";
/// WebSocket reconnects counter metric name.
pub const METRIC_WS_RECONNECTS: &str = "ws_reconnects_total";

/// Initialize all metric descriptions.
/// Call this once at startup to register metrics with descriptions.
pub fn init_metrics() {
    // Latency histograms
    describe_histogram!(
        METRIC_ORDER_SUBMIT_LATENCY,
        "Order submission latency in milliseconds"
    );
    describe_histogram!(
        METRIC_WS_MESSAGE_LATENCY,
        "WebSocket message processing latency in milliseconds"
    );
    describe_histogram!(
        METRIC_OPPORTUNITY_DETECTION_LATENCY,
        "Time to detect arbitrage opportunity in milliseconds"
    );
    describe_histogram!(
        METRIC_HTTP_REQUEST_LATENCY,
        "HTTP request latency in milliseconds"
    );
    describe_histogram!(
        METRIC_SIGNING_LATENCY,
        "Cryptographic signing latency in milliseconds"
    );
    describe_histogram!(
        METRIC_ORDERBOOK_FETCH_LATENCY,
        "Order book fetch latency in milliseconds"
    );

    // Counters
    describe_counter!(
        METRIC_ORDERS_SUBMITTED,
        "Total number of orders submitted"
    );
    describe_counter!(
        METRIC_ORDERS_FILLED,
        "Total number of orders filled"
    );
    describe_counter!(
        METRIC_ORDERS_FAILED,
        "Total number of orders that failed"
    );
    describe_counter!(
        METRIC_OPPORTUNITIES_DETECTED,
        "Total number of arbitrage opportunities detected"
    );
    describe_counter!(
        METRIC_OPPORTUNITIES_EXECUTED,
        "Total number of arbitrage opportunities executed"
    );
    describe_counter!(
        METRIC_WS_MESSAGES_RECEIVED,
        "Total number of WebSocket messages received"
    );
    describe_counter!(
        METRIC_WS_RECONNECTS,
        "Total number of WebSocket reconnections"
    );

    debug!("Metrics initialized");
}

/// Record order submission latency.
pub fn record_order_submit_latency(start: Instant) {
    let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
    histogram!(METRIC_ORDER_SUBMIT_LATENCY).record(latency_ms);
}

/// Record WebSocket message processing latency.
pub fn record_ws_message_latency(start: Instant) {
    let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
    histogram!(METRIC_WS_MESSAGE_LATENCY).record(latency_ms);
}

/// Record opportunity detection latency.
pub fn record_opportunity_detection_latency(start: Instant) {
    let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
    histogram!(METRIC_OPPORTUNITY_DETECTION_LATENCY).record(latency_ms);
}

/// Record HTTP request latency.
pub fn record_http_latency(start: Instant, endpoint: &str) {
    let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
    histogram!(METRIC_HTTP_REQUEST_LATENCY, "endpoint" => endpoint.to_string()).record(latency_ms);
}

/// Record signing operation latency.
pub fn record_signing_latency(start: Instant) {
    let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
    histogram!(METRIC_SIGNING_LATENCY).record(latency_ms);
}

/// Record order book fetch latency.
pub fn record_orderbook_fetch_latency(start: Instant, token_id: &str) {
    let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
    histogram!(METRIC_ORDERBOOK_FETCH_LATENCY, "token_id" => token_id.to_string()).record(latency_ms);
}

/// Increment order submitted counter.
pub fn inc_orders_submitted() {
    counter!(METRIC_ORDERS_SUBMITTED).increment(1);
}

/// Increment orders filled counter.
pub fn inc_orders_filled() {
    counter!(METRIC_ORDERS_FILLED).increment(1);
}

/// Increment orders failed counter.
pub fn inc_orders_failed() {
    counter!(METRIC_ORDERS_FAILED).increment(1);
}

/// Increment opportunities detected counter.
pub fn inc_opportunities_detected() {
    counter!(METRIC_OPPORTUNITIES_DETECTED).increment(1);
}

/// Increment opportunities executed counter.
pub fn inc_opportunities_executed() {
    counter!(METRIC_OPPORTUNITIES_EXECUTED).increment(1);
}

/// Increment WebSocket messages received counter.
pub fn inc_ws_messages_received() {
    counter!(METRIC_WS_MESSAGES_RECEIVED).increment(1);
}

/// Increment WebSocket reconnects counter.
pub fn inc_ws_reconnects() {
    counter!(METRIC_WS_RECONNECTS).increment(1);
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
    LatencyTimer::new(METRIC_ORDER_SUBMIT_LATENCY)
}

/// Create a latency timer for WebSocket message processing.
pub fn timer_ws_message() -> LatencyTimer {
    LatencyTimer::new(METRIC_WS_MESSAGE_LATENCY)
}

/// Create a latency timer for opportunity detection.
pub fn timer_opportunity_detection() -> LatencyTimer {
    LatencyTimer::new(METRIC_OPPORTUNITY_DETECTION_LATENCY)
}

/// Create a latency timer for signing operations.
pub fn timer_signing() -> LatencyTimer {
    LatencyTimer::new(METRIC_SIGNING_LATENCY)
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
