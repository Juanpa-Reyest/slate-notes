//! Concrete crypto adapter: Argon2id for key derivation and XChaCha20-Poly1305
//! for authenticated encryption.

use argon2::Argon2;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};

use crate::domain::encryption::{CryptoError, DerivedKey, Sealed};
use crate::ports::cipher::Cipher;

const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 24;

pub struct XChaChaCipher;

impl XChaChaCipher {
    pub fn new() -> Self {
        Self
    }
}

impl Default for XChaChaCipher {
    fn default() -> Self {
        Self::new()
    }
}

impl Cipher for XChaChaCipher {
    fn generate_salt(&self) -> Vec<u8> {
        let mut salt = vec![0u8; SALT_LEN];
        getrandom::getrandom(&mut salt).expect("operating system RNG must be available");
        salt
    }

    fn derive_key(&self, passphrase: &str, salt: &[u8]) -> Result<DerivedKey, CryptoError> {
        let mut key = [0u8; 32];
        Argon2::default()
            .hash_password_into(passphrase.as_bytes(), salt, &mut key)
            .map_err(|_| CryptoError::KeyDerivation)?;
        Ok(DerivedKey::new(key))
    }

    fn encrypt(&self, key: &DerivedKey, plaintext: &[u8]) -> Result<Sealed, CryptoError> {
        let cipher =
            XChaCha20Poly1305::new_from_slice(key.bytes()).map_err(|_| CryptoError::Encryption)?;

        let mut nonce_bytes = [0u8; NONCE_LEN];
        getrandom::getrandom(&mut nonce_bytes).map_err(|_| CryptoError::Encryption)?;
        let nonce = XNonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|_| CryptoError::Encryption)?;

        Ok(Sealed {
            nonce: nonce_bytes.to_vec(),
            ciphertext,
        })
    }

    fn decrypt(&self, key: &DerivedKey, sealed: &Sealed) -> Result<Vec<u8>, CryptoError> {
        if sealed.nonce.len() != NONCE_LEN {
            return Err(CryptoError::InvalidKeyOrTampered);
        }

        let cipher = XChaCha20Poly1305::new_from_slice(key.bytes())
            .map_err(|_| CryptoError::InvalidKeyOrTampered)?;
        let nonce = XNonce::from_slice(&sealed.nonce);

        cipher
            .decrypt(nonce, sealed.ciphertext.as_ref())
            .map_err(|_| CryptoError::InvalidKeyOrTampered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_then_open_roundtrips() {
        let cipher = XChaChaCipher::new();
        let salt = cipher.generate_salt();
        let key = cipher
            .derive_key("correct horse battery staple", &salt)
            .unwrap();

        let sealed = cipher.encrypt(&key, b"my secret note").unwrap();
        let plaintext = cipher.decrypt(&key, &sealed).unwrap();

        assert_eq!(plaintext, b"my secret note");
    }

    #[test]
    fn wrong_passphrase_cannot_decrypt() {
        let cipher = XChaChaCipher::new();
        let salt = cipher.generate_salt();
        let key = cipher.derive_key("right-passphrase", &salt).unwrap();
        let sealed = cipher.encrypt(&key, b"secret").unwrap();

        let wrong_key = cipher.derive_key("wrong-passphrase", &salt).unwrap();

        assert_eq!(
            cipher.decrypt(&wrong_key, &sealed),
            Err(CryptoError::InvalidKeyOrTampered)
        );
    }

    #[test]
    fn tampered_ciphertext_is_rejected() {
        let cipher = XChaChaCipher::new();
        let salt = cipher.generate_salt();
        let key = cipher.derive_key("passphrase", &salt).unwrap();
        let mut sealed = cipher.encrypt(&key, b"secret").unwrap();

        sealed.ciphertext[0] ^= 0xff;

        assert_eq!(
            cipher.decrypt(&key, &sealed),
            Err(CryptoError::InvalidKeyOrTampered)
        );
    }

    #[test]
    fn each_encryption_uses_a_fresh_nonce() {
        let cipher = XChaChaCipher::new();
        let salt = cipher.generate_salt();
        let key = cipher.derive_key("passphrase", &salt).unwrap();

        let first = cipher.encrypt(&key, b"same plaintext").unwrap();
        let second = cipher.encrypt(&key, b"same plaintext").unwrap();

        assert_ne!(first.nonce, second.nonce);
        assert_ne!(first.ciphertext, second.ciphertext);
    }

    #[test]
    fn generated_salt_has_expected_length_and_varies() {
        let cipher = XChaChaCipher::new();

        let first = cipher.generate_salt();
        let second = cipher.generate_salt();

        assert_eq!(first.len(), SALT_LEN);
        assert_ne!(first, second);
    }
}
