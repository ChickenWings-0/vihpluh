/** App facts from `app_info`: versions, paths, and what this platform can do. */
import { create } from "zustand";
import type { AppInfo } from "@/ipc/bindings/AppInfo";

interface AppStore {
  info: AppInfo | null;
  setInfo: (info: AppInfo) => void;
}

export const useAppStore = create<AppStore>((set) => ({
  info: null,
  setInfo: (info) => set({ info }),
}));
