import { useCallback, useEffect, useRef, useState } from "react";
import {
  downloadMutopia,
  fetchMutopiaPage,
  type MutopiaPiece,
} from "../lib/api";
import { useSession } from "../state/sessionStore";
import { log } from "../lib/logger";

/** Rows per page; matches the Mutopia listing's own page size. */
const PAGE_SIZE = 10;

/** How a single piece's download is going. */
type RowState = "busy" | "saved" | "error";

/**
 * The Download tab: browse the Mutopia Project's public-domain and Creative Commons piano
 * catalogue and save any piece's MIDI into a library folder. The listing and the downloads
 * come from the engine; this component only renders them and reports progress. The catalogue
 * is fetched when the tab is first opened, never at app start.
 */
export function DownloadPanel({
  active,
  onOpened,
}: {
  active: boolean;
  onOpened: () => void;
}) {
  const libraryFolders = useSession((state) => state.libraryFolders);
  const loadLibrary = useSession((state) => state.loadLibrary);
  const addLibraryFolder = useSession((state) => state.addLibraryFolder);
  const openPiece = useSession((state) => state.openPiece);

  const [folder, setFolder] = useState("");
  const [term, setTerm] = useState("");
  // The search the shown page was fetched with, so "Load more" stays consistent.
  const [applied, setApplied] = useState("");

  const [pieces, setPieces] = useState<MutopiaPiece[]>([]);
  const [listed, setListed] = useState(false);
  const [hasMore, setHasMore] = useState(false);
  const [loading, setLoading] = useState(false);
  const [listError, setListError] = useState<string | null>(null);

  const [rows, setRows] = useState<Record<number, RowState>>({});
  const [saved, setSaved] = useState<Record<number, string>>({});
  const [downloadError, setDownloadError] = useState<string | null>(null);

  // The row offset of the next page. A ref so a rapid second click cannot reuse a stale value.
  const nextStart = useRef(0);

  // The chosen folder can disappear (removed in the Library); fall back to the first one.
  const destination = libraryFolders.includes(folder)
    ? folder
    : (libraryFolders[0] ?? "");

  const load = useCallback(
    async (startAt: number, query: string, append: boolean) => {
      setLoading(true);
      setListError(null);
      try {
        const page = await fetchMutopiaPage(startAt, query);
        setPieces((current) =>
          append ? [...current, ...page.pieces] : page.pieces,
        );
        setHasMore(page.has_more);
        setApplied(query);
        nextStart.current = startAt + PAGE_SIZE;
        setListed(true);
      } catch (error) {
        log.error(`loading the Mutopia catalogue failed: ${String(error)}`);
        setListError(String(error));
      } finally {
        setLoading(false);
      }
    },
    [],
  );

  // The first page loads when the tab is first opened; nothing is fetched at app start.
  useEffect(() => {
    if (active && !listed && !loading) {
      void load(0, "", false);
    }
  }, [active, listed, loading, load]);

  const search = () => {
    nextStart.current = 0;
    void load(0, term, false);
  };

  const loadMore = () => void load(nextStart.current, applied, true);

  const run = async (piece: MutopiaPiece) => {
    if (!piece.midi_url || destination === "") {
      return;
    }
    setDownloadError(null);
    setRows((current) => ({ ...current, [piece.id]: "busy" }));
    try {
      const path = await downloadMutopia(piece.midi_url, destination);
      setSaved((current) => ({ ...current, [piece.id]: path }));
      setRows((current) => ({ ...current, [piece.id]: "saved" }));
      await loadLibrary();
    } catch (error) {
      log.error(`downloading "${piece.title}" failed: ${String(error)}`);
      setRows((current) => ({ ...current, [piece.id]: "error" }));
      setDownloadError(String(error));
    }
  };

  const busy = loading || Object.values(rows).includes("busy");
  const savedCount = Object.values(rows).filter(
    (state) => state === "saved",
  ).length;

  return (
    <div className="library">
      <div className="library-tools">
        <input
          type="search"
          className="download-search"
          placeholder="Search the catalogue…"
          value={term}
          onChange={(event) => setTerm(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              search();
            }
          }}
        />
        <button type="button" disabled={loading} onClick={search}>
          Search
        </button>
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
      </div>

      <p className="field-note mutopia-intro">
        Public-domain and Creative Commons piano music from the{" "}
        <a
          href="https://www.mutopiaproject.org"
          target="_blank"
          rel="noreferrer"
        >
          Mutopia Project
        </a>
        . Downloading saves the MIDI into a library folder.
      </p>

      <div className="mutopia-options">
        <label className="mutopia-dest">
          <span>Save into</span>
          <select
            value={destination}
            disabled={libraryFolders.length === 0}
            onChange={(event) => setFolder(event.target.value)}
          >
            {libraryFolders.length === 0 ? (
              <option value="">Add a library folder first</option>
            ) : (
              libraryFolders.map((path) => (
                <option key={path} value={path}>
                  {path}
                </option>
              ))
            )}
          </select>
        </label>
        <span className="mutopia-count">
          {pieces.length > 0 ? `${pieces.length} pieces` : ""}
          {savedCount > 0 ? ` · ${savedCount} saved` : ""}
        </span>
      </div>

      {busy ? (
        <div className="mutopia-bar" role="status" aria-label="Working">
          <span />
        </div>
      ) : null}

      {listError ? <p className="gate-error">{listError}</p> : null}
      {downloadError ? <p className="gate-error">{downloadError}</p> : null}

      <div className="library-list" aria-busy={busy}>
        {pieces.length === 0 && !loading && !listError ? (
          <p className="library-empty">
            {listed
              ? "No pieces matched that search."
              : "Loading the catalogue…"}
          </p>
        ) : null}

        {pieces.map((piece) => {
          const state = rows[piece.id];
          const path = saved[piece.id];
          return (
            <div className="mutopia-row" key={piece.id}>
              <div className="mutopia-info">
                <span className="library-name">{piece.title}</span>
                <span className="library-detail">
                  {[piece.composer, piece.style, piece.instrumentation]
                    .filter(Boolean)
                    .join(" · ")}
                </span>
                {piece.license ? (
                  <a
                    className="mutopia-license"
                    href={piece.license_url}
                    target="_blank"
                    rel="noreferrer"
                  >
                    {piece.license}
                  </a>
                ) : null}
              </div>
              <div className="mutopia-actions">
                {piece.midi_url ? (
                  <>
                    <button
                      type="button"
                      className="btn"
                      disabled={destination === "" || state === "busy"}
                      aria-busy={state === "busy"}
                      onClick={() => void run(piece)}
                    >
                      {state === "busy"
                        ? "Downloading…"
                        : state === "saved"
                          ? "Saved ✓"
                          : "Download"}
                    </button>
                    {state === "saved" && path ? (
                      <button
                        type="button"
                        className="btn"
                        onClick={() => void openPiece(path).then(onOpened)}
                      >
                        Play
                      </button>
                    ) : null}
                  </>
                ) : (
                  <span
                    className="mutopia-note"
                    title="Published as a ZIP of several MIDI files."
                  >
                    multi-file (zip)
                  </span>
                )}
              </div>
            </div>
          );
        })}

        {hasMore ? (
          <div className="mutopia-more">
            <button
              type="button"
              className="btn"
              disabled={loading}
              onClick={loadMore}
            >
              {loading ? "Loading…" : "Load more"}
            </button>
          </div>
        ) : null}
      </div>
    </div>
  );
}
