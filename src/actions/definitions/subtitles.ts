/** Subtitle actions that arrived with the Subtitles tab (KEYBINDS_AND_CONFIG §2, Phase 7). */
import {
  ArrowDown,
  ArrowUp,
  Captions,
  FilePlus,
  Link2,
  SkipBack,
  SkipForward,
  Type,
} from "lucide-react";
import { appPickMedia, subSetStyle, subStep, tracksAddExternal } from "@/ipc/commands";
import { useAutomatchStore } from "@/stores/automatch";
import { useUiStore } from "@/stores/ui";
import { clamp, config, hasMedia, player, round } from "../helpers";
import type { Action, Feedback } from "../types";

const hasSubtitle = () => (player()?.selected.sub ?? null) !== null;
const isLocal = () => player()?.media?.isUrl === false;

async function syncToCue(direction: 1 | -1): Promise<Feedback> {
  await subStep(direction);
  return {
    icon: direction === 1 ? SkipForward : SkipBack,
    label: direction === 1 ? "Next line shows now" : "Previous line shows now",
  };
}

async function setScale(delta: number): Promise<Feedback> {
  const scale = round(clamp((player()?.subStyle.scale ?? 1) + delta, 0.5, 3));
  await subSetStyle({ scale });
  return { icon: Type, label: "Subtitle size", value: `${Math.round(scale * 100)} %` };
}

async function setPosition(delta: number): Promise<Feedback> {
  const position = clamp(Math.round((player()?.subStyle.position ?? 100) + delta), 0, 100);
  await subSetStyle({ position });
  return {
    icon: delta < 0 ? ArrowUp : ArrowDown,
    label: "Subtitle position",
    value: `${position} %`,
  };
}

const sizeStep = () => config()?.controls.sub_size_step ?? 0.1;
/** Per key press, in percent of the picture height. */
const POSITION_STEP = 2;

export const subtitleActions: Action[] = [
  {
    id: "subtitles.sync_previous_cue",
    label: "Show the previous line now",
    group: "subtitles",
    icon: SkipBack,
    isEnabled: hasSubtitle,
    run: () => syncToCue(-1),
  },
  {
    id: "subtitles.sync_next_cue",
    label: "Show the next line now",
    group: "subtitles",
    icon: SkipForward,
    isEnabled: hasSubtitle,
    run: () => syncToCue(1),
  },
  {
    id: "subtitles.size_decrease",
    label: "Subtitle size −",
    group: "subtitles",
    icon: Type,
    repeatable: true,
    isEnabled: hasMedia,
    run: () => setScale(-sizeStep()),
  },
  {
    id: "subtitles.size_increase",
    label: "Subtitle size +",
    group: "subtitles",
    icon: Type,
    repeatable: true,
    isEnabled: hasMedia,
    run: () => setScale(sizeStep()),
  },
  {
    id: "subtitles.position_up",
    label: "Move subtitles up",
    group: "subtitles",
    icon: ArrowUp,
    repeatable: true,
    isEnabled: hasMedia,
    run: () => setPosition(-POSITION_STEP),
  },
  {
    id: "subtitles.position_down",
    label: "Move subtitles down",
    group: "subtitles",
    icon: ArrowDown,
    repeatable: true,
    isEnabled: hasMedia,
    run: () => setPosition(POSITION_STEP),
  },
  {
    id: "subtitles.add_file",
    label: "Add subtitle file…",
    group: "subtitles",
    icon: FilePlus,
    isEnabled: hasMedia,
    run: async () => {
      const paths = await appPickMedia("subtitles");
      let select = true;
      for (const path of paths) {
        await tracksAddExternal("sub", path, select);
        select = false;
      }
      return paths.length > 0 ? { icon: Captions, label: "Subtitle file added" } : undefined;
    },
  },
  {
    id: "subtitles.automatch",
    label: "Auto-match subtitles in folder",
    group: "subtitles",
    icon: Link2,
    isEnabled: () => hasMedia() && isLocal(),
    run: async () => {
      // The result card lives in the Subtitles tab; nothing is saved until Apply.
      useUiStore.getState().showSidebarTab("subtitles");
      await useAutomatchStore.getState().run(false);
      return undefined;
    },
  },
];
