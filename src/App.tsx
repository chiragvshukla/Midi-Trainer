import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { useAppStore } from "./state/appStore";
import { useSession } from "./state/sessionStore";
import { usePcKeyboard } from "./lib/usePcKeyboard";
import { SoundfontGate } from "./components/SoundfontGate";
import { LibraryView } from "./components/LibraryView";
import { FallingNotes } from "./piano-roll/FallingNotes";
import { ScoreView } from "./score/ScoreView";
import type { DownloadProgress, Judgement, PlayerNote } from "./lib/api";
import "./App.css";

/** Which of the three screens is on. */
type View = "library" | "keys" | "sheet";

function App() {
  const [view, setView] = useState<View>("library");
  const appInfo = useAppStore((state) => state.appInfo);
  const loadAppInfo = useAppStore((state) => state.loadAppInfo);

  const soundfont = useSession((state) => state.soundfont);
  const soundfontError = useSession((state) => state.soundfontError);
  const settings = useSession((state) => state.settings);
  const devices = useSession((state) => state.devices);
  const follow = useSession((state) => state.follow);
  const startup = useSession((state) => state.startup);
  const loadDevices = useSession((state) => state.loadDevices);
  const connectDevice = useSession((state) => state.connectDevice);
  const updateDownload = useSession((state) => state.updateDownload);
  const recordJudgements = useSession((state) => state.recordJudgements);
  const pressPitch = useSession((state) => state.pressPitch);
  const openPiece = useSession((state) => state.openPiece);
  const resume = useSession((state) => state.resume);
  const stop = useSession((state) => state.stop);
  const toggleFollow = useSession((state) => state.toggleFollow);
  const changeTempo = useSession((state) => state.changeTempo);
  const changeTranspose = useSession((state) => state.changeTranspose);
  const changeVolume = useSession((state) => state.changeVolume);

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
      <header className="toolbar">
        <span className="app-title">MidiTrainer</span>
        {appInfo && <span className="version-badge">v{appInfo.version}</span>}
        <button type="button" onClick={() => void chooseFile()}>
          Open…
        </button>
        <button type="button" onClick={() => void resume()}>
          Play
        </button>
        <button type="button" onClick={() => void stop()}>
          Stop
        </button>
        <div className="view-toggle">
          <button
            type="button"
            className={view === "library" ? "active" : ""}
            onClick={() => setView("library")}
          >
            Library
          </button>
          <button
            type="button"
            className={view === "keys" ? "active" : ""}
            onClick={() => setView("keys")}
          >
            Keys
          </button>
          <button
            type="button"
            className={view === "sheet" ? "active" : ""}
            onClick={() => setView("sheet")}
          >
            Sheet
          </button>
        </div>
        {devices.length > 0 && (
          <select
            value={settings?.midi_device ?? ""}
            onChange={(event) => void connectDevice(event.target.value || null)}
          >
            <option value="">No MIDI input</option>
            {devices.map((name) => (
              <option key={name} value={name}>
                {name}
              </option>
            ))}
          </select>
        )}
        <label>
          <input
            type="checkbox"
            checked={follow}
            onChange={(event) => void toggleFollow(event.target.checked)}
          />
          Follow me
        </label>
        <label>
          Tempo
          <input
            type="range"
            min={25}
            max={200}
            step={5}
            value={settings?.tempo_percent ?? 100}
            onChange={(event) => void changeTempo(Number(event.target.value))}
          />
          {settings?.tempo_percent ?? 100}%
        </label>
        <label>
          Transpose
          <input
            type="range"
            min={-12}
            max={12}
            step={1}
            value={settings?.transpose ?? 0}
            onChange={(event) =>
              void changeTranspose(Number(event.target.value))
            }
          />
          {settings?.transpose ?? 0}
        </label>
        <label>
          Volume
          <input
            type="range"
            min={0}
            max={1}
            step={0.05}
            value={settings?.volume ?? 1}
            onChange={(event) => void changeVolume(Number(event.target.value))}
          />
        </label>
      </header>
      <main className="workspace">
        <div className={view === "library" ? "" : "hidden"}>
          <LibraryView onOpened={() => setView("keys")} />
        </div>
        <div className={view === "keys" ? "" : "hidden"}>
          <FallingNotes />
        </div>
        <div className={view === "sheet" ? "" : "hidden"}>
          <ScoreView />
        </div>
      </main>
    </div>
  );
}

export default App;
