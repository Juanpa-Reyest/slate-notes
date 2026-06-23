//! In-memory master-recovery repository. Backs both the test suite and the
//! volatile DEBUG storage mode.

use crate::domain::vault::{MasterRecord, VaultError};
use crate::ports::vault_repository::VaultRepository;

#[derive(Default)]
pub struct MemoryVaultRepository {
    record: Option<MasterRecord>,
}

impl VaultRepository for MemoryVaultRepository {
    fn load(&self) -> Result<Option<MasterRecord>, VaultError> {
        Ok(self.record.clone())
    }

    fn save(&mut self, record: MasterRecord) -> Result<(), VaultError> {
        self.record = Some(record);
        Ok(())
    }
}
