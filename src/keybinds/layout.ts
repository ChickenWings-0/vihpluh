/**
 * Reads the OS keyboard layout once so key caps show the glyph printed on the user's keys
 * (KEYBINDS_AND_CONFIG §3.1). Webviews without the Keyboard Layout Map API keep US names.
 */
import { setLayoutGlyphs } from "./normalize";

interface KeyboardApi {
  getLayoutMap: () => Promise<Iterable<[string, string]>>;
}

export function loadKeyboardLayout(): void {
  const keyboard = (navigator as Navigator & { keyboard?: KeyboardApi }).keyboard;
  if (!keyboard?.getLayoutMap) return;
  keyboard
    .getLayoutMap()
    .then((map) => setLayoutGlyphs(map))
    .catch(() => undefined);
}
