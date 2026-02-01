//! WebSocket client for Polymarket CLOB market data feed.
//!
//! Features:
//! - Automatic reconnection with exponential backoff
//! - Heartbeat/ping-pong handling
//! - SmallVec optimization for price levels

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info, warn};

use super::types::{BookUpdate, PriceLevel, WsEventType};
use crate::error::WsError;
use crate::metrics;

/// L2 book state maintained from WebSocket updates.
#[derive(Debug, Clone, Default)]
pub struct L2BookState {
    /// Bid levels: price -> size.
    pub bids: HashMap<Decimal, Decimal>,
    /// Ask levels: price -> size.
    pub asks: HashMap<Decimal, Decimal>,
    /// Last update timestamp (milliseconds).
    pub last_timestamp_ms: Option<i64>,
    /// Last hash for debugging.
    pub last_hash: Option<String>,
}

impl L2BookState {
    /// Apply a full book snapshot.
    pub fn apply_snapshot(&mut self, bids: Vec<WsLevel>, asks: Vec<WsLevel>) {
        self.bids.clear();
        self.asks.clear();

        for level in bids {
            if let (Some(price), Some(size)) = (level.price_decimal(), level.size_decimal()) {
                if size > Decimal::ZERO {
                    self.bids.insert(price, size);
                }
            }
        }

        for level in asks {
            if let (Some(price), Some(size)) = (level.price_decimal(), level.size_decimal()) {
                if size > Decimal::ZERO {
                    self.asks.insert(price, size);
                }
            }
        }
    }

    /// Apply a price change delta.
    pub fn apply_delta(&mut self, change: &WsPriceChange) {
        let price = match change.price.parse::<Decimal>() {
            Ok(p) => p,
            Err(_) => return,
        };
        let size = match change.size.parse::<Decimal>() {
            Ok(s) => s,
            Err(_) => return,
        };

        let book = match change.side.to_uppercase().as_str() {
            "BUY" => &mut self.bids,
            "SELL" => &mut self.asks,
            _ => return,
        };

        if size <= Decimal::ZERO {
            book.remove(&price);
        } else {
            book.insert(price, size);
        }

        if let Some(hash) = &change.hash {
            self.last_hash = Some(hash.clone());
        }
    }

    /// Convert to sorted price level vectors.
    pub fn to_levels(&self) -> (Vec<PriceLevel>, Vec<PriceLevel>) {
        let mut bids: Vec<PriceLevel> = self
            .bids
            .iter()
            .filter(|(_, &size)| size > Decimal::ZERO)
            .map(|(&price, &size)| PriceLevel { price, size })
            .collect();
        bids.sort_by(|a, b| b.price.cmp(&a.price)); // Descending

        let mut asks: Vec<PriceLevel> = self
            .asks
            .iter()
            .filter(|(_, &size)| size > Decimal::ZERO)
            .map(|(&price, &size)| PriceLevel { price, size })
            .collect();
        asks.sort_by(|a, b| a.price.cmp(&b.price)); // Ascending

        (bids, asks)
    }
}

/// Price level from WebSocket.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WsLevel {
    /// Price as string.
    pub price: String,
    /// Size as string.
    pub size: String,
}

impl WsLevel {
    /// Parse price to Decimal.
    pub fn price_decimal(&self) -> Option<Decimal> {
        self.price.parse().ok()
    }

    /// Parse size to Decimal.
    pub fn size_decimal(&self) -> Option<Decimal> {
        self.size.parse().ok()
    }
}

/// Price change from WebSocket.
#[derive(Debug, Clone, Deserialize)]
pub struct WsPriceChange {
    /// Asset ID.
    pub asset_id: Option<String>,
    /// Price as string.
    pub price: String,
    /// Size as string.
    pub size: String,
    /// Side: "BUY" or "SELL".
    pub side: String,
    /// Optional hash.
    pub hash: Option<String>,
}

