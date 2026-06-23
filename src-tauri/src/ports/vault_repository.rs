//! Persistence boundary for the master-recovery record. The application depends
//! on this trait, not on SQLite.

use crate::domain::vault::{MasterRecord, VaultError};

pub trait VaultRepository {
    /// Load the single master-recovery record, or `None` if recovery has not
    /// been set up yet.
    fn load(&self) -> Result<Option<MasterRecord>, VaultError>;

    /// Create or replace the master-recovery record.
    fn save(&mut self, record: MasterRecord) -> Result<(), VaultError>;
}
