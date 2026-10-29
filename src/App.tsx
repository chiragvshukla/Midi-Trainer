import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { useAppStore } from "./state/appStore";
import { useSession } from "./state/sessionStore";
import { usePcKeyboard } from "./lib/usePcKeyboard";
import { SoundfontGate } from "./components/SoundfontGate";
import { LibraryView } from "./components/LibraryView";
import { LyricsBar } from "./components/LyricsBar";
import { MainMenu, type Section } from "./components/MainMenu";
import { PracticeStatsBar } from "./components/PracticeStatsBar";
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

/** Which of the two piece displays is on within the Synth section. */
type PieceView = "keys" | "sheet";

function App() {
  const [section, setSection] = useState<Section>("library");
  const [view, setView] = useState<PieceView>("keys");
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
  const recordStats = useSession((state) => state.recordStats);
  const pressPitch = useSession((state) => state.pressPitch);
  const openPiece = useSession((state) => state.openPiece);
  const resume = useSession((state) => state.resume);
  const stop = useSession((state) => state.stop);
  const toggleFollow = useSession((state) => state.toggleFollow);
  const changeHand = useSession((state) => state.changeHand);
  const changeTempo = useSession((state) => state.changeTempo);
  const changeTranspose = useSession((state) => state.changeTranspose);
  const changeMusicVolume = useSession((state) => state.changeMusicVolume);
  const changePlayerVolume = useSession((state) => state.changePlayerVolume);

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
      <header className="toolbar">
        <MainMenu section={section} onSelect={setSection} />
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
        {section === "synth" && (
          <>
            <div className="view-toggle">
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
            <TrackMenu />
          </>
        )}
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
        <div className="view-toggle" role="group" aria-label="Practice hand">
          {(["Both", "Left", "Right"] as const).map((hand) => (
            <button
              key={hand}
              type="button"
              className={
                (settings?.player_hand ?? "Both") === hand ? "active" : ""
              }
              onClick={() => void changeHand(hand)}
            >
              {hand}
            </button>
          ))}
        </div>
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
          Music
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
        <label>
          My notes
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
      </header>
      <main className="workspace">
        <div className={section === "library" ? "" : "hidden"}>
          <LibraryView
            onOpened={() => {
              setSection("synth");
              setView("keys");
            }}
          />
        </div>
        <div className={section === "synth" && view === "keys" ? "" : "hidden"}>
          <FallingNotes />
        </div>
        <div
          className={section === "synth" && view === "sheet" ? "" : "hidden"}
        >
          <ScoreView />
        </div>
      </main>
      {section === "synth" && <LyricsBar />}
      <PracticeStatsBar />
    </div>
  );
}

export default App;
