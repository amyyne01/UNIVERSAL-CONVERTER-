// Revealing a downloaded file, with the one outcome the UI used to swallow.
//
// A file recorded weeks ago may have been moved, renamed or deleted outside the
// app. `shell.showItemInFolder` on a path that no longer exists opens nothing and
// reports nothing, so the click just died silently and the user was left thinking
// the app was broken. Main now answers whether the file was actually there, and
// this is the single place that turns that answer into something the user reads —
// both call sites (the recent strip and the queue) route through it, so the
// message can never drift between them.
import { useAppStore } from '@/store';

/** Reveal a finished download in the file manager. Names the file when it's gone. */
export async function revealFile(filePath: string | undefined, title?: string): Promise<void> {
  const { showError } = useAppStore.getState();
  const name = title?.trim() ? `“${title.trim()}”` : 'That file';

  if (!filePath?.trim()) {
    showError(`${name} has no file on this PC — the download never finished.`);
    return;
  }

  try {
    const result = await window.electronAPI.shell.showItemInFolder(filePath);
    if (result?.ok) return;
    showError(
      result?.reason === 'no-path'
        ? `${name} has no file on this PC — the download never finished.`
        : `${name} isn’t where it was saved. It was moved, renamed or deleted outside the app.`,
    );
  } catch {
    // A rejected invoke means the path failed the boundary's confinement check —
    // the same thing to the user: it is not somewhere we can open.
    showError(`${name} couldn’t be opened. It may no longer be in your downloads folder.`);
  }
}
