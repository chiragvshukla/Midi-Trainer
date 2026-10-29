import { useEffect } from "react";
import { keyDown, keyUp } from "./api";
import { log } from "./logger";
import { useSession } from "../state/sessionStore";

/**
 * Plays notes from the PC keyboard. Every key event is forwarded to the backend, which owns the
 * key-to-pitch mapping and ignores keys that play no note. Pressed keys are tracked so a window
 * blur (which loses the keyup) releases them instead of leaving notes stuck on.
 */
export function usePcKeyboard(): void {
  useEffect(() => {
    const pressed = new Map<string, number>();

    const send = (action: Promise<unknown>, context: string) => {
      void action.catch((error) =>
        log.error(`key input failed for ${context}: ${String(error)}`),
      );
    };

    const down = (event: KeyboardEvent) => {
      if (event.repeat || pressed.has(event.code)) {
        return;
      }
      // -1 marks "waiting for the backend to tell us the pitch".
      pressed.set(event.code, -1);
      send(
        keyDown(event.code).then((pitch) => {
          // The key may have been released while the answer was in flight.
          if (pitch !== null && pressed.get(event.code) === -1) {
            pressed.set(event.code, pitch);
            useSession.getState().pressPitch(pitch, true);
          }
        }),
        event.code,
      );
    };

    const up = (event: KeyboardEvent) => {
      const pitch = pressed.get(event.code);
      pressed.delete(event.code);
      send(keyUp(event.code), event.code);
      if (pitch !== undefined && pitch >= 0) {
        useSession.getState().pressPitch(pitch, false);
      }
    };

    const releaseAll = () => {
      for (const [code, pitch] of pressed) {
        send(keyUp(code), code);
        if (pitch >= 0) {
          useSession.getState().pressPitch(pitch, false);
        }
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
