// The in-memory repositories back both the test suite and the volatile DEBUG
// build storage mode (see `AppState::memory`), so they must be compiled outside
// `#[cfg(test)]` whenever debug assertions are on.
#[cfg(any(test, debug_assertions))]
pub mod memory_note_repository;
// The SQLite repositories back RELEASE builds (and the test suite), but a plain
// DEBUG `run()` uses the in-memory backend, so they read as dead code there.
#[cfg_attr(all(debug_assertions, not(test)), allow(dead_code))]
pub mod sqlite_note_repository;
pub mod xchacha_cipher;

#[cfg(any(test, debug_assertions))]
pub mod memory_vault_repository;
#[cfg_attr(all(debug_assertions, not(test)), allow(dead_code))]
pub mod sqlite_vault_repository;
