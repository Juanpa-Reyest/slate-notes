//! SQLite-backed vault persistence: a single-row `vault` table.

use std::path::Path;

use rusqlite::{params, Connection};

use crate::domain::encryption::Sealed;
use crate::domain::vault::{VaultError, VaultRecord};
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
                "CREATE TABLE IF NOT EXISTS vault (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    version INTEGER NOT NULL,
                    salt BLOB NOT NULL,
                    sentinel_nonce BLOB NOT NULL,
                    sentinel_ciphertext BLOB NOT NULL
                )",
                [],
            )
            .map_err(storage)?;
        Ok(())
    }
}

impl VaultRepository for SqliteVaultRepository {
    fn load(&self) -> Result<Option<VaultRecord>, VaultError> {
        let mut statement = self
            .connection
            .prepare("SELECT version, salt, sentinel_nonce, sentinel_ciphertext FROM vault WHERE id = 1")
            .map_err(storage)?;

        let mut rows = statement.query([]).map_err(storage)?;
        match rows.next().map_err(storage)? {
            Some(row) => {
                let version: u32 = row.get(0).map_err(storage)?;
                let salt: Vec<u8> = row.get(1).map_err(storage)?;
                let nonce: Vec<u8> = row.get(2).map_err(storage)?;
                let ciphertext: Vec<u8> = row.get(3).map_err(storage)?;
                Ok(Some(VaultRecord {
                    version,
                    salt,
                    sentinel: Sealed { nonce, ciphertext },
                }))
            }
            None => Ok(None),
        }
    }

    fn save(&mut self, record: VaultRecord) -> Result<(), VaultError> {
        self.connection
            .execute(
                "INSERT OR REPLACE INTO vault (id, version, salt, sentinel_nonce, sentinel_ciphertext)
                 VALUES (1, ?1, ?2, ?3, ?4)",
                params![
                    record.version,
                    record.salt,
                    record.sentinel.nonce,
                    record.sentinel.ciphertext
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

    #[test]
    fn load_returns_none_when_empty() {
        let repository = in_memory();
        assert_eq!(repository.load().unwrap(), None);
    }

    #[test]
    fn save_then_load_round_trips() {
        let mut repository = in_memory();
        let record = VaultRecord {
            version: 1,
            salt: vec![1, 2, 3, 4],
            sentinel: Sealed {
                nonce: vec![9, 8, 7],
                ciphertext: vec![5, 5, 5, 5],
            },
        };

        repository.save(record.clone()).unwrap();

        assert_eq!(repository.load().unwrap(), Some(record));
    }

    #[test]
    fn save_replaces_existing_record() {
        let mut repository = in_memory();
        repository
            .save(VaultRecord {
                version: 1,
                salt: vec![1],
                sentinel: Sealed {
                    nonce: vec![1],
                    ciphertext: vec![1],
                },
            })
            .unwrap();

        let replacement = VaultRecord {
            version: 1,
            salt: vec![2],
            sentinel: Sealed {
                nonce: vec![2],
                ciphertext: vec![2],
            },
        };
        repository.save(replacement.clone()).unwrap();

        assert_eq!(repository.load().unwrap(), Some(replacement));
    }
}
