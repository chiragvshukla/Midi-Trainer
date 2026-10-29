import { useEffect } from "react";
import { playbackPosition } from "./api";
import { useSession } from "../state/sessionStore";

/** While true the shared poll stands still, so a dragged playhead is not yanked back to the
 * engine's position. The transport sets it for the duration of a drag. */
let paused = false;

/** Stops the shared position poll while the user scrubs the transport. */
export function pausePositionPoll(): void {
  paused = true;
}

/** Resumes the shared position poll once scrubbing ends. */
export function resumePositionPoll(): void {
  paused = false;
}

/**
 * Polls the engine's playhead once per animation frame and writes it to the session store,
 * which wraps an active loop. Mounted once by the app shell: every view reads `positionMs`
 * from the store instead of running its own IPC poll.
 */
export function usePlaybackPosition(): void {
  useEffect(() => {
    let frame = 0;
    let wrapping = false;
    const tick = () => {
      frame = requestAnimationFrame(tick);
      // Nothing to poll unless a piece is loaded and moving, and a scrub is settled elsewhere.
      const session = useSession.getState();
      if (!session.piece || !session.playing || paused) {
        return;
      }
      playbackPosition()
        .then((position) => {
          if (paused) {
            return;
          }
          const state = useSession.getState();
          state.setPositionMs(position);
          // Wrap the loop by seeking through the store, so the matcher rewinds too. The flag
          // stops the seek repeating on the frames before the engine reports the new position.
          if (
            state.looping &&
            state.loopStartMs !== null &&
            state.loopEndMs !== null
          ) {
            if (position >= state.loopEndMs) {
              if (!wrapping) {
                wrapping = true;
                void state.seekTo(state.loopStartMs);
              }
            } else {
              wrapping = false;
            }
          }
        })
        .catch(() => {});
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, []);
}
