//! Order execution and verification.

use std::time::Duration;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;
use tracing::{debug, error, info, instrument, warn};

use super::order::{OrderParams, OrderState, OrderStatus, Side, TimeInForce};
use crate::error::TradingError;
use crate::market::PolymarketClient;
use crate::signing;

/// Order submission request body.
#[derive(Debug, Clone, Serialize)]
pub struct OrderRequest {
    /// Token ID to trade.
    pub token_id: String,
    /// Order side (BUY/SELL).
    pub side: String,
    /// Limit price.
    pub price: String,
    /// Order size.
    pub size: String,
    /// Fee rate basis points.
    pub fee_rate_bps: String,
    /// Nonce for order uniqueness.
    pub nonce: String,
    /// Expiration timestamp.
    pub expiration: String,
    /// Taker address.
    pub taker: String,
    /// Maker address.
    pub maker: String,
    /// Signature type.
    pub signature_type: u8,
    /// Order signature.
    pub signature: String,
    /// Time in force.
    pub order_type: String,
    /// Neg risk flag.
    pub neg_risk: bool,
}

/// Order submission result.
#[derive(Debug, Clone, Deserialize)]
pub struct SubmitResult {
    /// Order ID (various field names).
    #[serde(alias = "orderID", alias = "orderId", alias = "order_id", alias = "id")]
    pub order_id: Option<String>,
    /// Error message if any.
    pub error: Option<String>,
    /// Error code if any.
    pub error_code: Option<String>,
    /// Success flag.
    pub success: Option<bool>,
}

/// Submit a single order using the Polymarket CLOB API.
///
/// CRITICAL: Always uses neg_risk=true for BTC 15min markets.
#[instrument(skip(client, params), fields(token = %params.token_id, side = ?params.side))]
pub async fn submit_order(
    client: &PolymarketClient,
    params: &OrderParams,
) -> Result<String, TradingError> {
    // Validate parameters
    params.validate().map_err(TradingError::InvalidParams)?;

    debug!(
        price = %params.price,
        size = %params.size,
        tif = ?params.tif,
        "Submitting order"
    );

    // Get wallet address
    let address = client.get_address()?;

    // Generate auth headers
    let auth_headers = signing::generate_auth_headers(client.private_key(), client.signature_type()).await?;

    // Generate nonce and expiration
    let nonce = chrono::Utc::now().timestamp_millis().to_string();
    let expiration = (chrono::Utc::now().timestamp() + 3600).to_string(); // 1 hour from now

    // Convert side to string
    let side_str = match params.side {
        Side::Buy => "BUY",
        Side::Sell => "SELL",
    };

    // Convert time in force to order type
    let order_type = match params.tif {
        TimeInForce::FOK => "FOK",
        TimeInForce::FAK => "GTC", // FAK maps to GTC
        TimeInForce::GTC => "GTC",
    };

    // Create order message to sign
    // Format: token_id + side + price + size + nonce + expiration
    let order_message = format!(
        "{}:{}:{}:{}:{}:{}",
        params.token_id,
        side_str,
        params.price,
        params.size,
        nonce,
        expiration
    );

    // Sign the order
    let signature_bytes = signing::sign_message(client.private_key(), order_message.as_bytes()).await?;
    let signature = format!("0x{}", hex::encode(&signature_bytes));

    // Build order request
    let order_request = OrderRequest {
        token_id: params.token_id.clone(),
        side: side_str.to_string(),
        price: params.price.to_string(),
        size: params.size.to_string(),
        fee_rate_bps: "0".to_string(),
        nonce,
        expiration,
        taker: "0x0000000000000000000000000000000000000000".to_string(),
        maker: address,
        signature_type: client.signature_type(),
        signature,
        order_type: order_type.to_string(),
        neg_risk: true, // CRITICAL: Always true for BTC 15min markets
    };

    // Submit order via API
    let url = format!("{}/order", client.clob_url());

    let mut request = client.http().post(&url).json(&order_request);
    for (key, value) in auth_headers {
        request = request.header(&key, &value);
    }

    let response = request.send().await.map_err(|e| {
        TradingError::SubmissionFailed(format!("HTTP request failed: {}", e))
    })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(TradingError::SubmissionFailed(format!(
            "Order submission failed: HTTP {} - {}",
            status, body
        )));
    }

    let result: SubmitResult = response.json().await.map_err(|e| {
        TradingError::SubmissionFailed(format!("Failed to parse response: {}", e))
    })?;

    // Check for errors in response
    if let Some(error) = result.error {
        return Err(TradingError::SubmissionFailed(error));
    }

    // Extract order ID
    let order_id = result.order_id.ok_or_else(|| {
        TradingError::SubmissionFailed("No order ID in response".to_string())
    })?;

    info!(
        order_id = %order_id,
        token_id = %params.token_id,
        side = ?params.side,
        price = %params.price,
        size = %params.size,
        "Order submitted successfully"
    );

    Ok(order_id)
}

