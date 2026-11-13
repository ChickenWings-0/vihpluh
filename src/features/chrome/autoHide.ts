/**
 * Chrome visibility (DESIGN_SYSTEM §7.1), as a plain controller so it can be tested with fake
 * timers. React wiring lives in `useAutoHide`.
 *
 * - The bars hide after `delayMs` without pointer movement, only while playing, and never
 *   while something holds them (pointer over a bar, a menu open, a slider dragged, focus in
 *   the chrome).
 * - In a window, auto-hide can be switched off (`interface.autohide_in_windowed`).
 * - In fullscreen the top bar appears only when the pointer reaches the top edge zone or a
 *   top-bar control has keyboard focus; ordinary pointer movement shows the bottom bar only.
 * - Keyboard actions never reveal the bars (they flash the OSD instead); `reveal()` is the
 *   one exception, used by `player.show_progress`.
 * - Zen hides everything, including the cursor, and ignores the pointer.
 */

export interface AutoHideInput {
  playing: boolean;
  fullscreen: boolean;
  autohideWindowed: boolean;
  held: boolean;
  zen: boolean;
  /** A control in the top bar has keyboard focus (fullscreen rule). */
  topFocused: boolean;
}

export interface Visibility {
  bottom: boolean;
  top: boolean;
  cursorHidden: boolean;
}

export const TOP_EDGE_PX = 48;
/** How long `reveal()` keeps the bottom bar up. */
export const REVEAL_MS = 1500;

export class AutoHide {
  private input: AutoHideInput = {
    playing: false,
    fullscreen: false,
    autohideWindowed: true,
    held: false,
    zen: false,
    topFocused: false,
  };
  private idle = false;
  private atTopEdge = false;
  private revealedUntil = 0;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private revealTimer: ReturnType<typeof setTimeout> | undefined;
  private last: Visibility | undefined;

  constructor(
    private delayMs: number,
    private readonly onChange: (v: Visibility) => void,
    private readonly now: () => number = Date.now,
  ) {}

  setDelay(delayMs: number): void {
    this.delayMs = delayMs;
  }

  setInput(input: AutoHideInput): void {
    this.input = input;
    if (!this.canHide()) {
      this.idle = false;
      this.clearTimer();
    } else if (!this.idle && !this.timer) {
      this.startTimer();
    }
    this.emit();
  }

  /** Any pointer movement over the window. `y` is the CSS pixel offset from the top. */
  pointerMove(y: number): void {
    if (this.input.zen) return;
    this.idle = false;
    this.atTopEdge = y < TOP_EDGE_PX;
    this.startTimer();
    this.emit();
  }

  /** The pointer left the window: the top-edge zone no longer applies. */
  pointerLeave(): void {
    this.atTopEdge = false;
    this.emit();
  }

  /** Shows the bottom bar briefly without counting as pointer activity. */
  reveal(): void {
    this.revealedUntil = this.now() + REVEAL_MS;
    if (this.revealTimer) clearTimeout(this.revealTimer);
    this.revealTimer = setTimeout(() => this.emit(), REVEAL_MS);
    this.emit();
  }

  dispose(): void {
    this.clearTimer();
    if (this.revealTimer) clearTimeout(this.revealTimer);
  }

  compute(): Visibility {
    const { zen, fullscreen, held, topFocused } = this.input;
    if (zen) return { bottom: false, top: false, cursorHidden: true };
    const hidden = this.canHide() && this.idle;
    const revealed = this.now() < this.revealedUntil;
    const bottom = !hidden || revealed;
    const top = fullscreen ? this.atTopEdge || topFocused || (held && this.atTopEdge) : !hidden;
    return { bottom, top, cursorHidden: !bottom && !top };
  }

  private canHide(): boolean {
    const { playing, fullscreen, autohideWindowed, held } = this.input;
    return playing && !held && (fullscreen || autohideWindowed);
  }

  private startTimer(): void {
    this.clearTimer();
    if (!this.canHide()) return;
    this.timer = setTimeout(() => {
      this.timer = undefined;
      this.idle = true;
      this.emit();
    }, this.delayMs);
  }

  private clearTimer(): void {
    if (this.timer) clearTimeout(this.timer);
    this.timer = undefined;
  }

  private emit(): void {
    const next = this.compute();
    const last = this.last;
    if (
      !last ||
      last.bottom !== next.bottom ||
      last.top !== next.top ||
      last.cursorHidden !== next.cursorHidden
    ) {
      this.last = next;
      this.onChange(next);
    }
  }
}
