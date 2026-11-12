//! Types crossing IPC. All derive `TS` and are exported to `src/ipc/bindings/` by
//! `pnpm gen:bindings`.

use serde::Serialize;
use ts_rs::TS;

use crate::paths::PathsDto;
use crate::video::Capabilities;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppInfo {
    pub version: String,
    pub mpv_version: String,
    pub ffmpeg_version: String,
    pub capabilities: Capabilities,
    pub paths: PathsDto,
}
