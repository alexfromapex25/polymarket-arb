//! Application configuration loaded from environment variables.

use rust_decimal::Decimal;
use serde::Deserialize;

/// Application configuration loaded from environment variables.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    // === Polymarket Credentials ===
    /// Wallet private key (hex, starts with 0x).
    pub polymarket_private_key: String,

    /// Optional pre-generated API key.
    #[serde(default)]
    pub polymarket_api_key: Option<String>,

    /// Optional API secret.
    #[serde(default)]
    pub polymarket_api_secret: Option<String>,

    /// Optional API passphrase.
    #[serde(default)]
    pub polymarket_api_passphrase: Option<String>,

    // === Wallet Configuration ===
    /// Signature type: 0=EOA, 1=Magic.link, 2=Gnosis Safe.
    #[serde(default)]
    pub polymarket_signature_type: u8,

    /// Proxy wallet address (required for Magic.link).
    #[serde(default)]
    pub polymarket_funder: Option<String>,

    // === Trading Parameters ===
    /// Maximum combined cost to trigger arbitrage (e.g., 0.991).
    #[serde(default = "default_target_cost")]
    pub target_pair_cost: Decimal,

    /// Number of shares per trade (minimum 5).
    #[serde(default = "default_order_size")]
    pub order_size: Decimal,

    /// Order type: FOK, FAK, or GTC.
    #[serde(default = "default_order_type")]
    pub order_type: String,

    /// Balance safety margin (1.2 = 20% extra).
    #[serde(default = "default_balance_margin")]
    pub balance_margin: Decimal,

    // === Operation Modes ===
    /// Simulation mode (no real orders).
    #[serde(default = "default_true")]
    pub dry_run: bool,

    /// Starting balance for simulation.
    #[serde(default = "default_sim_balance")]
    pub sim_balance: Decimal,

    /// Minimum seconds between trade executions.
    #[serde(default = "default_cooldown")]
    pub cooldown_seconds: u64,

    // === Market Discovery ===
    /// Force specific market slug (bypasses auto-discovery).
    #[serde(default)]
    pub polymarket_market_slug: Option<String>,

    // === WebSocket Configuration ===
    /// Enable WebSocket market feed instead of polling.
    #[serde(default)]
    pub use_wss: bool,

    /// WebSocket base URL.
    #[serde(default = "default_ws_url")]
    pub polymarket_ws_url: String,

    /// CLOB API base URL.
    #[serde(default = "default_clob_url")]
    pub polymarket_clob_url: String,

    // === Server Configuration ===
    /// HTTP server port for health/metrics endpoints.
    #[serde(default = "default_port")]
    pub port: u16,

    /// Log level (trace, debug, info, warn, error).
    #[serde(default = "default_log_level")]
    pub rust_log: String,

    /// Enable verbose logging.
    #[serde(default)]
    pub verbose: bool,
}

fn default_target_cost() -> Decimal {
    Decimal::new(991, 3) // 0.991
}

fn default_order_size() -> Decimal {
    Decimal::new(5, 0) // 5 shares
}

fn default_order_type() -> String {
    "FOK".to_string()
}

fn default_balance_margin() -> Decimal {
    Decimal::new(12, 1) // 1.2
}

fn default_true() -> bool {
    true
}

fn default_sim_balance() -> Decimal {
    Decimal::new(100, 0) // $100
}

fn default_cooldown() -> u64 {
    10
}

fn default_ws_url() -> String {
    "wss://ws-subscriptions-clob.polymarket.com".to_string()
}

fn default_clob_url() -> String {
    "https://clob.polymarket.com".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Config {
    /// Load configuration from environment, reading .env file first.
    pub fn load() -> Result<Self, envy::Error> {
        dotenvy::dotenv().ok();
        envy::from_env()
    }

    /// Check if the configuration is valid.
    pub fn validate(&self) -> Result<(), String> {
        if self.polymarket_private_key.is_empty() {
            return Err("POLYMARKET_PRIVATE_KEY is required".to_string());
        }

        if !self.polymarket_private_key.starts_with("0x") {
            return Err("POLYMARKET_PRIVATE_KEY must start with 0x".to_string());
        }

        if self.order_size < Decimal::new(5, 0) {
            return Err("ORDER_SIZE must be at least 5".to_string());
        }

        if self.target_pair_cost >= Decimal::ONE {
            return Err("TARGET_PAIR_COST must be less than 1.0".to_string());
        }

        Ok(())
    }

    /// Get the effective order type (uppercase).
    pub fn order_type_upper(&self) -> String {
        self.order_type.to_uppercase()
    }

    /// Check if using Magic.link (signature_type == 1).
    pub fn is_magic_link(&self) -> bool {
        self.polymarket_signature_type == 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values_are_sensible() {
        assert_eq!(default_target_cost(), Decimal::new(991, 3));
        assert_eq!(default_order_size(), Decimal::new(5, 0));
        assert_eq!(default_order_type(), "FOK");
        assert!(default_true());
    }

    #[test]
    fn validate_rejects_empty_private_key() {
        let config = Config {
            polymarket_private_key: "".to_string(),
            polymarket_api_key: None,
            polymarket_api_secret: None,
            polymarket_api_passphrase: None,
            polymarket_signature_type: 0,
            polymarket_funder: None,
            target_pair_cost: default_target_cost(),
            order_size: default_order_size(),
            order_type: default_order_type(),
            balance_margin: default_balance_margin(),
            dry_run: true,
            sim_balance: default_sim_balance(),
            cooldown_seconds: default_cooldown(),
            polymarket_market_slug: None,
            use_wss: false,
            polymarket_ws_url: default_ws_url(),
            polymarket_clob_url: default_clob_url(),
            port: default_port(),
            rust_log: default_log_level(),
            verbose: false,
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_invalid_private_key_prefix() {
        let config = Config {
            polymarket_private_key: "abc123".to_string(),
            polymarket_api_key: None,
            polymarket_api_secret: None,
            polymarket_api_passphrase: None,
            polymarket_signature_type: 0,
            polymarket_funder: None,
            target_pair_cost: default_target_cost(),
            order_size: default_order_size(),
            order_type: default_order_type(),
            balance_margin: default_balance_margin(),
            dry_run: true,
            sim_balance: default_sim_balance(),
            cooldown_seconds: default_cooldown(),
            polymarket_market_slug: None,
            use_wss: false,
            polymarket_ws_url: default_ws_url(),
            polymarket_clob_url: default_clob_url(),
            port: default_port(),
            rust_log: default_log_level(),
            verbose: false,
        };

        assert!(config.validate().is_err());
    }
}
