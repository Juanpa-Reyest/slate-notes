# Local Markdown Notes Launcher — Technical Design

This design turns the product requirements into a maintainable Linux desktop application: a Tauri shell with a Rust application core, SQLite persistence, real encryption for protected notes, and a lightweight web UI for the floating launcher experience.

## Architecture at a glance

| Area | Decision |
| --- | --- |
| Desktop shell | Tauri 2 |
| Backend | Rust |
| Frontend | Svelte + TypeScript by default; React remains acceptable if team familiarity wins. |
| Storage | SQLite in the Linux user data directory. |
| Database access | Repository layer; default recommendation is `sqlx` with migrations. |
| Encryption | Argon2id key derivation + authenticated encryption. |
| UX model | Floating launcher opened by global shortcut and tray action. |
| Architecture style | Layered / ports-and-adapters. |

## Core boundaries

The app should be organized so that Tauri, SQLite, and the UI are replaceable details, not the heart of the system.

```text
UI / Tauri commands
        ↓
Application use cases
        ↓
Domain models and rules
        ↓
Ports / interfaces
        ↓
Infrastructure adapters: SQLite, crypto, filesystem, keyring, config
```

## Proposed project structure

```text
.
├── docs/
│   ├── product-requirements.md
│   └── technical-design.md
├── src/                         # Frontend
│   ├── app/
│   ├── components/
│   ├── features/
│   │   ├── launcher/
│   │   ├── notes/
│   │   ├── markdown/
│   │   └── settings/
│   ├── lib/
│   └── main.ts
├── src-tauri/                   # Tauri + Rust backend
│   ├── migrations/
│   ├── src/
│   │   ├── main.rs
│   │   ├── commands/
│   │   ├── app/
│   │   ├── domain/
│   │   ├── ports/
│   │   ├── infra/
│   │   │   ├── db/
│   │   │   ├── crypto/
│   │   │   ├── config/
│   │   │   └── filesystem/
│   │   └── state/
│   └── tauri.conf.json
└── package.json
```

## Rust backend design

### Domain layer

The domain layer contains pure concepts and validation rules.

Core models:

- `Note`
- `NoteId`
- `Category`
- `Tag`
- `NoteColor`
- `ProtectedNoteMetadata`
- `MarkdownContent`

Core rules:

- A note must have a title or content.
- Protected notes must never expose plaintext through persistence models.
- Archived notes remain searchable by metadata unless explicitly excluded.
- Encrypted note content is unavailable until unlocked in the current session.

### Application layer

The application layer owns use cases.

Initial use cases:

- `CreateNote`
- `UpdateNote`
- `DeleteNote`
- `ArchiveNote`
- `FavoriteNote`
- `SearchNotes`
- `AssignCategory`
- `AssignTags`
- `ProtectNote`
- `UnlockNote`
- `LockNote`
- `ExportNoteAsMarkdown`

Rules:

- Use cases receive input DTOs and return output DTOs.
- Use cases depend on ports, not concrete SQLite or crypto implementations.
- Tauri commands call use cases; they do not implement business rules.

### Ports

Ports define what the app needs from the outside world.

Recommended ports:

- `NoteRepository`
- `CategoryRepository`
- `TagRepository`
- `SearchRepository`
- `EncryptionService`
- `Clock`
- `AppConfigStore`
- `MarkdownExporter`

### Infrastructure layer

Adapters implement ports.

Initial adapters:

- SQLite repositories.
- Argon2id key derivation service.
- Authenticated encryption service.
- Linux user data/config path resolver.
- Markdown file exporter.

## Tauri command boundary

Tauri commands should stay thin.

Example commands:

- `create_note(input)`
- `update_note(input)`
- `delete_note(note_id)`
- `search_notes(query)`
- `unlock_note(note_id, passphrase)`
- `lock_note(note_id)`
- `protect_note(note_id, passphrase)`
- `export_note(note_id, target_path)`
- `show_launcher()`
- `hide_launcher()`

Command responsibilities:

