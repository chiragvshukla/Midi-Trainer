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
import { TrackMenu, trackName } from "./components/TrackMenu";
import { FallingNotes } from "./piano-roll/FallingNotes";
import { ScoreView } from "./score/ScoreView";
import type {
  DownloadProgress,
  Hand,
  Judgement,
  PlayerNote,
  PracticeStats,
  Skill,
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
  const piece = useSession((state) => state.piece);
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
  const changeSkill = useSession((state) => state.changeSkill);
  const changePart = useSession((state) => state.changePart);
  const changeHandSplit = useSession((state) => state.changeHandSplit);
  const changePlayAlong = useSession((state) => state.changePlayAlong);
  const changeTempo = useSession((state) => state.changeTempo);
  const changeTranspose = useSession((state) => state.changeTranspose);
  const changeMusicVolume = useSession((state) => state.changeMusicVolume);
  const changePlayerVolume = useSession((state) => state.changePlayerVolume);
  const changeOutputVolume = useSession((state) => state.changeOutputVolume);

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

  // Hand practice only makes sense when there is a single playing part to split.
  const playingTracks = (piece?.tracks ?? []).filter(
    (track) => track.channel !== 9,
  );
  const singleTrack = playingTracks.length === 1;
  const handPractice = settings?.hand_split ?? false;
  const handValue =
    singleTrack && handPractice ? (settings?.player_hand ?? "Both") : "Both";

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
          Mode
          <select
            value={settings?.skill ?? "Strict"}
            onChange={(event) => void changeSkill(event.target.value as Skill)}
          >
            <option value="Listen">Listen</option>
            <option value="Follow">Follow</option>
            <option value="Strict">Strict</option>
          </select>
        </label>
        <label>
          Your part
          <select
            value={
              settings?.player_track != null
                ? `track:${settings.player_track}`
                : `hand:${handValue}`
            }
            onChange={(event) => {
              const value = event.target.value;
              if (value.startsWith("track:")) {
                void changePart(
                  settings?.player_hand ?? "Both",
                  Number(value.slice(6)),
                );
              } else {
                void changePart(value.slice(5) as Hand, null);
              }
            }}
          >
            <option value="hand:Both">Both hands</option>
            {singleTrack && handPractice && (
              <>
                <option value="hand:Left">Left hand</option>
                <option value="hand:Right">Right hand</option>
              </>
            )}
            {playingTracks.map((track) => (
              <option key={track.index} value={`track:${track.index}`}>
                {trackName(track)}
              </option>
            ))}
          </select>
        </label>
        {singleTrack && (
          <label>
            <input
              type="checkbox"
              checked={handPractice}
              onChange={(event) => void changeHandSplit(event.target.checked)}
            />
            Practice by hand
          </label>
        )}
        {handValue !== "Both" && (settings?.skill ?? "Strict") === "Follow" && (
          <label>
            <input
              type="checkbox"
              checked={settings?.play_along ?? false}
              onChange={(event) => void changePlayAlong(event.target.checked)}
            />
            App plays the other hand
          </label>
        )}
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
        <label>
          Output
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
