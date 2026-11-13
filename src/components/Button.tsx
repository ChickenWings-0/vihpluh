/**
 * Text buttons (DESIGN_SYSTEM §8.4): ghost, secondary, primary (at most one per view), and
 * danger. Radius 4, 8–12 px padding, text-base weight 500, 6 px icon–label gap. A pointer click
 * returns focus to the player root (§13) unless the button opens something that takes focus.
 */
import { type ButtonHTMLAttributes, type ComponentType, forwardRef, type MouseEvent } from "react";
import { focusPlayerRoot } from "@/actions/definitions/window";

export type ButtonVariant = "ghost" | "secondary" | "primary" | "danger";
export type ButtonSize = "sm" | "md" | "lg";

const VARIANT: Record<ButtonVariant, string> = {
  ghost: "text-fg enabled:hover:bg-hover enabled:active:bg-active",
  secondary: "border border-line bg-raised text-fg enabled:hover:border-line-strong",
  primary: "bg-accent text-on-accent enabled:hover:bg-accent-hover enabled:active:bg-accent-active",
  danger: "text-danger enabled:hover:bg-danger-soft",
};

const SIZE: Record<ButtonSize, string> = { sm: "h-6 px-2", md: "h-7 px-2.5", lg: "h-8 px-3" };

interface Props extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  icon?: ComponentType<{ size?: number; strokeWidth?: number; "aria-hidden"?: boolean }>;
  keepFocus?: boolean;
}

export const Button = forwardRef<HTMLButtonElement, Props>(function Button(
  {
    variant = "secondary",
    size = "md",
    icon: Icon,
    keepFocus,
    className = "",
    onClick,
    children,
    ...rest
  },
  ref,
) {
  return (
    <button
      ref={ref}
      type="button"
      onClick={(e: MouseEvent<HTMLButtonElement>) => {
        onClick?.(e);
        if (e.detail > 0 && !keepFocus) focusPlayerRoot();
      }}
      className={`inline-flex shrink-0 items-center gap-1.5 rounded-control font-medium text-base transition-colors duration-(--dur-instant) disabled:text-fg-off ${SIZE[size]} ${VARIANT[variant]} ${className}`}
      {...rest}
    >
      {Icon && <Icon size={16} strokeWidth={1.5} aria-hidden />}
      {children}
    </button>
  );
});
