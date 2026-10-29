import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { useAppStore } from "./state/appStore";
import { useSession } from "./state/sessionStore";
import { usePcKeyboard } from "./lib/usePcKeyboard";
import { SoundfontGate } from "./components/SoundfontGate";
import { FallingNotes } from "./piano-roll/FallingNotes";
import type { DownloadProgress, Judgement, PlayerNote } from "./lib/api";
import "./App.css";

function App() {
  const appInfo = useAppStore((state) => state.appInfo);
  const loadAppInfo = useAppStore((state) => state.loadAppInfo);

  const soundfont = useSession((state) => state.soundfont);
  const soundfontError = useSession((state) => state.soundfontError);
  const settings = useSession((state) => state.settings);
  const follow = useSession((state) => state.follow);
  const initSoundfont = useSession((state) => state.initSoundfont);
  const loadSettings = useSession((state) => state.loadSettings);
  const updateDownload = useSession((state) => state.updateDownload);
  const recordJudgements = useSession((state) => state.recordJudgements);
  const pressPitch = useSession((state) => state.pressPitch);
  const openPiece = useSession((state) => state.openPiece);
  const stop = useSession((state) => state.stop);
  const toggleFollow = useSession((state) => state.toggleFollow);
  const changeTempo = useSession((state) => state.changeTempo);
  const changeTranspose = useSession((state) => state.changeTranspose);
  const changeVolume = useSession((state) => state.changeVolume);

  usePcKeyboard();

  useEffect(() => {
    void loadAppInfo();
    void loadSettings();
    void initSoundfont();

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
    loadSettings,
    initSoundfont,
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
        <button type="button" onClick={() => void stop()}>
          Stop
        </button>
        {settings && settings.recent_files.length > 0 && (
          <select
            defaultValue=""
            onChange={(event) => {
              if (event.target.value) void openPiece(event.target.value);
            }}
          >
            <option value="">Recent…</option>
            {settings.recent_files.map((path) => (
              <option key={path} value={path}>
                {path}
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
        <FallingNotes />
      </main>
    </div>
  );
}

export default App;
