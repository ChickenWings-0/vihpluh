/**
 * Fuzzy matching for the command palette: the query's characters must appear in order.
 * Runs of consecutive characters and matches at the start of a word score higher, so
 * "tf" finds "Toggle fullscreen" before "Stream info".
 */

/** A score (higher is better), or null when `text` does not contain `query` in order. */
export function fuzzyScore(query: string, text: string): number | null {
  const q = query.toLowerCase().replace(/\s+/g, "");
  if (q === "") return 0;
  const t = text.toLowerCase();
  let score = 0;
  let at = 0;
  let run = 0;
  for (const char of q) {
    const found = t.indexOf(char, at);
    if (found === -1) return null;
    const wordStart = found === 0 || /[\s./_:-]/.test(t[found - 1] ?? "");
    run = found === at && at > 0 ? run + 1 : 0;
    score += 1 + run * 2 + (wordStart ? 4 : 0) - Math.min(3, (found - at) * 0.1);
    at = found + 1;
  }
  // Shorter texts win ties: the match covers more of them.
  return score - t.length * 0.01;
}

/** `items` that match, best first; an empty query keeps the given order. */
export function fuzzyFilter<T>(query: string, items: readonly T[], text: (item: T) => string): T[] {
  if (query.trim() === "") return [...items];
  return items
    .map((item, index) => ({ item, index, score: fuzzyScore(query, text(item)) }))
    .filter((hit): hit is { item: T; index: number; score: number } => hit.score !== null)
    .sort((a, b) => b.score - a.score || a.index - b.index)
    .map((hit) => hit.item);
}
