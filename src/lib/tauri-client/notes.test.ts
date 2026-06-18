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

  it("reveals protected content while unlocked and blanks it when locked", async () => {
    await notes.createVault("master");
    const target = (await notes.listNotes())[0];
    await notes.protectNote(target.id);

    let listed = await notes.listNotes();
    expect(listed.find((note) => note.id === target.id)?.content).not.toBe("");

    await notes.lockVault();
    listed = await notes.listNotes();
    const locked = listed.find((note) => note.id === target.id)!;
    expect(locked.isProtected).toBe(true);
    expect(locked.content).toBe("");
  });

  it("rejects unlocking with a wrong passphrase", async () => {
    await notes.createVault("correct");
    await notes.lockVault();

    await expect(notes.unlockVault("incorrect")).rejects.toThrow();
  });

  it("refuses to protect a note when the vault is locked", async () => {
    const target = (await notes.listNotes())[0];
    await expect(notes.protectNote(target.id)).rejects.toThrow();
  });
});
