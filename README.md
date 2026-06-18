# Slate

Slate is a local-first Linux Markdown notes launcher prototype built with Tauri 2, Rust, SvelteKit, and TypeScript.

## Prototype Scope

- Floating launcher-style UI with focused search.
- In-memory notes behind a Rust application boundary.
- Create, list, search, update, favorite, archive, and delete notes.
- Markdown editing and preview.
- Placeholder metadata for protected notes, colors, and categories.

SQLite persistence and real encryption are intentionally out of scope for this first foundation.

## Quick Start

```bash
npm install
npm run check
. "$HOME/.cargo/env" && cargo check --manifest-path src-tauri/Cargo.toml
npm run tauri dev
```

If Linux Tauri system packages are missing, install them outside this project according to your distro policy. Do not replace the scaffold with `create-tauri-app --force`.

## Project Docs

- `docs/product-requirements.md` defines the product direction and MVP scope.
- `docs/technical-design.md` defines the layered architecture and future SQLite/encryption path.

## Current Architecture

| Area | Current prototype |
| --- | --- |
| Commands | Thin Tauri handlers in `src-tauri/src/commands/`. |
| Application | Notes use cases in `src-tauri/src/app/`. |
| Domain | Note model and validation in `src-tauri/src/domain/`. |
| Ports | Repository abstraction in `src-tauri/src/ports/`. |
| Infrastructure | In-memory adapter in `src-tauri/src/infra/`. |
| State | Seeded app state in `src-tauri/src/state/`. |
| Frontend | Launcher UI in `src/routes/+page.svelte` and typed Tauri client in `src/lib/tauri-client/`. |