- Validate transport-level input shape.
- Call one application use case.
- Convert application errors into UI-safe errors.

Command non-responsibilities:

- SQL.
- Cryptographic details.
- Markdown rendering rules.
- Business validation beyond basic input shape.

## Frontend design

Use Svelte + TypeScript for the MVP because it keeps the UI small, fast, and simple. The app does not need a heavy frontend architecture to succeed.

Frontend responsibilities:

- Floating launcher shell.
- Notes list.
- Search input.
- Markdown editor.
- Markdown preview.
- Category/tag/color controls.
- Protected note unlock UI.
- Settings UI.

Frontend non-responsibilities:

- Raw SQLite access.
- Key derivation.
- Encryption/decryption internals.
- Persisted source of truth.

Recommended frontend modules:

```text
src/features/launcher/      # Floating shell, keyboard behavior, quick actions
src/features/notes/         # Notes list, note editor, metadata controls
src/features/markdown/      # Editor and preview integration
src/features/settings/      # Shortcut, autolock, storage path display
src/lib/tauri-client/       # Typed wrappers around Tauri commands
```

## Data model

SQLite should be migration-driven from the first commit that introduces persistence.

### Initial schema direction

```sql
CREATE TABLE notes (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  content_plaintext TEXT,
  encrypted_payload BLOB,
  is_protected INTEGER NOT NULL DEFAULT 0,
  encryption_version INTEGER,
  encryption_salt BLOB,
  encryption_nonce BLOB,
  category_id TEXT,
  color TEXT,
  is_favorite INTEGER NOT NULL DEFAULT 0,
  is_archived INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  last_opened_at TEXT,
  FOREIGN KEY (category_id) REFERENCES categories(id)
);

CREATE TABLE categories (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  color TEXT,
  sort_order INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE tags (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL
);

CREATE TABLE note_tags (
  note_id TEXT NOT NULL,
  tag_id TEXT NOT NULL,
  PRIMARY KEY (note_id, tag_id),
  FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE,
  FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
);

CREATE TABLE app_settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```

### Search direction

MVP search can start with indexed title/category/tag queries and simple content search for unprotected notes. Once the basic flow is stable, add SQLite FTS5 for unprotected or currently unlocked searchable content.

Protected notes should only expose searchable encrypted content if the app deliberately builds an encrypted-search strategy later. Do not fake this.

## Encryption design

Protected notes must be encrypted before persistence.

### Recommended crypto flow

1. User enters a passphrase.
2. Generate a random salt per protected note.
3. Derive a content encryption key with Argon2id.
4. Generate a random nonce.
5. Encrypt Markdown content using authenticated encryption.
6. Store ciphertext plus salt, nonce, and algorithm version.
7. Keep decrypted content only in memory while the note is unlocked.
8. Clear unlocked content on manual lock, timeout, or app exit.

Recommended crates:

- `argon2`
- `chacha20poly1305` or an audited AES-GCM implementation
- `rand`
- `zeroize`

### Security rules

- Never log note content, passphrases, salts, nonces, or derived keys.
- Never store the passphrase.
- Never store decrypted protected note content in SQLite.
- Do not use homegrown encryption.
- Do not silently weaken encryption to make recovery easier.
- If recovery is introduced later, design it explicitly as a separate requirement.

## Runtime flows

### Open launcher

1. User presses the global shortcut or tray action.
2. Tauri shows/focuses the floating window.
3. UI focuses the search/quick capture input.
4. Recent and favorite notes are displayed.

### Create normal note

1. User creates a note from the launcher.
2. UI calls `create_note`.
3. Application validates input.
4. Repository persists plaintext content.
5. UI receives the created note summary.

### Protect note

1. User chooses to protect an existing note.
2. UI asks for passphrase confirmation.
3. Application encrypts current content.
4. Repository replaces plaintext content with encrypted payload.
5. Note becomes protected and locked or unlocked depending on user action.

### Unlock protected note

1. User opens a protected note.
2. UI asks for passphrase.
3. Application derives key and attempts decryption.
4. If successful, decrypted content is returned for the active session only.
5. If unsuccessful, UI receives a safe error message.

