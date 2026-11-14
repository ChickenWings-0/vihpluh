/**
 * The control for each kind of setting (KEYBINDS_AND_CONFIG §6 rule 4): toggle, segmented
 * control or select, slider with an exact field, chip list, colour swatch, path with Browse.
 * Every control shows the value it is given and reports a new one; saving is the row's job.
 */
import { FolderOpen, X } from "lucide-react";
import { type KeyboardEvent, useEffect, useRef, useState } from "react";
import { Button } from "@/components/Button";
import { type Option, Segmented, Select, Toggle } from "@/components/Controls";
import { NumberField, useThrottle } from "@/components/ParamRow";
import { Slider } from "@/components/Slider";
import { Tooltip } from "@/components/Tooltip";
import { appPickMedia } from "@/ipc/commands";
import { ACCENT_SWATCHES, isHex } from "@/lib/color";
import { useEqStore } from "@/stores/eq";
import { reportError } from "@/stores/osd";
import { usePlayer } from "@/stores/player";
import { FontPicker } from "../sidebar/FontPicker";
import type { Choice, Control } from "./schema";

const FIELD =
  "h-7 rounded-control border border-line bg-raised px-2 text-base text-fg outline-none placeholder:text-fg-3 hover:border-line-strong focus:border-accent";

const decimals = (step: number) => Math.max(0, -Math.floor(Math.log10(step)));

export function formatNumber(value: number, step: number, unit?: string): string {
  const text = Number(value.toFixed(decimals(step))).toString();
  if (!unit) return text;
  return unit === "×" || unit === "%" ? `${text}${unit === "%" ? " %" : "×"}` : `${text} ${unit}`;
}

interface ControlProps {
  control: Control;
  label: string;
  value: unknown;
  defaultValue: unknown;
  onChange: (value: unknown) => void;
}

export function SettingControl({ control, label, value, defaultValue, onChange }: ControlProps) {
  switch (control.kind) {
    case "toggle":
      return (
        <Toggle checked={value === true} onChange={onChange} label={label} showLabel={false} />
      );
    case "choice":
      return (
        <ChoiceControl
          options={control.options}
          other={control.other}
          value={String(value)}
          label={label}
          onChange={onChange}
        />
      );
    case "number":
      return (
        <NumberControl
          control={control}
          label={label}
          value={typeof value === "number" ? value : control.min}
          defaultValue={typeof defaultValue === "number" ? defaultValue : control.min}
          onChange={onChange}
        />
      );
    case "keyword":
      return <KeywordControl control={control} label={label} value={value} onChange={onChange} />;
    case "text":
      return (
        <TextControl
          label={label}
          value={String(value ?? "")}
          placeholder={control.placeholder}
          mono={control.mono}
          onChange={onChange}
        />
      );
    case "path":
      return (
        <div className="flex min-w-0 items-center gap-1.5">
          <TextControl
            label={label}
            value={String(value ?? "")}
            placeholder={control.placeholder}
            onChange={onChange}
          />
          <Button
            icon={FolderOpen}
            keepFocus
            onClick={() =>
              void appPickMedia("folder")
                .then((picked) => {
                  if (picked[0]) onChange(picked[0]);
                })
                .catch(reportError)
            }
          >
            Browse…
          </Button>
        </div>
      );
    case "list":
      return (
        <ListControl
          label={label}
          value={Array.isArray(value) ? (value as string[]) : []}
          placeholder={control.placeholder}
          suggestions={control.suggestions}
          onChange={onChange}
        />
      );
    case "color":
      return (
        <ColorControl
          label={label}
          value={String(value)}
          swatches={control.swatches}
          onChange={onChange}
        />
      );
    case "audio_device":
      return <DeviceControl label={label} value={String(value)} onChange={onChange} />;
    case "font":
      return (
        <div className="w-56">
          <FontPicker value={String(value ?? "")} onChange={onChange} />
        </div>
      );
    case "eq_preset":
      return <EqPresetControl label={label} value={String(value)} onChange={onChange} />;
  }
}

function ChoiceControl({
  options,
  other,
  value,
  label,
  onChange,
}: {
  options: Choice[];
  other?: boolean;
  value: string;
  label: string;
  onChange: (value: string) => void;
}) {
  // A value typed in the config file that the list does not offer is still shown.
  const all: Option<string>[] =
    other && !options.some((o) => o.value === value)
      ? [...options, { value, label: value }]
      : options;
  if (all.length <= 4) {
    return <Segmented options={all} value={value} onChange={onChange} label={label} fit />;
  }
  return <Select options={all} value={value} onChange={onChange} label={label} className="w-44" />;
}

