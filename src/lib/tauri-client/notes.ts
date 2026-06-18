import { invoke } from "@tauri-apps/api/core";

export type Note = {
  id: string;
  title: string;
  content: string;
  category: string;
  tags: string[];
  color: string;
  isFavorite: boolean;
  isArchived: boolean;
  isProtected: boolean;
  createdAt: string;
  updatedAt: string;
};

export type CreateNoteInput = {
  title: string;
  content: string;
  category?: string;
};

export type UpdateNoteInput = {
  id: string;
  title: string;
  content: string;
  category: string;
  tags: string[];
  color: string;
};

export type VaultStatus = {
  initialized: boolean;
  unlocked: boolean;
};

const isTauriRuntime = () =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const now = "2026-06-09T14:30:00.000Z";

let previewNotes: Note[] = [
  {
    id: "preview-1",
    title: "Sprint notes and launch checklist",
    content:
      "# Sprint notes\n\n- Validate two-zone layout at split widths\n- Keep notes and categories in the same left sidebar\n- Confirm Markdown preview stays readable\n\n## Release risks\n\nThe editor must remain usable before the Tauri backend is available in browser preview.",
    category: "Work",
    tags: ["release", "ui"],
    color: "violet",
    isFavorite: true,
    isArchived: false,
    isProtected: false,
    createdAt: "2026-06-03T09:00:00.000Z",
    updatedAt: "2026-06-09T12:40:00.000Z",
  },
  {
    id: "preview-2",
    title: "Protected account recovery draft",
    content:
      "# Recovery draft\n\nThis sample represents a protected note state without exposing sensitive production data.\n\n> Browser preview uses in-memory mock notes only.",
    category: "Personal",
    tags: ["private"],
    color: "emerald",
    isFavorite: false,
    isArchived: false,
    isProtected: false,
    createdAt: "2026-05-28T17:20:00.000Z",
    updatedAt: "2026-06-08T19:05:00.000Z",
  },
  {
    id: "preview-3",
    title: "Ideas for fast capture mode",
    content:
      "# Fast capture\n\nUse `Ctrl+N` for a new note and `Ctrl+K` to return to search.\n\n| Shortcut | Action |\n| --- | --- |\n| Ctrl+K | Search |\n| Ctrl+S | Save |\n| Ctrl+N | New note |",
    category: "Ideas",
    tags: ["shortcuts", "launcher"],
    color: "amber",
    isFavorite: true,
    isArchived: false,
    isProtected: false,
    createdAt: "2026-06-01T10:10:00.000Z",
    updatedAt: "2026-06-07T11:30:00.000Z",
  },
  {
    id: "preview-4",
    title: "Archived import experiment",
    content:
      "# Import experiment\n\nOld importer notes kept here so the archived filter has a real visual state.",
    category: "Archive",
    tags: ["import"],
    color: "slate",
    isFavorite: false,
    isArchived: true,
    isProtected: false,
    createdAt: "2026-04-18T08:30:00.000Z",
    updatedAt: "2026-05-02T16:15:00.000Z",
  },
];

let previewVault: VaultStatus = { initialized: false, unlocked: false };
let previewPassphrase = "";
const previewSecrets: Record<string, string> = {};

function cloneNote(note: Note) {
  return { ...note, tags: [...note.tags] };
}

// Mirror the backend: protected notes reveal plaintext only while unlocked,
// and are blanked when locked so nothing leaks to the UI.
function presentPreview(note: Note): Note {
  const clone = cloneNote(note);
  if (clone.isProtected) {
    clone.content = previewVault.unlocked ? previewSecrets[clone.id] ?? clone.content : "";
  }
  return clone;
}

function previewList() {
  return Promise.resolve(previewNotes.map(presentPreview));
}

function previewSearch(query: string) {
  const normalized = query.trim().toLowerCase();
  const all = previewNotes.map(presentPreview);
  if (!normalized) return Promise.resolve(all);

  return Promise.resolve(
    all.filter(
      (note) =>
        note.title.toLowerCase().includes(normalized) ||
        note.content.toLowerCase().includes(normalized) ||
        note.category.toLowerCase().includes(normalized) ||
        note.tags.some((tag) => tag.toLowerCase().includes(normalized)),
    ),
  );
}

function touch(note: Note) {
  note.updatedAt = now;
  return cloneNote(note);
}