### Export note

1. User chooses export.
2. If the note is protected and locked, require unlock first.
3. Application resolves Markdown content.
4. Exporter writes a `.md` file to the selected path.

## Linux integration

Minimum integration:

- Tauri global shortcut plugin.
- Tauri tray support.
- Linux user data directory for SQLite.
- Linux config directory for preferences.
- App icon and desktop metadata.

Avoid hardcoded absolute paths. The app should work after installation, not only from the development workspace.

## Testing strategy

Testing starts with the backend because that is where correctness and security matter most.

| Layer | Tests |
| --- | --- |
| Domain | Pure unit tests for validation and note state transitions. |
| Application | Use case tests with in-memory/fake repositories. |
| Infrastructure | SQLite migration and repository integration tests. |
| Crypto | Round-trip encryption/decryption tests and wrong-passphrase tests. |
| UI | Component tests for launcher, editor, protected-note states. |
| E2E | Basic create/search/edit/protect/unlock/export flows. |

Critical test cases:

- Protected note content is not persisted as plaintext.
- Wrong passphrase cannot unlock a protected note.
- Auto-lock clears decrypted content from session state.
- Export refuses locked protected notes.
- Search does not leak locked encrypted content.

## Error handling

Use typed backend errors and map them to UI-safe messages.

Error categories:

- Validation error.
- Not found.
- Storage error.
- Encryption error.
- Unlock failed.
- Export error.
- Configuration error.

Do not expose raw SQL errors, filesystem internals, or cryptographic details directly to the UI.

## Logging

Logging must help diagnose the app without leaking private notes.

Allowed:

- App startup/shutdown events.
- Migration success/failure.
- Command names and durations.
- Error categories.

Forbidden:

- Note content.
- Passphrases.
- Derived keys.
- Ciphertext dumps.
- Full export paths if they reveal sensitive user data.

## Implementation milestones

### Milestone 1 — Project foundation

- Initialize Tauri + Svelte + TypeScript.
- Add Rust module structure.
- Add SQLite migration setup.
- Add basic app state wiring.

### Milestone 2 — Basic notes

- Create, edit, delete, archive, and favorite notes.
- Add categories, tags, and colors.
- Store data in SQLite.

### Milestone 3 — Launcher UX

- Floating window behavior.
- Global shortcut.
- Tray icon.
- Quick capture/search focus.

### Milestone 4 — Markdown

- Markdown editor.
- Preview renderer.
- Export note as `.md`.

### Milestone 5 — Protected notes

- Passphrase flow.
- Encryption/decryption.
- Manual lock.
- Auto-lock.
- Tests proving protected content is not persisted as plaintext.

### Milestone 6 — Packaging readiness

- Linux metadata.
- Icons.
- Data/config path verification.
- Basic release build documentation.

## Acceptance checklist

- [ ] Tauri commands are thin wrappers around application use cases.
- [ ] Domain and application tests run without launching the UI.
- [ ] SQLite access is isolated behind repositories.
- [ ] Protected notes are encrypted before persistence.
- [ ] The launcher can be opened from a shortcut and tray action.
- [ ] Markdown edit and preview flows work for common syntax.
- [ ] The app stores data in Linux-appropriate user directories.
- [ ] Logs do not leak note content or security-sensitive material.

## Open decisions

| Decision | Recommendation |
| --- | --- |
| Frontend framework | Start with Svelte + TypeScript unless there is a strong React preference. |
| SQLite library | Start with `sqlx`; keep repositories abstract enough to switch if needed. |
| Authenticated cipher | Prefer XChaCha20-Poly1305 for nonce safety; AES-GCM is acceptable if implemented carefully. |
| Keyring | Do not store passphrases in MVP; consider Linux keyring only for optional session convenience later. |
| Full-text search | Add FTS5 after basic search works. |

## Next step

Create the initial implementation plan: small work units that build the foundation first, then notes, launcher behavior, Markdown, encryption, and packaging readiness.
