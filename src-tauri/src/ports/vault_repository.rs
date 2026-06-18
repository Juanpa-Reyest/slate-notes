//! Persistence boundary for the vault record. The application depends on this
//! trait, not on SQLite.

use crate::domain::vault::{VaultError, VaultRecord};

pub trait VaultRepository {
    /// Load the single vault record, or `None` if no vault exists yet.
    fn load(&self) -> Result<Option<VaultRecord>, VaultError>;

    /// Create or replace the vault record.
    fn save(&mut self, record: VaultRecord) -> Result<(), VaultError>;
}
