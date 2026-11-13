/**
 * The standard parameter row (DESIGN_SYSTEM §8.5): label, exact value field, reset button,
 * and a slider with a detent at the default. Live while dragging (throttled to 30 ms); the
 * field takes typed values (Enter commits, Esc reverts) and steps with ↑/↓ (Shift ×10,
 * Alt ÷10). The wheel steps only while the row is hovered *and* focused.
 */
import { RotateCcw } from "lucide-react";
import {
  type KeyboardEvent,
  useCallback,
  useEffect,
  useRef,
  useState,
  type WheelEvent,
} from "react";
import { Slider } from "./Slider";
import { Tooltip } from "./Tooltip";

const THROTTLE_MS = 30;

/** Calls `fn` at most every `ms`, always delivering the last value. */
export function useThrottle<T>(fn: (value: T) => void, ms: number = THROTTLE_MS) {
  const last = useRef(0);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const pending = useRef<{ value: T } | null>(null);
  const latestFn = useRef(fn);
  latestFn.current = fn;
  useEffect(() => () => clearTimeout(timer.current), []);
  return useCallback(
    (value: T) => {
      const now = performance.now();
      const wait = last.current + ms - now;
      if (wait <= 0) {
        clearTimeout(timer.current);
        timer.current = undefined;
        pending.current = null;
        last.current = now;
        latestFn.current(value);
        return;
      }
      pending.current = { value };
      if (timer.current === undefined) {
        timer.current = setTimeout(() => {
          timer.current = undefined;
          last.current = performance.now();
          const next = pending.current;
          pending.current = null;
          if (next) latestFn.current(next.value);
        }, wait);
      }
    },
    [ms],
  );
}

export interface ParamRowProps {
  label: string;
  hint?: string;
  value: number;
  min: number;
  max: number;
  step: number;
  defaultValue: number;
  /** Field text for a value, e.g. `−350 ms`, `1.25×`. */
  format: (value: number) => string;
  onChange: (value: number) => void;
  /** Scale labels under the track: [left, middle, right]. */
  scale?: [string, string, string];
  disabled?: boolean;
}

const decimals = (step: number) => Math.max(0, -Math.floor(Math.log10(step)));

export function ParamRow({
  label,
  hint,
  value,
  min,
  max,
  step,
  defaultValue,
  format,
  onChange,
  scale,
  disabled,
}: ParamRowProps) {
  // While dragging or typing, show the local value until the core's echo catches up.
  const [draft, setDraft] = useState<number | null>(null);
  const draftSince = useRef(0);
  const send = useThrottle(onChange);
  const clamp = (v: number) => Math.min(max, Math.max(min, v));
  const shown = draft ?? value;

  useEffect(() => {
    if (draft === null) return;
    const caughtUp = Math.abs(value - draft) < step / 2;
    if (caughtUp && performance.now() - draftSince.current > 150) setDraft(null);
  }, [value, draft, step]);
  useEffect(() => {
    if (draft === null) return;
    const t = setTimeout(() => setDraft(null), 1000);
    return () => clearTimeout(t);
  }, [draft]);

  const change = (v: number) => {
    const next = clamp(Number(v.toFixed(decimals(step) + 1)));
    draftSince.current = performance.now();
    setDraft(next);
    send(next);
  };

  const row = useRef<HTMLDivElement>(null);
  const onWheel = (e: WheelEvent<HTMLDivElement>) => {
    if (disabled || !row.current?.contains(document.activeElement)) return;
    e.preventDefault();
    change(shown + (e.deltaY < 0 ? step : -step));
  };

  const isDefault = Math.abs(shown - defaultValue) < step / 2;

  return (
    <div
      ref={row}
      onWheel={onWheel}
      className={`flex flex-col gap-0.5 ${disabled ? "pointer-events-none opacity-50" : ""}`}
    >
      <div className="flex h-6 items-center gap-1">
        {hint ? (
          <Tooltip label={hint}>
            <span className="flex-1 cursor-default truncate text-base text-fg-2">{label}</span>
          </Tooltip>
        ) : (
          <span className="flex-1 truncate text-base text-fg-2">{label}</span>
        )}
        <NumberField
          label={label}
          value={shown}
          step={step}
          format={format}
          onChange={change}
          disabled={disabled}
        />
        <span className="flex w-6 justify-center">
          {!isDefault && (
            <Tooltip label={`Reset to ${format(defaultValue)}`}>
              <button
                type="button"
                aria-label={`Reset ${label}`}
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => change(defaultValue)}
                className="flex h-6 w-6 items-center justify-center rounded-control text-fg-2 hover:bg-hover hover:text-fg"
              >
                <RotateCcw size={14} strokeWidth={1.5} aria-hidden />
              </button>
            </Tooltip>
          )}
        </span>
      </div>
      <Slider
        value={shown}
        min={min}
        max={max}
        step={step}
        defaultValue={defaultValue}
        label={label}
        valueText={format(shown)}
        onChange={change}
        onCommit={change}
      />
      {scale && (
        <div className="flex justify-between text-fg-3 text-xs tabular">
          <span>{scale[0]}</span>
          <span>{scale[1]}</span>
          <span>{scale[2]}</span>
        </div>
      )}
    </div>
  );
}

/** Exact value field: shows the formatted value, edits the bare number. */
export function NumberField({
  label,
  value,
  step,
  format,
  onChange,
  disabled,
  width = "w-18",
}: {
  label: string;
  value: number;
  step: number;
  format: (value: number) => string;
  onChange: (value: number) => void;
  disabled?: boolean;
  width?: string;
}) {
  const [text, setText] = useState<string | null>(null);
  const editing = text !== null;
  const commit = () => {
    if (text === null) return;
    const parsed = Number.parseFloat(text.replace(/[−–]/g, "-").replace(/[^0-9.+-]/g, ""));
    setText(null);
    if (Number.isFinite(parsed)) onChange(parsed);
  };
  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      commit();
      e.currentTarget.blur();
    } else if (e.key === "Escape") {
      setText(null);
      e.currentTarget.blur();
      e.stopPropagation();
    } else if (e.key === "ArrowUp" || e.key === "ArrowDown") {
      e.preventDefault();
      const factor = e.shiftKey ? 10 : e.altKey ? 0.1 : 1;
      const next = value + (e.key === "ArrowUp" ? 1 : -1) * step * factor;
      setText(null);
      onChange(next);
    }
  };
  return (
    <input
      type="text"
      inputMode="decimal"
      aria-label={`${label} value`}
      disabled={disabled}
      value={editing ? text : format(value)}
      onFocus={(e) => {
        setText(String(Number(value.toFixed(decimals(step)))));
        requestAnimationFrame(() => e.target.select());
      }}
      onChange={(e) => setText(e.target.value)}
      onBlur={commit}
      onKeyDown={onKeyDown}
      className={`tabular h-6 ${width} rounded-control border border-line bg-raised px-1.5 text-right font-mono text-fg text-sm outline-none hover:border-line-strong focus:border-accent`}
    />
  );
}
