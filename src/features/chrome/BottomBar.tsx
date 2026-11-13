/**
 * Bottom bar (DESIGN_SYSTEM §7.4): one 52 px row. Transport left-aligned (not centred),
 * elapsed, the seek bar taking all remaining width, total/remaining (click to switch),
 * volume, track buttons, panel toggles, fullscreen. Collapses by width per §6.4.
 */
import {
  AudioLines,
  Captions,
  ChevronUp,
  Expand,
  Layers,
  Pause,
  Play,
  Shrink,
  SkipBack,
  SkipForward,
} from "lucide-react";
import { useRef } from "react";
import { IconButton } from "@/components/IconButton";
import { Popover } from "@/components/Popover";
import { QueueGlyph, SidebarRightGlyph } from "@/icons/mono";
import { configSet } from "@/ipc/commands";
import { formatTime } from "@/lib/time";
import { useAnimationFrame } from "@/lib/useAnimationFrame";
import { collapseLevel, useWindowWidth } from "@/lib/useWindowWidth";
import { useConfig } from "@/stores/config";
import { reportError } from "@/stores/osd";
import { usePlayer } from "@/stores/player";
import { getDuration, getPosition } from "@/stores/time";
import { useUiStore } from "@/stores/ui";
import { SeekBar } from "./SeekBar";
import { MuteButton, VolumeCone } from "./VolumeCone";

function Elapsed() {
  const el = useRef<HTMLSpanElement>(null);
  useAnimationFrame(() => {
    if (el.current) el.current.textContent = formatTime(getPosition());
  });
  return (
    <span
      ref={el}
      role="timer"
      aria-label="Elapsed"
      className="tabular shrink-0 text-base text-fg"
    />
  );
}

function TotalOrRemaining() {
  const el = useRef<HTMLButtonElement>(null);
  const mode = useConfig((c) => c.interface.time_display) ?? "total";
  useAnimationFrame(() => {
    if (!el.current) return;
    const dur = getDuration();
    el.current.textContent =
      mode === "remaining" ? `−${formatTime(dur - getPosition())}` : formatTime(dur);
  });
  return (
    <button
      ref={el}
      type="button"
      aria-label={
        mode === "remaining" ? "Time left (click for total)" : "Length (click for time left)"
      }
      onClick={(e) => {
        const next = mode === "remaining" ? "total" : "remaining";
        void configSet("interface.time_display", next).catch(reportError);
        if (e.detail > 0) document.getElementById("player-root")?.focus();
      }}
      className="tabular shrink-0 rounded-control px-1 text-base text-fg-2 hover:text-fg"
    />
  );
}

function PlayPause() {
  const playing = usePlayer((s) => s.status === "playing") ?? false;
  return (
    <IconButton
      icon={playing ? Pause : Play}
      label={playing ? "Pause" : "Play"}
      actionId="player.toggle_pause"
      size="play"
      solid
      on={false}
    />
  );
}

export function BottomBar() {
  const level = collapseLevel(useWindowWidth());
  const fullscreen = useUiStore((s) => s.window.mode === "fullscreen");

  return (
    <div className="flex h-13 w-full items-center gap-3 px-2">
      <div className="flex shrink-0 items-center gap-1">
        {level < 2 && (
          <IconButton icon={SkipBack} label="Previous" actionId="queue.previous" solid />
        )}
        <PlayPause />
        {level < 2 && <IconButton icon={SkipForward} label="Next" actionId="queue.next" solid />}
      </div>

      <Elapsed />
      <SeekBar />
      {level < 1 && <TotalOrRemaining />}

      <div className="flex shrink-0 items-center gap-1">
        <MuteButton />
        {level < 1 ? (
          <VolumeCone />
        ) : (
          <Popover
            side="top"
            width="w-auto"
            trigger={
              <button
                type="button"
                aria-label="Volume slider"
                className="flex h-8 w-4 items-center justify-center rounded-control text-fg-2 hover:bg-hover hover:text-fg"
              >
                <ChevronUp size={14} strokeWidth={1.5} aria-hidden />
              </button>
            }
          >
            <div className="px-2 py-1">
              <VolumeCone />
            </div>
          </Popover>
        )}
      </div>

      <div className="flex shrink-0 items-center gap-1">
        {level < 2 ? (
          <>
            <IconButton icon={Captions} label="Subtitles" actionId="ui.sidebar_subtitles" />
            <IconButton icon={AudioLines} label="Audio" actionId="ui.sidebar_audio" />
          </>
        ) : (
          <IconButton icon={Layers} label="Tracks" actionId="ui.sidebar_audio" />
        )}
      </div>

      {level < 3 && (
        <div className="flex shrink-0 items-center gap-1">
          <IconButton icon={QueueGlyph} label="Queue" actionId="queue.toggle_panel" />
          <IconButton icon={SidebarRightGlyph} label="Sidebar" actionId="ui.toggle_sidebar" />
        </div>
      )}

      <IconButton
        icon={fullscreen ? Shrink : Expand}
        label={fullscreen ? "Exit fullscreen" : "Fullscreen"}
        actionId="window.toggle_fullscreen"
        on={false}
      />
    </div>
  );
}