/// Submit multiple orders as fast as possible.
#[instrument(skip(client, orders))]
pub async fn submit_orders_fast(
    client: &PolymarketClient,
    orders: Vec<OrderParams>,
    tif: TimeInForce,
) -> Vec<Result<String, TradingError>> {
    if orders.is_empty() {
        return Vec::new();
    }

    debug!(count = orders.len(), "Submitting orders fast");

    // Submit sequentially for now
    let mut results = Vec::with_capacity(orders.len());

    for params in orders {
        let mut p = params;
        p.tif = tif;
        results.push(submit_order(client, &p).await);
    }

    // Log summary
    let success_count = results.iter().filter(|r| r.is_ok()).count();
    let fail_count = results.len() - success_count;
    info!(
        total = results.len(),
        success = success_count,
        failed = fail_count,
        "Batch order submission complete"
    );

    results
}

/// Poll for order status until terminal or timeout.
#[instrument(skip(client), fields(order_id = %order_id))]
pub async fn wait_for_terminal_order(
    client: &PolymarketClient,
    order_id: &str,
    requested_size: Decimal,
    timeout: Duration,
    poll_interval: Duration,
) -> OrderState {
    let start = std::time::Instant::now();

    loop {
        if start.elapsed() >= timeout {
            warn!("Order status polling timed out");
            return OrderState {
                order_id: order_id.to_string(),
                is_terminal: false,
                is_filled: false,
                ..Default::default()
            };
        }

        match get_order_status(client, order_id).await {
            Ok(state) => {
                // Check if fully filled by size
                if let Some(filled) = state.filled_size {
                    if filled >= requested_size {
                        return OrderState {
                            is_terminal: true,
                            is_filled: true,
                            ..state
                        };
                    }
                }

                // Check if terminal status
                if let Some(status) = state.status {
                    if status.is_terminal() {
                        return OrderState {
                            is_terminal: true,
                            is_filled: status.is_filled(),
                            ..state
                        };
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "Error getting order status");
            }
        }

        sleep(poll_interval).await;
    }
}

/// Get current order status from the API.
pub async fn get_order_status(
    client: &PolymarketClient,
    order_id: &str,
) -> Result<OrderState, TradingError> {
    debug!(order_id = %order_id, "Getting order status");

    // Use the REST API to get order status
    let url = format!("{}/order/{}", client.clob_url(), order_id);

    let response = client
        .http()
        .get(&url)
        .send()
        .await
        .map_err(|e| TradingError::StatusFailed {
            order_id: order_id.to_string(),
            reason: format!("HTTP request failed: {}", e),
        })?;

    if !response.status().is_success() {
        return Err(TradingError::StatusFailed {
            order_id: order_id.to_string(),
            reason: format!("HTTP {}", response.status()),
        });
    }

    let json: serde_json::Value =
        response.json().await.map_err(|e| TradingError::StatusFailed {
            order_id: order_id.to_string(),
            reason: format!("Failed to parse response: {}", e),
        })?;

    // Parse the response - handle various field name conventions
    let status = json
        .get("status")
        .or_else(|| json.get("orderStatus"))
        .or_else(|| json.get("order_status"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<OrderStatus>().ok());

    let filled_size = parse_decimal_field(&json, &["filled", "filledSize", "filled_size", "sizeFilled"]);
    let remaining_size = parse_decimal_field(&json, &["remaining", "remainingSize", "remaining_size", "sizeRemaining"]);
    let original_size = parse_decimal_field(&json, &["size", "originalSize", "original_size"]);

    let is_terminal = status.map(|s| s.is_terminal()).unwrap_or(false);
    let is_filled = status.map(|s| s.is_filled()).unwrap_or(false);

    Ok(OrderState {
        order_id: order_id.to_string(),
        status,
        filled_size,
        remaining_size,
        original_size,
        is_terminal,
        is_filled,
    })
}

/// Parse a decimal field from JSON, trying multiple field names.
fn parse_decimal_field(json: &serde_json::Value, keys: &[&str]) -> Option<Decimal> {
    for key in keys {
        if let Some(value) = json.get(*key) {
            // Try as string first
            if let Some(s) = value.as_str() {
                if let Ok(d) = s.parse::<Decimal>() {
                    return Some(d);
                }
            }
            // Try as number
            if let Some(n) = value.as_f64() {
                if let Ok(d) = Decimal::try_from(n) {
                    return Some(d);
                }
            }
        }
    }
    None
}

/// Cancel one or more orders.
#[instrument(skip(client))]
pub async fn cancel_orders(
    client: &PolymarketClient,
    order_ids: &[String],
) -> Result<(), TradingError> {
    if order_ids.is_empty() {
        return Ok(());
    }

    debug!(count = order_ids.len(), "Cancelling orders");

    let auth_headers = signing::generate_auth_headers(client.private_key(), client.signature_type()).await?;

    for order_id in order_ids {
        let url = format!("{}/order/{}", client.clob_url(), order_id);

        let mut request = client.http().delete(&url);
        for (key, value) in &auth_headers {
            request = request.header(key, value);
        }

        match request.send().await {
            Ok(response) => {
                if response.status().is_success() {
                    info!(order_id = %order_id, "Order cancelled");
                } else {
                    error!(order_id = %order_id, status = %response.status(), "Failed to cancel order");
                    return Err(TradingError::CancelFailed {
                        order_id: order_id.clone(),
                        reason: format!("HTTP {}", response.status()),
                    });
                }
            }
            Err(e) => {
                error!(order_id = %order_id, error = %e, "Failed to cancel order");
                return Err(TradingError::CancelFailed {
                    order_id: order_id.clone(),
                    reason: format!("{}", e),
                });
            }
        }
    }

    Ok(())
}

/// Cancel all open orders for the account.
#[instrument(skip(client))]
pub async fn cancel_all_orders(client: &PolymarketClient) -> Result<u32, TradingError> {
    debug!("Cancelling all open orders");

    let auth_headers = signing::generate_auth_headers(client.private_key(), client.signature_type()).await?;

    let url = format!("{}/cancel-all", client.clob_url());

    let mut request = client.http().delete(&url);
    for (key, value) in auth_headers {
        request = request.header(&key, &value);
    }

    let response = request.send().await.map_err(|e| {
        TradingError::CancelFailed {
            order_id: "all".to_string(),
            reason: format!("{}", e),
        }
    })?;

    if !response.status().is_success() {
        return Err(TradingError::CancelFailed {
            order_id: "all".to_string(),
            reason: format!("HTTP {}", response.status()),
        });
    }

    // Try to parse response to get count
    let json: serde_json::Value = response.json().await.unwrap_or_default();
    let count = json
        .get("canceled")
        .and_then(|v| v.as_array())
        .map(|a| a.len() as u32)
        .unwrap_or(0);

    info!(count = count, "Cancelled all open orders");

    Ok(count)
}

/// Extract order ID from API response.
pub fn extract_order_id(result: &serde_json::Value) -> Option<String> {
    // Try various field names
    for key in ["orderID", "orderId", "order_id", "id"] {
        if let Some(id) = result.get(key).and_then(|v| v.as_str()) {
            return Some(id.to_string());
        }
    }

    // Try nested fields
    for key in ["order", "data", "result"] {
        if let Some(nested) = result.get(key) {
            if let Some(id) = extract_order_id(nested) {
                return Some(id);
            }
        }
    }

    None
}

/// Default timeout for order operations.
pub const DEFAULT_ORDER_TIMEOUT: Duration = Duration::from_secs(3);

/// Default poll interval for order status.
pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(250);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_order_id_various_formats() {
        let json1 = serde_json::json!({"orderID": "abc123"});
        assert_eq!(extract_order_id(&json1), Some("abc123".to_string()));

        let json2 = serde_json::json!({"orderId": "def456"});
        assert_eq!(extract_order_id(&json2), Some("def456".to_string()));

        let json3 = serde_json::json!({"order": {"id": "ghi789"}});
        assert_eq!(extract_order_id(&json3), Some("ghi789".to_string()));

        let json4 = serde_json::json!({"error": "something"});
        assert_eq!(extract_order_id(&json4), None);
    }

    #[test]
    fn parse_decimal_field_works() {
        let json = serde_json::json!({
            "filled": "10.5",
            "remaining": 5.25,
            "size": "100"
        });

        assert_eq!(
            parse_decimal_field(&json, &["filled"]),
            Some(Decimal::new(105, 1))
        );
        assert_eq!(
            parse_decimal_field(&json, &["remaining"]),
            Some(Decimal::new(525, 2))
        );
        assert_eq!(
            parse_decimal_field(&json, &["missing"]),
            None
        );
    }
}
