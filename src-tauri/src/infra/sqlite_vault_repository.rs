//! SQLite-backed master-recovery persistence: a single-row `master_recovery`
//! table holding the X25519 keypair (public in clear, private sealed).

use std::path::Path;

use rusqlite::{params, Connection};

use crate::domain::encryption::Sealed;
use crate::domain::vault::{MasterRecord, VaultError};
use crate::ports::vault_repository::VaultRepository;

pub struct SqliteVaultRepository {
    connection: Connection,
}

impl SqliteVaultRepository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, VaultError> {
        let connection = Connection::open(path).map_err(storage)?;
        let repository = Self { connection };
        repository.migrate()?;
        Ok(repository)
    }

    fn migrate(&self) -> Result<(), VaultError> {
        self.connection
            .execute(
                "CREATE TABLE IF NOT EXISTS master_recovery (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    version INTEGER NOT NULL,
                    kdf_salt BLOB NOT NULL,
                    public_key BLOB NOT NULL,
                    private_nonce BLOB NOT NULL,
                    private_ciphertext BLOB NOT NULL
                )",
                [],
            )
            .map_err(storage)?;
        Ok(())
    }
}

impl VaultRepository for SqliteVaultRepository {
    fn load(&self) -> Result<Option<MasterRecord>, VaultError> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT version, kdf_salt, public_key, private_nonce, private_ciphertext
                 FROM master_recovery WHERE id = 1",
            )
            .map_err(storage)?;

        let mut rows = statement.query([]).map_err(storage)?;
        match rows.next().map_err(storage)? {
            Some(row) => {
                let version: u32 = row.get(0).map_err(storage)?;
                let kdf_salt: Vec<u8> = row.get(1).map_err(storage)?;
                let public_key: Vec<u8> = row.get(2).map_err(storage)?;
                let nonce: Vec<u8> = row.get(3).map_err(storage)?;
                let ciphertext: Vec<u8> = row.get(4).map_err(storage)?;
                Ok(Some(MasterRecord {
                    version,
                    kdf_salt,
                    public_key,
                    private_key_sealed: Sealed { nonce, ciphertext },
                }))
            }
            None => Ok(None),
        }
    }

    fn save(&mut self, record: MasterRecord) -> Result<(), VaultError> {
        self.connection
            .execute(
                "INSERT OR REPLACE INTO master_recovery
                 (id, version, kdf_salt, public_key, private_nonce, private_ciphertext)
                 VALUES (1, ?1, ?2, ?3, ?4, ?5)",
                params![
                    record.version,
                    record.kdf_salt,
                    record.public_key,
                    record.private_key_sealed.nonce,
                    record.private_key_sealed.ciphertext
                ],
            )
            .map_err(storage)?;
        Ok(())
    }
}

fn storage(error: rusqlite::Error) -> VaultError {
    VaultError::Storage(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_memory() -> SqliteVaultRepository {
        let connection = Connection::open_in_memory().unwrap();
        let repository = SqliteVaultRepository { connection };
        repository.migrate().unwrap();
        repository
    }

    fn record() -> MasterRecord {
        MasterRecord {
            version: 1,
            kdf_salt: vec![1, 2, 3, 4],
            public_key: vec![9; 32],
            private_key_sealed: Sealed {
                nonce: vec![9, 8, 7],
                ciphertext: vec![5, 5, 5, 5],
            },
        }
    }

    #[test]
    fn load_returns_none_when_empty() {
        let repository = in_memory();
        assert_eq!(repository.load().unwrap(), None);
    }

    #[test]
    fn save_then_load_round_trips() {
        let mut repository = in_memory();
        let record = record();
        repository.save(record.clone()).unwrap();
        assert_eq!(repository.load().unwrap(), Some(record));
    }

    #[test]
    fn save_replaces_existing_record() {
        let mut repository = in_memory();
        repository.save(record()).unwrap();

        let replacement = MasterRecord {
            version: 2,
            kdf_salt: vec![2],
            public_key: vec![1; 32],
            private_key_sealed: Sealed {
                nonce: vec![2],
                ciphertext: vec![2],
            },
        };
        repository.save(replacement.clone()).unwrap();

        assert_eq!(repository.load().unwrap(), Some(replacement));
    }
}
