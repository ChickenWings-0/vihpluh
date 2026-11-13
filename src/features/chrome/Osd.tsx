/**
 * OSD flash and toasts (DESIGN_SYSTEM §8.9). The flash sits at the configured position, 16 px
 * from the top (below the top bar when it is visible). Toasts stack bottom-left above the bar.
 */
import { CircleAlert, CircleCheck, Info, TriangleAlert, X } from "lucide-react";
import { KeyCap } from "@/components/KeyCap";
import { useConfig } from "@/stores/config";
import { type Toast, useOsdStore } from "@/stores/osd";
import { useUiStore } from "@/stores/ui";

export function OsdFlash() {
  const flash = useOsdStore((s) => s.flash);
  const position = useConfig((c) => c.interface.osd_position) ?? "top-center";
  const topVisible = useUiStore((s) => s.topVisible);
  if (!flash) return null;
  const Icon = flash.icon;
  const horizontal =
    position === "top-left"
      ? "left-4"
      : position === "top-right"
        ? "right-4"
        : "left-1/2 -translate-x-1/2";
  const boost = flash.bar !== undefined && flash.bar > 1;
  return (
    <div
      key={flash.id}
      role="status"
      aria-live="polite"
      className={`pointer-events-none absolute z-(--z-osd) flex items-center gap-3 rounded-surface bg-surface/92 px-3 py-2 text-fg ${horizontal} ${
        topVisible ? "top-14" : "top-4"
      }`}
    >
      {Icon && <Icon size={20} strokeWidth={1.5} className="text-fg-2" aria-hidden />}
      <span className="font-semibold text-md">{flash.label}</span>
      {flash.bar !== undefined && (
        <span className="relative h-1.5 w-24 overflow-hidden rounded-control bg-track">
          <span
            className={`absolute inset-y-0 left-0 ${boost ? "bg-danger" : "bg-accent"}`}
            style={{ width: `${Math.min(1, flash.bar) * 100}%` }}
          />
        </span>
      )}
      {flash.value && <span className="tabular font-semibold text-osd">{flash.value}</span>}
      {flash.badge && <KeyCap binding={flash.badge} />}
    </div>
  );
}

const TOAST_ICON = {
  info: { icon: Info, className: "text-fg-2" },
  success: { icon: CircleCheck, className: "text-success" },
  warn: { icon: TriangleAlert, className: "text-warning" },
  error: { icon: CircleAlert, className: "text-danger" },
} as const;

function ToastRow({ toast }: { toast: Toast }) {
  const dismiss = useOsdStore((s) => s.dismissToast);
  const { icon: Icon, className } = TOAST_ICON[toast.kind];
  return (
    <div
      role={toast.kind === "error" ? "alert" : "status"}
      className="pointer-events-auto flex w-80 items-start gap-2 rounded-surface border border-line-strong bg-raised p-2 text-base text-fg shadow-float"
    >
      <Icon size={16} strokeWidth={1.5} className={`mt-0.5 shrink-0 ${className}`} aria-hidden />
      <span className="selectable min-w-0 flex-1 break-words">{toast.text}</span>
      {toast.action && (
        <button
          type="button"
          onClick={() => {
            toast.action?.run();
            dismiss(toast.id);
          }}
          className="shrink-0 rounded-control px-1.5 font-medium text-accent-fg hover:bg-hover"
        >
          {toast.action.label}
        </button>
      )}
      <button
        type="button"
        aria-label="Dismiss"
        onClick={() => dismiss(toast.id)}
        className="shrink-0 rounded-control p-0.5 text-fg-2 hover:bg-hover hover:text-fg"
      >
        <X size={14} strokeWidth={1.5} aria-hidden />
      </button>
    </div>
  );
}

export function Toasts() {
  const toasts = useOsdStore((s) => s.toasts);
  const bottomVisible = useUiStore((s) => s.bottomVisible);
  if (toasts.length === 0) return null;
  return (
    <div
      className={`pointer-events-none absolute left-3 z-(--z-toast) flex flex-col gap-2 ${
        bottomVisible ? "bottom-16" : "bottom-3"
      }`}
    >
      {toasts.map((toast) => (
        <ToastRow key={toast.id} toast={toast} />
      ))}
    </div>
  );
}
