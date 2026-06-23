//! Domain types for the per-note protection model with asymmetric master
//! recovery.
//!
//! There is NO single master vault that guards all notes. Instead:
//!
//! * Each protected note carries its own random Data Encryption Key (DEK). The
//!   DEK is wrapped two ways: under a key derived from the NOTE's own password,
//!   and escrowed to the master's X25519 PUBLIC key. The note content is sealed
//!   with the DEK. See [`ProtectedPayload`].
//! * A single [`MasterRecord`] holds the master X25519 keypair: the public key
//!   in clear (so any note can be escrowed with NO master prompt), and the
//!   private key sealed under a key derived from the master passphrase. The
//!   private key is only ever recovered to rescue a note whose password was
//!   forgotten.

use serde::{Deserialize, Serialize};

use crate::domain::encryption::{CryptoError, EciesSealed, Sealed};

/// The persisted master-recovery record. Created ONCE via `set_up_recovery`.
///
/// `public_key` is stored in clear and is used to escrow every protected note's
/// DEK without ever prompting for the master passphrase. `private_key_sealed`
/// is the X25519 private key encrypted under the master-pass-derived key; a
/// wrong master passphrase is detected by AEAD failure when opening it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterRecord {
    pub version: u32,
    /// Salt for deriving the key-encryption key from the master passphrase.
    pub kdf_salt: Vec<u8>,
    /// X25519 public key (32 bytes), stored in clear.
    pub public_key: Vec<u8>,
    /// X25519 private key sealed under the master-pass-derived key.
    pub private_key_sealed: Sealed,
}

/// The JSON payload stored in a protected note's `content` column. The notes
/// table schema does not change: this serializes to a String.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtectedPayload {
    pub version: u32,
    /// Per-note salt for deriving the note-password key.
    pub note_salt: Vec<u8>,
    /// The note plaintext sealed with the random DEK.
    pub content: Sealed,
    /// The DEK sealed under the note-password-derived key.
    pub dek_by_pass: Sealed,
    /// The DEK escrowed to the master X25519 public key.
    pub dek_escrow: EciesSealed,
}

/// Recovery/auth status for the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryStatus {
    /// Whether the master-recovery record has been set up.
    pub recovery_initialized: bool,
    /// Whether a protected note is currently open (a transient DEK is held).
    pub active_note_open: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VaultError {
    /// Master recovery has already been set up; it cannot be created twice.
    AlreadyExists,
    /// Master recovery has not been set up yet.
    NotInitialized,
    /// The note is not the currently open protected note; reveal it first.
    Locked,
    /// The supplied passphrase (note password or master passphrase) was wrong.
    InvalidPassphrase,
    /// The stored protected payload could not be parsed.
    CorruptPayload,
    /// A cryptographic operation failed.
    Crypto(CryptoError),
    /// The storage backend failed. Only the SQLite repositories construct this
    /// (RELEASE/test); a plain DEBUG `run()` uses the in-memory backend.
    #[cfg_attr(all(debug_assertions, not(test)), allow(dead_code))]
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
            VaultError::AlreadyExists => f.write_str("master recovery is already set up"),
            VaultError::NotInitialized => {
                f.write_str("master recovery has not been set up yet")
            }
            VaultError::Locked => f.write_str("the note is not open"),
            VaultError::InvalidPassphrase => f.write_str("invalid passphrase"),
            VaultError::CorruptPayload => f.write_str("protected content is corrupt"),
            VaultError::Crypto(error) => write!(f, "crypto error: {error}"),
            VaultError::Storage(message) => write!(f, "storage error: {message}"),
        }
    }
}

impl std::error::Error for VaultError {}