/// WebSocket event from Polymarket.
#[derive(Debug, Clone, Deserialize)]
pub struct WsEvent {
    /// Event type: "book" or "price_change".
    pub event_type: Option<String>,
    /// Asset ID (for book events).
    pub asset_id: Option<String>,
    /// Bid levels (for book events).
    pub bids: Option<Vec<WsLevel>>,
    /// Ask levels (for book events).
    pub asks: Option<Vec<WsLevel>>,
    /// Price changes (for price_change events).
    pub price_changes: Option<Vec<WsPriceChange>>,
    /// Timestamp in milliseconds.
    pub timestamp: Option<i64>,
    /// Hash for debugging.
    pub hash: Option<String>,
}

/// WebSocket subscription message.
#[derive(Debug, Serialize)]
struct SubscribeMessage {
    /// Message type.
    #[serde(rename = "type")]
    msg_type: String,
    /// Asset IDs to subscribe to.
    assets_ids: Vec<String>,
}

/// Reconnection configuration for WebSocket.
#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    /// Initial backoff delay in milliseconds.
    pub initial_delay_ms: u64,
    /// Maximum backoff delay in seconds.
    pub max_delay_s: u64,
    /// Backoff multiplier (e.g., 2.0 for exponential).
    pub backoff_multiplier: f64,
    /// Heartbeat interval in seconds.
    pub heartbeat_interval_s: u64,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            initial_delay_ms: 1000,
            max_delay_s: 30,
            backoff_multiplier: 2.0,
            heartbeat_interval_s: 30,
        }
    }
}

impl ReconnectConfig {
    /// Create from config values.
    pub fn from_config(max_delay_s: u64, heartbeat_interval_s: u64) -> Self {
        Self {
            max_delay_s,
            heartbeat_interval_s,
            ..Default::default()
        }
    }

    /// Calculate next delay with exponential backoff.
    pub fn next_delay(&self, attempt: u32) -> Duration {
        let delay_ms = self.initial_delay_ms as f64 * self.backoff_multiplier.powi(attempt as i32);
        let max_delay_ms = self.max_delay_s * 1000;
        let clamped_ms = delay_ms.min(max_delay_ms as f64) as u64;
        Duration::from_millis(clamped_ms)
    }
}

/// WebSocket connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not connected.
    Disconnected,
    /// Attempting to connect.
    Connecting,
    /// Connected and subscribed.
    Connected,
    /// Reconnecting after disconnect.
    Reconnecting,
}

/// Manages WebSocket connection and L2 book state.
pub struct MarketWebSocket {
    /// Book state per asset ID.
    books: DashMap<String, L2BookState>,
    /// WebSocket base URL.
    ws_url: String,
    /// Reconnection configuration.
    reconnect_config: ReconnectConfig,
    /// Connection state (atomic for thread safety).
    connected: Arc<AtomicBool>,
    /// Reconnection attempt counter.
    reconnect_attempts: Arc<AtomicU64>,
    /// Last successful message timestamp.
    last_message_time: Arc<std::sync::RwLock<Option<Instant>>>,
}

impl MarketWebSocket {
    /// Create a new WebSocket client.
    pub fn new(ws_url: String) -> Self {
        Self {
            books: DashMap::new(),
            ws_url,
            reconnect_config: ReconnectConfig::default(),
            connected: Arc::new(AtomicBool::new(false)),
            reconnect_attempts: Arc::new(AtomicU64::new(0)),
            last_message_time: Arc::new(std::sync::RwLock::new(None)),
        }
    }

    /// Create with custom reconnection config.
    pub fn with_reconnect_config(ws_url: String, config: ReconnectConfig) -> Self {
        Self {
            books: DashMap::new(),
            ws_url,
            reconnect_config: config,
            connected: Arc::new(AtomicBool::new(false)),
            reconnect_attempts: Arc::new(AtomicU64::new(0)),
            last_message_time: Arc::new(std::sync::RwLock::new(None)),
        }
    }

    /// Check if currently connected.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    /// Get reconnection attempt count.
    pub fn reconnect_attempts(&self) -> u64 {
        self.reconnect_attempts.load(Ordering::SeqCst)
    }

    /// Get book state for an asset.
    pub fn get_book(&self, asset_id: &str) -> Option<L2BookState> {
        self.books.get(asset_id).map(|b| b.clone())
    }

