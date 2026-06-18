# Local Markdown Notes Launcher — Product Requirements

This project is a local-first Linux desktop notes app built for fast personal capture, Markdown-heavy writing, and private encrypted notes. The first release should feel like a lightweight floating launcher, while keeping the architecture clean enough to evolve into a public Ubuntu-distributable application.

## Product direction

| Area | Decision |
| --- | --- |
| Primary user | One local Linux user first; future public users should remain possible. |
| App style | Floating launcher/window, not a heavy traditional desktop workspace. |
| Storage | Local-only by default. No cloud sync in the MVP. |
| Core writing format | Markdown with rendered preview. |
| Privacy model | Real encryption for protected notes, not just UI hiding. |
| Architecture target | Maintainable layered architecture from day one. |
| Recommended stack | Tauri + Rust + SQLite + lightweight web frontend. |

## Goals

- Capture notes quickly from anywhere in Linux.
- Organize notes by category, tags, colors, favorites, and status.
- Write comfortably in Markdown and preview the rendered result.
- Protect sensitive notes with passphrase-based encryption.
- Keep all user data local and portable.
- Prepare the project for future Linux-friendly packaging and publication.

## Non-goals for the MVP

- Multi-user accounts.
- Cloud synchronization.
- Real-time collaboration.
- Mobile apps.
- Notion-style block editing.
- AI features.
- Complex plugin system.

## MVP scope

### 1. Floating launcher experience

The app should open quickly as a compact floating window from a global shortcut or tray action.

Minimum behavior:

- Open/hide with a configurable global shortcut.
- Stay available from the system tray.
- Support quick note creation.
- Search existing notes from the launcher.
- Open a selected note in the same floating experience.

### 2. Notes management

Each note should support:

- Title.
- Markdown content.
- Category.
- Optional tags.
- Optional color.
- Favorite flag.
- Protected/unprotected state.
- Created and updated timestamps.
- Archived state.

Required actions:

- Create note.
- Edit note.
- Delete note.
- Archive/unarchive note.
- Mark/unmark as favorite.
- Assign category, tags, and color.
- Export a note as `.md`.

### 3. Markdown editing and preview

The editor should support Markdown-first writing.

Minimum Markdown support:

- Headings.
- Paragraphs.
- Bold and italic.
- Ordered and unordered lists.
- Checklists.
- Links.
- Tables.
- Inline code.
- Code blocks.
- Blockquotes.

Required modes:

- Edit mode.
- Preview mode.
- Optional split mode after the MVP if it does not slow delivery.

### 4. Search and filtering

The app should support fast local discovery.

Minimum filters:

- Text search by title.
- Text search by note content when the note is not encrypted or is currently unlocked.
- Category filter.
- Tag filter.
- Favorites filter.
- Archived filter.

### 5. Encryption for protected notes

Protected notes must use real cryptographic encryption.

Minimum requirements:

- Derive encryption keys from a passphrase using a memory-hard key derivation function.
- Encrypt note content using authenticated encryption.
- Never store the raw passphrase.
- Lock protected notes manually.
- Auto-lock protected notes after inactivity.
- Clearly communicate that forgotten passphrases cannot safely recover encrypted content unless a recovery mechanism is designed later.

Recommended implementation direction:

- Argon2id for key derivation.
- XChaCha20-Poly1305 or AES-GCM for authenticated encryption.
- Store encryption metadata per protected note: salt, nonce, algorithm version, and ciphertext.

### 6. Local storage

SQLite should store metadata and note records locally.

Expected stored data:

- Notes metadata.
- Plain content for unprotected notes.
- Ciphertext and encryption metadata for protected notes.
- Categories.
- Tags.
- App preferences.

The database location should follow Linux user data directory conventions instead of hardcoded project paths.

### 7. Backup and portability

The MVP should support basic data ownership.

Minimum capabilities:

- Export individual notes as Markdown.
- Document where local data is stored.
- Plan for full database backup/export in a later release.

## Architecture requirements

The implementation must keep framework, database, and UI details outside the business rules.

### Proposed layers

| Layer | Responsibility | Should not know about |
| --- | --- | --- |
| Domain | Note, category, tag, encryption rules, validation rules. | Tauri, SQLite, UI framework. |
| Application | Use cases such as create note, search notes, encrypt note, export note. | UI components and database-specific SQL details. |
| Infrastructure | SQLite repositories, crypto implementation, filesystem, Linux keyring, app config. | UI rendering. |
| Interface/UI | Floating launcher, editor, preview, tray, shortcut interactions. | SQL, raw cryptographic details. |

### Architectural principles

- Business rules must be testable without launching the UI.
- Tauri commands should call application use cases, not contain business logic.
- SQLite access should be hidden behind repository interfaces.
- Encryption should be encapsulated behind a dedicated service/port.
- UI state should not become the source of truth for persisted notes.
- The project should be package-ready, not a local script disguised as an app.

## Linux integration requirements

The application should behave like a good Linux desktop citizen.

Minimum expectations:

- Global shortcut support.
- System tray support.
- Linux-friendly configuration and data directories.
- Clear application icon and metadata.
- Controlled logs without leaking note content.
- Future packaging path for Snap, Flatpak, or Debian packaging.

## UX principles

- Fast over fancy.
- Keyboard-first where possible.
- Never make capture feel heavy.
- Make protected notes visibly different from normal notes.
- Avoid destructive actions without confirmation or undo strategy.
- Keep Markdown visible and predictable; do not hide the source format behind magic.

## Future candidates after MVP

- Full database export/import.
- Split Markdown editor/preview mode.
- Custom themes.
- Attachments and images.
- Advanced full-text search.
- Optional Linux keyring integration for session unlock.
- Optional Snap/Flatpak publishing.
- Optional sync provider, only if local-first remains intact.

## MVP acceptance checklist

- [ ] The app opens as a floating launcher from a global shortcut.
- [ ] The app can stay available from the system tray.
- [ ] A user can create, edit, delete, archive, and favorite notes.
- [ ] Notes can be categorized, tagged, and color-coded.
- [ ] Markdown content can be edited and previewed.
- [ ] Local search works across normal unlocked content and metadata.
- [ ] Protected notes encrypt their content before persistence.
- [ ] Protected notes can be locked, unlocked, and auto-locked.
- [ ] Notes can be exported as `.md`.
- [ ] The core business logic is separated from UI, Tauri, and SQLite details.

## Open decisions

| Decision | Default recommendation |
| --- | --- |
| Frontend framework | Svelte or React; choose based on desired UI speed and project familiarity. |
| Markdown editor library | Pick one that supports source editing cleanly and does not force block editing. |
| Encryption recovery | No recovery in MVP; document the tradeoff clearly. |
| Packaging target | Start with Tauri Linux bundles; evaluate Snap/Flatpak when MVP stabilizes. |

## Next step

Create the initial technical design that maps this MVP into project structure, crate/module boundaries, database schema, and first implementation milestones.
