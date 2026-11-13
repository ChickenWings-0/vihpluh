/**
 * Global keyboard dispatch (KEYBINDS_AND_CONFIG §3.2): normalise, apply the text-field and
 * focused-control rules, look up the active context then global, dispatch.
 *
 * Media keys are not handled here: they reach the core through the OS media controls (SMTC /
 * MPRIS) even when vihpluh is unfocused, and handling them here too would toggle twice.
 */
import { useEffect } from "react";
import { dispatch } from "@/actions";
import { CONTROL_KEYS, resolveBinding } from "@/keybinds/engine";
import { bindingFromKeyboard } from "@/keybinds/normalize";
import { useKeymapStore } from "@/stores/keymap";
import { useUiStore } from "@/stores/ui";

function isTextInput(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  if (target instanceof HTMLTextAreaElement) return true;
  if (target instanceof HTMLInputElement) {
    return !["button", "checkbox", "radio", "range", "submit", "reset", "color", "file"].includes(
      target.type,
    );
  }
  return false;
}

/** A control the user reached with the keyboard keeps its native keys (§3.2 rule 3). */
function ownsNativeKeys(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement) || target.id === "player-root") return false;
  if (target === document.body) return false;
  const interactive = target.matches(
    "button, select, [role=slider], [role=menuitem], [role=tab], [role=option], a[href]",
  );
  return interactive && target.matches(":focus-visible");
}

/**
 * A menu, popover, or dialog is open: it handles Escape itself. Tooltips do not count — they
 * are transient and close on Escape too, but must not swallow the key (a lingering hover
 * tooltip would otherwise block leaving fullscreen).
 */
function floatingLayerOpen(): boolean {
  return (
    document.querySelector(
      "[data-radix-popper-content-wrapper] [role=menu], [data-radix-popper-content-wrapper] [role=dialog], [role=dialog]",
    ) !== null
  );
}

export function useKeyboard(): void {
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.isComposing) return;
      if (e.code.startsWith("Media")) return;
      const binding = bindingFromKeyboard(e);
      if (!binding) return;
      if (binding === "Escape" && floatingLayerOpen()) return;
      const ui = useUiStore.getState();
      const match = resolveBinding(useKeymapStore.getState().lookup, binding, {
        context: ui.keyContext,
        inTextInput: isTextInput(e.target),
        focusedControlKeys: ownsNativeKeys(e.target) ? CONTROL_KEYS : new Set(),
      });
      if (!match) return;
      e.preventDefault();
      e.stopPropagation();
      if (match.kind === "custom") {
        void dispatch(match.binding.action, {
          trigger: "key",
          binding,
          args: match.binding.args,
          repeat: e.repeat,
        });
      } else {
        void dispatch(match.actionId, { trigger: "key", binding, repeat: e.repeat });
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);
}
