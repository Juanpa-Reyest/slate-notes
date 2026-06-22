//! The vault use cases under STRICT PER-NOTE authentication.
//!
//! There is NO persistent unlocked session and NO auto-lock timer. Each
//! protected operation derives its key from a passphrase supplied at that
//! moment. The service is stateless: it never holds a derived key. Callers that
//! need to reuse a key transiently (e.g. autosave of the currently-open note)
//! own the `DerivedKey` returned by `create`/`verify_key` and pass it back into
//! `seal_with`/`open_with`.

use crate::domain::encryption::{DerivedKey, Sealed, PAYLOAD_VERSION};
use crate::domain::vault::{VaultError, VaultRecord, VAULT_SENTINEL};
use crate::ports::cipher::Cipher;
use crate::ports::vault_repository::VaultRepository;

pub struct VaultService<R, C> {
    repository: R,
    cipher: C,
}

impl<R, C> VaultService<R, C>
where
    R: VaultRepository,
    C: Cipher,
{
    pub fn new(repository: R, cipher: C) -> Self {
        Self { repository, cipher }
    }

    /// Whether a vault record exists yet.
    pub fn is_initialized(&self) -> Result<bool, VaultError> {
        Ok(self.repository.load()?.is_some())
    }

    /// Create the vault for the first time. Returns the derived key to the
    /// caller; the service keeps no session.
    pub fn create(&mut self, passphrase: &str) -> Result<DerivedKey, VaultError> {
        if self.repository.load()?.is_some() {
            return Err(VaultError::AlreadyExists);
        }

        let salt = self.cipher.generate_salt();
        let key = self.cipher.derive_key(passphrase, &salt)?;
        let sentinel = self.cipher.encrypt(&key, VAULT_SENTINEL)?;

        self.repository.save(VaultRecord {
            version: PAYLOAD_VERSION,
            salt,
            sentinel,
        })?;

        Ok(key)
    }

    /// Verify a passphrase against the stored sentinel and return the derived
    /// key. `NotInitialized` if no vault exists; `InvalidPassphrase` on mismatch.
    pub fn verify_key(&self, passphrase: &str) -> Result<DerivedKey, VaultError> {
        let record = self.repository.load()?.ok_or(VaultError::NotInitialized)?;

        let key = self.cipher.derive_key(passphrase, &record.salt)?;
        let opened = self
            .cipher
            .decrypt(&key, &record.sentinel)
            .map_err(|_| VaultError::InvalidPassphrase)?;

        if opened != VAULT_SENTINEL {
            return Err(VaultError::InvalidPassphrase);
        }

        Ok(key)
    }

    /// Encrypt plaintext note content into a single storable string using the
    /// supplied derived key.
    pub fn seal_with(&self, key: &DerivedKey, plaintext: &str) -> Result<String, VaultError> {
        let sealed = self.cipher.encrypt(key, plaintext.as_bytes())?;
        Ok(encode_sealed(&sealed))
    }

    /// Decrypt stored protected content back to plaintext using the supplied key.
    pub fn open_with(&self, key: &DerivedKey, stored: &str) -> Result<String, VaultError> {
        let sealed = decode_sealed(stored).ok_or(VaultError::InvalidPassphrase)?;
        let bytes = self
            .cipher
            .decrypt(key, &sealed)
            .map_err(|_| VaultError::InvalidPassphrase)?;
        String::from_utf8(bytes).map_err(|_| VaultError::InvalidPassphrase)
    }
}

/// Serialize a sealed payload to a string for storage in the note content column.
fn encode_sealed(sealed: &Sealed) -> String {
    serde_json::to_string(sealed).expect("a sealed payload always serializes")
}

/// Parse a stored string back into a sealed payload (None if it is not one).
fn decode_sealed(stored: &str) -> Option<Sealed> {
    serde_json::from_str(stored).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::memory_vault_repository::MemoryVaultRepository;
    use crate::infra::xchacha_cipher::XChaChaCipher;

    fn service() -> VaultService<MemoryVaultRepository, XChaChaCipher> {
        VaultService::new(MemoryVaultRepository::default(), XChaChaCipher::new())
    }

    #[test]
    fn is_initialized_reports_false_then_true() {
        let mut vault = service();
        assert!(!vault.is_initialized().unwrap());
        vault.create("master-pass").unwrap();
        assert!(vault.is_initialized().unwrap());
    }

    #[test]
    fn create_returns_a_usable_key() {
        let mut vault = service();
        let key = vault.create("master-pass").unwrap();

        // The returned key seals and opens content without re-prompting.
        let sealed = vault.seal_with(&key, "top secret").unwrap();
        assert_eq!(vault.open_with(&key, &sealed).unwrap(), "top secret");
    }

    #[test]
    fn create_twice_fails() {
        let mut vault = service();
        vault.create("master-pass").unwrap();
        assert_eq!(
            vault.create("master-pass").err(),
            Some(VaultError::AlreadyExists)
        );
    }

    #[test]
    fn verify_key_before_creation_reports_not_initialized() {
        let vault = service();
        assert_eq!(
            vault.verify_key("whatever").err(),
            Some(VaultError::NotInitialized)
        );
    }

    #[test]
    fn verify_key_succeeds_with_correct_passphrase() {
        let mut vault = service();
        let created = vault.create("master-pass").unwrap();
        let sealed = vault.seal_with(&created, "body").unwrap();

        let verified = vault.verify_key("master-pass").unwrap();
        // A freshly verified key opens content sealed by the original key.
        assert_eq!(vault.open_with(&verified, &sealed).unwrap(), "body");
    }

    #[test]
    fn verify_key_fails_with_wrong_passphrase() {
        let mut vault = service();
        vault.create("master-pass").unwrap();
        assert_eq!(
            vault.verify_key("wrong-pass").err(),
            Some(VaultError::InvalidPassphrase)
        );
    }

    #[test]
    fn seal_then_open_roundtrips() {
        let mut vault = service();
        let key = vault.create("master-pass").unwrap();

        let sealed = vault.seal_with(&key, "classified").unwrap();
        assert_eq!(vault.open_with(&key, &sealed).unwrap(), "classified");
    }

    #[test]
    fn open_with_wrong_key_fails() {
        let mut creator = service();
        let key = creator.create("master-pass").unwrap();
        let sealed = creator.seal_with(&key, "classified").unwrap();

        // A key derived from a different passphrase/salt cannot open the payload.
        let mut other = service();
        let other_key = other.create("different-pass").unwrap();
        assert_eq!(
            creator.open_with(&other_key, &sealed).err(),
            Some(VaultError::InvalidPassphrase)
        );
    }
}
