/**
 * Debounced autosave controller.
 *
 * Keeps save policy out of the UI component so it can be unit-tested in
 * isolation. The controller coalesces rapid edits into a single write,
 * never runs two saves concurrently, and exposes a flush() for moments
 * that must persist immediately (explicit save, switching notes, closing).
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
  /** Persist any pending change right now, bypassing the timer. */
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
  let pending = false; // a change is waiting to be saved
  let inFlight = false; // a save is currently running
  let queued = false; // a change arrived while a save was in flight

  function setStatus(status: SaveStatus) {
    onStatus?.(status);
  }

  function clearTimer() {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
  }

  async function runSave(): Promise<void> {
    clearTimer();

    if (inFlight) {
      // Don't run two saves at once; remember to save again afterwards.
      queued = true;
      return;
    }
    if (!pending) return;

    pending = false;
    inFlight = true;
    setStatus("saving");
    try {
      await save();
      setStatus("saved");
    } catch {
      // Keep the change dirty so a later edit or flush can retry it.
      pending = true;
      setStatus("error");
    } finally {
      inFlight = false;
      if (queued) {
        queued = false;
        schedule();
      }
    }
  }

  function schedule(): void {
    pending = true;
    setStatus("pending");
    clearTimer();
    timer = setTimeout(() => {
      void runSave();
    }, delayMs);
  }

  async function flush(): Promise<void> {
    clearTimer();
    if (!pending && !queued) return;
    await runSave();
  }

  function cancel(): void {
    clearTimer();
    pending = false;
    queued = false;
    setStatus("idle");
  }

  return { schedule, flush, cancel };
}
