/**
 * Settings search (KEYBINDS_AND_CONFIG §6 rule 5): every word of the query must appear in a
 * setting's label, tooltip, keywords, or dotted key. Label hits rank first; a pasted key
 * finds exactly its row.
 */
import type { Setting } from "./schema";

const words = (query: string) => query.toLowerCase().split(/\s+/).filter(Boolean);

function score(setting: Setting, tokens: string[]): number {
  const label = setting.label.toLowerCase();
  const path = setting.path.toLowerCase();
  const keywords = (setting.keywords ?? []).join(" ").toLowerCase();
  const tooltip = setting.tooltip.toLowerCase();
  let total = 0;
  for (const token of tokens) {
    if (path === token) total += 100;
    else if (label.includes(token)) total += 8;
    else if (keywords.includes(token)) total += 4;
    else if (path.includes(token)) total += 2;
    else if (tooltip.includes(token)) total += 1;
    else return 0;
  }
  return total;
}

/** The settings a query finds, best first; an empty query finds nothing. */
export function searchSettings(query: string, settings: readonly Setting[]): Setting[] {
  const tokens = words(query);
  if (tokens.length === 0) return [];
  return settings
    .map((setting, index) => ({ setting, index, score: score(setting, tokens) }))
    .filter((hit) => hit.score > 0)
    .sort((a, b) => b.score - a.score || a.index - b.index)
    .map((hit) => hit.setting);
}

export interface Piece {
  text: string;
  match: boolean;
}

/** Splits `text` into matched and unmatched runs for highlighting. */
export function highlight(text: string, query: string): Piece[] {
  const tokens = words(query);
  const lower = text.toLowerCase();
  const marked = new Array<boolean>(text.length).fill(false);
  for (const token of tokens) {
    let from = lower.indexOf(token);
    while (from !== -1) {
      marked.fill(true, from, from + token.length);
      from = lower.indexOf(token, from + token.length);
    }
  }
  const pieces: Piece[] = [];
  for (let i = 0; i < text.length; i++) {
    const match = marked[i] ?? false;
    const last = pieces[pieces.length - 1];
    if (last && last.match === match) last.text += text[i];
    else pieces.push({ text: text[i] ?? "", match });
  }
  return pieces;
}
