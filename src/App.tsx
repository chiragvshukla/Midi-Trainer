import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { useAppStore } from "./state/appStore";
import { useSession } from "./state/sessionStore";
import { usePcKeyboard } from "./lib/usePcKeyboard";
import { useShortcuts } from "./lib/useShortcuts";
import { useAppearance } from "./lib/theme";
import { SoundfontGate } from "./components/SoundfontGate";
import { LibraryView } from "./components/LibraryView";
import { LyricsBar } from "./components/LyricsBar";
import { PracticeStatsBar } from "./components/PracticeStatsBar";
import { SettingsPanel } from "./components/SettingsPanel";
import { AppearancePanel } from "./components/AppearancePanel";
import { ShortcutsHelp } from "./components/ShortcutsHelp";
import { Sidebar, type Section } from "./components/Sidebar";
import { TrackMenu } from "./components/TrackMenu";
import { TransportTimeline } from "./components/TransportTimeline";
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
  const [appearanceOpen, setAppearanceOpen] = useState(false);
  const [helpOpen, setHelpOpen] = useState(false);
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
  const piece = useSession((state) => state.piece);
  const startOver = useSession((state) => state.startOver);
  const previousBar = useSession((state) => state.previousBar);
  const settings = useSession((state) => state.settings);
  const changeMusicVolume = useSession((state) => state.changeMusicVolume);
  const changePlayerVolume = useSession((state) => state.changePlayerVolume);
  const changeOutputVolume = useSession((state) => state.changeOutputVolume);
  const theme = useAppearance((state) => state.resolved);
  const toggleTheme = useAppearance((state) => state.toggleTheme);

  usePcKeyboard();
  useShortcuts();

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
        onOpenSettings={() => setAppearanceOpen(true)}
        version={appInfo?.version ?? null}
      />
      <div className="main">
        <header className="transport">
          <div className="transport-bar">
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
              title="Back to the start"
              disabled={!piece}
              onClick={() => void startOver()}
            >
              ⏮ Start
            </button>
            <button
              type="button"
              className="btn"
              title="Previous bar"
              disabled={!piece}
              onClick={() => void previousBar()}
            >
              ◀ Bar
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
            <div className="volumes">
              <label
                className="volume"
                title={`Music ${Math.round((settings?.music_volume ?? 1) * 100)}%`}
              >
                <span>Music</span>
                <input
                  type="range"
                  min={0}
                  max={1}
                  step={0.05}
                  value={settings?.music_volume ?? 1}
                  onChange={(event) =>
                    void changeMusicVolume(Number(event.target.value))
                  }
                />
              </label>
              <label
                className="volume"
                title={`My notes ${Math.round((settings?.player_volume ?? 1) * 100)}%`}
              >
                <span>Notes</span>
                <input
                  type="range"
                  min={0}
                  max={1}
                  step={0.05}
                  value={settings?.player_volume ?? 1}
                  onChange={(event) =>
                    void changePlayerVolume(Number(event.target.value))
                  }
                />
              </label>
              <label
                className="volume"
                title={`Output ${Math.round((settings?.output_volume ?? 1) * 100)}%`}
              >
                <span>Output</span>
                <input
                  type="range"
                  min={0}
                  max={1}
                  step={0.05}
                  value={settings?.output_volume ?? 1}
                  onChange={(event) =>
                    void changeOutputVolume(Number(event.target.value))
                  }
                />
              </label>
            </div>
            <button
              type="button"
              className="icon-button"
              aria-label="Toggle light or dark theme"
              title="Toggle light or dark theme"
              onClick={toggleTheme}
            >
              {theme === "dark" ? "☀" : "🌙"}
            </button>
            <button
              type="button"
              className="icon-button"
              aria-label="Keyboard shortcuts"
              aria-expanded={helpOpen}
              title="Keyboard shortcuts"
              onClick={() => setHelpOpen(true)}
            >
              ?
            </button>
            <button
              type="button"
              className="icon-button"
              aria-label="Player settings"
              aria-expanded={settingsOpen}
              onClick={() => setSettingsOpen(true)}
            >
              ⚙
            </button>
          </div>
          {section === "practice" && <TransportTimeline />}
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
      <AppearancePanel
        open={appearanceOpen}
        onClose={() => setAppearanceOpen(false)}
      />
      <ShortcutsHelp open={helpOpen} onClose={() => setHelpOpen(false)} />
    </div>
  );
}

export default App;
