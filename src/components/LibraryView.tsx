import { useEffect, useMemo, useState } from "react";
import type { LibraryEntry } from "../lib/api";
import { log } from "../lib/logger";
import { normalizePath, useSession } from "../state/sessionStore";
import { NoteIcon } from "./NoteIcon";

/** A file's name without a MIDI extension, for display. */
function displayName(name: string): string {
  return name.replace(/\.(mid|midi|kar)$/i, "");
}

/** A full path's file name without a MIDI extension, for display. */
function baseName(path: string): string {
  return displayName(path.replace(/^.*[\\/]/, ""));
}

/** The last segment of a folder path, for a section header. */
function folderName(folder: string): string {
  return folder.replace(/[\\/]+$/, "").replace(/^.*[\\/]/, "") || folder;
}

/** A file size a person reads: "820 KB". Empty for a size that could not be read. */
function fileSize(bytes: number): string {
  if (bytes <= 0) {
    return "";
  }
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value < 10 && unit > 0 ? value.toFixed(1) : Math.round(value)} ${units[unit]}`;
}

/** How long ago a file was last changed, in a short phrase. Empty for an unknown time. */
function modifiedLabel(ms: number): string {
  if (ms <= 0) {
    return "";
  }
  const days = Math.floor((Date.now() - ms) / 86_400_000);
  if (days <= 0) {
    return "today";
  }
  if (days === 1) {
    return "yesterday";
  }
  if (days < 30) {
    return `${days} days ago`;
  }
  if (days < 365) {
    return `${Math.floor(days / 30)} mo ago`;
  }
  return `${Math.floor(days / 365)} yr ago`;
}

/** The size and modified date of a library file, as one line. */
function entryDetail(item: LibraryEntry): string {
  return [fileSize(item.size_bytes), modifiedLabel(item.modified_ms)]
    .filter(Boolean)
    .join(" · ");
}

/** A shared empty list, so a missing settings object does not make a new array each read. */
const NO_RECENT: string[] = [];
const NO_HIDDEN: string[] = [];

/**
 * The library browser: recent files, then one collapsible section per chosen folder, with a
 * filter across all of them. Opening an entry loads the piece paused and hands control back to
 * the app to show the piece views. Removing an entry hides it from the library and can delete
 * the file itself. Rendering only: the listings come from the session store.
 */
export function LibraryView({ onOpened }: { onOpened: () => void }) {
  const libraries = useSession((state) => state.libraries);
  const libraryFolders = useSession((state) => state.libraryFolders);
  const recent = useSession(
    (state) => state.settings?.recent_files ?? NO_RECENT,
  );
  const hiddenFiles = useSession(
    (state) => state.settings?.hidden_files ?? NO_HIDDEN,
  );
  const storageChoice = useSession(
    (state) => state.settings?.delete_from_storage ?? null,
  );
  const loadLibrary = useSession((state) => state.loadLibrary);
  const addLibraryFolder = useSession((state) => state.addLibraryFolder);
  const removeLibraryFolder = useSession((state) => state.removeLibraryFolder);
  const removeFromLibrary = useSession((state) => state.removeFromLibrary);
  const openPiece = useSession((state) => state.openPiece);
  const piecePath = useSession((state) => state.piecePath);
  const [filter, setFilter] = useState("");
  const [recentOpen, setRecentOpen] = useState(true);
  const [closedFolders, setClosedFolders] = useState<Set<string>>(new Set());
  const [pending, setPending] = useState<{ path: string; name: string } | null>(
    null,
  );
  const [remember, setRemember] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);

  const hidden = useMemo(
    () => new Set(hiddenFiles.map(normalizePath)),
    [hiddenFiles],
  );
  const needle = filter.trim().toLowerCase();

  const fullPath = (folder: string, entry: LibraryEntry): string =>
    `${folder.replace(/[\\/]+$/, "")}/${entry.relative_path}`;

  const open = (path: string): void => {
    void openPiece(path).then(onOpened);
  };

  const applyDelete = async (
    path: string,
    deleteStorage: boolean,
    rememberChoice: boolean,
  ): Promise<void> => {
    setDeleteError(null);
    setPending(null);
    try {
      await removeFromLibrary(path, deleteStorage, rememberChoice);
    } catch (error) {
      log.error(`deleting ${path} failed: ${String(error)}`);
      setDeleteError(String(error));
    }
  };

  // A remembered choice means no prompt: apply it straight away.
  const requestDelete = (path: string, name: string): void => {
    if (storageChoice === null) {
      setRemember(false);
      setPending({ path, name });
    } else {
      void applyDelete(path, storageChoice, false);
    }
  };

  useEffect(() => {
    if (!pending) {
      return;
    }
    const onKey = (event: KeyboardEvent): void => {
      if (event.key === "Escape") {
        setPending(null);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [pending]);

  const toggleFolder = (folder: string): void =>
    setClosedFolders((current) => {
      const next = new Set(current);
      if (next.has(folder)) {
        next.delete(folder);
      } else {
        next.add(folder);
      }
      return next;
    });

  const entry = (
    path: string,
    name: string,
    folder: string | null,
    detail?: string,
  ) => (
    <div
      key={path}
      className={path === piecePath ? "library-entry active" : "library-entry"}
    >
      <button type="button" className="library-open" onClick={() => open(path)}>
        <NoteIcon />
        <span className="library-meta">
          <span className="library-name">{name}</span>
          {folder ? <span className="library-dir">{folder}</span> : null}
          {detail ? <span className="library-detail">{detail}</span> : null}
        </span>
      </button>
      <button
        type="button"
        className="library-remove"
        aria-label={`Delete ${name} from the library`}
        title="Delete from library"
        onClick={() => requestDelete(path, name)}
      >
        🗑
      </button>
    </div>
  );

  // While filtering, a folder shows only matching files and disappears when none match. A file
  // removed from the library is never listed again.
  const groups = libraries
    .map((group) => ({
      folder: group.folder,
      entries: group.entries.filter((item) => {
        if (hidden.has(normalizePath(fullPath(group.folder, item)))) {
          return false;
        }
        return !needle || item.relative_path.toLowerCase().includes(needle);
      }),
    }))
    .filter((group) => !needle || group.entries.length > 0);

  const recentShown = recent.filter((path) => !hidden.has(normalizePath(path)));

  return (
    <div className="library">
      <div className="library-tools">
        <button type="button" onClick={() => void addLibraryFolder()}>
          Add folder…
        </button>
        <button
          type="button"
          disabled={libraryFolders.length === 0}
          onClick={() => void loadLibrary()}
        >
          Refresh
        </button>
        <input
          type="search"
          placeholder="Filter…"
          value={filter}
          disabled={libraryFolders.length === 0}
          onChange={(event) => setFilter(event.target.value)}
        />
        <span className="library-folder">
          {libraryFolders.length === 0
            ? "No folders chosen"
            : `${libraryFolders.length} folder${libraryFolders.length === 1 ? "" : "s"}`}
        </span>
      </div>
      {deleteError ? <p className="gate-error">{deleteError}</p> : null}
      <div className="library-list">
        <section className="library-recent">
          <button
            type="button"
            className="library-section"
            aria-expanded={recentOpen}
            onClick={() => setRecentOpen((open) => !open)}
          >
            <span className="library-caret">{recentOpen ? "▾" : "▸"}</span>
            Recent
          </button>
          {recentOpen &&
            (recentShown.length === 0 ? (
              <p className="library-empty">No recent files yet.</p>
            ) : (
              recentShown.map((path) => entry(path, baseName(path), null))
            ))}
        </section>
        {libraryFolders.length === 0 ? (
          <p className="library-empty">Add a folder to build your library.</p>
        ) : (
          groups.map((group) => {
            const isOpen = !closedFolders.has(group.folder);
            return (
              <section key={group.folder} className="library-folder-list">
                <div className="library-folder-head">
                  <button
                    type="button"
                    className="library-section"
                    aria-expanded={isOpen}
                    onClick={() => toggleFolder(group.folder)}
                  >
                    <span className="library-caret">{isOpen ? "▾" : "▸"}</span>
                    <span className="library-folder-name" title={group.folder}>
                      {folderName(group.folder)}
                    </span>
                  </button>
                  <button
                    type="button"
                    className="library-remove"
                    aria-label={`Remove ${group.folder}`}
                    title="Remove folder"
                    onClick={() => void removeLibraryFolder(group.folder)}
                  >
                    ×
                  </button>
                </div>
                {isOpen &&
                  (group.entries.length === 0 ? (
                    <p className="library-empty">
                      {needle
                        ? "No files match the filter."
                        : "No MIDI files found in this folder."}
                    </p>
                  ) : (
                    group.entries.map((item) =>
                      entry(
                        fullPath(group.folder, item),
                        displayName(item.name),
                        item.relative_path === item.name
                          ? null
                          : item.relative_path,
                        entryDetail(item),
                      ),
                    )
                  ))}
              </section>
            );
          })
        )}
      </div>

      {pending ? (
        <>
          <div className="drawer-scrim" onClick={() => setPending(null)} />
          <div
            className="modal delete-dialog"
            role="dialog"
            aria-modal="true"
            aria-label={`Delete ${pending.name} from the library`}
          >
            <header className="drawer-head">
              <h2>Delete “{pending.name}”?</h2>
              <button
                type="button"
                className="icon-button"
                aria-label="Cancel"
                onClick={() => setPending(null)}
              >
                ✕
              </button>
            </header>
            <div className="drawer-body">
              <p className="field-note">
                It is removed from your library. The file on disk is only
                deleted if you choose so.
              </p>
              <p className="field-path">{pending.path}</p>
              <label className="check">
                <input
                  type="checkbox"
                  checked={remember}
                  onChange={(event) => setRemember(event.target.checked)}
                />
                Never ask again
              </label>
              <div className="delete-actions">
                <button
                  type="button"
                  className="btn"
                  onClick={() => setPending(null)}
                >
                  Cancel
                </button>
                <button
                  type="button"
                  className="btn"
                  onClick={() =>
                    void applyDelete(pending.path, false, remember)
                  }
                >
                  Keep file
                </button>
                <button
                  type="button"
                  className="btn danger"
                  onClick={() => void applyDelete(pending.path, true, remember)}
                >
                  Delete file
                </button>
              </div>
            </div>
          </div>
        </>
      ) : null}
    </div>
  );
}
