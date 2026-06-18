# Slate

Slate is a local-first Linux desktop app for fast Markdown note-taking, with
passphrase-encrypted private notes. Built with Tauri 2, Rust, SvelteKit and
TypeScript.

## Features

- Create, edit, search, favorite, archive and delete Markdown notes
- Categories, colors and a calm, minimalist dark UI
- Live Markdown editing and preview
- Local-first storage in SQLite (no cloud, no account)
- Protected notes with **real encryption** (Argon2id key derivation +
  XChaCha20-Poly1305) behind a master-passphrase vault, with inactivity auto-lock
- Export any note to a `.md` file

## Tech stack

| Layer | Tech |
| --- | --- |
| Desktop shell | Tauri 2 |
| Frontend | SvelteKit + TypeScript (static SPA) |
| Backend | Rust, hexagonal architecture |
| Storage | SQLite (`rusqlite`, bundled) |
| Crypto | `argon2` + `chacha20poly1305` |

## Prerequisites

- **Rust** (stable) — https://rustup.rs
- **Node.js** 18+ and npm
- **Linux system libraries for Tauri** (Debian/Ubuntu):

  ```bash
  sudo apt update
  sudo apt install -y libwebkit2gtk-4.1-dev build-essential curl wget file \
    libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
  ```

  Other distros: see https://v2.tauri.app/start/prerequisites/

## Run it locally (development)

```bash
git clone git@github.com:Juanpa-Reyest/slate-notes.git
cd slate-notes
npm install
npm run tauri dev
```

`npm run tauri dev` starts the Vite dev server and launches the desktop window
with hot-reload. Notes are stored in your OS app-data directory, e.g.
`~/.local/share/dev.juanpa.slate/notes.sqlite`.

## Tests & checks

```bash
npm run check                                    # Frontend type-check (svelte-check)
npm test                                         # Frontend unit tests (Vitest)
cargo test --manifest-path src-tauri/Cargo.toml  # Backend unit tests (Rust)
```

## Build a release (package the app)

```bash
npm run tauri build
```

Artifacts land in `src-tauri/target/release/bundle/`:

- `deb/Slate_<version>_amd64.deb` — Debian/Ubuntu installer
- `rpm/Slate-<version>-1.x86_64.rpm` — Fedora/RHEL installer
- `appimage/Slate_<version>_amd64.AppImage` — portable single file
  (the first build fetches AppImage tooling, so it needs internet access)

Install the `.deb` on your machine:

```bash
sudo apt install ./src-tauri/target/release/bundle/deb/Slate_0.1.0_amd64.deb
```

## Distribution roadmap

- **Direct download**: publish the `.deb` / `.rpm` / `.AppImage` on GitHub Releases.
- **Ubuntu store / sandboxed**: package as **Snap** or **Flatpak** (both supported
  by Tauri) and publish to the Snap Store / Flathub.
- App data and logs follow Linux user-directory conventions; no telemetry.

## Architecture (hexagonal)

| Layer | Path | Responsibility |
| --- | --- | --- |
| Domain | `src-tauri/src/domain/` | Note + encryption rules; no framework knowledge |
| Application | `src-tauri/src/app/` | Use cases: notes, vault, secure-notes coordinator |
| Ports | `src-tauri/src/ports/` | Repository / cipher / clock contracts |
| Infrastructure | `src-tauri/src/infra/` | SQLite repositories, XChaCha20 cipher, system clock |
| Commands | `src-tauri/src/commands/` | Thin Tauri IPC handlers |
| Frontend | `src/routes/+page.svelte`, `src/lib/tauri-client/` | UI + typed client |

The business rules are testable without launching the UI: Tauri commands call
application use cases, SQLite sits behind repository traits, and encryption is
encapsulated behind a `Cipher` port.

## Project docs

- `docs/product-requirements.md` — product direction and MVP scope
- `docs/technical-design.md` — layered architecture and crypto path

## License

MIT
