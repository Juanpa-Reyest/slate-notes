import { beforeEach, describe, expect, it, vi } from "vitest";

// The real Tauri API is never used in browser-preview mode, but mocking it keeps
// the import clean under the test runner.
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

let notes: typeof import("./notes");

// Reset the module before each test so the in-memory preview state is fresh.
beforeEach(async () => {
  vi.resetModules();
  notes = await import("./notes");
});

describe("tauri-client browser-preview fallback", () => {
  it("lists seeded preview notes", async () => {
    const list = await notes.listNotes();
    expect(list.length).toBeGreaterThan(0);
  });

  it("creates a note at the front of the list", async () => {
    const before = (await notes.listNotes()).length;
    const created = await notes.createNote({ title: "Test note", content: "hi" });

    expect(created.title).toBe("Test note");
    const after = await notes.listNotes();
    expect(after.length).toBe(before + 1);
    expect(after[0].id).toBe(created.id);
  });

  it("searches by title and excludes non-matches", async () => {
    await notes.createNote({ title: "Unique Zephyr", content: "body" });

    const hits = await notes.searchNotes("zephyr");
    expect(hits.some((note) => note.title === "Unique Zephyr")).toBe(true);

    const misses = await notes.searchNotes("no-such-term-xyz");
    expect(misses.length).toBe(0);
  });

  it("setting up recovery does not open any note", async () => {
    const status = await notes.setUpRecovery("master");
    expect(status.recoveryInitialized).toBe(true);
    expect(status.activeNoteOpen).toBe(false);
  });

  it("protect requires recovery setup and verifies the passphrase", async () => {
    const target = (await notes.listNotes())[0];

    // No recovery set up yet → cannot protect.
    await expect(notes.protectNote(target.id, "master")).rejects.toThrow();

    await notes.setUpRecovery("master");
    // Wrong passphrase is rejected.
    await expect(notes.protectNote(target.id, "wrong")).rejects.toThrow();

    const protectedNote = await notes.protectNote(target.id, "master");
    expect(protectedNote.isProtected).toBe(true);
    // The protected note becomes the active (revealed) one.
    expect((await notes.recoveryStatus()).activeNoteOpen).toBe(true);
  });

  it("always blanks protected content in list and search", async () => {
    await notes.setUpRecovery("master");
    const target = (await notes.listNotes())[0];
    await notes.protectNote(target.id, "master");

    // Even though the note is active, list/search NEVER expose its content.
    const listed = await notes.listNotes();
    const fromList = listed.find((note) => note.id === target.id)!;
    expect(fromList.isProtected).toBe(true);
    expect(fromList.content).toBe("");

    const hits = await notes.searchNotes(target.title);
    const fromSearch = hits.find((note) => note.id === target.id)!;
    expect(fromSearch.content).toBe("");
  });

  it("reveals plaintext with the correct passphrase and rejects a wrong one", async () => {
    await notes.setUpRecovery("master");
    const target = (await notes.listNotes())[0];
    const original = target.content;
    await notes.protectNote(target.id, "master");

    // Drop the active key, then reveal again.
    await notes.clearActive();
    await expect(notes.revealNote(target.id, "wrong")).rejects.toThrow();

    const revealed = await notes.revealNote(target.id, "master");
    expect(revealed.content).toBe(original);
    expect((await notes.recoveryStatus()).activeNoteOpen).toBe(true);
  });

  it("rejects updating a protected note that is not active", async () => {
    await notes.setUpRecovery("master");
    const target = (await notes.listNotes())[0];
    await notes.protectNote(target.id, "master");
    await notes.clearActive();

    await expect(
      notes.updateNote({
        id: target.id,
        title: target.title,
        content: "tampered",
        category: target.category,
        tags: target.tags,
        color: target.color,
      }),
    ).rejects.toThrow();
  });

  it("clearActive drops the open state", async () => {
    await notes.setUpRecovery("master");
    const target = (await notes.listNotes())[0];
    await notes.protectNote(target.id, "master");
    expect((await notes.recoveryStatus()).activeNoteOpen).toBe(true);

    const status = await notes.clearActive();
    expect(status.activeNoteOpen).toBe(false);
    expect((await notes.recoveryStatus()).activeNoteOpen).toBe(false);
  });

  it("unprotect restores plaintext and clears the active state", async () => {
    await notes.setUpRecovery("master");
    const target = (await notes.listNotes())[0];
    const original = target.content;
    await notes.protectNote(target.id, "master");

    const restored = await notes.unprotectNote(target.id, "master");
    expect(restored.isProtected).toBe(false);
    expect(restored.content).toBe(original);
    expect((await notes.recoveryStatus()).activeNoteOpen).toBe(false);

    const listed = await notes.listNotes();
    expect(listed.find((note) => note.id === target.id)?.content).toBe(original);
  });

  it("recovers a protected note: rejects a wrong passphrase, then restores plaintext and drops protection", async () => {
    await notes.setUpRecovery("master");
    const target = (await notes.listNotes())[0];
    const original = target.content;
    await notes.protectNote(target.id, "master");
    await notes.clearActive();

    // A wrong recovery passphrase is rejected.
    await expect(notes.recoverNote(target.id, "wrong")).rejects.toThrow();

    // The correct recovery passphrase restores the content and removes protection.
    const recovered = await notes.recoverNote(target.id, "master");
    expect(recovered.isProtected).toBe(false);
    expect(recovered.content).toBe(original);
    expect((await notes.recoveryStatus()).activeNoteOpen).toBe(false);

    // The restored plaintext is visible in the list again.
    const listed = await notes.listNotes();
    expect(listed.find((note) => note.id === target.id)?.content).toBe(original);
  });
});
