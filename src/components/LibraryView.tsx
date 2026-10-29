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

/** A shared empty list, so a missing settings object does not make a new array each read. */
const NO_RECENT: string[] = [];

/**
 * The library browser: recent files, then the chosen folder's MIDI files, with a filter.
 * Opening an entry loads the piece paused and hands control back to the app to show the
 * piece views. Rendering only: the listings come from the session store.
 */
export function LibraryView({ onOpened }: { onOpened: () => void }) {
  const library = useSession((state) => state.library);
  const libraryFolder = useSession((state) => state.libraryFolder);
  const recent = useSession(
    (state) => state.settings?.recent_files ?? NO_RECENT,
  );
  const loadLibrary = useSession((state) => state.loadLibrary);
  const chooseLibraryFolder = useSession((state) => state.chooseLibraryFolder);
  const openPiece = useSession((state) => state.openPiece);
  const piecePath = useSession((state) => state.piecePath);
  const [filter, setFilter] = useState("");
  const [recentOpen, setRecentOpen] = useState(true);

  const needle = filter.trim().toLowerCase();
  const shown = needle
    ? library.filter((entry) =>
        entry.relative_path.toLowerCase().includes(needle),
      )
    : library;

  const fullPath = (entry: LibraryEntry): string =>
    `${(libraryFolder ?? "").replace(/[\\/]+$/, "")}/${entry.relative_path}`;

  const open = (path: string): void => {
    void openPiece(path).then(onOpened);
  };

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

  return (
    <div className="library">
      <div className="library-tools">
        <button type="button" onClick={() => void chooseLibraryFolder()}>
          {libraryFolder ? "Change folder…" : "Choose folder…"}
        </button>
        <button
          type="button"
          disabled={!libraryFolder}
          onClick={() => void loadLibrary()}
        >
          Refresh
        </button>
        <input
          type="search"
          placeholder="Filter…"
          value={filter}
          disabled={!libraryFolder}
          onChange={(event) => setFilter(event.target.value)}
        />
        <span className="library-folder">
          {libraryFolder ?? "No folder chosen"}
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
        <section className="library-folder-list">
          {!libraryFolder ? (
            <p className="library-empty">
              Choose a folder to build your library.
            </p>
          ) : shown.length === 0 ? (
            <p className="library-empty">
              {library.length === 0
                ? "No MIDI files found in this folder."
                : "No files match the filter."}
            </p>
          ) : (
            shown.map((item) =>
              entry(
                fullPath(item),
                displayName(item.name),
                item.relative_path === item.name ? null : item.relative_path,
              ),
            )
          )}
        </section>
      </div>
    </div>
  );
}