    /// Initialize books for asset IDs.
    pub fn init_books(&self, asset_ids: &[String]) {
        for id in asset_ids {
            self.books.insert(id.clone(), L2BookState::default());
        }
    }

    /// Check if connection appears stale (no messages in heartbeat interval).
    pub fn is_stale(&self) -> bool {
        if let Ok(time) = self.last_message_time.read() {
            if let Some(last) = *time {
                return last.elapsed() > Duration::from_secs(self.reconnect_config.heartbeat_interval_s * 2);
            }
        }
        // No messages received yet - not stale
        false
    }

    /// Run the WebSocket connection, yielding book updates.
    pub async fn run(
        &self,
        asset_ids: Vec<String>,
    ) -> Result<impl futures::Stream<Item = BookUpdate> + '_, WsError> {
        let url = format!("{}/ws/market", self.ws_url.trim_end_matches('/'));

        // Initialize books
        self.init_books(&asset_ids);

        info!(url = %url, assets = ?asset_ids, "Connecting to WebSocket");

        let (ws_stream, _) = connect_async(&url)
            .await
            .map_err(|e| WsError::ConnectionFailed(e.to_string()))?;

        self.connected.store(true, Ordering::SeqCst);
        self.reconnect_attempts.store(0, Ordering::SeqCst);

        let (mut write, read) = ws_stream.split();

        // Subscribe to assets
        let subscribe_msg = SubscribeMessage {
            msg_type: "MARKET".to_string(),
            assets_ids: asset_ids.clone(),
        };

        let msg_json = serde_json::to_string(&subscribe_msg)
            .map_err(|e| WsError::SendFailed(e.to_string()))?;

        write
            .send(Message::Text(msg_json))
            .await
            .map_err(|e| WsError::SendFailed(e.to_string()))?;

        info!("Subscribed to {} assets", asset_ids.len());

        // Process messages with metrics tracking
        let books = &self.books;
        let connected = self.connected.clone();
        let last_msg_time = self.last_message_time.clone();

        let stream = read.filter_map(move |msg| {
            let books = books;
            let connected = connected.clone();
            let last_msg_time = last_msg_time.clone();

            async move {
                // Update last message time on any message
                if let Ok(mut time) = last_msg_time.write() {
                    *time = Some(Instant::now());
                }

                match msg {
                    Ok(Message::Text(text)) => {
                        let start = Instant::now();
                        metrics::inc_ws_messages_received();
                        let result = Self::process_message(books, &text);
                        metrics::record_ws_message_latency(start);
                        result
                    }
                    Ok(Message::Ping(_)) => {
                        debug!("Received ping");
                        // Note: tungstenite auto-responds to pings
                        None
                    }
                    Ok(Message::Pong(_)) => {
                        debug!("Received pong");
                        None
                    }
                    Ok(Message::Close(frame)) => {
                        warn!(frame = ?frame, "WebSocket closed");
                        connected.store(false, Ordering::SeqCst);
                        None
                    }
                    Ok(_) => None,
                    Err(e) => {
                        error!(error = %e, "WebSocket error");
                        connected.store(false, Ordering::SeqCst);
                        None
                    }
                }
            }
        });

