import { invoke } from "@tauri-apps/api/core";

export interface AppInfo {
  version: string;
  os: string;
  arch: string;
}

export function fetchAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}

export function keyDown(code: string): Promise<void> {
  return invoke("key_down", { code });
}

export function keyUp(code: string): Promise<void> {
  return invoke("key_up", { code });
}
