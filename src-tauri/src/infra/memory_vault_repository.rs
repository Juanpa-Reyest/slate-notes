//! In-memory vault repository for tests.

use crate::domain::vault::{VaultError, VaultRecord};
use crate::ports::vault_repository::VaultRepository;

#[derive(Default)]
pub struct MemoryVaultRepository {
    record: Option<VaultRecord>,
}

impl VaultRepository for MemoryVaultRepository {
    fn load(&self) -> Result<Option<VaultRecord>, VaultError> {
        Ok(self.record.clone())
    }

    fn save(&mut self, record: VaultRecord) -> Result<(), VaultError> {
        self.record = Some(record);
        Ok(())
    }
}
