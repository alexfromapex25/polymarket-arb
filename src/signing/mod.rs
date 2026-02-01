//! Signing and authentication utilities for Polymarket.
//!
//! This module provides utilities for:
//! - Converting config signature types to SDK types
//! - Creating signers from private keys
//! - Computing wallet addresses

use alloy::signers::local::PrivateKeySigner;
use alloy::signers::Signer;
use polymarket_client_sdk::clob::types::SignatureType;
use tracing::debug;

use crate::error::TradingError;

/// Convert a u8 signature type from config to SDK SignatureType.
///
/// Signature types:
/// - 0: EOA (Externally Owned Account) - standard wallet
/// - 1: Magic.link - proxy wallet
/// - 2: Gnosis Safe - multi-sig
pub fn signature_type_from_u8(sig_type: u8) -> SignatureType {
    match sig_type {
        1 => SignatureType::Proxy,      // Magic.link proxy wallet
        2 => SignatureType::GnosisSafe, // Gnosis Safe multi-sig
        _ => SignatureType::Eoa,        // Default to EOA for 0 or any unknown value
    }
}

/// Create a LocalSigner from a hex-encoded private key.
///
/// The private key can be with or without the "0x" prefix.
pub fn create_signer(private_key: &str) -> Result<PrivateKeySigner, TradingError> {
    let key = private_key.strip_prefix("0x").unwrap_or(private_key);
    let bytes = hex::decode(key).map_err(|e| {
        TradingError::SigningError(format!("Invalid private key hex: {}", e))
    })?;

    if bytes.len() != 32 {
        return Err(TradingError::SigningError(format!(
            "Private key must be 32 bytes, got {}",
            bytes.len()
        )));
    }

    let mut key_bytes = [0u8; 32];
    key_bytes.copy_from_slice(&bytes);

    PrivateKeySigner::from_bytes(&key_bytes.into()).map_err(|e| {
        TradingError::SigningError(format!("Failed to create signer: {}", e))
    })
}

/// Get the wallet address from a private key.
pub fn address_from_private_key(private_key: &str) -> Result<String, TradingError> {
    let signer = create_signer(private_key)?;
    // Format address as checksummed hex
    Ok(format!("{:?}", signer.address()))
}

/// Sign a message with the private key.
pub async fn sign_message(private_key: &str, message: &[u8]) -> Result<Vec<u8>, TradingError> {
    let signer = create_signer(private_key)?;
    let signature = signer.sign_message(message).await.map_err(|e| {
        TradingError::SigningError(format!("Failed to sign message: {}", e))
    })?;
    Ok(signature.as_bytes().to_vec())
}

/// Generate CLOB authentication headers.
///
/// For EOA wallets, we need to sign a timestamp to prove ownership.
pub async fn generate_auth_headers(
    private_key: &str,
    _signature_type: u8,
) -> Result<Vec<(String, String)>, TradingError> {
    let signer = create_signer(private_key)?;
    let address = format!("{:?}", signer.address());

    // Generate timestamp
    let timestamp = chrono::Utc::now().timestamp_millis().to_string();

    // Create message to sign (Polymarket CLOB auth format)
    let message = format!("polymarket:{}", timestamp);

    // Sign the message
    let signature = signer.sign_message(message.as_bytes()).await.map_err(|e| {
        TradingError::SigningError(format!("Failed to sign auth message: {}", e))
    })?;

    debug!(address = %address, "Generated auth headers");

    Ok(vec![
        ("POLY_ADDRESS".to_string(), address),
        ("POLY_SIGNATURE".to_string(), format!("0x{}", hex::encode(signature.as_bytes()))),
        ("POLY_TIMESTAMP".to_string(), timestamp),
        ("POLY_NONCE".to_string(), "0".to_string()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_type_conversion() {
        assert!(matches!(signature_type_from_u8(0), SignatureType::Eoa));
        assert!(matches!(signature_type_from_u8(1), SignatureType::Proxy));
        assert!(matches!(
            signature_type_from_u8(2),
            SignatureType::GnosisSafe
        ));
        // Unknown defaults to EOA
        assert!(matches!(signature_type_from_u8(99), SignatureType::Eoa));
    }

    #[test]
    fn create_signer_valid_key() {
        // Valid 32-byte private key (not a real key, just for testing)
        let key = "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let result = create_signer(key);
        assert!(result.is_ok());
    }

    #[test]
    fn create_signer_without_prefix() {
        let key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let result = create_signer(key);
        assert!(result.is_ok());
    }

    #[test]
    fn create_signer_invalid_hex() {
        let key = "0xnot_valid_hex";
        let result = create_signer(key);
        assert!(result.is_err());
    }

    #[test]
    fn create_signer_wrong_length() {
        let key = "0x1234"; // Too short
        let result = create_signer(key);
        assert!(result.is_err());
    }

    #[test]
    fn address_from_key() {
        let key = "0x0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let result = address_from_private_key(key);
        assert!(result.is_ok());
        let addr = result.unwrap();
        assert!(addr.starts_with("0x"));
    }
}
