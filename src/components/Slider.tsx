/**
 * A plain horizontal slider (DESIGN_SYSTEM §8.5 track and thumb): 4 px track, fill from the
 * zero point for bipolar ranges, 12 × 16 thumb, keyboard stepping (Shift ×10, Alt ÷10),
 * double-click to reset, magnetic detent at the default. The full parameter row (label, value
 * field, reset button) is built on this in Phase 5.
 */
import { type KeyboardEvent, type PointerEvent, useRef } from "react";
import { useUiStore } from "@/stores/ui";

export interface SliderProps {
  value: number;
  min: number;
  max: number;
  step: number;
  /** Detent and reset value; the fill starts here when it lies inside the range. */
  defaultValue?: number;
  onChange: (value: number) => void;
  /** Called once when a drag or key press finishes. */
  onCommit?: (value: number) => void;
  label: string;
  valueText?: string;
  className?: string;
}

const DETENT_PX = 6;

export function Slider({
  value,
  min,
  max,
  step,
  defaultValue,
  onChange,
  onCommit,
  label,
  valueText,
  className = "",
}: SliderProps) {
  const track = useRef<HTMLDivElement>(null);
  const hold = useUiStore((s) => s.hold);
  const clamp = (v: number) => Math.min(max, Math.max(min, v));
  const snap = (v: number) => Math.round(v / step) * step;
  const fraction = (v: number) => (max === min ? 0 : (clamp(v) - min) / (max - min));

  const fromPointer = (clientX: number) => {
    const rect = track.current?.getBoundingClientRect();
    if (!rect || rect.width === 0) return value;
    let next = min + ((clientX - rect.left) / rect.width) * (max - min);
    if (defaultValue !== undefined) {
      const px = Math.abs(fraction(next) - fraction(defaultValue)) * rect.width;
      if (px <= DETENT_PX) next = defaultValue;
    }
    return clamp(snap(next));
  };

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    hold("drag", true);
    onChange(fromPointer(e.clientX));
  };
  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    if (e.currentTarget.hasPointerCapture(e.pointerId)) onChange(fromPointer(e.clientX));
  };
  const onPointerUp = (e: PointerEvent<HTMLDivElement>) => {
    if (!e.currentTarget.hasPointerCapture(e.pointerId)) return;
    e.currentTarget.releasePointerCapture(e.pointerId);
    hold("drag", false);
    onCommit?.(fromPointer(e.clientX));
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const factor = e.shiftKey ? 10 : e.altKey ? 0.1 : 1;
    let next: number | null = null;
    if (e.key === "ArrowRight" || e.key === "ArrowUp") next = value + step * factor;
    else if (e.key === "ArrowLeft" || e.key === "ArrowDown") next = value - step * factor;
    else if (e.key === "Home") next = min;
    else if (e.key === "End") next = max;
    if (next === null) return;
    e.preventDefault();
    e.stopPropagation();
    const v = clamp(next);
    onChange(v);
    onCommit?.(v);
  };

  const origin = defaultValue !== undefined && defaultValue > min ? fraction(defaultValue) : 0;
  const at = fraction(value);
  const [from, to] = at >= origin ? [origin, at] : [at, origin];

  return (
    <div
      role="slider"
      tabIndex={0}
      aria-label={label}
      aria-valuemin={min}
      aria-valuemax={max}
      aria-valuenow={value}
      aria-valuetext={valueText}
      onKeyDown={onKeyDown}
      onPointerDown={onPointerDown}
      // Pointer use never moves focus to a control (DESIGN_SYSTEM §13); Tab still does.
      onMouseDown={(e) => e.preventDefault()}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onDoubleClick={() => {
        if (defaultValue === undefined) return;
        onChange(defaultValue);
        onCommit?.(defaultValue);
      }}
      className={`group relative flex h-6 cursor-pointer touch-none items-center rounded-control ${className}`}
    >
      <div ref={track} className="relative h-1 w-full rounded-track bg-track">
        <div
          className="absolute inset-y-0 rounded-track bg-accent"
          style={{ left: `${from * 100}%`, width: `${(to - from) * 100}%` }}
        />
      </div>
      <div
        className="pointer-events-none absolute h-4 w-3 -translate-x-1/2 rounded-knob bg-fg transition-[width,height] duration-(--dur-fast) group-hover:h-4.5 group-hover:w-3.5"
        style={{ left: `${at * 100}%` }}
      />
    </div>
  );
}
