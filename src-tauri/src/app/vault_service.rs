//! Crypto/recovery use cases for the per-note protection model.
//!
//! This service is stateless: it never holds a derived key or a DEK. It owns
//! the master-recovery record (the X25519 keypair) and exposes the primitive
//! operations that [`super::secure_notes::SecureNotesService`] composes:
//!
//! * master-recovery setup and private-key recovery,
//! * generating a fresh DEK,
//! * sealing/opening note content with a DEK,
//! * wrapping/unwrapping a DEK under a note-password-derived key,
//! * escrowing/recovering a DEK to/from the master public key.
//!
//! All curve and symmetric crypto stays behind the `Cipher` port.

use crate::domain::encryption::{DerivedKey, EciesSealed, Sealed, PAYLOAD_VERSION};
use crate::domain::vault::{MasterRecord, VaultError};
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

    // --- Master recovery ---

    /// Whether the master-recovery record has been set up.
    pub fn is_initialized(&self) -> Result<bool, VaultError> {
        Ok(self.repository.load()?.is_some())
    }

    /// Set up master recovery for the first time: generate an X25519 keypair,
    /// seal the private key under a master-pass-derived key, and persist the
    /// record. Errors if recovery already exists.
    pub fn set_up_recovery(&mut self, master_pass: &str) -> Result<(), VaultError> {
        if self.repository.load()?.is_some() {
            return Err(VaultError::AlreadyExists);
        }

        let (public_key, private_key) = self.cipher.generate_keypair()?;

        let kdf_salt = self.cipher.generate_salt();
        let kek = self.cipher.derive_key(master_pass, &kdf_salt)?;
        let private_key_sealed = self.cipher.encrypt(&kek, &private_key)?;

        self.repository.save(MasterRecord {
            version: PAYLOAD_VERSION,
            kdf_salt,
            public_key,
            private_key_sealed,
        })?;

        Ok(())
    }

    /// The master X25519 public key, used to escrow note DEKs with no prompt.
    /// `NotInitialized` if recovery has not been set up.
    pub fn master_public_key(&self) -> Result<Vec<u8>, VaultError> {
        let record = self.repository.load()?.ok_or(VaultError::NotInitialized)?;
        Ok(record.public_key)
    }

    /// Recover the master X25519 private key by decrypting it with the
    /// master-pass-derived key. A wrong passphrase fails AEAD and surfaces as
    /// `InvalidPassphrase`.
    pub fn recover_private_key(&self, master_pass: &str) -> Result<Vec<u8>, VaultError> {
        let record = self.repository.load()?.ok_or(VaultError::NotInitialized)?;
        let kek = self.cipher.derive_key(master_pass, &record.kdf_salt)?;
        self.cipher
            .decrypt(&kek, &record.private_key_sealed)
            .map_err(|_| VaultError::InvalidPassphrase)
    }

    // --- DEK primitives ---

    /// Generate a fresh random Data Encryption Key.
    pub fn generate_dek(&self) -> Result<DerivedKey, VaultError> {
        Ok(self.cipher.generate_dek()?)
    }

    /// Seal note plaintext with a DEK.
    pub fn seal_content(&self, dek: &DerivedKey, plaintext: &str) -> Result<Sealed, VaultError> {
        Ok(self.cipher.encrypt(dek, plaintext.as_bytes())?)
    }

    /// Open sealed note content with a DEK back to plaintext.
    pub fn open_content(&self, dek: &DerivedKey, sealed: &Sealed) -> Result<String, VaultError> {
        let bytes = self
            .cipher
            .decrypt(dek, sealed)
            .map_err(|_| VaultError::InvalidPassphrase)?;
        String::from_utf8(bytes).map_err(|_| VaultError::CorruptPayload)
    }

    /// Wrap a DEK under a key derived from the note password, returning the
    /// sealed DEK and the note salt that was used.
    pub fn wrap_dek_by_pass(
        &self,
        dek: &DerivedKey,
        note_pass: &str,
        note_salt: &[u8],
    ) -> Result<Sealed, VaultError> {
        let pass_key = self.cipher.derive_key(note_pass, note_salt)?;
        Ok(self.cipher.encrypt(&pass_key, dek.bytes())?)
    }

    /// Unwrap a DEK from `dek_by_pass` using the note password and salt. A wrong
    /// note password fails AEAD and surfaces as `InvalidPassphrase`.
    pub fn unwrap_dek_by_pass(
        &self,
        dek_by_pass: &Sealed,
        note_pass: &str,
        note_salt: &[u8],
    ) -> Result<DerivedKey, VaultError> {
        let pass_key = self.cipher.derive_key(note_pass, note_salt)?;
        let bytes = self
            .cipher
            .decrypt(&pass_key, dek_by_pass)
            .map_err(|_| VaultError::InvalidPassphrase)?;
        to_dek(bytes)
    }

    /// Escrow a DEK to the master public key so it can be recovered without the
    /// note password.
    pub fn escrow_dek(&self, dek: &DerivedKey) -> Result<EciesSealed, VaultError> {
        let public_key = self.master_public_key()?;
        Ok(self.cipher.ecies_seal(&public_key, dek.bytes())?)
    }

    /// Recover a DEK from its escrow using the master private key.
    pub fn recover_dek_from_escrow(
        &self,
        private_key: &[u8],
        escrow: &EciesSealed,
    ) -> Result<DerivedKey, VaultError> {
        let bytes = self
            .cipher
            .ecies_open(private_key, escrow)
            .map_err(|_| VaultError::InvalidPassphrase)?;
        to_dek(bytes)
    }

    /// A fresh per-note salt.
    pub fn generate_salt(&self) -> Vec<u8> {
        self.cipher.generate_salt()
    }
}

