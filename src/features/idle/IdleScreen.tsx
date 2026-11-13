/**
 * Idle screen (DESIGN_SYSTEM §11.3): shown when nothing is loaded. Paints --bg-base (the only
 * full-window surface): open buttons, then up to five unfinished files ("Continue watching")
 * and three recent folders from history. With history off, only the open buttons show.
 * No logo splash, no tips. Enter on the first row is the default action.
 */
import { FileVideo, Folder, FolderOpen, Link, Play } from "lucide-react";
import { type ComponentType, useEffect, useState } from "react";
import { dispatch, isEnabled } from "@/actions";
import { Button } from "@/components/Button";
import { KeyCap } from "@/components/KeyCap";
import type { RecentEntry } from "@/ipc/bindings/RecentEntry";
import { libraryRecent, playerOpen } from "@/ipc/commands";
import { formatTime } from "@/lib/time";
import { useConfig } from "@/stores/config";
import { useShortcut } from "@/stores/keymap";
import { reportError } from "@/stores/osd";
import { usePlayer } from "@/stores/player";

type Glyph = ComponentType<{ size?: number; strokeWidth?: number; "aria-hidden"?: boolean }>;

function OpenButton({
  actionId,
  label,
  icon,
  primary,
}: {
  actionId: string;
  label: string;
  icon: Glyph;
  primary?: boolean;
}) {
  const shortcut = useShortcut(actionId);
  const enabled = isEnabled(actionId);
  return (
    <Button
      variant={primary ? "primary" : "secondary"}
      size="lg"
      icon={icon}
      disabled={!enabled}
      title={enabled ? undefined : "Coming in a later update"}
      onClick={() => void dispatch(actionId, { trigger: "button" })}
    >
      {label}
      {shortcut && primary && (
        <KeyCap
          binding={shortcut}
          className="ml-1 border-transparent bg-transparent text-on-accent"
        />
      )}
    </Button>
  );
}

/** Parent folder of a local path, in the platform's own spelling. */
export function folderOf(source: string): string | null {
  const cut = Math.max(source.lastIndexOf("/"), source.lastIndexOf("\\"));
  return cut > 0 ? source.slice(0, cut) : null;
}

/** The core resumes only past the first minute (`library::history::resume_point`). */
const RESUME_AFTER_S = 60;

/** Unfinished files, most recent first, and the distinct folders recent files live in. */
export function splitRecent(entries: RecentEntry[]): { resume: RecentEntry[]; folders: string[] } {
  const resume = entries.filter((e) => !e.finished && e.position > RESUME_AFTER_S).slice(0, 5);
  const folders: string[] = [];
  for (const e of entries) {
    if (/^[a-z][a-z0-9+.-]+:\/\//i.test(e.source)) continue;
    const folder = folderOf(e.source);
    if (folder && !folders.includes(folder)) folders.push(folder);
    if (folders.length === 3) break;
  }
  return { resume, folders };
}

function Eyebrow({ children }: { children: string }) {
  return (
    <h2 className="px-2 pt-2 text-left font-medium text-fg-3 text-xs uppercase tracking-[0.06em]">
      {children}
    </h2>
  );
}

function HistoryRow({
  icon: Icon,
  title,
  detail,
  progress,
  onOpen,
  autoFocus,
}: {
  icon: Glyph;
  title: string;
  detail?: string;
  progress?: number;
  onOpen: () => void;
  autoFocus?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onOpen}
      // biome-ignore lint/a11y/noAutofocus: the first row is the screen's default action (§11.3)
      autoFocus={autoFocus}
      className="relative flex h-11 w-full items-center gap-3 rounded-control px-2 text-left hover:bg-hover"
    >
      <Icon size={16} strokeWidth={1.5} aria-hidden />
      <span className="min-w-0 flex-1 truncate text-base text-fg">{title}</span>
      {detail && <span className="tabular shrink-0 text-fg-2 text-sm">{detail}</span>}
      {progress !== undefined && (
        <span className="absolute right-2 bottom-1 left-9 h-0.5 overflow-hidden rounded-control bg-track">
          <span
            className="block h-full bg-accent"
            style={{ width: `${Math.min(1, progress) * 100}%` }}
          />
        </span>
      )}
    </button>
  );
}

export function IdleScreen() {
  const status = usePlayer((s) => s.status);
  const error = usePlayer((s) => s.error);
  const historyOn = useConfig((c) => c.library.track_history) ?? true;
  const [recent, setRecent] = useState<RecentEntry[]>([]);
  const visible = status === "idle" || status === "error";

  useEffect(() => {
    if (!visible || !historyOn) return;
    let cancelled = false;
    libraryRecent(50)
      .then((rows) => {
        if (!cancelled) setRecent(rows);
      })
      .catch(reportError);
    return () => {
      cancelled = true;
    };
  }, [visible, historyOn]);

  // Not while loading: the first frame is moments away and a flash of the screen would show.
  if (!visible) return null;
  const { resume, folders } = historyOn ? splitRecent(recent) : { resume: [], folders: [] };
  const open = (source: string) =>
    void playerOpen({ sources: [source], mode: "replace" }).catch(reportError);

  return (
    <div className="pointer-events-none absolute inset-0 flex items-center justify-center bg-base">
      <div className="pointer-events-auto flex w-full max-w-130 flex-col items-center gap-4 px-4 text-center">
        {status === "error" && error && (
          <p role="alert" className="text-base text-danger">
            Can't play this file: {error}
          </p>
        )}
        <p className="text-fg-2 text-lg">Drop files here, or</p>
        <div className="flex flex-wrap items-center justify-center gap-2">
          <OpenButton actionId="app.open_file" label="Open file…" icon={FileVideo} primary />
          <OpenButton actionId="app.open_folder" label="Open folder…" icon={FolderOpen} />
          <OpenButton actionId="app.open_url" label="Open URL…" icon={Link} />
        </div>
        {(resume.length > 0 || folders.length > 0) && (
          <div className="flex w-full flex-col gap-1 pt-4">
            {resume.length > 0 && <Eyebrow>Continue watching</Eyebrow>}
            {resume.map((entry, i) => (
              <HistoryRow
                key={entry.mediaKey}
                icon={Play}
                title={entry.title}
                detail={
                  entry.duration
                    ? `${formatTime(entry.position)} / ${formatTime(entry.duration)}`
                    : formatTime(entry.position)
                }
                progress={entry.duration ? entry.position / entry.duration : undefined}
                onOpen={() => open(entry.source)}
                autoFocus={i === 0}
              />
            ))}
            {folders.length > 0 && <Eyebrow>Recent folders</Eyebrow>}
            {folders.map((folder, i) => (
              <HistoryRow
                key={folder}
                icon={Folder}
                title={folder}
                onOpen={() => open(folder)}
                autoFocus={resume.length === 0 && i === 0}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