        Ok(stream)
    }

    /// Run with automatic reconnection on disconnect.
    /// Returns a channel receiver that yields book updates.
    pub async fn run_with_reconnect(
        self: Arc<Self>,
        asset_ids: Vec<String>,
    ) -> mpsc::Receiver<BookUpdate> {
        let (tx, rx) = mpsc::channel(1000);

        let ws = self;
        let assets = asset_ids;

        tokio::spawn(async move {
            let mut attempt = 0u32;

            loop {
                info!(attempt = attempt, "Attempting WebSocket connection");

                match ws.run(assets.clone()).await {
                    Ok(stream) => {
                        attempt = 0; // Reset on successful connection

                        // Pin the stream to use with .next()
                        let mut stream = Box::pin(stream);

                        while let Some(update) = stream.next().await {
                            if tx.send(update).await.is_err() {
                                info!("Channel closed, stopping WebSocket");
                                return;
                            }
                        }

                        // Stream ended - connection closed
                        warn!("WebSocket stream ended, will reconnect");
                    }
                    Err(e) => {
                        error!(error = %e, attempt = attempt, "WebSocket connection failed");
                    }
                }

                // Calculate backoff delay
                let delay = ws.reconnect_config.next_delay(attempt);
                ws.reconnect_attempts.fetch_add(1, Ordering::SeqCst);
                metrics::inc_ws_reconnects();

                info!(delay_ms = delay.as_millis(), "Reconnecting after delay");
                tokio::time::sleep(delay).await;

                attempt = attempt.saturating_add(1);
            }
        });

        rx
    }

    /// Process a WebSocket message.
    fn process_message(
        books: &DashMap<String, L2BookState>,
        text: &str,
    ) -> Option<BookUpdate> {
        // Messages can be single objects or arrays
        let events: Vec<WsEvent> = if text.starts_with('[') {
            serde_json::from_str(text).ok()?
        } else {
            vec![serde_json::from_str(text).ok()?]
        };

        let mut last_update: Option<BookUpdate> = None;

        for event in events {
            let event_type = event.event_type.as_deref()?;

            match event_type {
                "book" => {
                    let asset_id = event.asset_id.as_ref()?;
                    if let Some(mut book) = books.get_mut(asset_id) {
                        book.apply_snapshot(
                            event.bids.unwrap_or_default(),
                            event.asks.unwrap_or_default(),
                        );
                        book.last_timestamp_ms = event.timestamp;
                        book.last_hash = event.hash.clone();
                    }
                    last_update = Some(BookUpdate {
                        asset_id: asset_id.clone(),
                        event_type: WsEventType::Book,
                    });
                }
                "price_change" => {
                    if let Some(changes) = &event.price_changes {
                        for change in changes {
                            if let Some(asset_id) = &change.asset_id {
                                if let Some(mut book) = books.get_mut(asset_id) {
                                    book.apply_delta(change);
                                    book.last_timestamp_ms = event.timestamp;
                                }
                                last_update = Some(BookUpdate {
                                    asset_id: asset_id.clone(),
                                    event_type: WsEventType::PriceChange,
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        last_update
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn l2_book_state_apply_snapshot() {
        let mut state = L2BookState::default();
        state.apply_snapshot(
            vec![
                WsLevel { price: "0.48".to_string(), size: "100".to_string() },
                WsLevel { price: "0.47".to_string(), size: "50".to_string() },
            ],
            vec![
                WsLevel { price: "0.50".to_string(), size: "100".to_string() },
            ],
        );

        assert_eq!(state.bids.len(), 2);
        assert_eq!(state.asks.len(), 1);
        assert_eq!(state.bids.get(&dec!(0.48)), Some(&dec!(100)));
    }

    #[test]
    fn l2_book_state_apply_delta() {
        let mut state = L2BookState::default();
        state.bids.insert(dec!(0.48), dec!(100));

        // Update existing level
        state.apply_delta(&WsPriceChange {
            asset_id: None,
            price: "0.48".to_string(),
            size: "150".to_string(),
            side: "BUY".to_string(),
            hash: None,
        });
        assert_eq!(state.bids.get(&dec!(0.48)), Some(&dec!(150)));

        // Remove level
        state.apply_delta(&WsPriceChange {
            asset_id: None,
            price: "0.48".to_string(),
            size: "0".to_string(),
            side: "BUY".to_string(),
            hash: None,
        });
        assert!(!state.bids.contains_key(&dec!(0.48)));
    }

    #[test]
    fn l2_book_state_to_levels_sorted() {
        let mut state = L2BookState::default();
        state.bids.insert(dec!(0.47), dec!(50));
        state.bids.insert(dec!(0.48), dec!(100));
        state.asks.insert(dec!(0.51), dec!(100));
        state.asks.insert(dec!(0.50), dec!(50));

        let (bids, asks) = state.to_levels();

        assert_eq!(bids[0].price, dec!(0.48)); // Highest first
        assert_eq!(bids[1].price, dec!(0.47));
        assert_eq!(asks[0].price, dec!(0.50)); // Lowest first
        assert_eq!(asks[1].price, dec!(0.51));
    }
}
