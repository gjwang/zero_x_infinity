//! Ed25519 signature verification for API authentication.
//!
//! This module provides signature verification using the Ed25519 algorithm.
//! The server stores only public keys, ensuring private keys never leave the client.

use ed25519_dalek::{Signature, Verifier, VerifyingKey};

/// Verify an Ed25519 signature.
///
/// # Arguments
/// * `public_key` - 32-byte Ed25519 public key
/// * `message` - The message that was signed
/// * `signature` - 64-byte Ed25519 signature
///
/// # Returns
/// `true` if signature is valid, `false` otherwise.
///
/// # Example
/// ```ignore
/// use zero_x_infinity::auth::signature;
/// let valid = signature::verify_ed25519(&pub_key, b"message", &sig);
/// ```
pub fn verify_ed25519(public_key: &[u8], message: &[u8], signature: &[u8]) -> bool {
    // Public key must be exactly 32 bytes
    let pk_bytes: [u8; 32] = match public_key.try_into() {
        Ok(b) => b,
        Err(_) => return false,
    };

    // Signature must be exactly 64 bytes
    let sig_bytes: [u8; 64] = match signature.try_into() {
        Ok(b) => b,
        Err(_) => return false,
    };

    // Parse public key
    let verifying_key = match VerifyingKey::from_bytes(&pk_bytes) {
        Ok(k) => k,
        Err(_) => return false,
    };

    // Parse signature
    let sig = Signature::from_bytes(&sig_bytes);

    // Verify
    verifying_key.verify(message, &sig).is_ok()
}

/// Generate a new Ed25519 keypair for testing.
///
/// Returns (private_key_bytes, public_key_bytes).
#[cfg(test)]
pub fn generate_keypair() -> ([u8; 32], [u8; 32]) {
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();

    let mut private_key = [0u8; 32];
    private_key.copy_from_slice(signing_key.as_bytes());

    let mut public_key = [0u8; 32];
    public_key.copy_from_slice(verifying_key.as_bytes());

    (private_key, public_key)
}

/// Sign a message with a private key (for testing).
#[cfg(test)]
pub fn sign_message(private_key: &[u8; 32], message: &[u8]) -> [u8; 64] {
    use ed25519_dalek::{Signer, SigningKey};

    let signing_key = SigningKey::from_bytes(private_key);
    let signature = signing_key.sign(message);
    signature.to_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_valid_signature() {
        let (private_key, public_key) = generate_keypair();
        let message = b"Hello, World!";
        let signature = sign_message(&private_key, message);

        assert!(verify_ed25519(&public_key, message, &signature));
    }

    #[test]
    fn test_verify_invalid_signature() {
        let (_, public_key) = generate_keypair();
        let message = b"Hello, World!";
        let bad_signature = [0u8; 64];

        assert!(!verify_ed25519(&public_key, message, &bad_signature));
    }

    #[test]
    fn test_verify_wrong_message() {
        let (private_key, public_key) = generate_keypair();
        let message = b"Hello, World!";
        let signature = sign_message(&private_key, message);

        let wrong_message = b"Wrong message";
        assert!(!verify_ed25519(&public_key, wrong_message, &signature));
    }

    #[test]
    fn test_verify_wrong_key() {
        let (private_key, _public_key) = generate_keypair();
        let (_, wrong_public_key) = generate_keypair();
        let message = b"Hello, World!";
        let signature = sign_message(&private_key, message);

        assert!(!verify_ed25519(&wrong_public_key, message, &signature));
    }

    #[test]
    fn test_invalid_key_length() {
        let message = b"Hello";
        let signature = [0u8; 64];

        // Too short public key
        assert!(!verify_ed25519(&[0u8; 16], message, &signature));
        // Too long public key
        assert!(!verify_ed25519(&[0u8; 64], message, &signature));
    }

    #[test]
    fn test_invalid_signature_length() {
        let (_, public_key) = generate_keypair();
        let message = b"Hello";

        // Too short signature
        assert!(!verify_ed25519(&public_key, message, &[0u8; 32]));
        // Too long signature
        assert!(!verify_ed25519(&public_key, message, &[0u8; 128]));
    }

    #[test]
    fn test_auth_payload_format() {
        // Test signing the actual auth payload format
        let (private_key, public_key) = generate_keypair();

        let api_key = "AK_7F3D8E2A1B5C9F04";
        let ts_nonce = "1703260800001";
        let method = "GET";
        let path = "/api/v1/orders";
        let body = "";

        let payload = format!("{}{}{}{}{}", api_key, ts_nonce, method, path, body);
        let signature = sign_message(&private_key, payload.as_bytes());

        assert!(verify_ed25519(&public_key, payload.as_bytes(), &signature));
    }
}
