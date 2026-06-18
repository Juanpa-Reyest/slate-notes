//! The crypto boundary. The application depends on this trait, never on a
//! concrete algorithm. Implementations live in `infra`.

use crate::domain::encryption::{CryptoError, DerivedKey, Sealed};

pub trait Cipher {
    /// Generate a fresh random salt for key derivation.
    fn generate_salt(&self) -> Vec<u8>;

    /// Derive a symmetric key from a passphrase and salt using a memory-hard KDF.
    fn derive_key(&self, passphrase: &str, salt: &[u8]) -> Result<DerivedKey, CryptoError>;

    /// Encrypt plaintext with a derived key, producing a fresh nonce each time.
    fn encrypt(&self, key: &DerivedKey, plaintext: &[u8]) -> Result<Sealed, CryptoError>;

    /// Decrypt a sealed payload with a derived key.
    fn decrypt(&self, key: &DerivedKey, sealed: &Sealed) -> Result<Vec<u8>, CryptoError>;
}
