import { useSession } from "../state/sessionStore";

/**
 * First-run screen: the baseline SoundFont is downloaded behind a visible progress bar, with a
 * cancel button while it runs and a retry after a failure. Playback stays silent until it passes.
 */
export function SoundfontGate() {
  const download = useSession((state) => state.download);
  const error = useSession((state) => state.soundfontError);
  const retry = useSession((state) => state.startup);
  const cancelDownload = useSession((state) => state.cancelDownload);

  const percent =
    download && download.total > 0
      ? Math.floor((download.received / download.total) * 100)
      : 0;

  return (
    <div className="gate">
      <h2>Preparing the piano sound</h2>
      {error ? (
        <>
          <p className="gate-error">
            The sound could not be set up. The app stays silent until it is
            fixed.
          </p>
          <p className="gate-error">{error}</p>
          <button type="button" onClick={() => void retry()}>
            Retry
          </button>
        </>
      ) : (
        <>
          <p>Downloading the sound font, one time only ({percent}%).</p>
          <progress
            value={download?.received ?? 0}
            max={download?.total ?? 1}
          />
          <button type="button" onClick={() => void cancelDownload()}>
            Cancel
          </button>
        </>
      )}
    </div>
  );
}
