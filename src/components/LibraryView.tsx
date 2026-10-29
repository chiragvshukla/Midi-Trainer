import { useState } from "react";
import type { LibraryEntry } from "../lib/api";
import { useSession } from "../state/sessionStore";
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

/** A shared empty list, so a missing settings object does not make a new array each read. */
const NO_RECENT: string[] = [];

/**
 * The library browser: recent files, then one collapsible section per chosen folder, with a
 * filter across all of them. Opening an entry loads the piece paused and hands control back to
 * the app to show the piece views. Rendering only: the listings come from the session store.
 */
export function LibraryView({ onOpened }: { onOpened: () => void }) {
  const libraries = useSession((state) => state.libraries);
  const libraryFolders = useSession((state) => state.libraryFolders);
  const recent = useSession(
    (state) => state.settings?.recent_files ?? NO_RECENT,
  );
  const loadLibrary = useSession((state) => state.loadLibrary);
  const addLibraryFolder = useSession((state) => state.addLibraryFolder);
  const removeLibraryFolder = useSession((state) => state.removeLibraryFolder);
  const openPiece = useSession((state) => state.openPiece);
  const piecePath = useSession((state) => state.piecePath);
  const [filter, setFilter] = useState("");
  const [recentOpen, setRecentOpen] = useState(true);
  const [closedFolders, setClosedFolders] = useState<Set<string>>(new Set());

  const needle = filter.trim().toLowerCase();

  const fullPath = (folder: string, entry: LibraryEntry): string =>
    `${folder.replace(/[\\/]+$/, "")}/${entry.relative_path}`;

  const open = (path: string): void => {
    void openPiece(path).then(onOpened);
  };

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

  const entry = (path: string, name: string, folder: string | null) => (
    <button
      key={path}
      type="button"
      className={path === piecePath ? "library-entry active" : "library-entry"}
      onClick={() => open(path)}
    >
      <NoteIcon />
      <span className="library-meta">
        <span className="library-name">{name}</span>
        {folder ? <span className="library-dir">{folder}</span> : null}
      </span>
    </button>
  );

  // While filtering, a folder shows only matching files and disappears when none match.
  const groups = libraries
    .map((group) => ({
      folder: group.folder,
      entries: needle
        ? group.entries.filter((item) =>
            item.relative_path.toLowerCase().includes(needle),
          )
        : group.entries,
    }))
    .filter((group) => !needle || group.entries.length > 0);

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
            (recent.length === 0 ? (
              <p className="library-empty">No recent files yet.</p>
            ) : (
              recent.map((path) => entry(path, baseName(path), null))
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
                      ),
                    )
                  ))}
              </section>
            );
          })
        )}
      </div>
    </div>
  );
}
