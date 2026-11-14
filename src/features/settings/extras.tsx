/**
 * The two pages with more than settings on them: Library lists the folders the library is
 * built from (KEYBINDS_AND_CONFIG §5.8), Performance shows the disk cache and clears it
 * (§5.10).
 */
import { FolderPlus, RefreshCw, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Button } from "@/components/Button";
import { Chip, Select } from "@/components/Controls";
import { Tooltip } from "@/components/Tooltip";
import type { CacheInfo } from "@/ipc/bindings/CacheInfo";
import type { FolderKind } from "@/ipc/bindings/FolderKind";
import type { LibraryFolder } from "@/ipc/bindings/LibraryFolder";
import {
  appPickMedia,
  cacheClear,
  cacheInfo,
  libraryAddFolder,
  libraryFolders,
  libraryRemoveFolder,
  libraryRescan,
} from "@/ipc/commands";
import { onLibraryScan } from "@/ipc/events";
import { reportError } from "@/stores/osd";

const KINDS: { value: FolderKind; label: string }[] = [
  { value: "video", label: "Video" },
  { value: "music", label: "Music" },
  { value: "mixed", label: "Video and music" },
];

const HEADING = "font-medium text-fg-3 text-xs uppercase tracking-[0.06em]";

export function LibraryFolders() {
  const [folders, setFolders] = useState<LibraryFolder[]>([]);
  const load = useCallback(() => void libraryFolders().then(setFolders).catch(reportError), []);
  useEffect(() => {
    load();
    return onLibraryScan((progress) => {
      if (progress.done) load();
    });
  }, [load]);

  const add = async () => {
    const [path] = await appPickMedia("folder");
    if (!path) return;
    await libraryAddFolder(path, "mixed", false);
    load();
  };

  return (
    <section aria-label="Library folders" data-folders className="flex flex-col gap-1 pt-4">
      <div className="flex h-7 items-center gap-2">
        <h4 className={`flex-1 ${HEADING}`}>Folders</h4>
        <Button
          variant="ghost"
          icon={RefreshCw}
          keepFocus
          disabled={!folders.some((f) => f.indexed)}
          onClick={() => void libraryRescan().catch(reportError)}
        >
          Rescan all
        </Button>
        <Button icon={FolderPlus} keepFocus onClick={() => void add().catch(reportError)}>
          Add folder…
        </Button>
      </div>
      {folders.length === 0 ? (
        <p className="py-4 text-base text-fg-3">
          No folders yet. Add the folders that hold your videos and music.
        </p>
      ) : (
        <ul className="flex flex-col">
          {folders.map((folder) => (
            <li
              key={folder.id}
              data-folder={folder.id}
              className="flex min-h-9 items-center gap-2 border-line border-b py-1"
            >
              <span className="selectable min-w-0 flex-1 truncate font-mono text-fg text-sm">
                {folder.path}
              </span>
              {folder.indexed ? (
                <span className="tabular shrink-0 text-fg-3 text-sm">
                  {folder.count} {folder.count === 1 ? "file" : "files"}
                </span>
              ) : (
                <Chip>shortcut only</Chip>
              )}
              <Select
                options={KINDS}
                value={folder.kind}
                label={`What ${folder.path} holds`}
                className="w-40"
                onChange={(kind) =>
                  void libraryAddFolder(folder.path, kind, folder.pinned, folder.indexed)
                    .then(load)
                    .catch(reportError)
                }
              />
              <Tooltip label="Remove from the library (files stay on disk)">
                <button
                  type="button"
                  aria-label={`Remove ${folder.path} from the library`}
                  onClick={() => void libraryRemoveFolder(folder.id).then(load).catch(reportError)}
                  className="flex h-7 w-7 shrink-0 items-center justify-center rounded-control text-fg-2 hover:bg-danger-soft hover:text-danger"
                >
                  <Trash2 size={16} strokeWidth={1.5} aria-hidden />
                </button>
              </Tooltip>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

export function CacheRow() {
  const [info, setInfo] = useState<CacheInfo | null>(null);
  const [busy, setBusy] = useState(false);
  const load = useCallback(() => void cacheInfo().then(setInfo).catch(reportError), []);
  useEffect(load, [load]);

  const clear = async () => {
    setBusy(true);
    try {
      await cacheClear();
      setInfo(await cacheInfo());
    } finally {
      setBusy(false);
    }
  };
  const used = info ? Math.min(1, info.limitBytes > 0 ? info.bytes / info.limitBytes : 0) : 0;

  return (
    <section aria-label="Disk cache" data-cache className="flex flex-col gap-2 pt-4">
      <h4 className={`flex h-7 items-center ${HEADING}`}>Disk cache</h4>
      <div className="flex items-center gap-3">
        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <span className="tabular text-base text-fg">
            {info
              ? `${formatBytes(info.bytes)} of ${formatBytes(info.limitBytes)} used`
              : "Measuring…"}
          </span>
          <div className="h-1 w-full rounded-track bg-track">
            <div className="h-full rounded-track bg-accent" style={{ width: `${used * 100}%` }} />
          </div>
          <span className="text-fg-3 text-sm">
            Seek-bar thumbnails and waveforms. They are made again when needed.
          </span>
        </div>
        <Button
          keepFocus
          disabled={busy || !info || info.bytes === 0}
          onClick={() => void clear().catch(reportError)}
        >
          Clear cache
        </Button>
      </div>
    </section>
  );
}
