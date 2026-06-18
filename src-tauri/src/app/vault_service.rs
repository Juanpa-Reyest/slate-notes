//! The vault use cases: create, unlock, lock, and (while unlocked) seal/open
//! protected content. The derived key lives only inside the live session and is
//! cleared on lock or after an inactivity timeout (auto-lock).

use crate::domain::encryption::{DerivedKey, Sealed, PAYLOAD_VERSION};
use crate::domain::vault::{VaultError, VaultRecord, VaultStatus, VAULT_SENTINEL};
use crate::ports::cipher::Cipher;
use crate::ports::clock::Clock;
use crate::ports::vault_repository::VaultRepository;

/// A live unlocked session: the derived key plus the last time it was used.
struct Session {
    key: DerivedKey,
    last_activity: u64,
}

pub struct VaultService<R, C, K> {
    repository: R,
    cipher: C,
    clock: K,
    auto_lock_secs: u64,
    session: Option<Session>,
}

impl<R, C, K> VaultService<R, C, K>
where
    R: VaultRepository,
    C: Cipher,
    K: Clock,
{
    pub fn new(repository: R, cipher: C, clock: K, auto_lock_secs: u64) -> Self {
        Self {
            repository,
            cipher,
            clock,
            auto_lock_secs,
            session: None,
        }
    }

    /// Clear the session if it has been idle past the auto-lock window.
    fn enforce_auto_lock(&mut self) {
        if let Some(session) = &self.session {
            let idle = self.clock.now_secs().saturating_sub(session.last_activity);
            if idle >= self.auto_lock_secs {
                self.session = None;
            }
        }
    }

    fn start_session(&mut self, key: DerivedKey) {
        self.session = Some(Session {
            key,
            last_activity: self.clock.now_secs(),
        });
    }

    pub fn status(&mut self) -> Result<VaultStatus, VaultError> {
        self.enforce_auto_lock();
        let initialized = self.repository.load()?.is_some();
        Ok(VaultStatus {
            initialized,
            unlocked: self.session.is_some(),
        })
    }

    /// Create the vault for the first time and leave it unlocked.
    pub fn create(&mut self, passphrase: &str) -> Result<(), VaultError> {
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

        self.start_session(key);
        Ok(())
    }

    /// Unlock an existing vault by verifying the passphrase against the sentinel.
    pub fn unlock(&mut self, passphrase: &str) -> Result<(), VaultError> {
        let record = self.repository.load()?.ok_or(VaultError::NotInitialized)?;

        let key = self.cipher.derive_key(passphrase, &record.salt)?;
        let opened = self
            .cipher
            .decrypt(&key, &record.sentinel)
            .map_err(|_| VaultError::InvalidPassphrase)?;

        if opened != VAULT_SENTINEL {
            return Err(VaultError::InvalidPassphrase);
        }

        self.start_session(key);
        Ok(())
    }

    /// Lock the vault, wiping the derived key from memory.
    pub fn lock(&mut self) {
        self.session = None;
    }

    /// Encrypt protected content. Requires an unlocked vault.
    pub fn seal(&mut self, plaintext: &[u8]) -> Result<Sealed, VaultError> {
        self.enforce_auto_lock();
        let now = self.clock.now_secs();
        let session = self.session.as_mut().ok_or(VaultError::Locked)?;
        let sealed = self.cipher.encrypt(&session.key, plaintext)?;
        session.last_activity = now;
        Ok(sealed)
    }

    /// Decrypt protected content. Requires an unlocked vault.
    pub fn open(&mut self, sealed: &Sealed) -> Result<Vec<u8>, VaultError> {
        self.enforce_auto_lock();
        let now = self.clock.now_secs();
        let session = self.session.as_mut().ok_or(VaultError::Locked)?;
        let plaintext = self
            .cipher
            .decrypt(&session.key, sealed)
            .map_err(|_| VaultError::InvalidPassphrase)?;
        session.last_activity = now;
        Ok(plaintext)
    }

    /// Encrypt plaintext note content into a single storable string. Unlocked only.
    pub fn protect(&mut self, plaintext: &str) -> Result<String, VaultError> {
        let sealed = self.seal(plaintext.as_bytes())?;
        Ok(encode_sealed(&sealed))
    }

    /// Decrypt stored protected content back to plaintext. Unlocked only.
    pub fn reveal(&mut self, stored: &str) -> Result<String, VaultError> {
        let sealed = decode_sealed(stored).ok_or(VaultError::InvalidPassphrase)?;
        let bytes = self.open(&sealed)?;
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
    use std::cell::Cell;
    use std::rc::Rc;

    use super::*;
    use crate::infra::memory_vault_repository::MemoryVaultRepository;
    use crate::infra::xchacha_cipher::XChaChaCipher;

    /// A controllable clock for deterministic auto-lock tests.
    #[derive(Clone)]
    struct FakeClock(Rc<Cell<u64>>);

    impl FakeClock {
        fn new(start: u64) -> Self {
            Self(Rc::new(Cell::new(start)))
        }
        fn advance(&self, secs: u64) {
            self.0.set(self.0.get() + secs);
        }
    }

    impl Clock for FakeClock {
        fn now_secs(&self) -> u64 {
            self.0.get()
        }
    }

    fn service(clock: FakeClock, auto_lock_secs: u64) -> VaultService<MemoryVaultRepository, XChaChaCipher, FakeClock> {
        VaultService::new(
            MemoryVaultRepository::default(),
            XChaChaCipher::new(),
            clock,
            auto_lock_secs,
        )
    }

    #[test]
    fn create_initializes_and_unlocks() {
        let mut vault = service(FakeClock::new(1000), 300);
        vault.create("master-pass").unwrap();

        let status = vault.status().unwrap();
        assert!(status.initialized);
        assert!(status.unlocked);
    }

    #[test]
    fn create_twice_fails() {
        let mut vault = service(FakeClock::new(1000), 300);
        vault.create("master-pass").unwrap();
        assert_eq!(vault.create("master-pass"), Err(VaultError::AlreadyExists));
    }

    #[test]
    fn unlock_with_correct_passphrase_succeeds() {
        let mut vault = service(FakeClock::new(1000), 300);
        vault.create("master-pass").unwrap();
        vault.lock();
        assert!(!vault.status().unwrap().unlocked);

        vault.unlock("master-pass").unwrap();
        assert!(vault.status().unwrap().unlocked);
    }

    #[test]
    fn unlock_with_wrong_passphrase_fails_and_stays_locked() {
        let mut vault = service(FakeClock::new(1000), 300);
        vault.create("master-pass").unwrap();
        vault.lock();

        assert_eq!(vault.unlock("wrong-pass"), Err(VaultError::InvalidPassphrase));
        assert!(!vault.status().unwrap().unlocked);
    }

    #[test]
    fn unlock_before_creation_reports_not_initialized() {
        let mut vault = service(FakeClock::new(1000), 300);
        assert_eq!(vault.unlock("whatever"), Err(VaultError::NotInitialized));
    }

    #[test]
    fn lock_clears_the_session() {
        let mut vault = service(FakeClock::new(1000), 300);
        vault.create("master-pass").unwrap();
        vault.lock();
        assert!(!vault.status().unwrap().unlocked);
    }

    #[test]
    fn auto_lock_relocks_after_inactivity() {
        let clock = FakeClock::new(1000);
        let mut vault = service(clock.clone(), 300);
        vault.create("master-pass").unwrap();
        assert!(vault.status().unwrap().unlocked);

        clock.advance(300);
        assert!(!vault.status().unwrap().unlocked);
        assert_eq!(vault.open(&dummy_sealed()), Err(VaultError::Locked));
    }

    #[test]
    fn activity_keeps_the_session_alive() {
        let clock = FakeClock::new(1000);
        let mut vault = service(clock.clone(), 300);
        vault.create("master-pass").unwrap();

        clock.advance(200);
        let sealed = vault.seal(b"note body").unwrap(); // refreshes activity
        clock.advance(200); // 200 since last activity, below 300
        assert!(vault.status().unwrap().unlocked);
        assert_eq!(vault.open(&sealed).unwrap(), b"note body");
    }

    #[test]
    fn seal_then_open_roundtrips_while_unlocked() {
        let mut vault = service(FakeClock::new(1000), 300);
        vault.create("master-pass").unwrap();

        let sealed = vault.seal(b"top secret").unwrap();
        assert_eq!(vault.open(&sealed).unwrap(), b"top secret");
    }

    #[test]
    fn seal_when_locked_fails() {
        let mut vault = service(FakeClock::new(1000), 300);
        vault.create("master-pass").unwrap();
        vault.lock();
        assert_eq!(vault.seal(b"secret"), Err(VaultError::Locked));
    }

    fn dummy_sealed() -> Sealed {
        Sealed {
            nonce: vec![0; 24],
            ciphertext: vec![0; 32],
        }
    }
}
