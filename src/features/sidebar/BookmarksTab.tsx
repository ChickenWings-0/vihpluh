/**
 * Bookmarks tab (DESIGN_SYSTEM §10.5): chapters from the file and the user's bookmarks as
 * one chronological list. Click a row to go there; bookmarks rename inline and delete with an
 * undo toast; the row the playhead is in carries the accent bar; hovering a row shows that
 * position's thumbnail.
 */
import { ChevronRight, Pencil, Plus, Star, X } from "lucide-react";
import { type KeyboardEvent, useEffect, useMemo, useRef, useState } from "react";
import { dispatch } from "@/actions";
import { removeBookmark } from "@/actions/definitions/markers";
import { Button } from "@/components/Button";
import { Segmented } from "@/components/Controls";
import { KeyCap } from "@/components/KeyCap";
import type { Chapter } from "@/ipc/bindings/Chapter";
import { bookmarkUpdate, playerSeek } from "@/ipc/commands";
import { thumbUrl } from "@/ipc/transport";
import { currentMarkerIndex, type Marker, type MarkerFilter, mergeMarkers } from "@/lib/markers";
import { formatTime } from "@/lib/time";
import { useAnimationFrame } from "@/lib/useAnimationFrame";
import { useBookmarkStore } from "@/stores/bookmarks";
import { useConfig } from "@/stores/config";
import { useShortcut } from "@/stores/keymap";
import { reportError } from "@/stores/osd";
import { usePlayer } from "@/stores/player";
import { getDuration, getPosition } from "@/stores/time";
import { bucketStep } from "../chrome/SeekBar";

const NO_CHAPTERS: Chapter[] = [];

const FILTERS: { value: MarkerFilter; label: string }[] = [
  { value: "all", label: "All" },
  { value: "chapters", label: "Chapters" },
  { value: "bookmarks", label: "Bookmarks" },
];

function RenameField({ id, label }: { id: number; label: string }) {
  const [text, setText] = useState(label);
  const stop = useBookmarkStore((s) => s.rename);
  const done = useRef(false);
  const commit = () => {
    if (done.current) return;
    done.current = true;
    const next = text.trim();
    if (next !== "" && next !== label) {
      void bookmarkUpdate(id, { label: next }).catch(reportError);
    }
    stop(null);
  };
  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") commit();
    else if (e.key === "Escape") {
      done.current = true;
      stop(null);
    } else return;
    e.preventDefault();
    e.stopPropagation();
  };
  return (
    <input
      type="text"
      // biome-ignore lint/a11y/noAutofocus: renaming was just asked for
      autoFocus
      aria-label="Bookmark name"
      value={text}
      maxLength={120}
      onFocus={(e) => e.target.select()}
      onChange={(e) => setText(e.target.value)}
      onBlur={commit}
      onKeyDown={onKeyDown}
      onClick={(e) => e.stopPropagation()}
      className="h-6 min-w-0 flex-1 rounded-control border border-accent bg-raised px-1.5 text-base text-fg outline-none"
    />
  );
}

