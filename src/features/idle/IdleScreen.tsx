/**
 * Idle screen (DESIGN_SYSTEM §11.3): shown when nothing is loaded. Paints --bg-base (the only
 * full-window surface), centred column of open buttons. Continue watching and recent folders
 * arrive with history in Phase 4. No logo splash, no tips.
 */
import { FileVideo, FolderOpen, Link } from "lucide-react";
import type { ComponentType } from "react";
import { dispatch, isEnabled } from "@/actions";
import { Button } from "@/components/Button";
import { KeyCap } from "@/components/KeyCap";
import { useShortcut } from "@/stores/keymap";
import { usePlayer } from "@/stores/player";

function OpenButton({
  actionId,
  label,
  icon,
  primary,
}: {
  actionId: string;
  label: string;
  icon: ComponentType<{ size?: number; strokeWidth?: number; "aria-hidden"?: boolean }>;
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

export function IdleScreen() {
  const status = usePlayer((s) => s.status);
  const error = usePlayer((s) => s.error);
  // Not while loading: the first frame is moments away and a flash of the screen would show.
  if (status !== "idle" && status !== "error") return null;
  return (
    <div className="pointer-events-none absolute inset-0 flex items-center justify-center bg-base">
      <div className="pointer-events-auto flex max-w-130 flex-col items-center gap-4 px-4 text-center">
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
      </div>
    </div>
  );
}
