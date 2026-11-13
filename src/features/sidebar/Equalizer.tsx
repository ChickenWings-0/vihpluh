/**
 * The equalizer (DESIGN_SYSTEM §10.2). The curve is the true summed biquad response
 * (`lib/eq.ts`), drawn under the nodes:
 * - 10-band: drag a node up/down (±12 dB, 0.5 dB steps); double-click resets it to 0.
 * - Parametric: up to 8 nodes; drag moves frequency and gain, the wheel changes Q,
 *   right-click picks the type or deletes, double-click on empty space adds a node.
 * Edits are live (throttled); the core applies gain moves to the running filter without a
 * rebuild. Preset chips draw their own curve; user presets can be saved and deleted.
 */
import { Plus, X } from "lucide-react";
import {
  type KeyboardEvent,
  type MouseEvent,
  type PointerEvent,
  useEffect,
  useMemo,
  useRef,
  useState,
  type WheelEvent,
} from "react";
import { dispatch } from "@/actions";
import { Button } from "@/components/Button";
import { Section, Segmented, Toggle } from "@/components/Controls";
import { DropdownMenu, MenuContent } from "@/components/Menu";
import { ParamRow, useThrottle } from "@/components/ParamRow";
import { Popover } from "@/components/Popover";
import type { EqBand } from "@/ipc/bindings/EqBand";
import type { EqPreset } from "@/ipc/bindings/EqPreset";
import type { EqState } from "@/ipc/bindings/EqState";
import { audioSetEq, eqPresetDelete, eqPresetSave } from "@/ipc/commands";
import { formatFreq, freqToX, GRAPHIC_FREQS, logFrequencies, response, xToFreq } from "@/lib/eq";
import { eqFromPreset, useEqStore } from "@/stores/eq";
import { reportError } from "@/stores/osd";
import { usePlayer } from "@/stores/player";

const DB = 12;
const HEIGHT = 132;
const PAD = 10;
const MAX_NODES = 8;

const flatGraphic = (): EqBand[] =>
  GRAPHIC_FREQS.map((freq) => ({ freq, gainDb: 0, q: 1, type: "peak" }));
const defaultParametric = (): EqBand[] => [
  { freq: 105, gainDb: 0, q: 0.7, type: "lowshelf" },
  { freq: 1000, gainDb: 0, q: 1, type: "peak" },
  { freq: 8000, gainDb: 0, q: 0.7, type: "highshelf" },
];

const fmtDb = (db: number) => `${db > 0 ? "+" : db < 0 ? "−" : ""}${Math.abs(db).toFixed(1)} dB`;
const snap = (v: number, step: number) => Math.round(v / step) * step;
const clampDb = (v: number) => Math.min(DB, Math.max(-DB, v));

