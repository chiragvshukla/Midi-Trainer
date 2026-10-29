import { useState } from "react";
import { useSession } from "../state/sessionStore";

/** Remembers a "not now" so the optional offer does not nag. */
const DISMISSED_KEY = "miditrainer.soundfontDismissed";

function wasDismissed(): boolean {
  try {
    return localStorage.getItem(DISMISSED_KEY) === "1";
  } catch {
    return false;
  }
}

/**
 * The one-time offer of the full instrument bank. The app already plays with the bundled
 * baseline font, so this is optional — dismissing it never leaves the app silent. Hidden once
 * the full bank or a custom font is in use, or after "Not now".
 */
export function SoundfontPrompt() {
  const soundfont = useSession((state) => state.soundfont);
  const download = useSession((state) => state.download);
  const soundfontError = useSession((state) => state.soundfontError);
  const downloadFull = useSession((state) => state.downloadFullSoundfont);
  const cancelDownload = useSession((state) => state.cancelDownload);
  const [dismissed, setDismissed] = useState(wasDismissed);

  if (dismissed || soundfont?.kind !== "Bundled") {
    return null;
  }

  const percent =
    download && download.total > 0
      ? Math.floor((download.received / download.total) * 100)
      : 0;

  const notNow = () => {
    try {
      localStorage.setItem(DISMISSED_KEY, "1");
    } catch {
      // A machine that forbids storage just forgets the choice; the card hides for this run.
    }
    setDismissed(true);
  };

  return (
    <div className="soundfont-prompt">
      <p className="prompt-title">Better sound? (optional)</p>
      <p className="prompt-text">
        MidiTrainer plays with a basic piano sound out of the box. Download the
        full 142&nbsp;MB instrument set for richer, more realistic instruments.
      </p>
      {soundfontError ? <p className="gate-error">{soundfontError}</p> : null}
      {download ? (
        <>
          <progress value={download.received} max={download.total || 1} />
          <p className="prompt-text">{percent}%</p>
          <button
            type="button"
            className="btn"
            onClick={() => void cancelDownload()}
          >
            Cancel
          </button>
        </>
      ) : (
        <>
          <button
            type="button"
            className="btn primary"
            onClick={() => void downloadFull()}
          >
            Download
          </button>
          <button type="button" className="btn" onClick={notNow}>
            Not now
          </button>
        </>
      )}
    </div>
  );
}
