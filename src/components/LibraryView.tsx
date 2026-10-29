import { useState } from "react";
import type { LibraryEntry } from "../lib/api";
import { useSession } from "../state/sessionStore";

/**
 * The library browser: pick a folder, filter its MIDI files, and open one. Opening loads the
 * piece paused and hands control back to the app to show the piece views. Rendering only:
 * the folder listing and the chosen folder come from the session store.
 */
export function LibraryView({ onOpened }: { onOpened: () => void }) {
  const library = useSession((state) => state.library);
  const libraryFolder = useSession((state) => state.libraryFolder);
  const loadLibrary = useSession((state) => state.loadLibrary);
  const chooseLibraryFolder = useSession((state) => state.chooseLibraryFolder);
  const openPiece = useSession((state) => state.openPiece);
  const piecePath = useSession((state) => state.piecePath);
  const [filter, setFilter] = useState("");

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
          shown.map((entry) => {
            const path = fullPath(entry);
            return (
              <button
                key={entry.relative_path}
                type="button"
                className={
                  path === piecePath ? "library-entry active" : "library-entry"
                }
                onClick={() => open(path)}
              >
                <span className="library-name">{entry.name}</span>
                {entry.relative_path !== entry.name ? (
                  <span className="library-dir">{entry.relative_path}</span>
                ) : null}
              </button>
            );
          })
        )}
      </div>
    </div>
  );
}