function Row({
  marker,
  current,
  onHover,
}: {
  marker: Marker;
  current: boolean;
  onHover: (marker: Marker | null, top: number) => void;
}) {
  const precise = useConfig((c) => c.controls.precise_seek) ?? true;
  const selected = useBookmarkStore(
    (s) => marker.kind === "bookmark" && s.selectedId === marker.id,
  );
  const renaming = useBookmarkStore(
    (s) => marker.kind === "bookmark" && s.renamingId === marker.id,
  );
  const select = useBookmarkStore((s) => s.select);
  const rename = useBookmarkStore((s) => s.rename);
  const isBookmark = marker.kind === "bookmark";
  const go = () => {
    select(isBookmark ? marker.id : null);
    void playerSeek({ kind: "absolute", value: marker.time, exact: precise }).catch(reportError);
  };
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.target !== e.currentTarget) return;
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      e.stopPropagation();
      go();
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      e.stopPropagation();
      const rows = [
        ...(e.currentTarget.parentElement?.querySelectorAll<HTMLElement>("[data-marker]") ?? []),
      ];
      rows[rows.indexOf(e.currentTarget) + (e.key === "ArrowDown" ? 1 : -1)]?.focus();
    }
  };
  return (
    // biome-ignore lint/a11y/useSemanticElements: a row with its own inner buttons cannot be a <button>
    <div
      role="button"
      tabIndex={0}
      data-marker={marker.key}
      data-current={current || undefined}
      aria-current={current ? "true" : undefined}
      aria-label={`${isBookmark ? "Bookmark" : "Chapter"}: ${marker.label}, ${formatTime(marker.time)}`}
      onClick={go}
      onKeyDown={onKeyDown}
      onFocus={() => select(isBookmark ? marker.id : null)}
      onPointerEnter={(e) => onHover(marker, e.currentTarget.getBoundingClientRect().top)}
      onPointerLeave={() => onHover(null, 0)}
      className={`group relative flex h-8 cursor-default items-center gap-2 rounded-control pr-1 pl-2 outline-none focus-visible:bg-hover ${
        selected ? "bg-accent-soft" : "hover:bg-hover"
      }`}
    >
      {current && <span className="absolute inset-y-1 left-0 w-0.5 bg-accent" aria-hidden />}
      {isBookmark ? (
        <Star size={14} strokeWidth={1.5} className="shrink-0 text-accent-fg" aria-hidden />
      ) : (
        <ChevronRight size={14} strokeWidth={1.5} className="shrink-0 text-fg-3" aria-hidden />
      )}
      <span className="tabular w-12 shrink-0 font-mono text-fg-2 text-sm">
        {formatTime(marker.time)}
      </span>
      {renaming && isBookmark ? (
        <RenameField id={marker.id} label={marker.label} />
      ) : (
        <span
          className={`min-w-0 flex-1 truncate text-base ${isBookmark ? "text-fg" : "text-fg-2"}`}
        >
          {marker.label}
        </span>
      )}
      {isBookmark && !renaming && (
        <span className="flex shrink-0 opacity-0 group-focus-within:opacity-100 group-hover:opacity-100">
          <button
            type="button"
            aria-label={`Rename ${marker.label}`}
            onClick={(e) => {
              e.stopPropagation();
              rename(marker.id);
            }}
            className="flex h-6 w-6 items-center justify-center rounded-control text-fg-2 hover:bg-active hover:text-fg"
          >
            <Pencil size={13} strokeWidth={1.5} aria-hidden />
          </button>
          <button
            type="button"
            aria-label={`Delete ${marker.label}`}
            onClick={(e) => {
              e.stopPropagation();
              void removeBookmark(marker.id).catch(reportError);
            }}
            className="flex h-6 w-6 items-center justify-center rounded-control text-fg-2 hover:bg-active hover:text-fg"
          >
            <X size={14} strokeWidth={1.5} aria-hidden />
          </button>
        </span>
      )}
    </div>
  );
}

