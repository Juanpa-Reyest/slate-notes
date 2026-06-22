//! Domain types for the protected-notes vault.
//!
//! A single vault guards all protected notes with one master passphrase. The
//! passphrase is never stored: we keep a salt plus a "sentinel" — a known
//! constant encrypted under the derived key — so a passphrase can be verified
//! by decrypting the sentinel and comparing it. There is no persistent unlocked
//! session: each protected operation re-derives its key from the passphrase
//! supplied at that moment (strict per-note authentication).

use serde::Serialize;

use crate::domain::encryption::{CryptoError, Sealed};

/// The fixed plaintext sealed into the sentinel. If it decrypts back to this,
/// the passphrase was correct.
pub const VAULT_SENTINEL: &[u8] = b"slate-vault-v1";

/// The persisted vault: salt for key derivation plus the encrypted sentinel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultRecord {
    pub version: u32,
    pub salt: Vec<u8>,
    pub sentinel: Sealed,
}

/// Whether a vault exists yet, and whether a protected note is currently open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatus {
    pub initialized: bool,
    /// True while a protected note is open and its transient key is held
    /// (under per-note auth there is no global unlocked session).
    pub unlocked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VaultError {
    /// A vault already exists; it cannot be created twice.
    AlreadyExists,
    /// No vault has been created yet.
    NotInitialized,
    /// The note is not the currently open protected note; reveal it first.
    Locked,
    /// The supplied passphrase did not unlock the vault.
    InvalidPassphrase,
    /// A cryptographic operation failed.
    Crypto(CryptoError),
    /// The storage backend failed.
    Storage(String),
}

impl From<CryptoError> for VaultError {
    fn from(error: CryptoError) -> Self {
        VaultError::Crypto(error)
    }
}

impl std::fmt::Display for VaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VaultError::AlreadyExists => f.write_str("a vault already exists"),
            VaultError::NotInitialized => f.write_str("no vault has been created yet"),
            VaultError::Locked => f.write_str("the vault is locked"),
            VaultError::InvalidPassphrase => f.write_str("invalid passphrase"),
            VaultError::Crypto(error) => write!(f, "crypto error: {error}"),
            VaultError::Storage(message) => write!(f, "storage error: {message}"),
        }
    }
}

impl std::error::Error for VaultError {}
