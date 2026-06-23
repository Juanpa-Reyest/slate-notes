//! Domain types for protected (encrypted) note content.
//!
//! These types describe an encrypted payload and the derived-key lifecycle, but
//! know nothing about the concrete cryptographic algorithms. The algorithms
//! live in `infra`, behind the `Cipher` port.

use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

/// Current on-disk format version for sealed payloads.
pub const PAYLOAD_VERSION: u32 = 1;

/// A symmetric key derived from a passphrase. It is held in memory only while
/// the vault is unlocked, and is wiped from memory when dropped.
pub struct DerivedKey([u8; 32]);

impl DerivedKey {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Raw key bytes, for the crypto implementation only.
    pub(crate) fn bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl Drop for DerivedKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// An encrypted payload: everything required to decrypt later EXCEPT the
/// passphrase-derived key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sealed {
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

/// A payload sealed to an X25519 public key via ECIES. The ephemeral public key
/// lets the holder of the matching X25519 private key reconstruct the shared
/// secret and open `sealed`. Used to escrow a note's DEK to the master key so
/// it can be recovered without the note password.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EciesSealed {
    /// The ephemeral X25519 public key (32 bytes) generated for this seal.
    pub ephemeral_public: Vec<u8>,
    /// The payload sealed under the key derived from the X25519 shared secret.
    pub sealed: Sealed,
}

/// Failures at the crypto boundary. Deliberately coarse: we never reveal whether
/// a decryption failure was caused by a wrong passphrase or by tampered data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CryptoError {
    /// The passphrase could not be turned into a key.
    KeyDerivation,
    /// The plaintext could not be encrypted.
    Encryption,
    /// Decryption failed: wrong passphrase or the data was tampered with.
    InvalidKeyOrTampered,
}

impl std::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            CryptoError::KeyDerivation => "could not derive a key from the passphrase",
            CryptoError::Encryption => "could not encrypt the content",
            CryptoError::InvalidKeyOrTampered => "wrong passphrase or corrupted data",
        };
        f.write_str(message)
    }
}

impl std::error::Error for CryptoError {}
