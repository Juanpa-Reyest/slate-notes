//! Concrete crypto adapter: Argon2id for key derivation and XChaCha20-Poly1305
//! for authenticated encryption.

use argon2::Argon2;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use x25519_dalek::{PublicKey, StaticSecret};

use crate::domain::encryption::{CryptoError, DerivedKey, EciesSealed, Sealed};
use crate::ports::cipher::Cipher;

const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 24;
const X25519_KEY_LEN: usize = 32;

/// Fixed salt used to derive a symmetric key from an X25519 shared secret. The
/// shared secret is already high-entropy, so a fixed salt is safe here; reusing
/// Argon2id keeps all symmetric-key derivation behind one primitive.
const ECIES_KDF_SALT: &[u8] = b"slate-ecies-kdf1";

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

    fn generate_dek(&self) -> Result<DerivedKey, CryptoError> {
        let mut dek = [0u8; 32];
        getrandom::getrandom(&mut dek).map_err(|_| CryptoError::KeyDerivation)?;
        Ok(DerivedKey::new(dek))
    }

    fn generate_keypair(&self) -> Result<(Vec<u8>, Vec<u8>), CryptoError> {
        let mut secret_bytes = [0u8; X25519_KEY_LEN];
        getrandom::getrandom(&mut secret_bytes).map_err(|_| CryptoError::KeyDerivation)?;
        let secret = StaticSecret::from(secret_bytes);
        let public = PublicKey::from(&secret);
        Ok((public.as_bytes().to_vec(), secret.to_bytes().to_vec()))
    }

    fn ecies_seal(
        &self,
        public_key: &[u8],
        plaintext: &[u8],
    ) -> Result<EciesSealed, CryptoError> {
        let recipient = parse_public_key(public_key)?;

        // Fresh ephemeral keypair for this seal.
        let mut ephemeral_bytes = [0u8; X25519_KEY_LEN];
        getrandom::getrandom(&mut ephemeral_bytes).map_err(|_| CryptoError::Encryption)?;
        let ephemeral_secret = StaticSecret::from(ephemeral_bytes);
        let ephemeral_public = PublicKey::from(&ephemeral_secret);

        let shared = ephemeral_secret.diffie_hellman(&recipient);
        let symmetric_key = derive_from_shared_secret(shared.as_bytes())?;
        let sealed = self.encrypt(&symmetric_key, plaintext)?;

        Ok(EciesSealed {
            ephemeral_public: ephemeral_public.as_bytes().to_vec(),
            sealed,
        })
    }

    fn ecies_open(
        &self,
        private_key: &[u8],
        sealed: &EciesSealed,
    ) -> Result<Vec<u8>, CryptoError> {
        let secret_bytes = to_array(private_key).ok_or(CryptoError::InvalidKeyOrTampered)?;
        let secret = StaticSecret::from(secret_bytes);
        let ephemeral = parse_public_key(&sealed.ephemeral_public)
            .map_err(|_| CryptoError::InvalidKeyOrTampered)?;

        let shared = secret.diffie_hellman(&ephemeral);
        let symmetric_key = derive_from_shared_secret(shared.as_bytes())
            .map_err(|_| CryptoError::InvalidKeyOrTampered)?;
        self.decrypt(&symmetric_key, &sealed.sealed)
    }
}

/// Parse a 32-byte X25519 public key from a slice.
fn parse_public_key(bytes: &[u8]) -> Result<PublicKey, CryptoError> {
    let array = to_array(bytes).ok_or(CryptoError::InvalidKeyOrTampered)?;
    Ok(PublicKey::from(array))
}

fn to_array(bytes: &[u8]) -> Option<[u8; X25519_KEY_LEN]> {
    if bytes.len() != X25519_KEY_LEN {
        return None;
    }
    let mut array = [0u8; X25519_KEY_LEN];
    array.copy_from_slice(bytes);
    Some(array)
}

/// Derive a symmetric XChaCha key from an X25519 shared secret using Argon2id
/// with a fixed salt. The shared secret carries the entropy; the fixed salt is
/// safe and keeps symmetric-key derivation behind a single primitive.
fn derive_from_shared_secret(shared: &[u8]) -> Result<DerivedKey, CryptoError> {
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(shared, ECIES_KDF_SALT, &mut key)
        .map_err(|_| CryptoError::KeyDerivation)?;
    Ok(DerivedKey::new(key))
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

    #[test]
    fn generated_dek_seals_and_opens_content() {
        let cipher = XChaChaCipher::new();
        let dek = cipher.generate_dek().unwrap();

        let sealed = cipher.encrypt(&dek, b"note body").unwrap();
        assert_eq!(cipher.decrypt(&dek, &sealed).unwrap(), b"note body");
    }

    #[test]
    fn ecies_roundtrips_through_keypair() {
        let cipher = XChaChaCipher::new();
        let (public, private) = cipher.generate_keypair().unwrap();

        let sealed = cipher.ecies_seal(&public, b"the dek bytes").unwrap();
        let opened = cipher.ecies_open(&private, &sealed).unwrap();

        assert_eq!(opened, b"the dek bytes");
        assert_eq!(public.len(), X25519_KEY_LEN);
        assert_eq!(private.len(), X25519_KEY_LEN);
    }

    #[test]
    fn ecies_uses_a_fresh_ephemeral_key_each_time() {
        let cipher = XChaChaCipher::new();
        let (public, _private) = cipher.generate_keypair().unwrap();

        let first = cipher.ecies_seal(&public, b"same").unwrap();
        let second = cipher.ecies_seal(&public, b"same").unwrap();

        assert_ne!(first.ephemeral_public, second.ephemeral_public);
        assert_ne!(first.sealed.ciphertext, second.sealed.ciphertext);
    }

    #[test]
    fn ecies_open_with_wrong_private_key_fails() {
        let cipher = XChaChaCipher::new();
        let (public, _private) = cipher.generate_keypair().unwrap();
        let (_other_public, other_private) = cipher.generate_keypair().unwrap();

        let sealed = cipher.ecies_seal(&public, b"secret dek").unwrap();

        assert_eq!(
            cipher.ecies_open(&other_private, &sealed),
            Err(CryptoError::InvalidKeyOrTampered)
        );
    }
}
