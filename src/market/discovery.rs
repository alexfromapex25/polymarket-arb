//! Market discovery strategies for finding active BTC 15-minute markets.

use regex::Regex;
use serde_json::Value;
use time::OffsetDateTime;
use tracing::{debug, info, instrument};

use super::types::{GammaMarket, Market, MarketData};
use crate::error::MarketError;

/// 15-minute window duration in seconds.
const BTC_15M_WINDOW: i64 = 900;

/// Polymarket event base URL.
const POLYMARKET_EVENT_URL: &str = "https://polymarket.com/event";

/// Gamma API base URL.
const GAMMA_API_URL: &str = "https://gamma-api.polymarket.com/markets";

/// Crypto 15M page URL.
const CRYPTO_15M_URL: &str = "https://polymarket.com/crypto/15M";

/// Find active BTC 15-minute market using multiple strategies.
#[instrument(skip(client))]
pub async fn discover_active_market(client: &reqwest::Client) -> Result<Market, MarketError> {
    // Strategy 1: Computed slugs for current/next windows
    info!("Searching for current BTC 15min market...");

    if let Ok(market) = try_computed_slugs(client).await {
        info!(slug = %market.slug, "Found market via computed slug");
        return Ok(market);
    }

    // Strategy 2: Gamma API
    if let Ok(market) = try_gamma_api(client).await {
        info!(slug = %market.slug, "Found market via Gamma API");
        return Ok(market);
    }

    // Strategy 3: Page scrape
    if let Ok(market) = try_page_scrape(client).await {
        info!(slug = %market.slug, "Found market via page scrape");
        return Ok(market);
    }

    Err(MarketError::NoActiveMarketFound)
}

/// Try computed slugs for current and upcoming 15-minute windows.
#[instrument(skip(client))]
async fn try_computed_slugs(client: &reqwest::Client) -> Result<Market, MarketError> {
    let now = OffsetDateTime::now_utc().unix_timestamp();

    for i in 0..7 {
        let ts = now + (i * BTC_15M_WINDOW);
        let ts_rounded = (ts / BTC_15M_WINDOW) * BTC_15M_WINDOW;
        let slug = format!("btc-updown-15m-{}", ts_rounded);

        debug!(slug = %slug, "Checking computed slug");

        match fetch_market_from_slug(client, &slug).await {
            Ok(market) => {
                // Check if market is still open
                if now < ts_rounded + BTC_15M_WINDOW {
                    return Ok(market);
                }
                debug!(slug = %slug, "Market exists but is closed");
            }
            Err(e) => {
                debug!(slug = %slug, error = %e, "Slug not found");
            }
        }
    }

    Err(MarketError::NoActiveMarketFound)
}

/// Try to find market via Gamma API.
#[instrument(skip(client))]
async fn try_gamma_api(client: &reqwest::Client) -> Result<Market, MarketError> {
    let response = client
        .get(GAMMA_API_URL)
        .query(&[("closed", "false"), ("limit", "500")])
        .header("User-Agent", "Mozilla/5.0")
        .send()
        .await?;

    let markets: Vec<GammaMarket> = response.json().await.map_err(|e| {
        MarketError::ParseError(format!("Failed to parse Gamma API response: {}", e))
    })?;

    let now = OffsetDateTime::now_utc().unix_timestamp();
    let pattern = Regex::new(r"^btc-updown-15m-(\d+)$").expect("valid regex");

    let mut candidates: Vec<(i64, String)> = Vec::new();

    for market in markets {
        if let Some(slug) = market.slug {
            if let Some(captures) = pattern.captures(&slug) {
                if let Some(ts_match) = captures.get(1) {
                    if let Ok(ts) = ts_match.as_str().parse::<i64>() {
                        if now < ts + BTC_15M_WINDOW {
                            candidates.push((ts, slug));
                        }
                    }
                }
            }
        }
    }

    if candidates.is_empty() {
        return Err(MarketError::NoActiveMarketFound);
    }

    // Sort by timestamp, prefer the earliest open market
    candidates.sort_by_key(|(ts, _)| *ts);

    let (_, slug) = candidates.into_iter().next().expect("non-empty candidates");
    fetch_market_from_slug(client, &slug).await
}

