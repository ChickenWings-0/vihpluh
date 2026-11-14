/**
 * Settings (DESIGN_SYSTEM §11.2): a centred panel over the scrim, playback carrying on
 * behind. A 200 px rail (search, ten categories, footer) and one scrolling page per category,
 * generated from the schema. Changes apply and save at once through `config_set`; nothing
 * here keeps a copy of a setting.
 */
import { ChevronLeft, ChevronRight, Info, RotateCcw, Search, X } from "lucide-react";
import { type ComponentType, useEffect, useMemo, useRef, useState } from "react";
import { focusPlayerRoot } from "@/actions/definitions/window";
import { Button } from "@/components/Button";
import { Tooltip } from "@/components/Tooltip";
import {
  ControlsDuo,
  type DuoProps,
  GeneralDuo,
  InterfaceDuo,
  LibraryDuo,
  MusicDuo,
  NetworkDuo,
  PerformanceDuo,
  PlaybackDuo,
  SubtitlesDuo,
  VideoDuo,
} from "@/icons/duotone";
import { appReveal, configReset, configSet } from "@/ipc/commands";
import { useAppStore } from "@/stores/app";
import { useConfigStore } from "@/stores/config";
import { useShortcut } from "@/stores/keymap";
import { reportError } from "@/stores/osd";
import { usePresetStore } from "@/stores/presets";
import { useUiStore } from "@/stores/ui";
import { SettingControl } from "./controls";
import { CacheRow, LibraryFolders } from "./extras";
import { HotkeyEditor } from "./HotkeyEditor";
import { PresetsManager } from "./PresetsManager";
import {
  CATEGORIES,
  type Category,
  isAvailable,
  SETTING_LIST,
  type Setting,
  sameValue,
  valueAt,
} from "./schema";
import { highlight, searchSettings } from "./search";

const ICONS: Record<Category, ComponentType<DuoProps>> = {
  general: GeneralDuo,
  interface: InterfaceDuo,
  controls: ControlsDuo,
  playback: PlaybackDuo,
  video: VideoDuo,
  audio: MusicDuo,
  subtitles: SubtitlesDuo,
  library: LibraryDuo,
  network: NetworkDuo,
  performance: PerformanceDuo,
};

/** The Presets manager, the one sub-page (of Playback). */
export const PRESETS_PAGE = "presets";

const HEADING = "font-medium text-fg-3 text-xs uppercase tracking-[0.06em]";

/** Controls that need the full row width sit under their label. */
const isWide = (setting: Setting) => setting.control.kind === "list";

function Marked({ text, query }: { text: string; query: string }) {
  if (!query.trim()) return <>{text}</>;
  return (
    <>
      {highlight(text, query).map((piece, i) =>
        piece.match ? (
          // biome-ignore lint/suspicious/noArrayIndexKey: pieces of one fixed string
          <mark key={i} className="rounded-knob bg-accent-soft text-accent-fg">
            {piece.text}
          </mark>
        ) : (
          // biome-ignore lint/suspicious/noArrayIndexKey: pieces of one fixed string
          <span key={i}>{piece.text}</span>
        ),
      )}
    </>
  );
}

function SettingRow({ setting, query }: { setting: Setting; query: string }) {
  const value = useConfigStore((s) => (s.config ? valueAt(s.config, setting.path) : undefined));
  const fallback = useConfigStore((s) =>
    s.defaults ? valueAt(s.defaults, setting.path) : undefined,
  );
  if (value === undefined) return null;
  const modified = fallback !== undefined && !sameValue(value, fallback);
  const set = (next: unknown) => void configSet(setting.path, next).catch(reportError);
  const wide = isWide(setting);

  return (
    <div
      data-setting={setting.path}
      data-modified={modified || undefined}
      className={`group flex min-h-10 gap-x-3 gap-y-1.5 border-line border-b py-2 ${
        wide ? "flex-col" : "flex-wrap items-center"
      }`}
    >
      <div className="flex min-w-48 flex-1 items-center gap-1.5">
        <span
          aria-hidden
          className={`h-1.5 w-1.5 shrink-0 rounded-control ${modified ? "bg-accent" : ""}`}
        />
        <span className="min-w-0 text-base text-fg">
          <Marked text={setting.label} query={query} />
        </span>
        <Tooltip label={setting.tooltip}>
          <button
            type="button"
            aria-label={`About “${setting.label}”: ${setting.tooltip}`}
            className="flex h-5 w-5 shrink-0 items-center justify-center rounded-control text-fg-3 hover:text-fg"
          >
            <Info size={14} strokeWidth={1.5} aria-hidden />
          </button>
        </Tooltip>
        {setting.restart && (
          <span className="inline-flex h-4.5 shrink-0 items-center rounded-control border border-warning px-1.5 text-warning text-xs">
            Applies after restart
          </span>
        )}
        {modified && (
          <Tooltip label="Reset to default">
            <button
              type="button"
              aria-label={`Reset “${setting.label}” to its default`}
              onClick={() => void configReset(setting.path).catch(reportError)}
              className="flex h-6 w-6 shrink-0 items-center justify-center rounded-control text-fg-2 opacity-0 hover:bg-hover hover:text-fg focus-visible:opacity-100 group-hover:opacity-100"
            >
              <RotateCcw size={14} strokeWidth={1.5} aria-hidden />
            </button>
          </Tooltip>
        )}
      </div>
      <div className={wide ? "pl-3" : "flex max-w-full shrink-0 justify-end"}>
        <SettingControl
          control={setting.control}
          label={setting.label}
          value={value}
          defaultValue={fallback}
          onChange={set}
        />
      </div>
      {query.trim() !== "" && (
        <p className="w-full pl-3 text-fg-3 text-sm">
          <Marked text={setting.tooltip} query={query} />{" "}
          <span className="font-mono text-xs">
            <Marked text={setting.path} query={query} />
          </span>
        </p>
      )}
    </div>
  );
}

