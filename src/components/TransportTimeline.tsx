import {
  useEffect,
  useRef,
  type PointerEvent as ReactPointerEvent,
} from "react";
import {
  pausePositionPoll,
  resumePositionPoll,
} from "../lib/usePlaybackPosition";
import { useSession } from "../state/sessionStore";

/** Shows a millisecond position as minutes:seconds. */
function clock(ms: number): string {
  const total = Math.max(0, Math.round(ms / 1000));
  const minutes = Math.floor(total / 60);
  const seconds = total % 60;
  return `${minutes}:${seconds.toString().padStart(2, "0")}`;
}

/** A position inside the piece as a 0..1 fraction of its length. */
function ratio(ms: number, duration: number): number {
  return duration > 0 ? Math.min(1, Math.max(0, ms / duration)) : 0;
}

/**
 * The transport timeline: a playhead the user can click or drag to scrub, plus the loop region
 * and its controls. The playhead position and loop wrap come from the shared position poll in
 * `usePlaybackPosition`; this only marks a drag as scrubbing so the poll leaves the playhead
 * alone until the seek completes. Every change goes through the session store, which rewinds
 * the engine's scoring through the same seek.
 */
export function TransportTimeline() {
  const piece = useSession((state) => state.piece);
  const loopStartMs = useSession((state) => state.loopStartMs);
  const loopEndMs = useSession((state) => state.loopEndMs);
  const looping = useSession((state) => state.looping);
  const positionMs = useSession((state) => state.positionMs);
  const setPositionMs = useSession((state) => state.setPositionMs);
  const seekTo = useSession((state) => state.seekTo);
  const setLoopStart = useSession((state) => state.setLoopStart);
  const setLoopEnd = useSession((state) => state.setLoopEnd);
  const toggleLoop = useSession((state) => state.toggleLoop);
  const clearLoop = useSession((state) => state.clearLoop);
  const trackRef = useRef<HTMLDivElement>(null);
  const draggingRef = useRef(false);

  const duration = piece?.duration_ms ?? 0;
  const start = loopStartMs ?? 0;
  const end = loopEndMs ?? duration;
  const hasRegion =
    loopStartMs !== null && loopEndMs !== null && loopEndMs > loopStartMs;

  if (!piece) {
    return null;
  }

  const positionAt = (clientX: number): number => {
    const box = trackRef.current?.getBoundingClientRect();
    if (!box || box.width === 0) {
      return 0;
    }
    const fraction = (clientX - box.left) / box.width;
    return Math.min(1, Math.max(0, fraction)) * duration;
  };

  const onPointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    draggingRef.current = true;
    pausePositionPoll();
    event.currentTarget.setPointerCapture(event.pointerId);
    const ms = positionAt(event.clientX);
    setPositionMs(ms);
    void seekTo(ms);
  };

  const onPointerMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (!draggingRef.current) {
      return;
    }
    setPositionMs(positionAt(event.clientX));
  };

  // Ends a drag wherever it ends: a normal release, a cancelled pointer, or a lost capture.
  // Without this a cancel would leave the shared poll paused, freezing the playhead for good.
  const endDrag = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (!draggingRef.current) {
      return;
    }
    draggingRef.current = false;
    resumePositionPoll();
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    void seekTo(useSession.getState().positionMs);
  };

  // Leaving the view mid-drag must not leave the shared poll paused.
  useEffect(() => () => resumePositionPoll(), []);

  return (
    <div className="timeline">
      <span className="timeline-clock">{clock(positionMs)}</span>
      <div
        ref={trackRef}
        className="timeline-track"
        role="slider"
        aria-label="Playback position"
        aria-valuemin={0}
        aria-valuemax={Math.round(duration)}
        aria-valuenow={Math.round(positionMs)}
        tabIndex={0}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={endDrag}
        onPointerCancel={endDrag}
        onLostPointerCapture={endDrag}
      >
        {hasRegion && (
          <div
            className="timeline-region"
            style={{
              left: `${ratio(start, duration) * 100}%`,
              width: `${ratio(end - start, duration) * 100}%`,
            }}
          />
        )}
        {loopStartMs !== null && (
          <div
            className="timeline-point start"
            style={{ left: `${ratio(loopStartMs, duration) * 100}%` }}
          />
        )}
        {loopEndMs !== null && (
          <div
            className="timeline-point end"
            style={{ left: `${ratio(loopEndMs, duration) * 100}%` }}
          />
        )}
        <div
          className="timeline-playhead"
          style={{ left: `${ratio(positionMs, duration) * 100}%` }}
        />
      </div>
      <span className="timeline-clock">{clock(duration)}</span>
      <div className="timeline-loop">
        <span className="timeline-loop-label">
          {hasRegion
            ? `Loop ${clock(start)}–${clock(end)}`
            : loopStartMs !== null
              ? `Start ${clock(loopStartMs)}`
              : loopEndMs !== null
                ? `End ${clock(loopEndMs)}`
                : "No loop"}
        </span>
        <button
          type="button"
          className="btn"
          onClick={() => void setLoopStart()}
        >
          Set start
        </button>
        <button type="button" className="btn" onClick={() => void setLoopEnd()}>
          Set end
        </button>
        <button
          type="button"
          className={looping ? "btn primary" : "btn"}
          aria-pressed={looping}
          disabled={!hasRegion}
          onClick={toggleLoop}
        >
          ↻ Loop
        </button>
        <button
          type="button"
          className="btn"
          disabled={!hasRegion && !loopStartMs && !loopEndMs}
          onClick={clearLoop}
        >
          Clear
        </button>
      </div>
    </div>
  );
}
