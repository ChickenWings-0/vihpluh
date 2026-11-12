import { useEffect, useState } from "react";
import { DebugOverlay } from "./features/debug/DebugOverlay";
import type { AppInfo } from "./ipc/bindings/AppInfo";
import { appInfo, appReady } from "./ipc/commands";

export function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);

  useEffect(() => {
    let cancelled = false;
    appInfo()
      .then((value) => {
        if (!cancelled) setInfo(value);
      })
      .catch((error: unknown) => console.error("app_info failed", error));
    return () => {
      cancelled = true;
    };
  }, []);

  // The window stays hidden until the first frame is on screen (ARCHITECTURE §4.3).
  useEffect(() => {
    const frame = requestAnimationFrame(() => {
      appReady().catch((error: unknown) => console.error("app_ready failed", error));
    });
    return () => cancelAnimationFrame(frame);
  }, []);

  return (
    <main className="h-full w-full">{import.meta.env.DEV && <DebugOverlay info={info} />}</main>
  );
}
