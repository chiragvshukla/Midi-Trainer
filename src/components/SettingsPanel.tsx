import { useSession } from "../state/sessionStore";
import { trackName } from "./TrackMenu";
import type { Hand, Skill } from "../lib/api";

/**
 * The settings drawer: the MIDI input, the practice mode and part, hand practice, tempo and
 * transpose, and the three volume sliders. Rendering only — every change goes through the
 * session store. Opens over the app; closes on the scrim or the close button.
 */
export function SettingsPanel({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const settings = useSession((state) => state.settings);
  const devices = useSession((state) => state.devices);
  const piece = useSession((state) => state.piece);
  const connectDevice = useSession((state) => state.connectDevice);
  const changeSkill = useSession((state) => state.changeSkill);
  const changePart = useSession((state) => state.changePart);
  const changeHandSplit = useSession((state) => state.changeHandSplit);
  const changePlayAlong = useSession((state) => state.changePlayAlong);
  const changeTempo = useSession((state) => state.changeTempo);
  const changeTranspose = useSession((state) => state.changeTranspose);
  const changeMusicVolume = useSession((state) => state.changeMusicVolume);
  const changePlayerVolume = useSession((state) => state.changePlayerVolume);
  const changeOutputVolume = useSession((state) => state.changeOutputVolume);

  if (!open) {
    return null;
  }

  // Hand practice only makes sense when there is a single playing part to split.
  const playingTracks = (piece?.tracks ?? []).filter(
    (track) => track.channel !== 9,
  );
  const singleTrack = playingTracks.length === 1;
  const handPractice = settings?.hand_split ?? false;
  const handValue =
    singleTrack && handPractice ? (settings?.player_hand ?? "Both") : "Both";

  return (
    <>
      <div className="drawer-scrim" onClick={onClose} />
      <aside className="drawer" role="dialog" aria-label="Settings">
        <header className="drawer-head">
          <h2>Settings</h2>
          <button
            type="button"
            className="icon-button"
            aria-label="Close settings"
            onClick={onClose}
          >
            ✕
          </button>
        </header>
        <div className="drawer-body">
          <section className="settings-group">
            <h3>Input</h3>
            <label className="field">
              <span>MIDI device</span>
              <select
                value={settings?.midi_device ?? ""}
                onChange={(event) =>
                  void connectDevice(event.target.value || null)
                }
              >
                <option value="">No MIDI input</option>
                {devices.map((name) => (
                  <option key={name} value={name}>
                    {name}
                  </option>
                ))}
              </select>
            </label>
          </section>

          <section className="settings-group">
            <h3>Practice</h3>
            <label className="field">
              <span>Mode</span>
              <select
                value={settings?.skill ?? "Strict"}
                onChange={(event) =>
                  void changeSkill(event.target.value as Skill)
                }
              >
                <option value="Listen">Listen</option>
                <option value="Follow">Follow</option>
                <option value="Strict">Strict</option>
              </select>
            </label>
            <label className="field">
              <span>Your part</span>
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
              <label className="check">
                <input
                  type="checkbox"
                  checked={handPractice}
                  onChange={(event) =>
                    void changeHandSplit(event.target.checked)
                  }
                />
                Practice by hand
              </label>
            )}
            {handValue !== "Both" &&
              (settings?.skill ?? "Strict") === "Follow" && (
                <label className="check">
                  <input
                    type="checkbox"
                    checked={settings?.play_along ?? false}
                    onChange={(event) =>
                      void changePlayAlong(event.target.checked)
                    }
                  />
                  App plays the other hand
                </label>
              )}
          </section>

          <section className="settings-group">
            <h3>Playback</h3>
            <label className="field">
              <span>Tempo · {settings?.tempo_percent ?? 100}%</span>
              <input
                type="range"
                min={25}
                max={200}
                step={5}
                value={settings?.tempo_percent ?? 100}
                onChange={(event) =>
                  void changeTempo(Number(event.target.value))
                }
              />
            </label>
            <label className="field">
              <span>Transpose · {settings?.transpose ?? 0}</span>
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
            </label>
          </section>

          <section className="settings-group">
            <h3>Volume</h3>
            <label className="field">
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
            <label className="field">
              <span>My notes</span>
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
            <label className="field">
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
          </section>
        </div>
      </aside>
    </>
  );
}
