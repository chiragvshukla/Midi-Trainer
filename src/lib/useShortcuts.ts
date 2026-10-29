import { useEffect } from "react";
import { useSession } from "../state/sessionStore";

/** Whether the key landed in a control that owns its keys, so we leave it alone. */
function ownsKeys(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLElement &&
    target.closest(
      'input, textarea, select, button, [contenteditable], [role="slider"]',
    ) !== null
  );
}

/**
 * The global transport shortcuts: Space toggles play and stop, Home returns to the start, and the
 * arrow keys step a bar. They never fire while a field, button or the timeline slider has focus,
 * or with a modifier held, and do nothing until a piece is loaded.
 */
export function useShortcuts(): void {
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (
        event.ctrlKey ||
        event.metaKey ||
        event.altKey ||
        ownsKeys(event.target)
      ) {
        return;
      }
      const session = useSession.getState();
      if (!session.piece) {
        return;
      }
      switch (event.code) {
        case "Space":
          event.preventDefault();
          void (session.playing ? session.stop() : session.resume());
          break;
        case "Home":
          event.preventDefault();
          void session.startOver();
          break;
        case "ArrowLeft":
          event.preventDefault();
          void session.previousBar();
          break;
        case "ArrowRight":
          event.preventDefault();
          void session.nextBar();
          break;
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);
}
