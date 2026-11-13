/**
 * Queue panel (DESIGN_SYSTEM §10.7): a virtualised list of 32 px rows. Click selects (Shift
 * extends, Ctrl toggles), double-click or Enter plays, drag reorders (the whole selection
 * moves together), right-click opens the row menu. Files dropped from the OS land at the
 * insertion line. Nothing here interrupts playback.
 */
import {
  closestCenter,
  DndContext,
  type DragEndEvent,
  PointerSensor,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import { SortableContext, useSortable, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { useVirtualizer } from "@tanstack/react-virtual";
import {
  FolderOpen,
  ListPlus,
  MoreHorizontal,
  Play,
  Repeat,
  Repeat1,
  Shuffle,
  Trash2,
  TriangleAlert,
  X,
} from "lucide-react";
import { type KeyboardEvent, type MouseEvent, useEffect, useRef, useState } from "react";
import { removeWithUndo } from "@/actions/definitions/queue";
import { focusPlayerRoot } from "@/actions/definitions/window";
import { IconButton } from "@/components/IconButton";
import { action, DropdownMenu, MenuContent, type MenuEntry, separator } from "@/components/Menu";
import type { QueueItem } from "@/ipc/bindings/QueueItem";
import { appReveal, queueMove, queuePlay } from "@/ipc/commands";
import { formatTime } from "@/lib/time";
import { reportError } from "@/stores/osd";
import { usePlayer } from "@/stores/player";
import { formatTotal, useQueueStore } from "@/stores/queue";
import { useUiStore } from "@/stores/ui";

const ROW = 32;

/** Three bars that move while playing and rest while paused (DESIGN_SYSTEM §8.11). */
function LevelGlyph({ playing }: { playing: boolean }) {
  return (
    <span aria-hidden="true" className="flex h-3 w-3 items-end gap-px">
      {[0, 1, 2].map((i) => (
        <span
          key={i}
          className={`w-[3px] origin-bottom bg-accent ${playing ? "animate-level" : ""}`}
          style={{ height: `${[60, 100, 40][i]}%`, animationDelay: `${i * 140}ms` }}
        />
      ))}
    </span>
  );
}

function Row({
  item,
  start,
  current,
  selected,
  playing,
  onSelect,
  onPlay,
  onMenu,
  onRemove,
}: {
  item: QueueItem;
  start: number;
  current: boolean;
  selected: boolean;
  playing: boolean;
  onSelect: (e: MouseEvent) => void;
  onPlay: () => void;
  onMenu: (e: MouseEvent) => void;
  onRemove: () => void;
}) {
  const { attributes, listeners, setNodeRef, transform, isDragging } = useSortable({ id: item.id });
  const dragShift = transform ? ` ${CSS.Translate.toString({ ...transform, x: 0 })}` : "";
  return (
    // Keyboard selection lives on the listbox (arrows, Enter, Delete), not on each row.
    // biome-ignore lint/a11y/useKeyWithClickEvents: rows are options of a keyboard-driven listbox
    <div
      ref={setNodeRef}
      {...attributes}
      {...listeners}
      role="option"
      aria-selected={selected}
      tabIndex={-1}
      title={item.error ?? item.source}
      onClick={onSelect}
      onDoubleClick={onPlay}
      onContextMenu={onMenu}
      style={{ transform: `translateY(${start}px)${dragShift}` }}
      className={`group absolute inset-x-0 top-0 flex h-8 cursor-default items-center gap-2 border-l-2 pr-1 pl-2 ${
        selected ? "border-accent bg-accent-soft" : "border-transparent hover:bg-hover"
      } ${isDragging ? "z-10 bg-raised shadow-float" : ""}`}
    >
      <span className="flex w-3 shrink-0 justify-center">
        {current && <LevelGlyph playing={playing} />}
        {!current && item.error && (
          <TriangleAlert
            size={12}
            strokeWidth={1.5}
            className="text-warning"
            aria-label="Can't play"
          />
        )}
      </span>
      <span
        className={`min-w-0 flex-1 truncate ${
          current ? "font-medium text-accent-fg" : item.error ? "text-fg-3" : "text-fg"
        }`}
      >
        {item.title}
      </span>
      {item.origin === "folder" && (
        <span className="shrink-0 rounded-control bg-active px-1 text-fg-2 text-xs">folder</span>
      )}
      <span className="tabular shrink-0 text-fg-2 text-sm">
        {item.duration ? formatTime(item.duration) : ""}
      </span>
      <button
        type="button"
        aria-label={`Remove ${item.title}`}
        onPointerDown={(e) => e.stopPropagation()}
        onClick={(e) => {
          e.stopPropagation();
          onRemove();
        }}
        className="shrink-0 rounded-control p-1 text-fg-2 opacity-0 hover:bg-active hover:text-fg focus-visible:opacity-100 group-hover:opacity-100"
      >
        <X size={14} strokeWidth={1.5} aria-hidden />
      </button>
    </div>
  );
}

export function QueuePanel() {
  const queue = useQueueStore((s) => s.queue);
  const selected = useQueueStore((s) => s.selected);
  const select = useQueueStore((s) => s.select);
  const playing = usePlayer((s) => s.status === "playing") ?? false;
  const setDock = useUiStore((s) => s.setDock);
  const setDropResolver = useUiStore((s) => s.setDropResolver);
  const dropIndex = useUiStore((s) => s.dropIndex);
  const hold = useUiStore((s) => s.hold);
  const scroller = useRef<HTMLDivElement>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; item: QueueItem } | null>(null);
  const [moreOpen, setMoreOpen] = useState(false);

  const items = queue?.items ?? [];
  const ids = items.map((i) => i.id);
  const virtual = useVirtualizer({
    count: items.length,
    getScrollElement: () => scroller.current,
    estimateSize: () => ROW,
    overscan: 8,
  });
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 4 } }));

  // Keep the playing item in view when the queue opens or the track changes.
  const currentIndex = items.findIndex((i) => i.id === queue?.currentId);
  useEffect(() => {
    if (currentIndex >= 0) virtual.scrollToIndex(currentIndex, { align: "auto" });
  }, [currentIndex, virtual]);

  // Files dropped from the OS onto the list land at the row under the pointer.
  useEffect(() => {
    setDropResolver((x, y) => {
      const el = scroller.current;
      if (!el) return null;
      const r = el.getBoundingClientRect();
      if (x < r.left || x > r.right || y < r.top || y > r.bottom) return null;
      const index = Math.round((y - r.top + el.scrollTop) / ROW);
      return Math.max(0, Math.min(items.length, index));
    });
    return () => setDropResolver(null);
  }, [setDropResolver, items.length]);

  const play = (item: QueueItem) => void queuePlay(item.id).catch(reportError);

  const onSelect = (item: QueueItem) => (e: MouseEvent) => {
    select(item.id, e.shiftKey ? "range" : e.ctrlKey || e.metaKey ? "toggle" : "replace");
    scroller.current?.focus({ preventScroll: true });
  };

  const onDragEnd = ({ active, over }: DragEndEvent) => {
    if (!over || active.id === over.id) return;
    const activeId = String(active.id);
    const overId = String(over.id);
    const moving = selected.includes(activeId)
      ? ids.filter((id) => selected.includes(id))
      : [activeId];
    if (moving.includes(overId)) return;
    const rest = ids.filter((id) => !moving.includes(id));
    const overIndex = rest.indexOf(overId);
    const downward = ids.indexOf(activeId) < ids.indexOf(overId);
    void queueMove(moving, downward ? overIndex + 1 : overIndex).catch(reportError);
  };

  // List keyboard: arrows move the selection (Shift extends), Home/End jump. Enter, Delete,
  // Alt+arrows, and Ctrl+A are queue-context actions in the keymap.
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.altKey || e.ctrlKey || e.metaKey) return;
    const { anchor } = useQueueStore.getState();
    const at = Math.max(0, ids.indexOf(selected[selected.length - 1] ?? anchor ?? ""));
    let next: number | null = null;
    if (e.key === "ArrowDown") next = Math.min(ids.length - 1, at + 1);
    else if (e.key === "ArrowUp") next = Math.max(0, at - 1);
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = ids.length - 1;
    const id = next !== null ? ids[next] : undefined;
    if (!id) return;
    e.preventDefault();
    e.stopPropagation();
    select(id, e.shiftKey ? "range" : "replace");
    virtual.scrollToIndex(next ?? 0, { align: "auto" });
  };

  const rowMenu = (item: QueueItem): MenuEntry[] => {
    const currentIdx = items.findIndex((i) => i.id === queue?.currentId);
    return [
      { kind: "item", label: "Play", icon: Play, onSelect: () => play(item) },
      {
        kind: "item",
        label: "Play next",
        icon: ListPlus,
        disabled: currentIdx < 0 || item.id === queue?.currentId,
        onSelect: () => {
          const rest = ids.filter((id) => id !== item.id);
          const at = rest.indexOf(queue?.currentId ?? "") + 1;
          void queueMove([item.id], at).catch(reportError);
        },
      },
      {
        kind: "item",
        label:
          selected.length > 1 && selected.includes(item.id)
            ? `Remove ${selected.length} items`
            : "Remove",
        icon: Trash2,
        onSelect: () => {
          const chosen = selected.includes(item.id)
            ? items.filter((i) => selected.includes(i.id))
            : [item];
          void removeWithUndo(chosen).catch(reportError);
        },
      },
      {
        kind: "item",
        label: "Show in folder",
        icon: FolderOpen,
        disabled: item.isUrl,
        onSelect: () => void appReveal(item.source).catch(reportError),
      },
      separator,
      { kind: "submenu", label: "Add to playlist", entries: [] },
    ];
  };

  const repeat = queue?.repeat ?? "off";

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex h-9 shrink-0 items-center gap-1 border-line border-b pr-1 pl-3">
        <span className="font-medium text-fg-3 text-xs uppercase tracking-[0.06em]">Queue</span>
        <span className="truncate text-fg-2 text-sm">
          {items.length > 0
            ? [` · ${items.length} items`, formatTotal(items)].filter(Boolean).join(" · ")
            : ""}
        </span>
        <span className="flex-1" />
        <IconButton
          icon={Shuffle}
          label="Shuffle"
          actionId="queue.toggle_shuffle"
          size="panel"
          tooltipSide="bottom"
        />
        <IconButton
          icon={repeat === "one" ? Repeat1 : Repeat}
          label={repeat === "off" ? "Repeat off" : repeat === "all" ? "Repeat all" : "Repeat one"}
          actionId="queue.cycle_repeat"
          size="panel"
          tooltipSide="bottom"
        />
        <DropdownMenu.Root
          open={moreOpen}
          onOpenChange={(open) => {
            setMoreOpen(open);
            hold("menu-queue", open);
          }}
        >
          <DropdownMenu.Trigger asChild>
            <IconButton
              icon={MoreHorizontal}
              label="More"
              size="panel"
              tooltipSide="bottom"
              keepFocus
              onClick={() => {}}
            />
          </DropdownMenu.Trigger>
          {moreOpen && (
            <MenuContent
              align="end"
              entries={[
                action("queue.save_as_playlist"),
                action("queue.stop_after_current"),
                separator,
                action("queue.clear"),
              ]}
              onCloseAutoFocus={(e) => {
                e.preventDefault();
                focusPlayerRoot();
              }}
            />
          )}
        </DropdownMenu.Root>
        <IconButton
          icon={X}
          label="Close"
          size="panel"
          tooltipSide="bottom"
          onClick={() => setDock(null)}
        />
      </div>

      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
        <SortableContext items={ids} strategy={verticalListSortingStrategy}>
          <div
            ref={scroller}
            role="listbox"
            aria-label="Queue"
            aria-multiselectable="true"
            tabIndex={0}
            onKeyDown={onKeyDown}
            className="relative min-h-0 flex-1 overflow-y-auto outline-none focus-visible:outline-2 focus-visible:-outline-offset-2"
          >
            <div className="relative" style={{ height: virtual.getTotalSize() }}>
              {virtual.getVirtualItems().map((row) => {
                const item = items[row.index];
                if (!item) return null;
                return (
                  <Row
                    key={item.id}
                    item={item}
                    start={row.start}
                    current={item.id === queue?.currentId}
                    selected={selected.includes(item.id)}
                    playing={playing}
                    onSelect={onSelect(item)}
                    onPlay={() => play(item)}
                    onMenu={(e) => {
                      e.preventDefault();
                      if (!selected.includes(item.id)) select(item.id, "replace");
                      setMenu({ x: e.clientX, y: e.clientY, item });
                    }}
                    onRemove={() => void removeWithUndo([item]).catch(reportError)}
                  />
                );
              })}
              {dropIndex !== null && (
                <div
                  aria-hidden="true"
                  className="pointer-events-none absolute inset-x-1 h-0.5 bg-accent"
                  style={{ top: dropIndex * ROW - 1 }}
                />
              )}
            </div>
            <p className="px-3 py-3 text-center text-fg-3 text-sm">
              {items.length === 0 ? "The queue is empty. " : ""}Drop files here to add them.
            </p>
          </div>
        </SortableContext>
      </DndContext>

      {menu && (
        <DropdownMenu.Root
          open
          modal={false}
          onOpenChange={(open) => {
            if (!open) setMenu(null);
          }}
        >
          <DropdownMenu.Trigger asChild>
            <div
              aria-hidden="true"
              className="pointer-events-none fixed h-0 w-0"
              style={{ left: menu.x, top: menu.y }}
            />
          </DropdownMenu.Trigger>
          <MenuContent
            entries={rowMenu(menu.item)}
            onCloseAutoFocus={(e) => {
              e.preventDefault();
              scroller.current?.focus({ preventScroll: true });
            }}
          />
        </DropdownMenu.Root>
      )}
    </div>
  );
}
