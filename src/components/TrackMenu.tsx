import { useEffect, useRef, useState } from "react";
import type { TrackInfo } from "../lib/api";
import { useSession } from "../state/sessionStore";

/** A track's display name: its own name, or its position when the file gives none. */
function trackName(track: TrackInfo): string {
  const name = track.name?.trim();
  return name ? name : `Track ${track.index + 1}`;
}

/**
 * The track picker: a checkbox per track in the loaded piece. Unchecking a track reloads the
 * piece without it, so the views, the sheet and playback all drop it together. Rendering only:
 * the selection and the reload live in the session store.
 */
export function TrackMenu() {
  const piece = useSession((state) => state.piece);
  const selectedTracks = useSession((state) => state.selectedTracks);
  const toggleTrack = useSession((state) => state.toggleTrack);
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) {
      return;
    }
    const closeOnOutside = (event: MouseEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", closeOnOutside);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("mousedown", closeOnOutside);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [open]);

  if (!piece || piece.tracks.length === 0) {
    return null;
  }

  const isSelected = (index: number) =>
    selectedTracks === null || selectedTracks.includes(index);

  return (
    <div className="menu" ref={rootRef}>
      <button
        type="button"
        className="menu-button"
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((current) => !current)}
      >
        Tracks ▾
      </button>
      {open && (
        <div className="menu-items" role="menu">
          {piece.tracks.map((track) => (
            <label key={track.index} className="track-item">
              <input
                type="checkbox"
                checked={isSelected(track.index)}
                onChange={() => void toggleTrack(track.index)}
              />
              <span>
                {trackName(track)} ({track.note_count})
              </span>
            </label>
          ))}
        </div>
      )}
    </div>
  );
}
