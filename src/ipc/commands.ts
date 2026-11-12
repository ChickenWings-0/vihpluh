/** Typed wrapper per core command (ARCHITECTURE §7.3). Components never call `invoke` directly. */
import type { AppInfo } from "./bindings/AppInfo";
import { invoke } from "./transport";

export const appInfo = () => invoke<AppInfo>("app_info");
export const appReady = () => invoke<void>("app_ready");
export const appQuit = () => invoke<void>("app_quit");
