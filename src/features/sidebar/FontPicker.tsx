/**
 * Subtitle font picker (DESIGN_SYSTEM §10.1): the installed families (`fonts_list`), each
 * rendered in its own face, searchable, and virtualised — a machine can have hundreds.
 */
import { useVirtualizer } from "@tanstack/react-virtual";
import { Check, ChevronDown } from "lucide-react";
import { type KeyboardEvent, useEffect, useMemo, useRef, useState } from "react";
import { Popover } from "@/components/Popover";
import { fontsList } from "@/ipc/commands";
import { reportError } from "@/stores/osd";
import { useUiStore } from "@/stores/ui";

const ROW = 28;

let cachedFonts: string[] | null = null;

export function FontPicker({
  value,
  onChange,
  disabled,
}: {
  value: string;
  onChange: (font: string) => void;
  disabled?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [fonts, setFonts] = useState<string[]>(cachedFonts ?? []);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const hold = useUiStore((s) => s.hold);
  const scroller = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open || cachedFonts) return;
    void fontsList()
      .then((list) => {
        cachedFonts = list;
        setFonts(list);
      })
      .catch(reportError);
  }, [open]);

  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return q ? fonts.filter((f) => f.toLowerCase().includes(q)) : fonts;
  }, [fonts, query]);

  const rows = useVirtualizer({
    count: shown.length,
    getScrollElement: () => scroller.current,
    estimateSize: () => ROW,
    overscan: 8,
  });

  // Opening starts at the current font.
  // biome-ignore lint/correctness/useExhaustiveDependencies: only when the list opens or loads
  useEffect(() => {
    if (!open) return;
    const index = Math.max(0, shown.indexOf(value));
    setActive(index);
    requestAnimationFrame(() => rows.scrollToIndex(index, { align: "center" }));
  }, [open, fonts]);

  const pick = (font: string | undefined) => {
    if (!font) return;
    onChange(font);
    setOpen(false);
    hold("font-picker", false);
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    const dir = e.key === "ArrowDown" ? 1 : e.key === "ArrowUp" ? -1 : 0;
    if (dir !== 0) {
      e.preventDefault();
      const next = Math.max(0, Math.min(shown.length - 1, active + dir));
      setActive(next);
      rows.scrollToIndex(next);
    } else if (e.key === "Enter") {
      e.preventDefault();
      pick(shown[active]);
    }
  };

  return (
    <Popover
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        setQuery("");
        hold("font-picker", next);
      }}
      side="left"
      align="start"
      width="w-64"
      trigger={
        <button
          type="button"
          aria-label={`Subtitle font: ${value || "default"}`}
          disabled={disabled}
          className="flex h-7 min-w-0 flex-1 items-center gap-2 rounded-control border border-line bg-raised px-2 text-left text-base text-fg hover:border-line-strong disabled:text-fg-off"
        >
          <span className="min-w-0 flex-1 truncate" style={{ fontFamily: `"${value}"` }}>
            {value || "Default"}
          </span>
          <ChevronDown size={14} strokeWidth={1.5} className="shrink-0 text-fg-2" aria-hidden />
        </button>
      }
    >
      <input
        type="text"
        // biome-ignore lint/a11y/noAutofocus: the list was opened to type into
        autoFocus
        value={query}
        placeholder="Search fonts…"
        aria-label="Search fonts"
        onChange={(e) => {
          setQuery(e.target.value);
          setActive(0);
          rows.scrollToIndex(0);
        }}
        onKeyDown={onKeyDown}
        className="mb-1 h-7 w-full rounded-control border border-line bg-surface px-2 text-base text-fg outline-none placeholder:text-fg-3 focus:border-accent"
      />
      <div ref={scroller} className="h-64 overflow-y-auto" role="listbox" aria-label="Fonts">
        <div className="relative w-full" style={{ height: rows.getTotalSize() }}>
          {rows.getVirtualItems().map((item) => {
            const font = shown[item.index] ?? "";
            const selected = font === value;
            return (
              <button
                key={font}
                type="button"
                role="option"
                aria-selected={selected}
                tabIndex={-1}
                onClick={() => pick(font)}
                onMouseEnter={() => setActive(item.index)}
                className={`absolute left-0 flex h-7 w-full items-center gap-2 rounded-control px-2 text-left text-base text-fg ${
                  item.index === active ? "bg-hover" : ""
                }`}
                style={{ transform: `translateY(${item.start}px)` }}
              >
                <span className="flex w-4 shrink-0 justify-center">
                  {selected && (
                    <Check size={14} strokeWidth={1.5} className="text-accent-fg" aria-hidden />
                  )}
                </span>
                <span className="min-w-0 flex-1 truncate" style={{ fontFamily: `"${font}"` }}>
                  {font}
                </span>
              </button>
            );
          })}
        </div>
        {shown.length === 0 && (
          <p className="px-2 py-3 text-base text-fg-3">
            {fonts.length === 0 ? "Loading fonts…" : "No font matches."}
          </p>
        )}
      </div>
    </Popover>
  );
}
