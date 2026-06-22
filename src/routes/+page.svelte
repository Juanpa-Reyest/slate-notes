<script lang="ts">
  import { onMount, tick } from "svelte";
  import { marked } from "marked";
  import {
    archiveNote,
    createNote,
    deleteNote,
    listNotes,
    searchNotes,
    toggleFavorite,
    updateNote,
    vaultStatus,
    createVault,
    unlockVault,
    lockVault,
    protectNote,
    unprotectNote,
    exportNote,
    type Note,
    type VaultStatus,
  } from "$lib/tauri-client/notes";
  import { createAutosave, type SaveStatus } from "$lib/autosave";

  type CategoryNavItem = {
    id: string;
    label: string;
    count: number;
    tone: string;
  };

  type CategoryOption = {
    label: string;
    tone: string;
  };

  type ColorOption = {
    value: string;
    label: string;
    description: string;
  };

  const DEFAULT_CATEGORIES: CategoryOption[] = [
    { label: "Inbox", tone: "cyan" },
    { label: "Work", tone: "blue" },
    { label: "Personal", tone: "green" },
    { label: "Ideas", tone: "gold" },
    { label: "Private", tone: "violet" },
    { label: "Learning", tone: "neutral" },
  ];

  const COLOR_OPTIONS: ColorOption[] = [
    { value: "slate", label: "Slate", description: "Default" },
    { value: "violet", label: "Violet", description: "Focus" },
    { value: "amber", label: "Amber", description: "Idea" },
    { value: "emerald", label: "Emerald", description: "Personal" },
  ];

  let notes = $state<Note[]>([]);
  let selectedNote = $state<Note | null>(null);
  let selectedCategory = $state("all");
  let searchQuery = $state("");
  let errorMessage = $state("");
  let saveStatus = $state<SaveStatus>("idle");
  let lastSavedAt = $state("");
  let isMarkdownEditing = $state(false);
  let isCategoryMenuOpen = $state(false);
  let isColorMenuOpen = $state(false);
  let searchInput = $state<HTMLInputElement | null>(null);

  let vault = $state<VaultStatus>({ initialized: false, unlocked: false });
  let isVaultDialogOpen = $state(false);
  let vaultMode = $state<"create" | "unlock">("unlock");
  let passphrase = $state("");
  let passphraseConfirm = $state("");
  let vaultError = $state("");
  let noticeMessage = $state("");

  let sourceNotes = $derived(notes);
  let categoryItems = $derived(buildCategoryItems(sourceNotes));
  let visibleNotes = $derived(filterNotes(sourceNotes, selectedCategory, searchQuery));
  let previewHtml = $derived(selectedNote ? marked.parse(selectedNote.content) : "");
  let isNoteLocked = $derived(!!selectedNote?.isProtected && !vault.unlocked);
  let saveLabel = $derived(formatSaveLabel(saveStatus, lastSavedAt));

  onMount(async () => {
    notes = await listNotes();
    vault = await vaultStatus();
    selectedNote = notes.find((note) => !note.isArchived) ?? notes[0] ?? null;
    await tick();
    searchInput?.focus();
  });

  function buildCategoryItems(noteList: Note[]): CategoryNavItem[] {
    const activeNotes = noteList.filter((note) => !note.isArchived);
    const baseItems: CategoryNavItem[] = [
      { id: "all", label: "All Notes", count: noteList.length, tone: "neutral" },
      { id: "favorites", label: "Favorites", count: noteList.filter((note) => note.isFavorite).length, tone: "gold" },
      { id: "protected", label: "Protected", count: noteList.filter((note) => note.isProtected).length, tone: "violet" },
      { id: "archived", label: "Archived", count: noteList.filter((note) => note.isArchived).length, tone: "muted" },
    ];

    const categoryCounts = activeNotes.reduce<Record<string, number>>((counts, note) => {
      const category = normalizedCategory(note.category?.trim() || "Inbox");
      counts[category] = (counts[category] ?? 0) + 1;
      return counts;
    }, {});

    const defaultItems = DEFAULT_CATEGORIES.map(({ label, tone }) => ({
      id: categoryId(label),
      label,
      count: categoryCounts[label] ?? 0,
      tone,
    }));

    return [...baseItems, ...defaultItems];
  }

  function categoryOption(category: string) {
    return DEFAULT_CATEGORIES.find((option) => option.label === category) ?? { label: category, tone: categoryTone(category) };
  }

  function colorOption(color: string) {
    return COLOR_OPTIONS.find((option) => option.value === color) ?? COLOR_OPTIONS[0];
  }

  function categoryId(category: string) {
    return `category:${category}`;
  }

  function noteExcerpt(note: Note) {
    const cleaned = note.content
      .replace(/[#*_`>|-]/g, " ")
      .replace(/\s+/g, " ")
      .trim();

    return cleaned || "No content yet. Enable Markdown edit to start writing.";
  }

  function isDefaultCategory(category: string) {
    return DEFAULT_CATEGORIES.some((option) => option.label === category);
  }

  function normalizedCategory(category: string) {
    return isDefaultCategory(category) ? category : "Inbox";
  }

  function categoryTone(category: string) {
    const normalized = category.toLowerCase();
    if (normalized.includes("work")) return "blue";
    if (normalized.includes("personal")) return "green";
    if (normalized.includes("idea")) return "gold";
    if (normalized.includes("private")) return "violet";
    if (normalized.includes("learning")) return "neutral";
    if (normalized.includes("inbox")) return "cyan";
    return "neutral";
  }

  function filterNotes(noteList: Note[], categoryId: string, query: string) {
    const normalizedQuery = query.trim().toLowerCase();

    return noteList.filter((note) => {
      const matchesCategory = (() => {
        if (categoryId === "all") return true;
        if (categoryId === "favorites") return note.isFavorite;
        if (categoryId === "protected") return note.isProtected;
        if (categoryId === "archived") return note.isArchived;
        if (categoryId.startsWith("category:")) {
          return !note.isArchived && normalizedCategory(note.category) === categoryId.replace("category:", "");
        }
        return true;
      })();

      const matchesQuery =
        !normalizedQuery ||
        note.title.toLowerCase().includes(normalizedQuery) ||
        note.content.toLowerCase().includes(normalizedQuery) ||
        note.category.toLowerCase().includes(normalizedQuery);

      return matchesCategory && matchesQuery;
    });
  }

  async function run(action: () => Promise<void>) {
    errorMessage = "";
    try {
      await action();
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : String(error);
    }
  }

  async function refresh(nextSelectedId = selectedNote?.id) {
    const nextNotes = searchQuery.trim()
      ? await searchNotes(searchQuery)
      : await listNotes();
    notes = nextNotes;

    const nextVisibleNotes = filterNotes(nextNotes, selectedCategory, searchQuery);
    selectedNote = nextVisibleNotes.find((note) => note.id === nextSelectedId) ?? nextVisibleNotes[0] ?? nextNotes[0] ?? null;
  }

  async function handleSearch() {
    await run(() => refresh());
  }

  async function handleCreate() {
    await run(async () => {
      await autosave.flush();
      const category = selectedCategory.startsWith("category:")
        ? selectedCategory.replace("category:", "")
        : "Inbox";

      const created = await createNote({
        title: searchQuery.trim() || "Untitled note",
        content: "",
        category,
      });
      searchQuery = "";
      selectedCategory = categoryId(created.category);
      await refresh(created.id);
      isMarkdownEditing = true;
    });
  }

  async function persistSelectedNote() {
    const note = selectedNote;
    if (!note) return;
    // Never overwrite a protected note's encrypted content while locked.
    if (note.isProtected && !vault.unlocked) return;

    await updateNote({
      id: note.id,
      title: note.title,
      content: note.content,
      category: note.category,
      tags: note.tags,
      color: note.color,
    });
  }

  const autosave = createAutosave(persistSelectedNote, {
    delayMs: 800,
    onStatus: (status) => {
      saveStatus = status;
      if (status === "saved") {
        lastSavedAt = formatClock();
        errorMessage = "";
      }
      if (status === "error") errorMessage = "Could not save the note.";
    },
  });

  // Mark the current note dirty so autosave persists it after a short pause.
  function scheduleAutosave() {
    if (!selectedNote) return;
    if (selectedNote.isProtected && !vault.unlocked) return;
    autosave.schedule();
  }

  // Explicit save (button / Ctrl+S) just flushes the pending autosave.
  async function handleSave() {
    await run(() => autosave.flush());
  }

  function formatClock() {
    return new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  }

  function formatSaveLabel(status: SaveStatus, savedAt: string): string {
    switch (status) {
      case "saving":
        return "Saving…";
      case "pending":
        return "Unsaved changes";
      case "error":
        return "Save failed";
      case "saved":
        return savedAt ? `Saved ${savedAt}` : "Saved";
      default:
        return "Saved";
    }
  }

  async function handleFavorite() {
    if (!selectedNote) return;
    const note = selectedNote;
    await run(async () => {
      await autosave.flush();
      const updated = await toggleFavorite(note.id);
      await refresh(updated.id);
    });
  }

  async function handleArchive() {
    if (!selectedNote) return;
    const note = selectedNote;
    await run(async () => {
      await autosave.flush();
      const updated = await archiveNote(note.id);
      await refresh(updated.id);
    });
  }

  async function handleDelete() {
    if (!selectedNote) return;
    const deletedId = selectedNote.id;
    await run(async () => {
      autosave.cancel();
      await deleteNote(deletedId);
      await refresh();
    });
  }

  function openVaultDialog() {
    vaultMode = vault.initialized ? "unlock" : "create";
    passphrase = "";
    passphraseConfirm = "";
    vaultError = "";
    isVaultDialogOpen = true;
  }

  function closeVaultDialog() {
    isVaultDialogOpen = false;
    passphrase = "";
    passphraseConfirm = "";
    vaultError = "";
  }

  async function submitVault() {
    vaultError = "";
    if (!passphrase) {
      vaultError = "Enter a passphrase.";
      return;
    }
    if (vaultMode === "create" && passphrase !== passphraseConfirm) {
      vaultError = "The passphrases do not match.";
      return;
    }

    try {
      vault =
        vaultMode === "create"
          ? await createVault(passphrase)
          : await unlockVault(passphrase);
      closeVaultDialog();
      await refresh();
    } catch (error) {
      vaultError = error instanceof Error ? error.message : String(error);
    }
  }

  async function handleVaultButton() {
    if (vault.unlocked) {
      await run(async () => {
        await autosave.flush();
        vault = await lockVault();
        await refresh();
      });
    } else {
      openVaultDialog();
    }
  }

  async function handleToggleProtection() {
    if (!selectedNote) return;
    const note = selectedNote;

    if (!vault.unlocked) {
      openVaultDialog();
      return;
    }

    await run(async () => {
      await autosave.flush();
      const updated = note.isProtected
        ? await unprotectNote(note.id)
        : await protectNote(note.id);
      await refresh(updated.id);
    });
  }

  async function handleExport() {
    if (!selectedNote) return;
    noticeMessage = "";
    errorMessage = "";
    try {
      const where = await exportNote(selectedNote.id);
      noticeMessage = `Exported to ${where}`;
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : String(error);
    }
  }

  async function handleShellKeydown(event: KeyboardEvent) {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
      event.preventDefault();
      searchInput?.focus();
    }

    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
      event.preventDefault();
      await handleSave();
    }

    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "n") {
      event.preventDefault();
      await handleCreate();
    }
  }

  async function selectCategory(categoryId: string) {
    await autosave.flush();
    selectedCategory = categoryId;
    const nextVisibleNotes = filterNotes(notes, categoryId, searchQuery);
    selectedNote = nextVisibleNotes[0] ?? null;
    isMarkdownEditing = false;
    isCategoryMenuOpen = false;
    isColorMenuOpen = false;
  }

  async function selectNote(note: Note) {
    if (note.id === selectedNote?.id) return;
    await autosave.flush();
    selectedNote = note;
    isMarkdownEditing = false;
    isCategoryMenuOpen = false;
    isColorMenuOpen = false;
  }

  function setNoteCategory(category: string) {
    if (!selectedNote) return;

    selectedNote.category = category;
    selectedCategory = categoryId(category);
    isCategoryMenuOpen = false;
    scheduleAutosave();
  }

  function setNoteColor(color: string) {
    if (!selectedNote) return;

    selectedNote.color = color;
    isColorMenuOpen = false;
    scheduleAutosave();
  }

  function toggleCategoryMenu() {
    isCategoryMenuOpen = !isCategoryMenuOpen;
    isColorMenuOpen = false;
  }

  function toggleColorMenu() {
    isColorMenuOpen = !isColorMenuOpen;
    isCategoryMenuOpen = false;
  }

  function colorLabel(color: string) {
    const labels: Record<string, string> = {
      amber: "Idea",
      emerald: "Personal",
      slate: "Default",
      violet: "Focus",
    };

    return labels[color] ?? color;
  }