/// Turn raw 32-byte DEK material back into a `DerivedKey`.
fn to_dek(bytes: Vec<u8>) -> Result<DerivedKey, VaultError> {
    if bytes.len() != 32 {
        return Err(VaultError::CorruptPayload);
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Ok(DerivedKey::new(key))
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
        vault.set_up_recovery("master-pass").unwrap();
        assert!(vault.is_initialized().unwrap());
    }

    #[test]
    fn set_up_recovery_twice_fails() {
        let mut vault = service();
        vault.set_up_recovery("master-pass").unwrap();
        assert_eq!(
            vault.set_up_recovery("master-pass").err(),
            Some(VaultError::AlreadyExists)
        );
    }

    #[test]
    fn recover_private_key_with_correct_pass_succeeds() {
        let mut vault = service();
        vault.set_up_recovery("master-pass").unwrap();
        let private = vault.recover_private_key("master-pass").unwrap();
        assert_eq!(private.len(), 32);
    }

    #[test]
    fn recover_private_key_with_wrong_pass_fails() {
        let mut vault = service();
        vault.set_up_recovery("master-pass").unwrap();
        assert_eq!(
            vault.recover_private_key("wrong-pass").err(),
            Some(VaultError::InvalidPassphrase)
        );
    }

    #[test]
    fn recover_before_setup_reports_not_initialized() {
        let vault = service();
        assert_eq!(
            vault.recover_private_key("whatever").err(),
            Some(VaultError::NotInitialized)
        );
    }

    #[test]
    fn dek_seals_and_opens_content() {
        let vault = service();
        let dek = vault.generate_dek().unwrap();
        let sealed = vault.seal_content(&dek, "secret body").unwrap();
        assert_eq!(vault.open_content(&dek, &sealed).unwrap(), "secret body");
    }

    #[test]
    fn wrap_then_unwrap_dek_by_pass_roundtrips() {
        let vault = service();
        let dek = vault.generate_dek().unwrap();
        let salt = vault.generate_salt();

        let wrapped = vault.wrap_dek_by_pass(&dek, "1234", &salt).unwrap();
        let unwrapped = vault.unwrap_dek_by_pass(&wrapped, "1234", &salt).unwrap();

        // The unwrapped DEK opens content sealed by the original DEK.
        let sealed = vault.seal_content(&dek, "body").unwrap();
        assert_eq!(vault.open_content(&unwrapped, &sealed).unwrap(), "body");
    }

    #[test]
    fn unwrap_dek_with_wrong_pass_fails() {
        let vault = service();
        let dek = vault.generate_dek().unwrap();
        let salt = vault.generate_salt();
        let wrapped = vault.wrap_dek_by_pass(&dek, "1234", &salt).unwrap();

        assert_eq!(
            vault.unwrap_dek_by_pass(&wrapped, "wrong", &salt).err(),
            Some(VaultError::InvalidPassphrase)
        );
    }

    #[test]
    fn escrow_then_recover_dek_roundtrips() {
        let mut vault = service();
        vault.set_up_recovery("master-pass").unwrap();
        let dek = vault.generate_dek().unwrap();

        let escrow = vault.escrow_dek(&dek).unwrap();
        let private = vault.recover_private_key("master-pass").unwrap();
        let recovered = vault.recover_dek_from_escrow(&private, &escrow).unwrap();

        let sealed = vault.seal_content(&dek, "escrowed body").unwrap();
        assert_eq!(
            vault.open_content(&recovered, &sealed).unwrap(),
            "escrowed body"
        );
    }
}
