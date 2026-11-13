/**
 * Playback tab (DESIGN_SYSTEM §10.4): speed, frame step and reverse, loop (file or A–B),
 * sleep timer, seek-bar toggles, and the stream inspector.
 */
import { StepBack, StepForward } from "lucide-react";
import { useEffect, useState } from "react";
import { dispatch } from "@/actions";
import { currentSleepStep, SLEEP_STEPS } from "@/actions/definitions/media";
import { Button } from "@/components/Button";
import { Section, Segmented, Toggle } from "@/components/Controls";
import { ParamRow } from "@/components/ParamRow";
import {
  configSet,
  playerSetAbLoop,
  playerSetLoopFile,
  playerSetSleepTimer,
  playerSetSpeed,
} from "@/ipc/commands";
import { formatTime, formatTimePrecise, parseTime } from "@/lib/time";
import { useConfig } from "@/stores/config";
import { reportError } from "@/stores/osd";
import { usePlayer } from "@/stores/player";
import { getPosition } from "@/stores/time";
import { useUiStore } from "@/stores/ui";
import { Inspector } from "./Inspector";

const SPEEDS = [0.5, 0.75, 1, 1.25, 1.5, 2];

function TimeField({
  label,
  value,
  onCommit,
}: {
  label: string;
  value: number | null;
  onCommit: (seconds: number) => void;
}) {
  const [text, setText] = useState<string | null>(null);
  const commit = () => {
    if (text === null) return;
    const parsed = parseTime(text);
    setText(null);
    if (parsed !== null) onCommit(parsed);
  };
  return (
    <input
      type="text"
      aria-label={label}
      placeholder="–:––.–––"
      value={text ?? (value === null ? "" : formatTimePrecise(value))}
      onChange={(e) => setText(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") e.currentTarget.blur();
        if (e.key === "Escape") {
          setText(null);
          e.currentTarget.blur();
          e.stopPropagation();
        }
      }}
      className="tabular h-6 w-24 rounded-control border border-line bg-raised px-1.5 font-mono text-fg text-sm outline-none placeholder:text-fg-3 focus:border-accent"
    />
  );
}

type LoopMode = "off" | "file" | "ab";

function Loop() {
  const loopFile = usePlayer((s) => s.loopFile) ?? false;
  const ab = usePlayer((s) => s.abLoop) ?? { a: null, b: null };
  const mode: LoopMode = ab.a !== null ? "ab" : loopFile ? "file" : "off";
  const [showAb, setShowAb] = useState(mode === "ab");
  const shown: LoopMode = showAb && mode === "off" ? "ab" : mode;

  const setMode = async (next: LoopMode) => {
    setShowAb(next === "ab");
    if (next !== "ab" && ab.a !== null) await playerSetAbLoop({ a: null, b: null });
    if ((next === "file") !== loopFile) await playerSetLoopFile(next === "file");
  };
  const setPoint = (which: "a" | "b", at: number) => {
    const next = { ...ab, [which]: at };
    if (next.a !== null && next.b !== null && next.b < next.a) [next.a, next.b] = [next.b, next.a];
    void playerSetAbLoop(next).catch(reportError);
  };

  return (
    <Section title="Loop">
      <Segmented<LoopMode>
        label="Loop"
        value={shown}
        options={[
          { value: "off", label: "Off" },
          { value: "file", label: "File" },
          { value: "ab", label: "A–B" },
        ]}
        onChange={(next) => void setMode(next).catch(reportError)}
      />
      {shown === "ab" && (
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="text-fg-2 text-sm">A</span>
          <TimeField label="Loop start (A)" value={ab.a} onCommit={(t) => setPoint("a", t)} />
          <span className="text-fg-2 text-sm">B</span>
          <TimeField label="Loop end (B)" value={ab.b} onCommit={(t) => setPoint("b", t)} />
          <Button size="sm" onClick={() => setPoint("a", Number(getPosition().toFixed(3)))}>
            Set A
          </Button>
          <Button
            size="sm"
            disabled={ab.a === null}
            onClick={() => setPoint("b", Number(getPosition().toFixed(3)))}
          >
            Set B
          </Button>
          <Button
            size="sm"
            variant="ghost"
            disabled={ab.a === null}
            onClick={() => void playerSetAbLoop({ a: null, b: null }).catch(reportError)}
          >
            Clear
          </Button>
        </div>
      )}
    </Section>
  );
}

