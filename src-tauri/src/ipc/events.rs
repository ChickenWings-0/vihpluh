//! Event names (ARCHITECTURE §7.4). Payload types live in `dto.rs`.

/// `{ rev } & Partial<PlayerState>` — shallow merge, coalesced to ≤ 30 Hz.
pub const PLAYER_PATCH: &str = "player://patch";
/// `TimeEvent` — 5 Hz while playing, immediately on seek/pause/load.
pub const PLAYER_TIME: &str = "player://time";
/// `InspectorStats` — 2 Hz while the stream inspector is subscribed.
pub const PLAYER_INSPECTOR: &str = "player://inspector";
/// `ResolvedKeymap` — after `keybinds.toml` changed.
pub const KEYBINDS_CHANGED: &str = "keybinds://changed";
/// `QueueState` — after any queue change.
pub const QUEUE_CHANGED: &str = "queue://changed";
/// `ConfigChanged` — one per changed setting.
pub const CONFIG_CHANGED: &str = "config://changed";
/// `OsdEvent` — notices that originate in the core.
pub const OSD_SHOW: &str = "osd://show";
/// `OpenEvent` — sources forwarded by a second instance (informational).
pub const APP_OPEN: &str = "app://open";
/// `DebugStats` — development overlay only.
pub const DEBUG_STATS: &str = "debug://stats";
