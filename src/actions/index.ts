/** Registers every action once, at startup. */
import { appActions } from "./definitions/app";
import { playbackActions } from "./definitions/playback";
import { stubActions } from "./definitions/stubs";
import { trackActions } from "./definitions/tracks";
import { windowActions } from "./definitions/window";
import { register } from "./registry";

let registered = false;

export function registerActions(): void {
  if (registered) return;
  registered = true;
  register([...playbackActions, ...trackActions, ...windowActions, ...appActions, ...stubActions]);
}

export { allActions, dispatch, getAction, isEnabled } from "./registry";