function Sleep() {
  const sleep = usePlayer((s) => s.sleep) ?? null;
  const step = SLEEP_STEPS[currentSleepStep()];
  const [left, setLeft] = useState<number | null>(sleep?.remaining ?? null);
  useEffect(() => setLeft(sleep?.remaining ?? null), [sleep?.remaining]);
  return (
    <Section
      title="Sleep"
      right={
        left !== null ? (
          <span className="tabular text-fg-2 text-sm">{formatTime(left)} left</span>
        ) : sleep ? (
          <span className="text-fg-2 text-sm">{step?.label}</span>
        ) : null
      }
    >
      <Segmented
        label="Sleep timer"
        value={String(currentSleepStep())}
        options={SLEEP_STEPS.map((s, i) => ({
          value: String(i),
          label: s.kind === "minutes" ? String(s.minutes) : s.label,
          hint: s.kind === "minutes" ? `${s.minutes} minutes` : undefined,
        }))}
        fit
        onChange={(index) => {
          const chosen = SLEEP_STEPS[Number(index)];
          if (chosen) {
            void playerSetSleepTimer({ kind: chosen.kind, minutes: chosen.minutes }).catch(
              reportError,
            );
          }
        }}
      />
    </Section>
  );
}

export function PlaybackTab() {
  const speed = usePlayer((s) => s.speed) ?? 1;
  const reverse = usePlayer((s) => s.direction === "backward") ?? false;
  const hasMedia = usePlayer((s) => s.media !== null) ?? false;
  const waveform = useConfig((c) => c.interface.seek_waveform) ?? false;
  const thumbnails = useConfig((c) => c.interface.seek_preview) ?? true;
  const chapters = useConfig((c) => c.interface.seek_chapter_marks) ?? true;
  const inspectorRequest = useUiStore((s) => s.inspectorRequest);
  const setConfig = (path: string, value: boolean) =>
    void configSet(path, value).catch(reportError);

  return (
    <>
      <Section title="Speed">
        <ParamRow
          label="Speed"
          value={speed}
          min={0.25}
          max={4}
          step={0.05}
          defaultValue={1}
          format={(v) => `${v.toFixed(2)}×`}
          onChange={(v) => void playerSetSpeed(v).catch(reportError)}
        />
        <div className="flex flex-wrap gap-1">
          {SPEEDS.map((v) => (
            <Button
              key={v}
              size="sm"
              aria-pressed={Math.abs(speed - v) < 0.001}
              className={Math.abs(speed - v) < 0.001 ? "border-accent text-accent-fg" : ""}
              onClick={() => void playerSetSpeed(v).catch(reportError)}
            >
              {v}×
            </Button>
          ))}
        </div>
      </Section>

      <Section title="Step">
        <div className="flex flex-wrap items-center gap-1">
          <Button
            size="sm"
            icon={StepBack}
            disabled={!hasMedia}
            onClick={() => void dispatch("player.frame_step_back", { trigger: "button" })}
          >
            Frame back
          </Button>
          <Button
            size="sm"
            icon={StepForward}
            disabled={!hasMedia}
            onClick={() => void dispatch("player.frame_step_forward", { trigger: "button" })}
          >
            Frame forward
          </Button>
          <span className="flex-1" />
          <Toggle
            label="Reverse"
            hint="Plays backwards. Needs memory for the cache; very high-bitrate files may stutter."
            checked={reverse}
            disabled={!hasMedia}
            onChange={() => void dispatch("player.toggle_reverse", { trigger: "button" })}
          />
        </div>
      </Section>

      <Loop />
      <Sleep />

      <Section title="Seek bar">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <Toggle
            label="Waveform"
            checked={waveform}
            onChange={(v) => setConfig("interface.seek_waveform", v)}
          />
          <Toggle
            label="Thumbnails"
            checked={thumbnails}
            onChange={(v) => setConfig("interface.seek_preview", v)}
          />
          <Toggle
            label="Chapter marks"
            checked={chapters}
            onChange={(v) => setConfig("interface.seek_chapter_marks", v)}
          />
        </div>
      </Section>

      <Inspector scrollRequest={inspectorRequest} />
    </>
  );
}
