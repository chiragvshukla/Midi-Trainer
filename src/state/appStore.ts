import { create } from "zustand";
import { fetchAppInfo, type AppInfo } from "../lib/api";
import { log } from "../lib/logger";

interface AppState {
  appInfo: AppInfo | null;
  loadAppInfo: () => Promise<void>;
}

export const useAppStore = create<AppState>((set) => ({
  appInfo: null,
  loadAppInfo: async () => {
    try {
      set({ appInfo: await fetchAppInfo() });
    } catch (error) {
      log.error(`failed to load app info: ${String(error)}`);
    }
  },
}));
