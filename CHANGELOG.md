# Changelog

All notable, user-facing changes to Slate are recorded here. The section for each
version is published verbatim as that release's notes on GitHub, so write it for
the people who use the app — what's new and what got fixed — not the code.

## [0.2.1]

### Changed
- Internal release-pipeline improvements only (single source of truth for the app
  version, an automated test gate, and one-click multi-OS releases). No
  user-facing changes in this version.

## [0.2.0]

### Added
- **Per-note passwords** — every protected note now has its own password,
  completely independent from the others. Different notes can use different
  passwords and never unlock each other.
- **Master recovery key** — forgot a note's password? Recover it with your master
  recovery passphrase and the note's content is preserved.
- **Spanish interface** — the whole app is now available in Spanish.

### Fixed
- Protected notes no longer show their content under a different note in the
  sidebar list.
- Creating a brand-new empty note no longer shows a false "could not save" error.
- Category counters no longer briefly flash to 0 while a note is being created.