/** The standard preview card, beside the row instead of above the seek bar. */
function HoverCard({ marker, top }: { marker: Marker; top: number }) {
  const mediaKey = usePlayer((s) => s.media?.key ?? "") ?? "";
  const previewable =
    usePlayer((s) => s.media !== null && s.media.kind === "video" && !s.media.isUrl) ?? false;
  const on = useConfig((c) => c.interface.seek_preview) ?? true;
  const [loaded, setLoaded] = useState<string | null>(null);
  const duration = getDuration();
  if (!on || !previewable || mediaKey === "" || duration <= 0) return null;
  const step = bucketStep(duration);
  const bucket = Math.min(Math.floor(marker.time / step), Math.floor(duration / step));
  const url = thumbUrl(mediaKey, step, bucket);
  return (
    <div
      className={`pointer-events-none fixed z-(--z-floating) flex -translate-x-full flex-col items-center gap-1 rounded-surface border border-line-strong bg-raised p-1 shadow-float ${
        loaded === url ? "opacity-100" : "opacity-0"
      }`}
      style={{ top: Math.max(48, top - 60), right: "auto", left: "var(--hover-card-left)" }}
    >
      <div className="h-33.75 w-60 overflow-hidden rounded-control border border-line-strong bg-video">
        <img
          src={url}
          alt=""
          onLoad={() => setLoaded(url)}
          className="h-full w-full object-contain"
        />
      </div>
      <div className="tabular px-1 text-base text-fg">{formatTime(marker.time)}</div>
    </div>
  );
}

export function BookmarksTab() {
  const chapters = usePlayer((s) => s.chapters) ?? NO_CHAPTERS;
  const hasMedia = usePlayer((s) => (s.media?.key ?? "") !== "") ?? false;
  const bookmarks = useBookmarkStore((s) => s.bookmarks);
  const filter = useBookmarkStore((s) => s.filter);
  const setFilter = useBookmarkStore((s) => s.setFilter);
  const renamingId = useBookmarkStore((s) => s.renamingId);
  const addKey = useShortcut("bookmarks.add");
  const markers = useMemo(
    () => mergeMarkers(chapters, bookmarks, filter),
    [chapters, bookmarks, filter],
  );
  const [current, setCurrent] = useState(-1);
  const [hover, setHover] = useState<{ marker: Marker; top: number } | null>(null);
  const addLabel = useRef<HTMLSpanElement>(null);
  const list = useRef<HTMLDivElement>(null);

  // The playhead's row and the "Add bookmark at …" time follow playback without re-rendering
  // more than a row change needs.
  useAnimationFrame(() => {
    const position = getPosition();
    const index = currentMarkerIndex(markers, position);
    if (index !== current) setCurrent(index);
    const text = formatTime(position);
    if (addLabel.current && addLabel.current.textContent !== text) {
      addLabel.current.textContent = text;
    }
  });

  // A row asked to be renamed from elsewhere (the "Rename" toast) scrolls into view.
  useEffect(() => {
    if (renamingId === null) return;
    list.current
      ?.querySelector(`[data-marker="b${renamingId}"]`)
      ?.scrollIntoView({ block: "nearest" });
  }, [renamingId]);

  return (
    <>
      <div className="flex flex-wrap items-center gap-2">
        <Button
          size="sm"
          icon={Plus}
          disabled={!hasMedia}
          onClick={() => void dispatch("bookmarks.add", { trigger: "button" })}
        >
          Add bookmark at <span ref={addLabel} className="tabular" />
        </Button>
        <Segmented
          label="Show"
          className="min-w-44 flex-1"
          options={FILTERS}
          value={filter}
          onChange={setFilter}
        />
      </div>
      <div
        ref={list}
        className="flex flex-col"
        style={{
          ["--hover-card-left" as string]: `${(list.current?.getBoundingClientRect().left ?? 0) - 12}px`,
        }}
      >
        {markers.map((marker, index) => (
          <Row
            key={marker.key}
            marker={marker}
            current={index === current}
            onHover={(m, top) => setHover(m ? { marker: m, top } : null)}
          />
        ))}
        {markers.length === 0 && (
          <p className="px-2 py-3 text-base text-fg-3">
            {!hasMedia
              ? "Nothing is playing."
              : filter === "chapters"
                ? "This file has no chapters."
                : "No bookmarks yet."}{" "}
            {hasMedia && filter !== "chapters" && addKey && (
              <>
                <KeyCap binding={addKey} /> adds one at the current position.
              </>
            )}
          </p>
        )}
        {hover && <HoverCard marker={hover.marker} top={hover.top} />}
      </div>
    </>
  );
}