function useWidth() {
  const ref = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(300);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const observer = new ResizeObserver(([entry]) => {
      if (entry) setWidth(Math.round(entry.contentRect.width));
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);
  return [ref, width] as const;
}

function curvePath(eq: EqState, width: number, height: number, points: number): string {
  const freqs = logFrequencies(points);
  const db = response(eq, freqs);
  return freqs
    .map((f, i) => {
      const x = PAD + freqToX(f) * (width - 2 * PAD);
      const y = ((DB - Math.max(-DB * 1.5, Math.min(DB * 1.5, db[i] ?? 0))) / (2 * DB)) * height;
      return `${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join("");
}

function Graph({ eq, onEdit }: { eq: EqState; onEdit: (eq: EqState) => void }) {
  const [wrap, width] = useWidth();
  const svg = useRef<SVGSVGElement>(null);
  const drag = useRef<number | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; index: number } | null>(null);
  const parametric = eq.mode === "parametric";
  const inner = width - 2 * PAD;
  const xOf = (f: number) => PAD + freqToX(f) * inner;
  const yOf = (db: number) => ((DB - db) / (2 * DB)) * HEIGHT;
  // The curve shows the shape even while the EQ is off (drawn muted).
  const path = useMemo(
    () => curvePath({ ...eq, enabled: true }, width, HEIGHT, Math.max(64, width / 3)),
    [eq, width],
  );
  const zero = yOf(0);

  const setBand = (index: number, patch: Partial<EqBand>) =>
    onEdit({
      ...eq,
      preset: null,
      bands: eq.bands.map((b, i) => (i === index ? { ...b, ...patch } : b)),
    });

  const pointAt = (e: { clientX: number; clientY: number }) => {
    const r = svg.current?.getBoundingClientRect();
    if (!r) return null;
    const fx = Math.min(1, Math.max(0, (e.clientX - r.left - PAD) / inner));
    const db = clampDb(snap(DB - ((e.clientY - r.top) / HEIGHT) * 2 * DB, 0.5));
    return { freq: Math.round(xToFreq(fx)), db };
  };

  const onDown = (index: number) => (e: PointerEvent<SVGRectElement>) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = index;
  };
  const onMove = (e: PointerEvent<SVGRectElement>) => {
    const index = drag.current;
    if (index === null) return;
    const p = pointAt(e);
    if (!p) return;
    setBand(
      index,
      parametric ? { freq: Math.min(20000, Math.max(20, p.freq)), gainDb: p.db } : { gainDb: p.db },
    );
  };
  const onUp = (e: PointerEvent<SVGRectElement>) => {
    drag.current = null;
    if (e.currentTarget.hasPointerCapture(e.pointerId))
      e.currentTarget.releasePointerCapture(e.pointerId);
  };
  const onWheel = (index: number) => (e: WheelEvent<SVGRectElement>) => {
    if (!parametric) return;
    e.preventDefault();
    const band = eq.bands[index];
    if (!band) return;
    const q = Math.min(10, Math.max(0.1, band.q * (e.deltaY < 0 ? 1.1 : 1 / 1.1)));
    setBand(index, { q: Number(q.toFixed(2)) });
  };
  const onKey = (index: number) => (e: KeyboardEvent<SVGRectElement>) => {
    const band = eq.bands[index];
    if (!band) return;
    const big = e.shiftKey ? 6 : 1;
    if (e.key === "ArrowUp" || e.key === "ArrowDown") {
      setBand(index, { gainDb: clampDb(band.gainDb + (e.key === "ArrowUp" ? 0.5 : -0.5) * big) });
    } else if (parametric && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
      const ratio = 2 ** ((e.key === "ArrowRight" ? 1 : -1) / (12 / big));
      setBand(index, { freq: Math.round(Math.min(20000, Math.max(20, band.freq * ratio))) });
    } else if (e.key === "Home") {
      setBand(index, { gainDb: 0 });
    } else return;
    e.preventDefault();
    e.stopPropagation();
  };
  const onEmptyDoubleClick = (e: MouseEvent<SVGRectElement>) => {
    if (!parametric || eq.bands.length >= MAX_NODES) return;
    const p = pointAt(e);
    if (!p) return;
    onEdit({
      ...eq,
      preset: null,
      bands: [...eq.bands, { freq: p.freq, gainDb: p.db, q: 1, type: "peak" }],
    });
  };

  const menuBand = menu ? eq.bands[menu.index] : undefined;

  return (
    <div ref={wrap} className="relative w-full select-none">
      <svg
        ref={svg}
        width={width}
        height={HEIGHT + 16}
        aria-label="Equalizer curve"
        className="block rounded-control bg-raised"
      >
        {/* biome-ignore lint/a11y/noStaticElementInteractions: a pointer shortcut; nodes are also reachable and editable by keyboard */}
        <rect
          x={0}
          y={0}
          width={width}
          height={HEIGHT}
          fill="transparent"
          onDoubleClick={onEmptyDoubleClick}
        />
        {[-DB, -6, 6, DB].map((db) => (
          <line
            key={db}
            x1={PAD}
            x2={width - PAD}
            y1={yOf(db)}
            y2={yOf(db)}
            stroke="var(--border-subtle)"
            pointerEvents="none"
            strokeWidth={1}
          />
        ))}
        {[100, 1000, 10000].map((f) => (
          <line
            key={f}
            x1={xOf(f)}
            x2={xOf(f)}
            y1={0}
            y2={HEIGHT}
            stroke="var(--border-subtle)"
            pointerEvents="none"
            strokeWidth={1}
          />
        ))}
        <line
          x1={PAD}
          x2={width - PAD}
          y1={zero}
          y2={zero}
          stroke="var(--border-strong)"
          pointerEvents="none"
          strokeWidth={1}
        />
        <path
          d={`${path}L${width - PAD},${zero}L${PAD},${zero}Z`}
          fill="var(--accent-muted)"
          opacity={eq.enabled ? 1 : 0.4}
          pointerEvents="none"
        />
        <path
          d={path}
          fill="none"
          stroke={eq.enabled ? "var(--accent)" : "var(--text-muted)"}
          strokeWidth={2}
          pointerEvents="none"
        />
        <text x={4} y={11} className="pointer-events-none fill-fg-3 text-xs">
          +{DB}
        </text>
        <text x={4} y={HEIGHT - 3} className="pointer-events-none fill-fg-3 text-xs">
          −{DB}
        </text>
        {(parametric ? [100, 1000, 10000] : GRAPHIC_FREQS).map((f) => (
          <text
            key={f}
            x={xOf(f)}
            y={HEIGHT + 12}
            textAnchor="middle"
            className="pointer-events-none fill-fg-3 text-xs tabular"
          >
            {formatFreq(f)}
          </text>
        ))}
        {eq.bands.map((band, index) => (
          <rect
            // biome-ignore lint/suspicious/noArrayIndexKey: bands are positional (equalizer@bN)
            key={index}
            role="slider"
            tabIndex={0}
            aria-label={`${formatFreq(band.freq)} Hz`}
            aria-valuemin={-DB}
            aria-valuemax={DB}
            aria-valuenow={band.gainDb}
            aria-valuetext={fmtDb(band.gainDb)}
            x={xOf(band.freq) - 5}
            y={yOf(band.gainDb) - 5}
            width={10}
            height={10}
            rx={3}
            fill={eq.enabled ? "var(--text-primary)" : "var(--text-muted)"}
            stroke="var(--accent)"
            strokeWidth={drag.current === index ? 2 : 0}
            className="cursor-grab outline-none focus-visible:stroke-2"
            onPointerDown={onDown(index)}
            onPointerMove={onMove}
            onPointerUp={onUp}
            onLostPointerCapture={() => {
              drag.current = null;
            }}
            onDoubleClick={() => setBand(index, { gainDb: 0 })}
            onWheel={onWheel(index)}
            onKeyDown={onKey(index)}
            onContextMenu={(e) => {
              if (!parametric) return;
              e.preventDefault();
              setMenu({ x: e.clientX, y: e.clientY, index });
            }}
          >
            <title>{`${formatFreq(band.freq)} Hz · ${fmtDb(band.gainDb)}${parametric ? ` · Q ${band.q}` : ""}`}</title>
          </rect>
        ))}
      </svg>
      {menu && menuBand && (
        <DropdownMenu.Root open modal={false} onOpenChange={(open) => !open && setMenu(null)}>
          <DropdownMenu.Trigger asChild>
            <div
              aria-hidden="true"
              className="pointer-events-none fixed h-0 w-0"
              style={{ left: menu.x, top: menu.y }}
            />
          </DropdownMenu.Trigger>
          <MenuContent
            entries={[
              ...(["peak", "lowshelf", "highshelf"] as const).map((type) => ({
                kind: "item" as const,
                label: type === "peak" ? "Peak" : type === "lowshelf" ? "Low shelf" : "High shelf",
                checked: menuBand.type === type,
                onSelect: () => setBand(menu.index, { type }),
              })),
              { kind: "separator" as const },
              {
                kind: "item" as const,
                label: "Delete node",
                disabled: eq.bands.length <= 1,
                onSelect: () =>
                  onEdit({
                    ...eq,
                    preset: null,
                    bands: eq.bands.filter((_, i) => i !== menu.index),
                  }),
              },
            ]}
          />
        </DropdownMenu.Root>
      )}
    </div>
  );
}

/** A preset chip draws that preset's curve as a sparkline above its name. */
function PresetChip({
  preset,
  active,
  onPick,
  onDelete,
}: {
  preset: EqPreset;
  active: boolean;
  onPick: () => void;
  onDelete?: () => void;
}) {
  const path = useMemo(() => {
    const eq = eqFromPreset(preset, true);
    const freqs = logFrequencies(20);
    const db = response(eq, freqs);
    return freqs
      .map((f, i) => {
        const x = 3 + freqToX(f) * 50;
        const y = 7 - (Math.max(-DB, Math.min(DB, db[i] ?? 0)) / DB) * 6;
        return `${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`;
      })
      .join("");
  }, [preset]);
  return (
    <div className="group relative">
      <button
        type="button"
        aria-pressed={active}
        aria-label={`${preset.name} preset`}
        onMouseDown={(e) => e.preventDefault()}
        onClick={onPick}
        className={`flex h-9 w-14 flex-col items-center justify-center rounded-control border ${
          active ? "border-accent bg-accent-soft" : "border-line bg-raised hover:border-line-strong"
        }`}
      >
        <svg width={56} height={14} aria-hidden="true">
          <path
            d={path}
            fill="none"
            stroke={active ? "var(--accent)" : "var(--text-secondary)"}
            strokeWidth={1.25}
          />
        </svg>
        <span
          className={`w-full truncate px-0.5 text-center text-xs ${active ? "text-accent-fg" : "text-fg-2"}`}
        >
          {preset.name}
        </span>
      </button>
      {onDelete && (
        <button
          type="button"
          aria-label={`Delete ${preset.name}`}
          onMouseDown={(e) => e.preventDefault()}
          onClick={onDelete}
          className="absolute -top-1.5 -right-1.5 hidden h-4 w-4 items-center justify-center rounded-control border border-line-strong bg-raised text-fg-2 hover:text-danger group-hover:flex"
        >
          <X size={10} strokeWidth={2} aria-hidden />
        </button>
      )}
    </div>
  );
}

const slug = (name: string) =>
  name
    .toLowerCase()
    .normalize("NFKD")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "")
    .slice(0, 40) || "preset";

function SavePreset({ eq, onSaved }: { eq: EqState; onSaved: (id: string) => void }) {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const save = async () => {
    const trimmed = name.trim();
    if (!trimmed) return;
    const id = slug(trimmed);
    try {
      await eqPresetSave({
        id,
        name: trimmed,
        mode: eq.mode,
        preampDb: eq.preampDb,
        bands: eq.bands,
        builtin: false,
      });
      setOpen(false);
      setName("");
      onSaved(id);
    } catch (error) {
      reportError(error);
    }
  };
  return (
    <Popover
      open={open}
      onOpenChange={setOpen}
      side="top"
      width="w-60"
      trigger={
        <button
          type="button"
          aria-label="Save preset"
          className="flex h-9 w-14 flex-col items-center justify-center gap-0.5 rounded-control border border-line border-dashed text-fg-2 text-xs hover:border-line-strong hover:text-fg"
        >
          <Plus size={14} strokeWidth={1.5} aria-hidden />
          Save
        </button>
      }
    >
      <form
        className="flex flex-col gap-2 p-1"
        onSubmit={(e) => {
          e.preventDefault();
          void save();
        }}
      >
        <label className="flex flex-col gap-1 text-fg-2 text-sm">
          Preset name
          <input
            // biome-ignore lint/a11y/noAutofocus: the popover exists to take this one field
            autoFocus
            value={name}
            maxLength={40}
            onChange={(e) => setName(e.target.value)}
            className="h-7 rounded-control border border-line bg-surface px-2 text-base text-fg outline-none focus:border-accent"
          />
        </label>
        <Button variant="primary" size="md" type="submit" keepFocus disabled={!name.trim()}>
          Save
        </Button>
      </form>
    </Popover>
  );
}

export function Equalizer() {
  const live = usePlayer((s) => s.eq);
  const presets = useEqStore((s) => s.presets);
  const loaded = useEqStore((s) => s.loaded);
  const load = useEqStore((s) => s.load);
  // Local copy while editing so the graph follows the pointer between core echoes.
  const [draft, setDraft] = useState<EqState | null>(null);
  const send = useThrottle((eq: EqState) => void audioSetEq(eq).catch(reportError));

  useEffect(() => {
    if (!loaded) void load();
  }, [loaded, load]);
  useEffect(() => {
    if (!draft) return;
    const t = setTimeout(() => setDraft(null), 600);
    return () => clearTimeout(t);
  }, [draft]);

  if (!live) return null;
  const eq = draft ?? live;
  const edit = (next: EqState) => {
    setDraft(next);
    send(next);
  };
  const pick = (preset: EqPreset) => edit(eqFromPreset(preset, true));

  return (
    <Section
      title="Equalizer"
      right={
        <Toggle
          label="Equalizer"
          showLabel={false}
          checked={eq.enabled}
          onChange={() => void dispatch("audio.toggle_equalizer", { trigger: "button" })}
        />
      }
    >
      <Segmented
        label="Equalizer mode"
        value={eq.mode === "parametric" ? "parametric" : "graphic10"}
        options={[
          { value: "graphic10", label: "10-band" },
          {
            value: "parametric",
            label: "Parametric",
            hint: "Up to 8 nodes. Wheel: width (Q). Right-click: type or delete. Double-click: add.",
          },
        ]}
        onChange={(mode) => {
          if (mode === eq.mode) return;
          edit({
            ...eq,
            mode,
            preset: mode === "graphic10" ? "flat" : null,
            bands: mode === "graphic10" ? flatGraphic() : defaultParametric(),
          });
        }}
      />
      <Graph eq={eq} onEdit={edit} />
      <ParamRow
        label="Preamp"
        value={eq.preampDb}
        min={-12}
        max={12}
        step={0.5}
        defaultValue={0}
        format={fmtDb}
        onChange={(preampDb) => edit({ ...eq, preset: null, preampDb })}
      />
      <fieldset className="flex flex-wrap gap-1.5">
        <legend className="sr-only">Presets</legend>
        {presets
          .filter((p) => p.mode === eq.mode || p.builtin)
          .map((preset) => (
            <PresetChip
              key={preset.id}
              preset={preset}
              active={eq.preset === preset.id}
              onPick={() => pick(preset)}
              onDelete={
                preset.builtin
                  ? undefined
                  : () =>
                      void eqPresetDelete(preset.id)
                        .then(() => load())
                        .catch(reportError)
              }
            />
          ))}
        <SavePreset
          eq={eq}
          onSaved={(id) => {
            void load();
            edit({ ...eq, preset: id });
          }}
        />
      </fieldset>
    </Section>
  );
}
