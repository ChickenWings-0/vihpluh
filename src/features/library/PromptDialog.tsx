/**
 * A one-line question over a scrim (Open URL, name a playlist). Asked for explicitly by an
 * action, never shown by itself. Enter confirms, Escape cancels; playback carries on behind.
 */
import { type FormEvent, useEffect, useState } from "react";
import { focusPlayerRoot } from "@/actions/definitions/window";
import { Button } from "@/components/Button";
import { reportError } from "@/stores/osd";
import { useUiStore } from "@/stores/ui";

export function PromptDialog() {
  const prompt = useUiStore((s) => s.prompt);
  const ask = useUiStore((s) => s.ask);
  const hold = useUiStore((s) => s.hold);
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setText(prompt?.initial ?? "");
    setBusy(false);
    hold("prompt", prompt !== null);
    return () => hold("prompt", false);
  }, [prompt, hold]);

  if (!prompt) return null;
  const close = () => {
    ask(null);
    focusPlayerRoot();
  };
  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (busy || text.trim() === "") return;
    setBusy(true);
    try {
      await prompt.onSubmit(text);
      close();
    } catch (error) {
      reportError(error);
      setBusy(false);
    }
  };

  return (
    <div
      className="absolute inset-0 z-(--z-overlay) flex items-start justify-center bg-scrim pt-[18vh]"
      onPointerDown={(e) => {
        if (e.target === e.currentTarget) close();
      }}
    >
      <form
        role="dialog"
        aria-modal="true"
        aria-label={prompt.title}
        onSubmit={(e) => void submit(e)}
        onKeyDown={(e) => {
          if (e.key === "Escape") {
            e.preventDefault();
            e.stopPropagation();
            close();
          }
        }}
        className="flex w-110 max-w-[calc(100%-32px)] flex-col gap-3 rounded-surface border border-line-strong bg-raised p-4 shadow-float"
      >
        <h2 className="font-medium text-fg text-md">{prompt.title}</h2>
        <label className="flex flex-col gap-1 text-fg-2 text-sm">
          {prompt.label}
          <input
            type="text"
            // biome-ignore lint/a11y/noAutofocus: the dialog was opened to be typed into
            autoFocus
            value={text}
            placeholder={prompt.placeholder}
            onFocus={(e) => e.target.select()}
            onChange={(e) => setText(e.target.value)}
            className="h-8 w-full rounded-control border border-line bg-surface px-2 text-base text-fg outline-none placeholder:text-fg-3 focus:border-accent"
          />
        </label>
        <div className="flex justify-end gap-2">
          <Button variant="ghost" keepFocus onClick={close}>
            Cancel
          </Button>
          <Button variant="primary" keepFocus type="submit" disabled={busy || text.trim() === ""}>
            {prompt.confirm}
          </Button>
        </div>
      </form>
    </div>
  );
}