function ResetPage({ settings }: { settings: Setting[] }) {
  const modified = useConfigStore((s) => {
    const { config, defaults } = s;
    if (!config || !defaults) return 0;
    return settings.filter((x) => !sameValue(valueAt(config, x.path), valueAt(defaults, x.path)))
      .length;
  });
  const reset = async () => {
    const { config, defaults } = useConfigStore.getState();
    if (!config || !defaults) return;
    for (const setting of settings) {
      if (!sameValue(valueAt(config, setting.path), valueAt(defaults, setting.path))) {
        await configReset(setting.path);
      }
    }
  };
  return (
    <div className="flex justify-end pt-4">
      <Button keepFocus disabled={modified === 0} onClick={() => void reset().catch(reportError)}>
        Reset this page
      </Button>
    </div>
  );
}

function CategoryPage({ category, settings }: { category: Category; settings: Setting[] }) {
  const setPage = useUiStore((s) => s.setSettingsPage);
  const presetCount = usePresetStore((s) => s.presets.length);
  return (
    <>
      {category === "controls" && (
        <h4 className={`flex h-7 items-center ${HEADING}`}>Steps and pointer</h4>
      )}
      {settings.map((setting) => (
        <SettingRow key={setting.path} setting={setting} query="" />
      ))}
      {category === "playback" && (
        <button
          type="button"
          data-open-presets
          onClick={() => setPage(PRESETS_PAGE)}
          className="flex min-h-10 w-full items-center gap-1.5 border-line border-b py-2 pl-3 text-left text-base text-fg hover:bg-hover"
        >
          <span className="flex-1">Presets</span>
          <span className="text-fg-3 text-sm">
            {presetCount === 0 ? "None" : `${presetCount} saved`}
          </span>
          <ChevronRight size={16} strokeWidth={1.5} className="text-fg-2" aria-hidden />
        </button>
      )}
      {category === "library" && <LibraryFolders />}
      {category === "performance" && <CacheRow />}
      <ResetPage settings={settings} />
      {category === "controls" && (
        <>
          <h4 className={`mt-4 flex h-7 items-center border-line border-t pt-2 ${HEADING}`}>
            Shortcuts
          </h4>
          <HotkeyEditor />
        </>
      )}
    </>
  );
}

