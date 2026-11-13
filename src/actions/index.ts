/** Registers every action once, at startup. */
import { appActions } from "./definitions/app";
import { mediaActions } from "./definitions/media";
import { playbackActions } from "./definitions/playback";
import { queueActions } from "./definitions/queue";
import { stubActions } from "./definitions/stubs";
import { subtitleActions } from "./definitions/subtitles";
import { trackActions } from "./definitions/tracks";
import { windowActions } from "./definitions/window";
import { register } from "./registry";

let registered = false;

export function registerActions(): void {
  if (registered) return;
  registered = true;
  register([
    ...playbackActions,
    ...queueActions,
    ...trackActions,
    ...subtitleActions,
    ...mediaActions,
    ...windowActions,
    ...appActions,
    ...stubActions,
  ]);
}

export { allActions, dispatch, getAction, isEnabled } from "./registry";
