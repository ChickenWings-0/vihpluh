/** Typed subscriptions to core events (ARCHITECTURE §7.4). */
import type { DebugStats } from "./bindings/DebugStats";
import { listen, type Unlisten } from "./transport";

export const onDebugStats = (handler: (stats: DebugStats) => void): Unlisten =>
  listen<DebugStats>("debug://stats", handler);