function NumberControl({
  control,
  label,
  value,
  defaultValue,
  onChange,
}: {
  control: Extract<Control, { kind: "number" }>;
  label: string;
  value: number;
  defaultValue: number;
  onChange: (value: number) => void;
}) {
  const { min, max, step, unit } = control;
  // While dragging, show the local value; the saved one catches up through the core.
  const [draft, setDraft] = useState<number | null>(null);
  const send = useThrottle(onChange, 150);
  const tidy = (v: number) => Math.min(max, Math.max(min, Number(v.toFixed(decimals(step) + 1))));
  useEffect(() => {
    if (draft === null) return;
    const timer = setTimeout(() => setDraft(null), 600);
    return () => clearTimeout(timer);
  }, [draft]);
  const shown = draft ?? value;
  const change = (v: number) => {
    const next = tidy(v);
    setDraft(next);
    send(next);
  };
  return (
    <div className="flex items-center gap-2">
      {control.slider !== false && (
        <Slider
          className="w-40"
          value={shown}
          min={min}
          max={max}
          step={step}
          defaultValue={defaultValue}
          label={label}
          valueText={formatNumber(shown, step, unit)}
          onChange={change}
          onCommit={change}
        />
      )}
      <NumberField
        label={label}
        value={shown}
        step={step}
        format={(v) => formatNumber(v, step, unit)}
        onChange={change}
        width="w-20"
      />
    </div>
  );
}

function KeywordControl({
  control,
  label,
  value,
  onChange,
}: {
  control: Extract<Control, { kind: "keyword" }>;
  label: string;
  value: unknown;
  onChange: (value: unknown) => void;
}) {
  const number = typeof value === "number" ? value : null;
  const [text, setText] = useState<string | null>(null);
  const commit = () => {
    if (text === null) return;
    const parsed = Number.parseInt(text.replace(/[^0-9]/g, ""), 10);
    setText(null);
    if (Number.isFinite(parsed)) onChange(Math.min(control.max, Math.max(control.min, parsed)));
  };
  return (
    <div className="flex items-center gap-2">
      <Segmented
        options={control.options}
        value={number === null ? String(value) : null}
        onChange={onChange}
        label={label}
        fit
      />
      <input
        type="text"
        inputMode="numeric"
        aria-label={`${label}: exact value in ${control.unit}`}
        value={text ?? (number === null ? "" : String(number))}
        placeholder={control.placeholder}
        onChange={(e) => setText(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === "Enter") e.currentTarget.blur();
          else if (e.key === "Escape") {
            e.stopPropagation();
            setText(null);
          }
        }}
        className={`${FIELD} tabular w-24 text-right font-mono text-sm`}
      />
    </div>
  );
}

function TextControl({
  label,
  value,
  placeholder,
  mono,
  onChange,
}: {
  label: string;
  value: string;
  placeholder?: string;
  mono?: boolean;
  onChange: (value: string) => void;
}) {
  const [text, setText] = useState<string | null>(null);
  const commit = () => {
    if (text !== null && text !== value) onChange(text.trim());
    setText(null);
  };
  return (
    <input
      type="text"
      aria-label={label}
      spellCheck={false}
      value={text ?? value}
      placeholder={placeholder}
      onChange={(e) => setText(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") e.currentTarget.blur();
        else if (e.key === "Escape" && text !== null) {
          e.stopPropagation();
          setText(null);
        }
      }}
      className={`${FIELD} w-64 min-w-0 ${mono ? "font-mono text-sm" : ""}`}
    />
  );
}

