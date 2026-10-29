import { invoke } from "@tauri-apps/api/core";

export interface AppInfo {
  name: string;
  version: string;
  os: string;
  arch: string;
}

export function fetchAppInfo(): Promise<AppInfo> {
  return invoke<AppInfo>("app_info");
}
