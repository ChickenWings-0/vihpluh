/**
 * The sidebar (DESIGN_SYSTEM §10): a 36 px tab row, then the active tab's body, which
 * scrolls. Labels hide below 340 px of dock width. The last tab is remembered in the session.
 * Subtitles and Bookmarks join the row in their own phases.
 */
import { AudioLines, SlidersHorizontal, Video, X } from "lucide-react";
import { type KeyboardEvent, useRef } from "react";
import { IconButton } from "@/components/IconButton";
import { Tooltip } from "@/components/Tooltip";
import { useShortcut } from "@/stores/keymap";
import { SIDEBAR_TABS, type SidebarTab, useUiStore } from "@/stores/ui";
import { AudioTab } from "./AudioTab";
import { PlaybackTab } from "./PlaybackTab";
import { VideoTab } from "./VideoTab";

const TABS: Record<SidebarTab, { label: string; icon: typeof Video; actionId: string }> = {
  audio: { label: "Audio", icon: AudioLines, actionId: "ui.sidebar_audio" },
  video: { label: "Video", icon: Video, actionId: "ui.sidebar_video" },
  playback: { label: "Playback", icon: SlidersHorizontal, actionId: "ui.sidebar_playback" },
};

function Tab({ tab, active, compact }: { tab: SidebarTab; active: boolean; compact: boolean }) {
  const { label, icon: Icon, actionId } = TABS[tab];
  const shortcut = useShortcut(actionId);
  const show = useUiStore((s) => s.showSidebarTab);
  return (
    <Tooltip label={label} shortcut={shortcut ?? undefined} side="bottom">
      <button
        type="button"
        role="tab"
        id={`sidebar-tab-${tab}`}
        aria-selected={active}
        aria-controls="sidebar-panel"
        aria-label={label}
        tabIndex={active ? 0 : -1}
        onMouseDown={(e) => e.preventDefault()}
        onClick={() => show(tab)}
        className={`flex h-9 min-w-0 flex-1 items-center justify-center gap-1.5 border-b-2 px-2 text-base ${
          active
            ? "border-accent bg-accent-soft text-fg"
            : "border-transparent text-fg-2 hover:bg-hover hover:text-fg"
        }`}
      >
        <Icon size={16} strokeWidth={1.5} aria-hidden />
        {!compact && <span className="truncate">{label}</span>}
      </button>
    </Tooltip>
  );
}

export function Sidebar({ width }: { width: number }) {
  const tab = useUiStore((s) => s.sidebarTab);
  const show = useUiStore((s) => s.showSidebarTab);
  const setDock = useUiStore((s) => s.setDock);
  const row = useRef<HTMLDivElement>(null);
  const compact = width < 340;

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const dir = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
    if (dir === 0) return;
    e.preventDefault();
    e.stopPropagation();
    const index = SIDEBAR_TABS.indexOf(tab);
    const next = SIDEBAR_TABS[(index + dir + SIDEBAR_TABS.length) % SIDEBAR_TABS.length];
    if (!next) return;
    show(next);
    requestAnimationFrame(() => document.getElementById(`sidebar-tab-${next}`)?.focus());
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex shrink-0 items-center border-line border-b pr-1">
        <div
          ref={row}
          role="tablist"
          aria-label="Sidebar"
          onKeyDown={onKeyDown}
          className="flex min-w-0 flex-1"
        >
          {SIDEBAR_TABS.map((t) => (
            <Tab key={t} tab={t} active={t === tab} compact={compact} />
          ))}
        </div>
        <IconButton
          icon={X}
          label="Close"
          size="panel"
          tooltipSide="bottom"
          onClick={() => setDock(null)}
        />
      </div>
      <div
        id="sidebar-panel"
        role="tabpanel"
        aria-labelledby={`sidebar-tab-${tab}`}
        className="min-h-0 flex-1 overflow-y-auto overscroll-contain"
      >
        <div className="flex flex-col gap-4 p-3">
          {tab === "audio" && <AudioTab />}
          {tab === "video" && <VideoTab />}
          {tab === "playback" && <PlaybackTab />}
        </div>
      </div>
    </div>
  );
}