/// Try to find market via page scraping.
#[instrument(skip(client))]
async fn try_page_scrape(client: &reqwest::Client) -> Result<Market, MarketError> {
    let response = client
        .get(CRYPTO_15M_URL)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        )
        .send()
        .await?;

    let text = response.text().await?;
    let now = OffsetDateTime::now_utc().unix_timestamp();

    // Try to find slugs in the HTML
    let pattern = Regex::new(r"btc-updown-15m-(\d+)").expect("valid regex");
    let mut timestamps: Vec<i64> = pattern
        .captures_iter(&text)
        .filter_map(|cap| cap.get(1))
        .filter_map(|m| m.as_str().parse().ok())
        .collect();

    timestamps.sort();
    timestamps.dedup();
    timestamps.reverse();

    // Find an open market
    for ts in &timestamps {
        if now < ts + BTC_15M_WINDOW {
            let slug = format!("btc-updown-15m-{}", ts);
            if let Ok(market) = fetch_market_from_slug(client, &slug).await {
                return Ok(market);
            }
        }
    }

    // If no open markets, try __NEXT_DATA__ extraction
    if let Some(market) = try_next_data_extraction(&text, client).await {
        return Ok(market);
    }

    Err(MarketError::NoActiveMarketFound)
}

/// Try to extract market from __NEXT_DATA__ script tag.
async fn try_next_data_extraction(html: &str, client: &reqwest::Client) -> Option<Market> {
    let pattern =
        Regex::new(r#"<script id="__NEXT_DATA__"[^>]*>(.*?)</script>"#).expect("valid regex");

    if let Some(captures) = pattern.captures(html) {
        if let Some(json_str) = captures.get(1) {
            if let Ok(data) = serde_json::from_str::<Value>(json_str.as_str()) {
                // Navigate through the nested structure to find slugs
                if let Some(slugs) = find_btc_slugs_in_json(&data) {
                    for slug in slugs {
                        if let Ok(market) = fetch_market_from_slug(client, &slug).await {
                            return Some(market);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Recursively find btc-updown-15m slugs in JSON data.
fn find_btc_slugs_in_json(value: &Value) -> Option<Vec<String>> {
    let pattern = Regex::new(r"^btc-updown-15m-\d+$").expect("valid regex");
    let mut slugs = Vec::new();

    fn recurse(value: &Value, pattern: &Regex, slugs: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                if let Some(Value::String(s)) = map.get("slug") {
                    if pattern.is_match(s) {
                        slugs.push(s.clone());
                    }
                }
                for v in map.values() {
                    recurse(v, pattern, slugs);
                }
            }
            Value::Array(arr) => {
                for v in arr {
                    recurse(v, pattern, slugs);
                }
            }
            _ => {}
        }
    }

    recurse(value, &pattern, &mut slugs);

    if slugs.is_empty() {
        None
    } else {
        Some(slugs)
    }
}

/// Fetch market information from a slug.
#[instrument(skip(client))]
pub async fn fetch_market_from_slug(
    client: &reqwest::Client,
    slug: &str,
) -> Result<Market, MarketError> {
    // Strip query params if present
    let slug = slug.split('?').next().expect("non-empty slug");
    let url = format!("{}/{}", POLYMARKET_EVENT_URL, slug);

    let response = client
        .get(&url)
        .header("User-Agent", "Mozilla/5.0")
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(MarketError::FetchFailed {
            slug: slug.to_string(),
            reason: format!("HTTP {}", response.status()),
        });
    }

    let text = response.text().await?;

    // Extract __NEXT_DATA__ JSON payload
    let pattern =
        Regex::new(r#"<script id="__NEXT_DATA__"[^>]*>(.*?)</script>"#).expect("valid regex");
    let captures = pattern.captures(&text).ok_or_else(|| {
        MarketError::ParseError("__NEXT_DATA__ payload not found on page".to_string())
    })?;

    let json_str = captures.get(1).expect("capture group exists").as_str();
    let payload: Value = serde_json::from_str(json_str)
        .map_err(|e| MarketError::ParseError(format!("Failed to parse JSON: {}", e)))?;

    // Navigate to find the market data
    let queries = payload
        .pointer("/props/pageProps/dehydratedState/queries")
        .and_then(|v| v.as_array())
        .ok_or_else(|| MarketError::ParseError("queries not found in payload".to_string()))?;

    let mut market_data: Option<MarketData> = None;

    for query in queries {
        if let Some(data) = query.pointer("/state/data") {
            if let Some(markets) = data.get("markets").and_then(|m| m.as_array()) {
                for m in markets {
                    let parsed: MarketData = serde_json::from_value(m.clone())
                        .map_err(|e| MarketError::ParseError(e.to_string()))?;
                    if parsed.slug.as_deref() == Some(slug) {
                        market_data = Some(parsed);
                        break;
                    }
                }
            }
            if market_data.is_some() {
                break;
            }
        }
    }

    let data = market_data.ok_or_else(|| MarketError::FetchFailed {
        slug: slug.to_string(),
        reason: "Market slug not found in dehydrated state".to_string(),
    })?;

    // Validate token IDs
    let clob_tokens = data.clob_token_ids.ok_or_else(|| MarketError::ParseError("No clobTokenIds".to_string()))?;
    if clob_tokens.len() != 2 {
        return Err(MarketError::ParseError(format!(
            "Expected 2 token IDs, got {}",
            clob_tokens.len()
        )));
    }

    // Extract timestamp from slug
    let ts_pattern = Regex::new(r"btc-updown-15m-(\d+)").expect("valid regex");
    let start_timestamp = ts_pattern
        .captures(slug)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse::<i64>().ok())
        .ok_or_else(|| MarketError::ParseError("Could not parse timestamp from slug".to_string()))?;

    Ok(Market {
        slug: slug.to_string(),
        id: data.id.unwrap_or_default(),
        up_token_id: clob_tokens[0].clone(),
        down_token_id: clob_tokens[1].clone(),
        start_timestamp,
        end_timestamp: start_timestamp + BTC_15M_WINDOW,
        question: data.question,
    })
}

/// Get the next market slug based on current slug.
pub fn next_slug(slug: &str) -> Result<String, MarketError> {
    let pattern = Regex::new(r"^(.+-)?(\d+)$").expect("valid regex");
    let captures = pattern.captures(slug).ok_or_else(|| {
        MarketError::ParseError(format!("Slug not in expected format: {}", slug))
    })?;

    let prefix = captures.get(1).map(|m| m.as_str()).unwrap_or("");
    let num: i64 = captures
        .get(2)
        .expect("capture exists")
        .as_str()
        .parse()
        .map_err(|_| MarketError::ParseError("Failed to parse timestamp".to_string()))?;

    Ok(format!("{}{}", prefix, num + BTC_15M_WINDOW))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_slug_increments_correctly() {
        assert_eq!(
            next_slug("btc-updown-15m-1765301400").unwrap(),
            "btc-updown-15m-1765302300"
        );
    }

    #[test]
    fn find_btc_slugs_in_json_finds_slugs() {
        let json = serde_json::json!({
            "markets": [
                {"slug": "btc-updown-15m-123"},
                {"slug": "other-market"}
            ]
        });
        let slugs = find_btc_slugs_in_json(&json).unwrap();
        assert_eq!(slugs, vec!["btc-updown-15m-123"]);
    }
}
