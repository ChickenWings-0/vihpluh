/**
 * Icon button (DESIGN_SYSTEM §8.4): no background, secondary icon; hover square; accent icon
 * when its state is on. Bound to an action, it dispatches that action, follows its enabled
 * state, and shows its live shortcut in the tooltip.
 *
 * Focus model (§13): a pointer click never leaves focus on the button — focus returns to the
 * player root, so Space after clicking Mute still toggles playback. Keyboard activation keeps
 * focus where it is.
 */
import { type ComponentType, forwardRef, type MouseEvent } from "react";
import { dispatch, getAction, isEnabled } from "@/actions";
import { focusPlayerRoot } from "@/actions/definitions/window";
import { useShortcut } from "@/stores/keymap";
import { usePlayerStore } from "@/stores/player";
import { useUiStore } from "@/stores/ui";
import { Tooltip } from "./Tooltip";

export type IconButtonSize = "bar" | "play" | "panel" | "window";

const SIZE: Record<IconButtonSize, { box: string; icon: number }> = {
  bar: { box: "h-8 w-8", icon: 18 },
  play: { box: "h-8 w-9", icon: 20 },
  panel: { box: "h-7 w-7", icon: 16 },
  window: { box: "h-10 w-11.5", icon: 16 },
};

/** Stock lucide icons and vihpluh's own glyphs share this shape. */
export type IconComponent = ComponentType<{
  size?: number;
  strokeWidth?: number;
  fill?: string;
  "aria-hidden"?: boolean | "true" | "false";
}>;

interface Props {
  icon: IconComponent;
  label: string;
  /** Dispatches this action on click (and takes label/shortcut/enabled state from it). */
  actionId?: string;
  onClick?: (e: MouseEvent<HTMLButtonElement>) => void;
  on?: boolean;
  disabled?: boolean;
  size?: IconButtonSize;
  /** Solid glyphs read better at speed for transport (DESIGN_SYSTEM §5.1). */
  solid?: boolean;
  tooltipSide?: "top" | "bottom";
  className?: string;
  /** Close-button styling: danger fill on hover. */
  danger?: boolean;
  /**
   * Keep focus after a pointer click. For menu and popover triggers: their content takes
   * focus, and moving focus away would close it.
   */
  keepFocus?: boolean;
}

/** Re-renders a component when anything that can change an action's state changes. */
function useActionTick(enabled: boolean) {
  usePlayerStore((s) => (enabled ? s.state?.rev : 0));
  useUiStore((s) => (enabled ? s.window : null));
  useUiStore((s) => (enabled ? s.zen : null));
}

export const IconButton = forwardRef<HTMLButtonElement, Props>(function IconButton(
  {
    icon: Icon,
    label,
    actionId,
    onClick,
    on,
    disabled,
    size = "bar",
    solid,
    tooltipSide = "top",
    className = "",
    danger,
    keepFocus,
    ...rest
  },
  ref,
) {
  useActionTick(Boolean(actionId));
  const shortcut = useShortcut(actionId ?? "");
  const action = actionId ? getAction(actionId) : undefined;
  const enabled = !disabled && (actionId ? isEnabled(actionId) : true);
  const active = on ?? action?.isOn?.() ?? false;
  const { box, icon } = SIZE[size];

  const handleClick = (e: MouseEvent<HTMLButtonElement>) => {
    if (onClick) onClick(e);
    else if (actionId) void dispatch(actionId, { trigger: "button" });
    // detail > 0: a pointer click (keyboard activation reports 0).
    if (e.detail > 0 && !keepFocus) focusPlayerRoot();
  };

  const glyph = (
    <Icon size={icon} strokeWidth={1.5} fill={solid ? "currentColor" : "none"} aria-hidden="true" />
  );

  return (
    <Tooltip
      label={action?.comingIn ? `${label} — coming in ${action.comingIn}` : label}
      shortcut={shortcut}
      side={tooltipSide}
    >
      <button
        ref={ref}
        type="button"
        aria-label={label}
        aria-pressed={on !== undefined || action?.isOn ? active : undefined}
        disabled={!enabled}
        onClick={handleClick}
        className={`inline-flex ${box} shrink-0 items-center justify-center rounded-control transition-colors duration-(--dur-instant) disabled:text-fg-off ${
          active ? "text-accent-fg" : "text-fg-2"
        } ${
          danger
            ? "enabled:hover:bg-danger enabled:hover:text-fg"
            : "enabled:hover:bg-hover enabled:hover:text-fg enabled:active:bg-active"
        } ${className}`}
        {...rest}
      >
        {glyph}
      </button>
    </Tooltip>
  );
});
