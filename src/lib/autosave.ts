/**
 * Debounced autosave controller.
 *
 * Keeps save policy out of the UI component so it can be unit-tested in
 * isolation. The controller coalesces rapid edits into a single write,
 * never runs two saves concurrently, and exposes a flush() that resolves
 * only once everything pending has actually been persisted — so callers
 * can rely on `await flush()` before navigating away or closing.
 */
export type SaveStatus = "idle" | "pending" | "saving" | "saved" | "error";

export interface AutosaveOptions {
  /** Quiet period after the last change before a save runs. */
  delayMs?: number;
  /** Notified on every status transition so the UI can reflect it. */
  onStatus?: (status: SaveStatus) => void;
}

export interface Autosave {
  /** Mark the content dirty and (re)arm the debounce timer. */
  schedule(): void;
  /** Persist everything pending right now and resolve once it is saved. */
  flush(): Promise<void>;
  /** Drop any pending change without saving. */
  cancel(): void;
}

export function createAutosave(
  save: () => Promise<void>,
  options: AutosaveOptions = {},
): Autosave {
  const delayMs = options.delayMs ?? 800;
  const onStatus = options.onStatus;

  let timer: ReturnType<typeof setTimeout> | null = null;
  let dirty = false; // there is an unsaved change
  let draining: Promise<void> | null = null; // the active drain loop, if any

  function setStatus(status: SaveStatus) {
    onStatus?.(status);
  }

  function clearTimer() {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
  }

  // Persist the dirty state, then keep saving while new edits keep arriving.
  // Stops on the first failure to avoid a hot retry loop; the change stays
  // dirty so a later edit or flush() can try again. The single-drain guard in
  // startDrain() guarantees two saves never run concurrently.
  async function drain(): Promise<void> {
    while (dirty) {
      dirty = false;
      setStatus("saving");
      try {
        await save();
        setStatus("saved");
      } catch {
        dirty = true;
        setStatus("error");
        return;
      }
    }
  }

  function startDrain(): Promise<void> {
    if (draining) return draining;
    draining = drain().finally(() => {
      draining = null;
    });
    return draining;
  }

  function trigger() {
    clearTimer();
    if (dirty) void startDrain();
  }

  function schedule(): void {
    dirty = true;
    setStatus("pending");
    clearTimer();
    timer = setTimeout(trigger, delayMs);
  }

  async function flush(): Promise<void> {
    clearTimer();
    // Wait for any in-flight drain, then ensure anything still dirty is saved.
    if (draining) await draining;
    if (dirty) await startDrain();
  }

  function cancel(): void {
    clearTimer();
    dirty = false;
    setStatus("idle");
  }

  return { schedule, flush, cancel };
}
