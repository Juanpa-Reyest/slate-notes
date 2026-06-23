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

export type RecoveryStatus = {
  recoveryInitialized: boolean;
  // "activeNoteOpen" means a protected note is currently open (a transient key is held).
  activeNoteOpen: boolean;
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

// Preview recovery state mirrors the redesigned backend model:
// - `recoveryInitialized` tracks whether a recovery passphrase has been set.
// - `activeId` is the id of the single protected note currently revealed
//   (the transient key concept); mocked here by remembering the passphrase
//   used to open it.
let previewVault: { recoveryInitialized: boolean } = { recoveryInitialized: false };
let previewPassphrase = "";
let previewActiveId: string | null = null;
// Per-note ciphertext (mocked as the stored plaintext secret).
const previewSecrets: Record<string, string> = {};

function previewRecoveryStatus(): RecoveryStatus {
  return { recoveryInitialized: previewVault.recoveryInitialized, activeNoteOpen: previewActiveId !== null };
}

function cloneNote(note: Note) {
  return { ...note, tags: [...note.tags] };
}

// Mirror the backend: list/search ALWAYS blank protected note content. To read a
// protected note the caller must reveal it explicitly via revealNote().
function presentPreview(note: Note): Note {
  const clone = cloneNote(note);
  if (clone.isProtected) {
    clone.content = "";
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
    // A protected note can only be updated while it is the active (revealed)
    // one; otherwise the backend has no key to re-seal it.
    if (note.isProtected && previewActiveId !== note.id) {
      return Promise.reject(new Error("The note is locked."));
    }

    note.title = input.title;
    note.category = input.category;
    note.tags = [...input.tags];
    note.color = input.color;
    if (note.isProtected) {
      // Active protected note: store the new content as the secret, keep the
      // persisted content blank so nothing leaks through list/search.
      previewSecrets[note.id] = input.content;
      note.content = "";
      const updated = touch(note);
      updated.content = input.content;
      return Promise.resolve(updated);
    }
    note.content = input.content;
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

export function recoveryStatus() {
  if (!isTauriRuntime()) return Promise.resolve(previewRecoveryStatus());

  return invoke<RecoveryStatus>("recovery_status");
}

export function setUpRecovery(passphrase: string) {
  if (!isTauriRuntime()) {
    // Setting up recovery stores the passphrase but does NOT open any note.
    previewPassphrase = passphrase;
    previewVault = { recoveryInitialized: true };
    return Promise.resolve(previewRecoveryStatus());
  }

  return invoke<RecoveryStatus>("set_up_recovery", { input: { passphrase } });
}

export function revealNote(id: string, passphrase: string) {
  if (!isTauriRuntime()) {
    const note = previewNotes.find((item) => item.id === id);
    if (!note) return Promise.reject(new Error("Note not found"));
    if (!note.isProtected) return Promise.reject(new Error("The note is not protected."));
    if (passphrase !== previewPassphrase) {
      return Promise.reject(new Error("Invalid passphrase."));
    }
    previewActiveId = id;
    const revealed = cloneNote(note);
    revealed.content = previewSecrets[id] ?? "";
    return Promise.resolve(revealed);
  }

  return invoke<Note>("reveal_note", { input: { id, passphrase } });
}

export function clearActive() {
  if (!isTauriRuntime()) {
    previewActiveId = null;
    return Promise.resolve(previewRecoveryStatus());
  }

  return invoke<RecoveryStatus>("clear_active");
}

export function protectNote(id: string, passphrase: string) {
  if (!isTauriRuntime()) {
    const note = previewNotes.find((item) => item.id === id);
    if (!note) return Promise.reject(new Error("Note not found"));
    // Protecting verifies the passphrase against the recovery setup.
    if (!previewVault.recoveryInitialized) {
      return Promise.reject(new Error("Recovery has not been set up yet."));
    }
    if (passphrase !== previewPassphrase) {
      return Promise.reject(new Error("Invalid passphrase."));
    }

    previewSecrets[id] = note.content;
    note.content = "";
    note.isProtected = true;
    previewActiveId = id;
    const protectedNote = cloneNote(note);
    protectedNote.content = previewSecrets[id];
    return Promise.resolve(protectedNote);
  }

  return invoke<Note>("protect_note", { input: { id, passphrase } });
}

export function unprotectNote(id: string, passphrase: string) {
  if (!isTauriRuntime()) {
    const note = previewNotes.find((item) => item.id === id);
    if (!note) return Promise.reject(new Error("Note not found"));
    if (!previewVault.recoveryInitialized) {
      return Promise.reject(new Error("Recovery has not been set up yet."));
    }
    if (passphrase !== previewPassphrase) {
      return Promise.reject(new Error("Invalid passphrase."));
    }

    note.content = previewSecrets[id] ?? note.content;
    delete previewSecrets[id];
    note.isProtected = false;
    if (previewActiveId === id) previewActiveId = null;
    return Promise.resolve(cloneNote(note));
  }

  return invoke<Note>("unprotect_note", { input: { id, passphrase } });
}

export function recoverNote(id: string, passphrase: string) {
  if (!isTauriRuntime()) {
    const note = previewNotes.find((item) => item.id === id);
    if (!note) return Promise.reject(new Error("Note not found"));
    if (!previewVault.recoveryInitialized) {
      return Promise.reject(new Error("Recovery has not been set up yet."));
    }
    if (passphrase !== previewPassphrase) {
      return Promise.reject(new Error("Invalid passphrase."));
    }

    note.content = previewSecrets[id] ?? note.content;
    delete previewSecrets[id];
    note.isProtected = false;
    if (previewActiveId === id) previewActiveId = null;
    return Promise.resolve(cloneNote(note));
  }

  return invoke<Note>("recover_note", { input: { id, passphrase } });
}

export function exportNote(id: string) {
  if (!isTauriRuntime()) {
    const note = previewNotes.find((item) => item.id === id);
    if (!note) return Promise.reject(new Error("Note not found"));
    // Never leak a protected secret: only the active (revealed) note exports
    // its plaintext.
    if (note.isProtected && previewActiveId !== id) {
      return Promise.reject(new Error("Reveal the note before exporting."));
    }

    const content = note.isProtected ? previewSecrets[id] ?? note.content : note.content;
    const title = note.title.trim() || "untitled";
    const document_ = `# ${title}\n\n${content}\n`;
    const stem =
      title.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "") || "untitled";

    const blob = new Blob([document_], { type: "text/markdown" });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = `${stem}.md`;
    anchor.click();
    URL.revokeObjectURL(url);

    return Promise.resolve("your browser downloads");
  }

  return invoke<string>("export_note", { input: { id } });
}
