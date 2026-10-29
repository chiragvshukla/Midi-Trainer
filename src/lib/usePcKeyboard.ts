import { useEffect } from "react";
import { keyDown, keyUp } from "./api";
import { log } from "./logger";

/**
 * Plays notes from the PC keyboard. Every key event is forwarded to the backend, which owns the
 * key-to-pitch mapping and ignores keys that play no note. Pressed keys are tracked so a window
 * blur (which loses the keyup) releases them instead of leaving notes stuck on.
 */
export function usePcKeyboard(): void {
  useEffect(() => {
    const pressed = new Set<string>();

    const send = (action: Promise<void>, code: string) => {
      void action.catch((error) =>
        log.error(`key input failed for ${code}: ${String(error)}`),
      );
    };

    const down = (event: KeyboardEvent) => {
      if (event.repeat || pressed.has(event.code)) {
        return;
      }
      pressed.add(event.code);
      send(keyDown(event.code), event.code);
    };

    const up = (event: KeyboardEvent) => {
      pressed.delete(event.code);
      send(keyUp(event.code), event.code);
    };

    const releaseAll = () => {
      for (const code of pressed) {
        send(keyUp(code), code);
      }
      pressed.clear();
    };

    window.addEventListener("keydown", down);
    window.addEventListener("keyup", up);
    window.addEventListener("blur", releaseAll);
    return () => {
      window.removeEventListener("keydown", down);
      window.removeEventListener("keyup", up);
      window.removeEventListener("blur", releaseAll);
    };
  }, []);
}
