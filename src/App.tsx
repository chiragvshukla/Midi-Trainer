import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { useAppStore } from "./state/appStore";
import { useSession } from "./state/sessionStore";
import { usePcKeyboard } from "./lib/usePcKeyboard";
import { SoundfontGate } from "./components/SoundfontGate";
import { LibraryView } from "./components/LibraryView";
import { LyricsBar } from "./components/LyricsBar";
import { PracticeStatsBar } from "./components/PracticeStatsBar";
import { SettingsPanel } from "./components/SettingsPanel";
import { Sidebar, type Section } from "./components/Sidebar";
import { TrackMenu } from "./components/TrackMenu";
import { FallingNotes } from "./piano-roll/FallingNotes";
import { ScoreView } from "./score/ScoreView";
import type {
  DownloadProgress,
  Judgement,
  PlayerNote,
  PracticeStats,
} from "./lib/api";
import "./App.css";

/** Which of the two piece displays is on within the Practice section. */
type PieceView = "keys" | "sheet";

function App() {
  const [section, setSection] = useState<Section>("library");
  const [view, setView] = useState<PieceView>("keys");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const appInfo = useAppStore((state) => state.appInfo);
  const loadAppInfo = useAppStore((state) => state.loadAppInfo);

  const soundfont = useSession((state) => state.soundfont);
  const soundfontError = useSession((state) => state.soundfontError);
  const startup = useSession((state) => state.startup);
  const loadDevices = useSession((state) => state.loadDevices);
  const updateDownload = useSession((state) => state.updateDownload);
  const recordJudgements = useSession((state) => state.recordJudgements);
  const recordStats = useSession((state) => state.recordStats);
  const pressPitch = useSession((state) => state.pressPitch);
  const openPiece = useSession((state) => state.openPiece);
  const resume = useSession((state) => state.resume);
  const stop = useSession((state) => state.stop);

  usePcKeyboard();

  useEffect(() => {
    void loadAppInfo();
    void loadDevices();
    // The saved settings must reach the engine after the SoundFont exists, or they are dropped.
    void startup();

    const unlisten = Promise.all([
      listen<DownloadProgress>("soundfont-progress", (event) =>
        updateDownload(event.payload),
      ),
      listen<Judgement[]>("note-judgements", (event) =>
        recordJudgements(event.payload),
      ),
      listen<PracticeStats>("practice-stats", (event) =>
        recordStats(event.payload),
      ),
      listen<PlayerNote[]>("player-notes", (event) => {
        for (const note of event.payload) {
          pressPitch(note.pitch, note.down);
        }
      }),
    ]);
    return () => {
      void unlisten.then((removers) => removers.forEach((remove) => remove()));
    };
  }, [
    loadAppInfo,
    loadDevices,
    startup,
    updateDownload,
    recordJudgements,
    pressPitch,
    recordStats,
  ]);

  const chooseFile = async () => {
    const path = await open({
      multiple: false,
      filters: [{ name: "MIDI", extensions: ["mid", "kar"] }],
    });
    if (typeof path === "string") {
      void openPiece(path);
    }
  };

  const ready =
    soundfont !== null && soundfont.installed && soundfontError === null;

  return (
    <div className="app">
      {!ready && <SoundfontGate />}
      <Sidebar
        section={section}
        onSelect={setSection}
        version={appInfo?.version ?? null}
      />
      <div className="main">
        <header className="transport">
          <button
            type="button"
            className="btn primary"
            onClick={() => void resume()}
          >
            ▶ Play
          </button>
          <button type="button" className="btn" onClick={() => void stop()}>
            ■ Stop
          </button>
          <button
            type="button"
            className="btn"
            onClick={() => void chooseFile()}
          >
            Open…
          </button>
          {section === "practice" && (
            <>
              <div className="view-toggle" role="tablist">
                <button
                  type="button"
                  role="tab"
                  aria-selected={view === "keys"}
                  className={view === "keys" ? "active" : ""}
                  onClick={() => setView("keys")}
                >
                  Keys
                </button>
                <button
                  type="button"
                  role="tab"
                  aria-selected={view === "sheet"}
                  className={view === "sheet" ? "active" : ""}
                  onClick={() => setView("sheet")}
                >
                  Sheet
                </button>
              </div>
              <TrackMenu />
            </>
          )}
          <span className="transport-spacer" />
          <button
            type="button"
            className="icon-button"
            aria-label="Settings"
            aria-expanded={settingsOpen}
            onClick={() => setSettingsOpen(true)}
          >
            ⚙
          </button>
        </header>
        <main className="content">
          <div className={section === "library" ? "pane" : "pane hidden"}>
            <LibraryView
              onOpened={() => {
                setSection("practice");
                setView("keys");
              }}
            />
          </div>
          <div
            className={
              section === "practice" && view === "keys" ? "pane" : "pane hidden"
            }
          >
            <FallingNotes />
          </div>
          <div
            className={
              section === "practice" && view === "sheet"
                ? "pane"
                : "pane hidden"
            }
          >
            <ScoreView />
          </div>
        </main>
        {section === "practice" && (
          <>
            <LyricsBar />
            <PracticeStatsBar />
          </>
        )}
      </div>
      <SettingsPanel
        open={settingsOpen}
        onClose={() => setSettingsOpen(false)}
      />
    </div>
  );
}

export default App;
