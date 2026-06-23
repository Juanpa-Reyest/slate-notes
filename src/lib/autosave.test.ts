import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createAutosave, type SaveStatus } from "./autosave";

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("createAutosave", () => {
  it("does not save before the debounce delay elapses", () => {
    const save = vi.fn().mockResolvedValue(undefined);
    const autosave = createAutosave(save, { delayMs: 800 });

    autosave.schedule();
    vi.advanceTimersByTime(799);

    expect(save).not.toHaveBeenCalled();
  });

  it("saves once after the debounce delay", async () => {
    const save = vi.fn().mockResolvedValue(undefined);
    const autosave = createAutosave(save, { delayMs: 800 });

    autosave.schedule();
    await vi.advanceTimersByTimeAsync(800);

    expect(save).toHaveBeenCalledTimes(1);
  });

  it("coalesces rapid changes into a single save", async () => {
    const save = vi.fn().mockResolvedValue(undefined);
    const autosave = createAutosave(save, { delayMs: 800 });

    autosave.schedule();
    vi.advanceTimersByTime(300);
    autosave.schedule();
    vi.advanceTimersByTime(300);
    autosave.schedule();
    await vi.advanceTimersByTimeAsync(800);

    expect(save).toHaveBeenCalledTimes(1);
  });

  it("flush saves immediately when a change is pending and clears the timer", async () => {
    const save = vi.fn().mockResolvedValue(undefined);
    const autosave = createAutosave(save, { delayMs: 800 });

    autosave.schedule();
    await autosave.flush();

    expect(save).toHaveBeenCalledTimes(1);

    // The pending timer must not fire a second save after flush.
    await vi.advanceTimersByTimeAsync(800);
    expect(save).toHaveBeenCalledTimes(1);
  });

  it("flush is a no-op when nothing is pending", async () => {
    const save = vi.fn().mockResolvedValue(undefined);
    const autosave = createAutosave(save, { delayMs: 800 });

    await autosave.flush();

    expect(save).not.toHaveBeenCalled();
  });

  it("cancel drops the pending save", async () => {
    const save = vi.fn().mockResolvedValue(undefined);
    const autosave = createAutosave(save, { delayMs: 800 });

    autosave.schedule();
    autosave.cancel();
    await vi.advanceTimersByTimeAsync(800);

    expect(save).not.toHaveBeenCalled();
  });

  it("reports status transitions: pending -> saving -> saved", async () => {
    const statuses: SaveStatus[] = [];
    const save = vi.fn().mockResolvedValue(undefined);
    const autosave = createAutosave(save, {
      delayMs: 800,
      onStatus: (status) => statuses.push(status),
    });

    autosave.schedule();
    expect(statuses).toContain("pending");

    await vi.advanceTimersByTimeAsync(800);
    expect(statuses).toEqual(["pending", "saving", "saved"]);
  });

  it("reports an error status when the save fails", async () => {
    const statuses: SaveStatus[] = [];
    const save = vi.fn().mockRejectedValue(new Error("boom"));
    const autosave = createAutosave(save, {
      delayMs: 800,
      onStatus: (status) => statuses.push(status),
    });

    autosave.schedule();
    await vi.advanceTimersByTimeAsync(800);

    expect(statuses).toEqual(["pending", "saving", "error"]);
  });

  it("flush waits for an in-flight save to finish before resolving", async () => {
    const order: string[] = [];
    let resolveSave: () => void = () => {};
    const save = vi.fn().mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          resolveSave = () => {
            order.push("saved");
            resolve();
          };
        }),
    );
    const autosave = createAutosave(save, { delayMs: 800 });

    autosave.schedule();
    await vi.advanceTimersByTimeAsync(800); // save is now in flight
    expect(save).toHaveBeenCalledTimes(1);

    let flushed = false;
    const flushPromise = autosave.flush().then(() => {
      order.push("flush-resolved");
      flushed = true;
    });

    // flush must NOT resolve while the save is still running.
    await Promise.resolve();
    await Promise.resolve();
    expect(flushed).toBe(false);

    resolveSave();
    await flushPromise;
    expect(flushed).toBe(true);
    expect(order).toEqual(["saved", "flush-resolved"]);
  });

  it("does not start a new save while one is already in flight", async () => {
    let resolveSave: () => void = () => {};
    const save = vi.fn().mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          resolveSave = resolve;
        }),
    );
    const autosave = createAutosave(save, { delayMs: 800 });

    autosave.schedule();
    await vi.advanceTimersByTimeAsync(800);
    expect(save).toHaveBeenCalledTimes(1);

    // A change arrives mid-save; it must wait, not run concurrently.
    autosave.schedule();
    await vi.advanceTimersByTimeAsync(800);
    expect(save).toHaveBeenCalledTimes(1);

    // Once the in-flight save finishes, the queued change is flushed.
    resolveSave();
    await Promise.resolve();
    await vi.advanceTimersByTimeAsync(800);
    expect(save).toHaveBeenCalledTimes(2);
  });
});