function ListControl({
  label,
  value,
  placeholder,
  suggestions,
  onChange,
}: {
  label: string;
  value: string[];
  placeholder: string;
  suggestions?: string[];
  onChange: (value: string[]) => void;
}) {
  const [text, setText] = useState("");
  const add = (raw: string) => {
    const items = raw
      .split(",")
      .map((item) => item.trim())
      .filter((item) => item !== "" && !value.includes(item));
    if (items.length > 0) onChange([...value, ...items]);
    setText("");
  };
  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter" || e.key === ",") {
      e.preventDefault();
      add(text);
    } else if (e.key === "Backspace" && text === "" && value.length > 0) {
      onChange(value.slice(0, -1));
    } else if (e.key === "Escape" && text !== "") {
      e.stopPropagation();
      setText("");
    }
  };
  const unused = (suggestions ?? []).filter((s) => !value.includes(s));
  return (
    <div className="flex w-full flex-col gap-1.5">
      {/* biome-ignore lint/a11y/useSemanticElements: a chip field; a fieldset would bring its own frame */}
      <div
        role="group"
        aria-label={label}
        className="flex min-h-7 flex-wrap items-center gap-1 rounded-control border border-line bg-raised p-1 focus-within:border-accent"
      >
        {value.map((item) => (
          <span
            key={item}
            className="inline-flex h-5 items-center gap-1 rounded-control bg-active pr-0.5 pl-1.5 font-mono text-fg text-xs"
          >
            {item}
            <button
              type="button"
              aria-label={`Remove ${item}`}
              onClick={() => onChange(value.filter((v) => v !== item))}
              className="flex h-4 w-4 items-center justify-center rounded-control text-fg-2 hover:bg-hover hover:text-fg"
            >
              <X size={12} strokeWidth={1.5} aria-hidden />
            </button>
          </span>
        ))}
        <input
          type="text"
          aria-label={placeholder}
          spellCheck={false}
          value={text}
          placeholder={placeholder}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={onKeyDown}
          onBlur={() => add(text)}
          className="h-5 min-w-24 flex-1 bg-transparent px-1 text-base text-fg outline-none placeholder:text-fg-3"
        />
      </div>
      {unused.length > 0 && (
        <div className="flex flex-wrap gap-1">
          {unused.map((s) => (
            <Button key={s} size="sm" keepFocus onClick={() => onChange([...value, s])}>
              + {s}
            </Button>
          ))}
        </div>
      )}
    </div>
  );
}

function ColorControl({
  label,
  value,
  swatches,
  onChange,
}: {
  label: string;
  value: string;
  swatches?: boolean;
  onChange: (value: string) => void;
}) {
  const [text, setText] = useState<string | null>(null);
  const send = useThrottle(onChange, 150);
  const current = value.toUpperCase();
  const commit = () => {
    if (text === null) return;
    const hex = (text.startsWith("#") ? text : `#${text}`).toUpperCase();
    setText(null);
    if (isHex(hex) && hex !== current) onChange(hex);
  };
  return (
    <div className="flex flex-wrap items-center justify-end gap-2">
      {swatches && (
        <div role="radiogroup" aria-label={`${label} presets`} className="flex gap-1">
          {ACCENT_SWATCHES.map((swatch) => (
            <Tooltip key={swatch.hex} label={swatch.name}>
              {/* biome-ignore lint/a11y/useSemanticElements: a colour swatch cannot be a native radio */}
              <button
                type="button"
                role="radio"
                aria-checked={current === swatch.hex}
                aria-label={swatch.name}
                onClick={() => onChange(swatch.hex)}
                style={{ backgroundColor: swatch.hex }}
                className={`h-5 w-5 rounded-control border ${
                  current === swatch.hex
                    ? "border-fg outline outline-1 outline-fg outline-offset-1"
                    : "border-line-strong"
                }`}
              />
            </Tooltip>
          ))}
        </div>
      )}
      <input
        type="color"
        aria-label={`${label}: pick a colour`}
        value={isHex(value) ? value.toLowerCase() : "#000000"}
        onChange={(e) => send(e.target.value.toUpperCase())}
        className="h-7 w-8 cursor-pointer rounded-control border border-line bg-raised p-0.5"
      />
      <input
        type="text"
        aria-label={`${label}: hex value`}
        spellCheck={false}
        maxLength={7}
        value={text ?? current}
        onChange={(e) => setText(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === "Enter") e.currentTarget.blur();
          else if (e.key === "Escape" && text !== null) {
            e.stopPropagation();
            setText(null);
          }
        }}
        className={`${FIELD} w-20 font-mono text-sm ${
          text !== null && !isHex(text.startsWith("#") ? text : `#${text}`) ? "border-danger" : ""
        }`}
      />
    </div>
  );
}

function DeviceControl({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  const devices = usePlayer((s) => s.audioDevices) ?? [];
  const options: Option<string>[] = devices.map((d) => ({
    value: d.name,
    label: d.name === "auto" ? "Auto" : d.description || d.name,
  }));
  if (!options.some((o) => o.value === "auto")) options.unshift({ value: "auto", label: "Auto" });
  if (!options.some((o) => o.value === value)) options.push({ value, label: value });
  return (
    <Select options={options} value={value} onChange={onChange} label={label} className="w-64" />
  );
}

function EqPresetControl({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
}) {
  const presets = useEqStore((s) => s.presets);
  const loaded = useEqStore((s) => s.loaded);
  const asked = useRef(false);
  useEffect(() => {
    if (loaded || asked.current) return;
    asked.current = true;
    void useEqStore.getState().load();
  }, [loaded]);
  const options: Option<string>[] = presets.map((p) => ({ value: p.id, label: p.name }));
  if (!options.some((o) => o.value === value)) options.push({ value, label: value });
  return (
    <Select options={options} value={value} onChange={onChange} label={label} className="w-44" />
  );
}