</script>

<svelte:head>
  <title>Slate</title>
</svelte:head>

<svelte:window onkeydown={handleShellKeydown} onblur={() => autosave.flush()} />

{#snippet btnIcon(name: string)}
  <svg class="btn-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    {#if name === "star"}
      <path d="M12 3.5l2.6 5.3 5.9.85-4.25 4.15 1 5.85L12 16.9l-5.25 2.75 1-5.85L3.5 9.65l5.9-.85z" />
    {:else if name === "archive"}
      <rect x="3.5" y="4.5" width="17" height="4" rx="1" />
      <path d="M5 8.5v10a1 1 0 001 1h12a1 1 0 001-1v-10" />
      <path d="M10 12.5h4" />
    {:else if name === "lock"}
      <rect x="5" y="11" width="14" height="9" rx="2" />
      <path d="M8 11V8a4 4 0 018 0v3" />
    {:else if name === "unlock"}
      <rect x="5" y="11" width="14" height="9" rx="2" />
      <path d="M8 11V7.5a4 4 0 017.7-1.5" />
    {:else if name === "trash"}
      <path d="M4 7h16" />
      <path d="M9 7V5a1 1 0 011-1h4a1 1 0 011 1v2" />
      <path d="M6.5 7l1 12.5a1 1 0 001 .9h7a1 1 0 001-.9L18 7" />
    {:else if name === "check"}
      <path d="M5 12.5l4.2 4.2L19 7" />
    {:else if name === "plus"}
      <path d="M12 5.5v13M5.5 12h13" />
    {:else if name === "edit"}
      <path d="M4 20h4l9.5-9.5-4-4L4 16z" />
      <path d="M13.5 6.5l4 4" />
    {:else if name === "download"}
      <path d="M12 4v10" />
      <path d="M8 11l4 4 4-4" />
      <path d="M5 19h14" />
    {/if}
  </svg>
{/snippet}

<main class="screen">
  <section class="app-shell" aria-label="WS AI Note launcher">
    <aside class="sidebar" aria-label="Notes navigation">
      <div class="brand">
        <span class="brand-mark" aria-hidden="true">
          <svg viewBox="0 0 24 24" fill="none" stroke-linecap="round" stroke-width="2.2">
            <line x1="7" y1="8" x2="13" y2="8" style="stroke: var(--accent)" />
            <circle cx="15.4" cy="8" r="1.15" style="fill: var(--accent); stroke: none" />
            <line x1="7" y1="12.4" x2="17" y2="12.4" style="stroke: var(--text)" />
            <line x1="7" y1="16.8" x2="11.6" y2="16.8" style="stroke: var(--text)" />
          </svg>
        </span>
        <div>
          <p>Slate</p>
          <small>Local Markdown notes</small>
        </div>
      </div>

      <div class="sidebar-tools">
        <div class="search-box" role="search">
          <span aria-hidden="true">⌕</span>
          <input
            bind:this={searchInput}
            bind:value={searchQuery}
            oninput={handleSearch}
            placeholder="Search notes…"
            aria-label="Search notes"
          />
          <kbd>Ctrl K</kbd>
        </div>

        <button class="new-button" type="button" onclick={handleCreate}>{@render btnIcon("plus")}<span>New note</span></button>

        <button class="vault-toggle" type="button" onclick={handleVaultButton}>
          {#if vault.unlocked}
            {@render btnIcon("unlock")}<span>Lock vault</span>
          {:else if vault.initialized}
            {@render btnIcon("lock")}<span>Unlock vault</span>
          {:else}
            {@render btnIcon("lock")}<span>Set up vault</span>
          {/if}
        </button>
      </div>

      {#if errorMessage}
        <p class="error" role="alert">{errorMessage}</p>
      {/if}

      {#if noticeMessage}
        <p class="notice" role="status">{noticeMessage}</p>
      {/if}

      <nav class="category-nav" aria-label="Categories">
        {#each categoryItems as item}
          <button
            class:active={selectedCategory === item.id}
            class={["category-button", item.tone]}
            type="button"
            onclick={() => selectCategory(item.id)}
          >
            <span class="category-dot" aria-hidden="true"></span>
            <span>{item.label}</span>
            <strong>{item.count}</strong>
          </button>
        {/each}
      </nav>

      <section class="notes-section" aria-label="Notes list">
        <div class="notes-meta">
          <strong>{visibleNotes.length} notes</strong>
          <span>{categoryItems.find((item) => item.id === selectedCategory)?.label ?? "All Notes"}</span>
        </div>

        <div class="notes-list" aria-label="Filtered notes">
          {#if visibleNotes.length}
            {#each visibleNotes as note}
              <button
                class:active={selectedNote?.id === note.id}
                class:archived={note.isArchived}
                type="button"
                onclick={() => selectNote(note)}
              >
                <span class={["note-color", note.color]} aria-hidden="true"></span>
                <span class="note-copy">
                  <span class="note-card-topline">
                    <strong>{note.title || "Untitled note"}</strong>
                    <small>{note.isProtected ? "🔒 " : ""}{normalizedCategory(note.category)}</small>
                  </span>
                  <span class="note-excerpt">{noteExcerpt(note)}</span>
                </span>
              </button>
            {/each}
          {:else}
            <div class="empty-list">
              <strong>No notes here</strong>
              <p>Create one in this category or clear the search.</p>
            </div>
          {/if}
        </div>
      </section>

      <footer class="signature">juanpa reyest <span aria-hidden="true">|</span> development engineer</footer>
    </aside>

    <section class="content-panel" aria-label="Selected note content">
      {#if selectedNote}
        <header class="content-header">
          <div class="title-stack">
            <label for="note-title">Note title</label>
            <input
              id="note-title"
              bind:value={selectedNote.title}
              oninput={scheduleAutosave}
              onblur={() => autosave.flush()}
              disabled={isNoteLocked}
              aria-label="Note title"
            />
          </div>

          <div class="actions" aria-label="Note actions">
            <button type="button" onclick={handleFavorite}>{@render btnIcon("star")}<span>{selectedNote.isFavorite ? "Starred" : "Star"}</span></button>
            <button type="button" onclick={handleArchive}>{@render btnIcon("archive")}<span>{selectedNote.isArchived ? "Restore" : "Archive"}</span></button>
            <button type="button" onclick={handleToggleProtection}>{@render btnIcon(selectedNote.isProtected ? "unlock" : "lock")}<span>{selectedNote.isProtected ? "Unprotect" : "Protect"}</span></button>
            <button type="button" onclick={handleExport}>{@render btnIcon("download")}<span>Export</span></button>
            <button class="danger" type="button" onclick={handleDelete}>{@render btnIcon("trash")}<span>Delete</span></button>
            <button class={["save", "save-status", saveStatus]} type="button" onclick={handleSave} title="Save now (Ctrl+S)">{@render btnIcon("check")}<span>{saveLabel}</span></button>
          </div>
        </header>

        <div class="meta-strip" aria-label="Note metadata">
          <div class="field-group">
            <span class="field-label">Category</span>
            <div class="custom-menu">
              <button
                class="custom-trigger"
                type="button"
                aria-haspopup="listbox"
                aria-expanded={isCategoryMenuOpen}
                onclick={toggleCategoryMenu}
              >
                <span class={["category-dot", categoryOption(selectedNote.category).tone]} aria-hidden="true"></span>
                <strong>{normalizedCategory(selectedNote.category)}</strong>
                <span aria-hidden="true">⌄</span>
              </button>

              {#if isCategoryMenuOpen}
                <div class="menu-popover" role="listbox" aria-label="Default categories">
                  {#each DEFAULT_CATEGORIES as option}
                    <button
                      class:active={normalizedCategory(selectedNote?.category ?? "Inbox") === option.label}
                      type="button"
                      role="option"
                      aria-selected={normalizedCategory(selectedNote?.category ?? "Inbox") === option.label}
                      onclick={() => setNoteCategory(option.label)}
                    >
                      <span class={["category-dot", option.tone]} aria-hidden="true"></span>
                      <span>{option.label}</span>
                    </button>
                  {/each}
                </div>
              {/if}
            </div>
          </div>

          <div class="field-group color-field">
            <span class="field-label">Color</span>
            <div class="custom-menu">
              <button
                class="custom-trigger"
                type="button"
                aria-haspopup="listbox"
                aria-expanded={isColorMenuOpen}
                onclick={toggleColorMenu}
              >
                <span class={["color-swatch", selectedNote.color]} aria-hidden="true"></span>
                <strong>{colorOption(selectedNote.color).label}</strong>
                <span aria-hidden="true">⌄</span>
              </button>

              {#if isColorMenuOpen}
                <div class="menu-popover color-popover" role="listbox" aria-label="Note colors">
                  {#each COLOR_OPTIONS as option}
                    <button
                      class:active={selectedNote?.color === option.value}
                      type="button"
                      role="option"
                      aria-selected={selectedNote?.color === option.value}
                      onclick={() => setNoteColor(option.value)}
                    >
                      <span class={["color-swatch", option.value]} aria-hidden="true"></span>
                      <span>
                        <strong>{option.label}</strong>
                        <small>{option.description}</small>
                      </span>
                    </button>
                  {/each}
                </div>
              {/if}
            </div>
          </div>

          <span class={["status-chip", isNoteLocked ? "locked" : "open"]}>
            {#if selectedNote.isProtected}
              {isNoteLocked ? "🔒 Locked" : "🔓 Protected"}
            {:else}
              Open note
            {/if}
          </span>

          <span class="status-chip">{colorLabel(selectedNote.color)}</span>
        </div>

        {#if selectedNote.isProtected && !vault.unlocked}
          <div class="locked-panel">
            <span class="locked-icon" aria-hidden="true">🔒</span>
            <h2>This note is protected</h2>
            <p>Unlock the vault to read and edit its content.</p>
            <button class="save" type="button" onclick={openVaultDialog}>Unlock vault</button>
          </div>
        {:else}
        <div class="editor-layout" class:preview-only={!isMarkdownEditing}>
          {#if isMarkdownEditing}
            <section class="editor-pane" aria-label="Markdown source editor">
              <div class="pane-label">
                <span>Markdown edit</span>
                <kbd>Ctrl S</kbd>
              </div>
              <textarea
                bind:value={selectedNote.content}
                oninput={scheduleAutosave}
                onblur={() => autosave.flush()}
                aria-label="Markdown editor"
                spellcheck="false"
              ></textarea>
            </section>
          {/if}

          <section class="preview-pane" aria-label="Markdown preview">
            <div class="pane-label">
              <span>Preview</span>
              <button class="edit-markdown-button" type="button" onclick={() => (isMarkdownEditing = !isMarkdownEditing)}>
                {@render btnIcon("edit")}<span>{isMarkdownEditing ? "Hide editor" : "Edit Markdown"}</span>
              </button>
            </div>
            <article class="preview">
              {@html previewHtml}
            </article>
          </section>
        </div>
        {/if}
      {:else}
        <div class="empty-content">
          <span class="brand-mark large" aria-hidden="true">
            <svg viewBox="0 0 24 24" fill="none" stroke-linecap="round" stroke-width="2.2">
              <line x1="7" y1="8" x2="13" y2="8" style="stroke: var(--accent)" />
              <circle cx="15.4" cy="8" r="1.15" style="fill: var(--accent); stroke: none" />
              <line x1="7" y1="12.4" x2="17" y2="12.4" style="stroke: var(--text)" />
              <line x1="7" y1="16.8" x2="11.6" y2="16.8" style="stroke: var(--text)" />
            </svg>
          </span>
          <h1>No note selected</h1>
          <p>Choose a category, pick a note, or create a new one.</p>
          <button class="save" type="button" onclick={handleCreate}>Create note</button>
        </div>
      {/if}
    </section>
  </section>

  {#if isVaultDialogOpen}
    <div class="vault-overlay" role="dialog" aria-modal="true" aria-label="Vault passphrase">
      <div class="vault-dialog">
        <h2>{vaultMode === "create" ? "Set up your vault" : "Unlock your vault"}</h2>
        <p>
          {vaultMode === "create"
            ? "Choose a master passphrase. It protects all protected notes and is never stored. If you forget it, protected notes cannot be recovered."
            : "Enter your master passphrase to read protected notes."}
        </p>

        <label for="vault-pass">Passphrase</label>
        <!-- svelte-ignore a11y_autofocus -->
        <input id="vault-pass" type="password" autocomplete="off" bind:value={passphrase} />

        {#if vaultMode === "create"}
          <label for="vault-confirm">Confirm passphrase</label>
          <input id="vault-confirm" type="password" autocomplete="off" bind:value={passphraseConfirm} />
        {/if}

        {#if vaultError}
          <p class="vault-error" role="alert">{vaultError}</p>
        {/if}

        <div class="vault-actions">
          <button type="button" onclick={closeVaultDialog}>Cancel</button>
          <button class="save" type="button" onclick={submitVault}>
            {vaultMode === "create" ? "Create vault" : "Unlock"}
          </button>
        </div>
      </div>
    </div>
  {/if}
</main>

<style>
  button,
  input,
  textarea {
    font: inherit;
  }

  button {
    cursor: pointer;
  }

  .btn-icon {
    width: 15px;
    height: 15px;
    flex: 0 0 auto;
  }

  button:focus-visible,
  input:focus-visible,
  textarea:focus-visible {
    outline: 2px solid var(--accent-line);
    outline-offset: 2px;
  }

  .screen {
    width: 100vw;
    height: 100vh;
    padding: 0;
    overflow: hidden;
  }

  .app-shell {
    width: 100%;
    height: 100%;
    display: grid;
    grid-template-columns: minmax(288px, clamp(310px, 31vw, 430px)) minmax(0, 1fr);
    gap: 1px;
    overflow: hidden;
    background: var(--line);
  }

  .sidebar,
  .content-panel {
    min-width: 0;
    min-height: 0;
  }

  .sidebar {
    display: grid;
    grid-template-rows: auto auto auto auto 1fr auto;
    gap: 12px;
    padding: clamp(12px, 1.2vw, 16px);
    background: var(--bg-raised);
    overflow: hidden;
  }

  .brand {
    display: flex;
    gap: 10px;
    align-items: center;
  }

  .brand-mark {
    width: 32px;
    height: 32px;
    display: inline-grid;
    flex: 0 0 auto;
    place-items: center;
    border-radius: 8px;
    color: var(--accent);
    background: var(--accent-bg);
    font-weight: 700;
  }

  .brand-mark.large {
    width: 52px;
    height: 52px;
    border-radius: 14px;
    font-size: 1.3rem;
  }

  .brand-mark svg {
    width: 60%;
    height: 60%;
  }

  .signature {
    padding-top: 10px;
    border-top: 1px solid var(--line);
    color: var(--text-3);
    font-size: 0.68rem;
    letter-spacing: 0.01em;
  }

  .signature span {
    margin: 0 5px;
    opacity: 0.6;
  }

  .brand p,
  .brand small {
    margin: 0;
  }

  .brand p {
    color: var(--text);
    font-size: 0.92rem;
    font-weight: 600;
  }

  .brand small {
    font-size: 0.76rem;
  }

  .brand small,
  .notes-meta span,
  .note-copy small,
  .empty-list p,
  .empty-content p {
    color: var(--text-2);
  }

  .category-nav {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 4px;
  }

  .category-button {
    width: 100%;
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: 8px;
    align-items: center;
    min-height: 32px;
    padding: 6px 8px;
    border: 1px solid transparent;
    border-radius: 7px;
    color: var(--text-2);
    background: transparent;
    font-size: 0.86rem;
    text-align: left;
  }

  .category-button span:nth-child(2) {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .category-button:hover {
    background: var(--surface-hover);
    color: var(--text);
  }

  .category-button.active {
    color: var(--text);
    background: var(--surface-active);
  }

  .category-button strong {
    color: var(--text-3);
    font-size: 0.74rem;
    font-weight: 500;
  }

  .category-dot,
  .note-color {
    width: 8px;
    height: 8px;
    border-radius: 999px;
    background: #7a7d84;
  }

  .category-button.cyan .category-dot,
  .category-dot.cyan,
  .note-color.slate {
    background: #6b93b0;
  }

  .category-button.blue .category-dot,
  .category-dot.blue {
    background: #6f8fbf;
  }

  .category-button.green .category-dot,
  .category-dot.green,
  .note-color.emerald {
    background: #6fae8e;
  }

  .category-button.gold .category-dot,
  .category-dot.gold,
  .note-color.amber {
    background: #c2a368;
  }

  .category-button.violet .category-dot,
  .category-dot.violet,
  .note-color.violet {
    background: #9a86c4;
  }

  .category-button.muted .category-dot,
  .category-dot.muted,
  .category-dot.neutral {
    background: #7a7d84;
  }

  .color-swatch {
    width: 12px;
    height: 12px;
    border-radius: 999px;
  }

  .color-swatch.slate {
    background: #6b93b0;
  }

  .color-swatch.violet {
    background: #9a86c4;
  }

  .color-swatch.amber {
    background: #c2a368;
  }

  .color-swatch.emerald {
    background: #6fae8e;
  }

  .sidebar-tools {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 8px;
  }

  .search-box {
    min-width: 0;
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: 8px;
    align-items: center;
    min-height: 38px;
    padding: 7px 10px;
    border: 1px solid var(--line);
    border-radius: 8px;
    color: var(--text-3);
    background: var(--bg);
  }

  .search-box input {
    min-width: 0;
    border: 0;
    outline: 0;
    color: var(--text);
    background: transparent;
    font-size: 0.9rem;
  }

  kbd {
    padding: 2px 6px;
    border: 1px solid var(--line);
    border-radius: 6px;
    color: var(--text-3);
    background: var(--bg-raised);
    font-size: 0.68rem;
    font-weight: 500;
    white-space: nowrap;
  }

  .new-button,
  .save {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    border: 1px solid var(--accent-line);
    border-radius: 8px;
    color: var(--accent);
    background: var(--accent-bg);
    font-weight: 600;
  }

  .new-button {
    min-height: 38px;
    padding: 0 12px;
    font-size: 0.85rem;
    white-space: nowrap;
  }

  .new-button:hover,
  .save:hover {
    background: rgba(139, 155, 212, 0.18);
  }

  .notes-meta {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    align-items: center;
    color: var(--text-2);
    font-size: 0.78rem;
  }

  .notes-section {
    min-height: 0;
    display: grid;
    grid-template-rows: auto 1fr;
    gap: 6px;
    padding-top: 8px;
    border-top: 1px solid var(--line);
  }

  .notes-list {
    min-height: 0;
    display: grid;
    gap: 2px;
    align-content: start;
    overflow: auto;
    padding-right: 2px;
  }

  .notes-list button {
    width: 100%;
    position: relative;
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 10px;
    min-height: 0;
    padding: 10px 10px;
    border: 0;
    border-radius: 7px;
    color: var(--text);
    background: transparent;
    text-align: left;
  }

  .notes-list button:hover {
    background: var(--surface-hover);
  }

  .notes-list button.active {
    color: var(--text);
    background: var(--surface-active);
  }

  .notes-list button.archived {
    opacity: 0.5;
  }

  .note-color {
    margin-top: 6px;
  }

  .note-copy {
    min-width: 0;
    display: grid;
    gap: 4px;
  }

  .note-card-topline {
    min-width: 0;
    display: grid;
    gap: 2px;
  }

  .note-card-topline strong,
  .note-card-topline small,
  .note-excerpt {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .note-card-topline strong {
    font-size: 0.88rem;
    font-weight: 600;
  }

  .note-card-topline small {
    color: var(--text-3);
    font-size: 0.74rem;
  }

  .note-excerpt {
    color: var(--text-2);
    font-size: 0.78rem;
    line-height: 1.4;
  }

  .status-chip {
    width: fit-content;
    padding: 3px 8px;
    border: 1px solid var(--line);
    border-radius: 6px;
    color: var(--text-2);
    background: transparent;
    font-size: 0.68rem;
    font-weight: 500;
  }

  .content-panel {
    display: grid;
    grid-template-rows: auto auto 1fr;
    gap: clamp(10px, 1vw, 14px);
    padding: clamp(14px, 1.4vw, 22px);
    background: var(--bg);
    color: var(--text);
    overflow: hidden;
  }

  .content-header {
    display: flex;
    align-items: start;
    justify-content: space-between;
    gap: 12px;
  }

  .title-stack {
    min-width: 0;
    display: grid;
    gap: 4px;
    flex: 1;
  }

  .title-stack label,
  .pane-label {
    color: var(--text-3);
    font-size: 0.72rem;
    font-weight: 500;
  }

  .title-stack input {
    width: 100%;
    border: 0;
    outline: 0;
    color: var(--text);
    background: transparent;
    font-size: clamp(1.15rem, 2vw, 1.6rem);
    font-weight: 600;
    line-height: 1.2;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    justify-content: flex-end;
    max-width: 360px;
  }

  .actions button {
    min-height: 32px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border: 1px solid var(--line-strong);
    border-radius: 6px;
    color: var(--text);
    background: var(--btn);
    padding: 6px 12px;
    font-size: 0.82rem;
    font-weight: 500;
  }

  .actions button:hover {
    border-color: var(--accent-line);
    background: var(--btn-hover);
    color: var(--text);
  }

  .actions .danger {
    color: var(--danger);
  }

  .actions .danger:hover {
    background: var(--danger-bg);
  }

  .actions .save {
    border-color: var(--accent-line);
    color: var(--accent);
    background: var(--accent-bg);
    min-width: 124px;
    justify-content: center;
  }

  /* Autosave status cues so the user can tell saved from unsaved at a glance. */
  .actions .save-status.pending {
    color: var(--text-2);
    border-color: var(--line-strong);
    background: var(--btn);
  }

  .actions .save-status.saving {
    color: var(--text-3);
    border-color: var(--line-strong);
    background: var(--btn);
  }

  .actions .save-status.error {
    color: var(--danger);
    border-color: var(--danger);
    background: var(--danger-bg);
  }

  .title-stack input:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .meta-strip {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: end;
  }

  .field-group {
    position: relative;
    min-width: 172px;
    display: grid;
    gap: 4px;
  }

  .field-label {
    color: var(--text-3);
    font-size: 0.72rem;
    font-weight: 500;
  }

  .custom-menu {
    position: relative;
  }

  .custom-trigger {
    width: 100%;
    min-height: 36px;
    display: grid;
    grid-template-columns: auto 1fr auto;
    gap: 8px;
    align-items: center;
    border: 1px solid var(--line-strong);
    border-radius: 7px;
    color: var(--text);
    background: var(--btn);
    padding: 7px 10px;
    font-size: 0.85rem;
    text-align: left;
  }

  .custom-trigger:hover {
    border-color: var(--accent-line);
    background: var(--btn-hover);
  }

  .custom-trigger strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 500;
  }

  .menu-popover {
    position: absolute;
    z-index: 20;
    top: calc(100% + 6px);
    left: 0;
    width: min(240px, 78vw);
    display: grid;
    gap: 2px;
    padding: 6px;
    border: 1px solid var(--line-strong);
    border-radius: 9px;
    background: var(--bg-raised);
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.45);
  }

  .menu-popover button {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 9px;
    align-items: center;
    min-height: 34px;
    border: 0;
    border-radius: 6px;
    color: var(--text-2);
    background: transparent;
    padding: 7px 8px;
    text-align: left;
  }

  .menu-popover button:hover,
  .menu-popover button.active {
    color: var(--text);
    background: var(--surface-active);
  }

  .color-popover button span:last-child {
    display: grid;
    gap: 2px;
  }

  .color-popover small {
    color: var(--text-3);
    font-size: 0.72rem;
  }

  .status-chip {
    min-height: 30px;
    display: inline-flex;
    align-items: center;
    color: var(--text-2);
  }

  .status-chip.locked {
    color: var(--accent);
    border-color: var(--accent-line);
  }

  .status-chip.open {
    color: var(--text-2);
  }

  .editor-layout {
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(280px, 1.02fr) minmax(280px, 0.98fr);
    gap: 12px;
  }

  .editor-layout.preview-only {
    grid-template-columns: 1fr;
  }

  .editor-pane,
  .preview-pane {
    min-width: 0;
    min-height: 0;
    display: grid;
    grid-template-rows: auto 1fr;
    gap: 8px;
  }

  .pane-label {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .edit-markdown-button {
    min-height: 28px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border: 1px solid var(--line-strong);
    border-radius: 6px;
    color: var(--text);
    background: var(--btn);
    padding: 4px 12px;
    font-size: 0.76rem;
    font-weight: 500;
  }

  .edit-markdown-button:hover {
    border-color: var(--accent-line);
    background: var(--btn-hover);
    color: var(--text);
  }

  textarea,
  .preview {
    width: 100%;
    min-width: 0;
    min-height: 0;
    border: 1px solid var(--line);
    border-radius: 8px;
    color: var(--text);
    background: var(--bg);
    line-height: 1.7;
  }

  textarea {
    resize: none;
    outline: none;
    padding: 16px;
    font-family: "SFMono-Regular", Consolas, "Liberation Mono", monospace;
    font-size: 0.92rem;
  }

  .preview {
    overflow: auto;
    padding: 18px 20px;
    font-size: 0.94rem;
  }

  .preview :global(h1),
  .preview :global(h2),
  .preview :global(h3),
  .preview :global(p) {
    margin-bottom: 0.75rem;
  }

  .preview :global(h1),
  .preview :global(h2),
  .preview :global(h3) {
    margin-top: 0.75em;
    font-weight: 600;
  }

  .preview :global(h1) {
    font-size: 1.4rem;
  }

  .preview :global(h2) {
    font-size: 1.2rem;
  }

  .preview :global(h3) {
    font-size: 1.05rem;
  }

  .preview :global(a) {
    color: var(--accent);
  }

  .preview :global(pre) {
    overflow: auto;
    padding: 12px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.04);
  }

  .preview :global(code) {
    font-family: "SFMono-Regular", Consolas, "Liberation Mono", monospace;
  }

  .preview :global(blockquote) {
    margin: 0 0 0.75rem;
    padding-left: 12px;
    border-left: 2px solid var(--line-strong);
    color: var(--text-2);
  }

  .preview :global(table) {
    width: 100%;
    border-collapse: collapse;
  }

  .preview :global(th),
  .preview :global(td) {
    padding: 8px;
    border: 1px solid var(--line);
  }

  .error {
    margin: 0;
    padding: 10px 12px;
    border: 1px solid var(--danger-bg);
    border-radius: 8px;
    color: var(--danger);
    background: var(--danger-bg);
  }

  .notice {
    margin: 0;
    padding: 10px 12px;
    border: 1px solid rgba(111, 174, 142, 0.25);
    border-radius: 8px;
    color: #7fbf9a;
    background: rgba(111, 174, 142, 0.12);
    font-size: 0.82rem;
    word-break: break-all;
  }

  .empty-list,
  .empty-content {
    padding: 24px;
    text-align: center;
  }

  .empty-list {
    color: var(--text-2);
  }

  .empty-list p {
    margin: 8px 0 0;
  }

  .empty-content {
    align-self: center;
    justify-self: center;
    max-width: 360px;
  }

  .empty-content h1 {
    margin: 14px 0 8px;
    font-weight: 600;
  }

  .empty-content .save {
    margin-top: 16px;
    min-height: 38px;
    padding: 0 14px;
  }

  .vault-toggle {
    grid-column: 1 / -1;
    min-height: 34px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    border: 1px solid var(--line-strong);
    border-radius: 8px;
    color: var(--text);
    background: var(--btn);
    font-size: 0.82rem;
    font-weight: 500;
  }

  .vault-toggle:hover {
    border-color: var(--accent-line);
    background: var(--btn-hover);
    color: var(--text);
  }

  .locked-panel {
    min-height: 0;
    display: grid;
    align-content: center;
    justify-items: center;
    gap: 8px;
    padding: 24px;
    border: 1px solid var(--line);
    border-radius: 8px;
    text-align: center;
    color: var(--text-2);
  }

  .locked-icon {
    font-size: 1.8rem;
  }

  .locked-panel h2 {
    margin: 4px 0 0;
    color: var(--text);
    font-weight: 600;
  }

  .locked-panel .save {
    margin-top: 10px;
    min-height: 36px;
    padding: 0 14px;
  }

  .vault-overlay {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    padding: 20px;
    background: rgba(0, 0, 0, 0.55);
  }

  .vault-dialog {
    width: min(420px, 100%);
    display: grid;
    gap: 8px;
    padding: 20px;
    border: 1px solid var(--line-strong);
    border-radius: 12px;
    background: var(--bg-raised);
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
  }

  .vault-dialog h2 {
    margin: 0;
    color: var(--text);
    font-weight: 600;
  }

  .vault-dialog p {
    margin: 0 0 6px;
    color: var(--text-2);
    font-size: 0.84rem;
    line-height: 1.5;
  }

  .vault-dialog label {
    color: var(--text-3);
    font-size: 0.72rem;
  }

  .vault-dialog input {
    min-height: 38px;
    border: 1px solid var(--line);
    border-radius: 8px;
    color: var(--text);
    background: var(--bg);
    padding: 0 10px;
    font-size: 0.9rem;
  }

  .vault-error {
    margin: 0;
    color: var(--danger);
    font-size: 0.82rem;
  }

  .vault-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 8px;
  }

  .vault-actions button {
    min-height: 36px;
    border: 1px solid var(--line-strong);
    border-radius: 8px;
    color: var(--text);
    background: var(--btn);
    padding: 0 14px;
    font-size: 0.85rem;
    font-weight: 500;
  }

  .vault-actions button:hover {
    border-color: var(--accent-line);
    background: var(--btn-hover);
  }

  .vault-actions .save {
    border-color: var(--accent-line);
    color: var(--accent);
    background: var(--accent-bg);
  }

  @media (max-width: 1120px) {
    .editor-layout {
      grid-template-columns: 1fr;
    }
  }

  @media (max-width: 860px) {
    .app-shell {
      grid-template-columns: minmax(280px, 34vw) minmax(0, 1fr);
    }

    .sidebar-tools {
      grid-template-columns: 1fr;
    }

    .new-button {
      width: 100%;
    }

    .content-header {
      display: grid;
    }

    .actions {
      justify-content: start;
      max-width: none;
    }
  }

  @media (max-width: 680px) {
    .screen {
      width: 100%;
      height: auto;
      min-height: 100vh;
      padding: 0;
      overflow: auto;
    }

    .app-shell {
      height: auto;
      min-height: 100vh;
      grid-template-columns: 1fr;
      border-radius: 0;
    }

    .sidebar,
    .content-panel {
      min-height: auto;
    }

    .category-nav {
      grid-template-columns: repeat(auto-fit, minmax(148px, 1fr));
    }

    .notes-list {
      max-height: 280px;
    }
  }

  @media (max-width: 560px) {
    .search-box {
      grid-template-columns: auto 1fr;
    }

    .search-box kbd,
    .pane-label kbd {
      display: none;
    }

    .field-group {
      width: 100%;
    }
  }
</style>
