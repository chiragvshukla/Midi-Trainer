import { invoke } from "@tauri-apps/api/core";

type LogLevel = "error" | "warn" | "info" | "debug" | "trace";

function send(level: LogLevel, message: string): void {
  // Logging must never break the UI, so failures are swallowed here.
  void invoke("log_message", { level, message }).catch(() => {});
}

export const log = {
  error: (message: string) => send("error", message),
  warn: (message: string) => send("warn", message),
  info: (message: string) => send("info", message),
  debug: (message: string) => send("debug", message),
  trace: (message: string) => send("trace", message),
};