export function SettingsOverlay() {
  const open = useUiStore((s) => s.overlay === "settings");
  const page = useUiStore((s) => s.settingsPage);
  const setPage = useUiStore((s) => s.setSettingsPage);
  const setOverlay = useUiStore((s) => s.setOverlay);
  const setKeyContext = useUiStore((s) => s.setKeyContext);
  const hold = useUiStore((s) => s.hold);
  const searchRequest = useUiStore((s) => s.settingsSearchRequest);
  const info = useAppStore((s) => s.info);
  const searchKey = useShortcut("settings.focus_search");
  const [query, setQuery] = useState("");
  const input = useRef<HTMLInputElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  const scroller = useRef<HTMLDivElement>(null);

  const available = useMemo(
    () => SETTING_LIST.filter((s) => isAvailable(s, info?.capabilities)),
    [info],
  );
  const hits = useMemo(() => searchSettings(query, available), [query, available]);
  const searching = query.trim() !== "";

  // Open: hold the bars, take the keyboard context and focus. Close: give them back.
  useEffect(() => {
    if (!open) return;
    hold("settings", true);
    setKeyContext("settings");
    panel.current?.focus({ preventScroll: true });
    return () => {
      hold("settings", false);
      setKeyContext("global");
    };
  }, [open, hold, setKeyContext]);

  useEffect(() => {
    if (open && searchRequest > 0) {
      input.current?.focus();
      input.current?.select();
    }
  }, [open, searchRequest]);

  // A new page starts at its top.
  // biome-ignore lint/correctness/useExhaustiveDependencies: scrolls when the page or search changes
  useEffect(() => {
    scroller.current?.scrollTo({ top: 0 });
  }, [page, searching]);

  if (!open) return null;
  const close = () => {
    setOverlay(null);
    focusPlayerRoot();
  };
  const category = CATEGORIES.find((c) => c.id === page);
  const title = searching
    ? `${hits.length} ${hits.length === 1 ? "result" : "results"}`
    : page === PRESETS_PAGE
      ? "Presets"
      : (category?.label ?? "Settings");

  return (
    <div
      className="absolute inset-0 z-(--z-overlay) flex items-center justify-center bg-scrim"
      onPointerDown={(e) => {
        if (e.target === e.currentTarget) close();
      }}
    >
      <div
        ref={panel}
        role="dialog"
        aria-modal="true"
        aria-label="Settings"
        data-overlay="settings"
        tabIndex={-1}
        className="flex h-[calc(100%-32px)] max-h-160 w-[calc(100%-32px)] max-w-230 overflow-hidden rounded-surface border border-line-strong bg-surface shadow-float outline-none"
      >
        <nav
          aria-label="Settings categories"
          className="flex w-50 shrink-0 flex-col gap-2 border-line border-r bg-base p-2"
        >
          <label className="flex h-8 shrink-0 items-center gap-2 rounded-control border border-line bg-raised px-2 focus-within:border-accent">
            <Search size={14} strokeWidth={1.5} className="shrink-0 text-fg-3" aria-hidden />
            <input
              ref={input}
              type="text"
              value={query}
              placeholder={searchKey ? `Search…  (${searchKey})` : "Search…"}
              aria-label="Search settings"
              spellCheck={false}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape" && query !== "") {
                  e.preventDefault();
                  e.stopPropagation();
                  setQuery("");
                }
              }}
              className="h-full min-w-0 flex-1 bg-transparent text-base text-fg outline-none placeholder:text-fg-3"
            />
          </label>
          <ul className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto">
            {CATEGORIES.map(({ id, label }) => {
              const Icon = ICONS[id];
              const count = hits.filter((s) => s.category === id).length;
              const current =
                !searching && (page === id || (id === "playback" && page === PRESETS_PAGE));
              const dimmed = searching && count === 0;
              return (
                <li key={id}>
                  <button
                    type="button"
                    data-category={id}
                    aria-current={current ? "page" : undefined}
                    onClick={() => {
                      setQuery("");
                      setPage(id);
                    }}
                    className={`flex h-8 w-full items-center gap-2 rounded-control px-2 text-left text-base ${
                      current ? "bg-active text-fg" : "text-fg-2 hover:bg-hover hover:text-fg"
                    } ${dimmed ? "opacity-40" : ""}`}
                  >
                    <Icon size={18} />
                    <span className="min-w-0 flex-1 truncate">{label}</span>
                    {searching && count > 0 && (
                      <span className="tabular text-fg-3 text-xs">{count}</span>
                    )}
                  </button>
                </li>
              );
            })}
          </ul>
          <div className="flex shrink-0 flex-col gap-1 border-line border-t pt-2">
            <Button
              variant="ghost"
              keepFocus
              disabled={!info}
              onClick={() => {
                if (info) void appReveal(info.paths.config).catch(reportError);
              }}
            >
              Open config folder
            </Button>
            <span className="selectable px-2.5 font-mono text-fg-3 text-xs">
              vihpluh {info?.version ?? ""}
            </span>
          </div>
        </nav>

        <div className="flex min-w-0 flex-1 flex-col">
          <header className="flex h-12 shrink-0 items-center gap-2 border-line border-b pr-2 pl-5">
            {page === PRESETS_PAGE && !searching && (
              <Tooltip label="Back to Playback">
                <button
                  type="button"
                  aria-label="Back to Playback"
                  onClick={() => setPage("playback")}
                  className="-ml-2 flex h-7 w-7 items-center justify-center rounded-control text-fg-2 hover:bg-hover hover:text-fg"
                >
                  <ChevronLeft size={16} strokeWidth={1.5} aria-hidden />
                </button>
              </Tooltip>
            )}
            <h2 className="min-w-0 flex-1 truncate font-medium text-fg text-lg">{title}</h2>
            <Tooltip label="Close" shortcut="Escape">
              <button
                type="button"
                aria-label="Close settings"
                onClick={close}
                className="flex h-7 w-7 items-center justify-center rounded-control text-fg-2 hover:bg-hover hover:text-fg"
              >
                <X size={16} strokeWidth={1.5} aria-hidden />
              </button>
            </Tooltip>
          </header>
          <div
            ref={scroller}
            data-settings-page={page}
            className="min-h-0 flex-1 overflow-y-auto px-5 pb-5"
          >
            {searching ? (
              hits.length === 0 ? (
                <p className="py-8 text-center text-base text-fg-3">
                  No setting matches “{query.trim()}”.
                </p>
              ) : (
                CATEGORIES.filter((c) => hits.some((s) => s.category === c.id)).map((c) => (
                  <section key={c.id} aria-label={c.label}>
                    <h3 className={`flex h-8 items-end pb-1 ${HEADING}`}>{c.label}</h3>
                    {hits
                      .filter((s) => s.category === c.id)
                      .map((setting) => (
                        <SettingRow key={setting.path} setting={setting} query={query} />
                      ))}
                  </section>
                ))
              )
            ) : page === PRESETS_PAGE ? (
              <PresetsManager />
            ) : category ? (
              <CategoryPage
                category={category.id}
                settings={available.filter((s) => s.category === category.id)}
              />
            ) : null}
          </div>
        </div>
      </div>
    </div>
  );
}
