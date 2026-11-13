/**
 * Places the two bars over the video and animates them (DESIGN_SYSTEM §6.1, §7.1): translate
 * 8 px + fade, 120 ms in / 200 ms out. Pointer over a bar, or keyboard focus inside one, holds
 * the chrome visible.
 */
import type { FocusEvent, ReactNode } from "react";
import { useUiStore } from "@/stores/ui";
import { BottomBar } from "./BottomBar";
import { TopBar } from "./TopBar";

function Bar({
  edge,
  visible,
  children,
}: {
  edge: "top" | "bottom";
  visible: boolean;
  children: ReactNode;
}) {
  const hold = useUiStore((s) => s.hold);
  const pointerReason = `pointer-${edge}`;
  const focusReason = `focus-${edge}`;
  const onFocus = (e: FocusEvent<HTMLDivElement>) => {
    if (e.target.matches(":focus-visible")) hold(focusReason, true);
  };
  const onBlur = (e: FocusEvent<HTMLDivElement>) => {
    if (!e.currentTarget.contains(e.relatedTarget as Node | null)) hold(focusReason, false);
  };
  const hiddenShift = edge === "top" ? "-translate-y-2" : "translate-y-2";
  return (
    <div
      role="toolbar"
      aria-label={edge === "top" ? "Window" : "Playback"}
      onPointerEnter={() => hold(pointerReason, true)}
      onPointerLeave={() => hold(pointerReason, false)}
      onFocus={onFocus}
      onBlur={onBlur}
      className={`absolute inset-x-0 z-(--z-chrome) bg-surface/92 transition-[opacity,translate] ${
        edge === "top" ? "top-0 border-line border-b" : "bottom-0 border-line border-t"
      } ${
        visible
          ? "translate-y-0 opacity-100 duration-(--dur-fast) ease-out"
          : `pointer-events-none ${hiddenShift} opacity-0 duration-(--dur-slow) ease-in`
      }`}
    >
      {children}
    </div>
  );
}

export function Chrome() {
  const top = useUiStore((s) => s.topVisible);
  const bottom = useUiStore((s) => s.bottomVisible);
  return (
    <>
      <Bar edge="top" visible={top}>
        <TopBar />
      </Bar>
      <Bar edge="bottom" visible={bottom}>
        <BottomBar />
      </Bar>
    </>
  );
}