export function listNotes() {
  if (!isTauriRuntime()) return previewList();

  return invoke<Note[]>("list_notes");
}

export function searchNotes(query: string) {
  if (!isTauriRuntime()) return previewSearch(query);

  return invoke<Note[]>("search_notes", { input: { query } });
}

export function createNote(input: CreateNoteInput) {
  if (!isTauriRuntime()) {
    const note: Note = {
      id: `preview-${crypto.randomUUID()}`,
      title: input.title || "Untitled note",
      content: input.content,
      category: input.category?.trim() || "Inbox",
      tags: [],
      color: "slate",
      isFavorite: false,
      isArchived: false,
      isProtected: false,
      createdAt: now,
      updatedAt: now,
    };
    previewNotes = [note, ...previewNotes];
    return Promise.resolve(cloneNote(note));
  }

  return invoke<Note>("create_note", { input });
}

export function updateNote(input: UpdateNoteInput) {
  if (!isTauriRuntime()) {
    const note = previewNotes.find((item) => item.id === input.id);
    if (!note) return Promise.reject(new Error("Note not found"));

    note.title = input.title;
    note.content = input.content;
    note.category = input.category;
    note.tags = [...input.tags];
    note.color = input.color;
    return Promise.resolve(touch(note));
  }

  return invoke<Note>("update_note", { input });
}

export function toggleFavorite(id: string) {
  if (!isTauriRuntime()) {
    const note = previewNotes.find((item) => item.id === id);
    if (!note) return Promise.reject(new Error("Note not found"));

    note.isFavorite = !note.isFavorite;
    return Promise.resolve(touch(note));
  }

  return invoke<Note>("toggle_favorite", { input: { id } });
}

export function archiveNote(id: string) {
  if (!isTauriRuntime()) {
    const note = previewNotes.find((item) => item.id === id);
    if (!note) return Promise.reject(new Error("Note not found"));

    note.isArchived = !note.isArchived;
    return Promise.resolve(touch(note));
  }

  return invoke<Note>("archive_note", { input: { id } });
}

export function deleteNote(id: string) {
  if (!isTauriRuntime()) {
    previewNotes = previewNotes.filter((note) => note.id !== id);
    return Promise.resolve();
  }

  return invoke<void>("delete_note", { input: { id } });
}

export function vaultStatus() {
  if (!isTauriRuntime()) return Promise.resolve({ ...previewVault });

  return invoke<VaultStatus>("vault_status");
}

export function createVault(passphrase: string) {
  if (!isTauriRuntime()) {
    previewPassphrase = passphrase;
    previewVault = { initialized: true, unlocked: true };
    return Promise.resolve({ ...previewVault });
  }

  return invoke<VaultStatus>("create_vault", { input: { passphrase } });
}

export function unlockVault(passphrase: string) {
  if (!isTauriRuntime()) {
    if (passphrase !== previewPassphrase) {
      return Promise.reject(new Error("Invalid passphrase."));
    }
    previewVault = { initialized: true, unlocked: true };
    return Promise.resolve({ ...previewVault });
  }

  return invoke<VaultStatus>("unlock_vault", { input: { passphrase } });
}

export function lockVault() {
  if (!isTauriRuntime()) {
    previewVault = { ...previewVault, unlocked: false };
    return Promise.resolve({ ...previewVault });
  }

  return invoke<VaultStatus>("lock_vault");
}

export function protectNote(id: string) {
  if (!isTauriRuntime()) {
    const note = previewNotes.find((item) => item.id === id);
    if (!note) return Promise.reject(new Error("Note not found"));
    if (!previewVault.unlocked) return Promise.reject(new Error("The vault is locked."));

    previewSecrets[id] = note.content;
    note.isProtected = true;
    return Promise.resolve(presentPreview(note));
  }

  return invoke<Note>("protect_note", { input: { id } });
}

export function unprotectNote(id: string) {
  if (!isTauriRuntime()) {
    const note = previewNotes.find((item) => item.id === id);
    if (!note) return Promise.reject(new Error("Note not found"));
    if (!previewVault.unlocked) return Promise.reject(new Error("The vault is locked."));

    note.content = previewSecrets[id] ?? note.content;
    delete previewSecrets[id];
    note.isProtected = false;
    return Promise.resolve(cloneNote(note));
  }

  return invoke<Note>("unprotect_note", { input: { id } });
}
