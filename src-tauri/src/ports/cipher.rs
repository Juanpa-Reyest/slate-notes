//! The crypto boundary. The application depends on this trait, never on a
//! concrete algorithm. Implementations live in `infra`.

use crate::domain::encryption::{CryptoError, DerivedKey, EciesSealed, Sealed};

pub trait Cipher {
    /// Generate a fresh random salt for key derivation.
    fn generate_salt(&self) -> Vec<u8>;

    /// Derive a symmetric key from a passphrase and salt using a memory-hard KDF.
    fn derive_key(&self, passphrase: &str, salt: &[u8]) -> Result<DerivedKey, CryptoError>;

    /// Generate a fresh random 32-byte Data Encryption Key (DEK).
    fn generate_dek(&self) -> Result<DerivedKey, CryptoError>;

    /// Encrypt plaintext with a derived key, producing a fresh nonce each time.
    fn encrypt(&self, key: &DerivedKey, plaintext: &[u8]) -> Result<Sealed, CryptoError>;

    /// Decrypt a sealed payload with a derived key.
    fn decrypt(&self, key: &DerivedKey, sealed: &Sealed) -> Result<Vec<u8>, CryptoError>;

    /// Generate a fresh X25519 keypair, returning `(public_key, private_key)` as
    /// 32-byte vectors.
    fn generate_keypair(&self) -> Result<(Vec<u8>, Vec<u8>), CryptoError>;

    /// Seal `plaintext` to an X25519 public key using ECIES: a fresh ephemeral
    /// keypair is generated, a shared secret is computed against `public_key`, a
    /// symmetric key is derived from it, and `plaintext` is encrypted under that
    /// key. The holder of the matching private key can open it with `ecies_open`.
    fn ecies_seal(
        &self,
        public_key: &[u8],
        plaintext: &[u8],
    ) -> Result<EciesSealed, CryptoError>;

    /// Open an `EciesSealed` payload using the X25519 private key matching the
    /// public key it was sealed to.
    fn ecies_open(
        &self,
        private_key: &[u8],
        sealed: &EciesSealed,
    ) -> Result<Vec<u8>, CryptoError>;
}
