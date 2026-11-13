# vihpluh — Keybinds and Configuration

Source of truth for every **action**, every **default shortcut**, and every **setting**. If it is not in this
document it does not exist; adding one means adding it here in the same change.

Related: IPC and storage → [ARCHITECTURE](ARCHITECTURE.md); how settings and the hotkey editor look → [DESIGN_SYSTEM](DESIGN_SYSTEM.md) §11.2.

## Contents

1. [Model](#1-model)
2. [Action registry and default shortcuts](#2-action-registry-and-default-shortcuts)
3. [Key notation and `keybinds.toml`](#3-key-notation-and-keybindstoml)
4. [Hotkey editor and shortcut OSD badge](#4-hotkey-editor-and-shortcut-osd-badge)
5. [`config.toml` reference](#5-configtoml-reference)
6. [Settings UI rules](#6-settings-ui-rules)
7. [Worked examples](#7-worked-examples)

---

## 1. Model

```
 shipped defaults (compiled in)          user files (config dir, only overrides)
 ───────────────────────────────         ──────────────────────────────────────
 resources/default.toml            ⊕     config.toml        →  effective Config
 resources/keymaps/<base>.toml     ⊕     keybinds.toml      →  effective Keymap
 resources/eq-presets.toml         ⊕     eq-presets.toml    →  EQ preset list
```

- **Actions** are the unit of behaviour. A button, a menu item, a context-menu entry, a shortcut, and the
  command palette all dispatch the same action ID. Actions take no free-form arguments from the UI; step sizes come from config.
- **Shortcuts** bind key strings to action IDs, per **context**.
- **Settings** are typed keys grouped into ten categories. The settings UI is generated from one schema; the file on disk holds only what the user changed.
- User files live in the OS config directory (ARCHITECTURE §9.1), never in the repo. Both files are watched: saving an edit in a text editor applies it live.
- Files carry `schema = 1`. Unknown keys are preserved and reported once in the log (never deleted); invalid values fall back to the default and raise a toast naming the key.

---

## 2. Action registry and default shortcuts

Default keymap `vihpluh` — a blend: YouTube/mpv muscle memory for transport, VLC letters for tracks and sync.
Keys are physical US-layout positions (§3.1). "—" = unbound by default.

### Playback

| Action ID | Label | Default keys |
|---|---|---|
| `player.toggle_pause` | Play / Pause | `Space`, `K`, `MouseLeft` |
| `player.stop` | Stop and close media | `Ctrl+W` |
| `player.seek_back_small` / `player.seek_forward_small` | Seek ∓ small step (5 s) | `Left` / `Right`, `Shift+WheelDown` / `Shift+WheelUp` |
| `player.seek_back_medium` / `player.seek_forward_medium` | Seek ∓ medium step (10 s) | `J` / `L` |
| `player.seek_back_large` / `player.seek_forward_large` | Seek ∓ large step (60 s) | `Ctrl+Left` / `Ctrl+Right` |
| `player.seek_back_exact` / `player.seek_forward_exact` | Seek ∓ 1 s, frame-exact | `Shift+Left` / `Shift+Right` |
| `player.seek_percent_0` … `player.seek_percent_9` | Jump to 0 % … 90 % | `0` … `9` |
| `player.seek_start` | Jump to start | `Home` |
| `player.frame_step_back` / `player.frame_step_forward` | Previous / next frame (pauses) | `,` / `.` |
| `player.speed_down` / `player.speed_up` | Speed ∓ step (0.1×) | `[` / `]` |
| `player.speed_halve` / `player.speed_double` | Speed ÷2 / ×2 | `Shift+[` / `Shift+]` |
| `player.speed_reset` | Normal speed | `Backspace` |
| `player.toggle_reverse` | Reverse playback | — |
| `player.volume_up` / `player.volume_down` | Volume ± step (5) | `Up` / `Down`, `WheelUp` / `WheelDown` |
| `player.volume_up_fine` / `player.volume_down_fine` | Volume ± 1 | `Shift+Up` / `Shift+Down` |
| `player.toggle_mute` | Mute | `M` |
| `player.ab_loop_cycle` | A–B loop: set A → set B → clear | `Shift+L` |
| `player.sleep_timer_cycle` | Sleep timer: off → 15 → 30 → 45 → 60 → end of file | `Ctrl+T` |
| `player.screenshot` | Screenshot with subtitles | `S` |
| `player.screenshot_clean` | Screenshot without subtitles | `Shift+S` |
| `player.show_progress` | Flash time and progress | `O` |

### Queue, chapters, bookmarks

| Action ID | Label | Default keys |
|---|---|---|
| `queue.next` / `queue.previous` | Next / previous item | `N` / `P`, `MouseForward` / `MouseBack`, `MediaTrackNext` / `MediaTrackPrevious` |
| `queue.toggle_panel` | Show queue | `Q` |
| `queue.cycle_repeat` | Repeat: off → all → one | `R` |
| `queue.toggle_shuffle` | Shuffle | `Shift+R` |
| `queue.stop_after_current` | Stop after this item | — |
| `queue.clear` | Clear queue | — |
| `queue.save_as_playlist` | Save queue as playlist… | — |
| `markers.next` / `markers.previous` | Next / previous chapter or bookmark | `PageDown` / `PageUp`, `Shift+N` / `Shift+P` |
| `bookmarks.add` | Add bookmark here | `Ctrl+B` |
| `bookmarks.toggle_panel` | Show bookmarks and chapters | `B` |

### Audio

| Action ID | Label | Default keys |
|---|---|---|
| `audio.cycle_track` | Next audio track | `A` |
| `audio.cycle_device` | Next audio output device | `Shift+A` |
| `audio.delay_decrease` / `audio.delay_increase` | Audio delay ∓ step (50 ms) | `Ctrl+-` / `Ctrl+=` |
| `audio.delay_reset` | Reset audio delay | — |
| `audio.toggle_equalizer` | Equalizer on/off | `E` |
| `audio.toggle_normalize` | Volume normalisation on/off | — |

### Subtitles

| Action ID | Label | Default keys |
|---|---|---|
| `subtitles.cycle_track` | Next subtitle track | `V` |
| `subtitles.toggle_visibility` | Show / hide subtitles | `Shift+V` |
| `subtitles.delay_decrease` / `subtitles.delay_increase` | Subtitle delay ∓ step (50 ms) | `G` / `H` |
| `subtitles.sync_previous_cue` / `subtitles.sync_next_cue` | Shift so the previous / next line shows now | `Ctrl+G` / `Ctrl+H` |
| `subtitles.delay_reset` | Reset subtitle delay | `Alt+G` |
| `subtitles.size_decrease` / `subtitles.size_increase` | Subtitle size ∓ | `Alt+-` / `Alt+=` |
| `subtitles.position_up` / `subtitles.position_down` | Move subtitles up / down | `Alt+Up` / `Alt+Down` |
| `subtitles.add_file` | Add subtitle file… | `Alt+V` |
| `subtitles.automatch` | Auto-match subtitles in folder | — |

### Video

| Action ID | Label | Default keys |
|---|---|---|
| `video.cycle_aspect` | Next aspect ratio | `X` |
| `video.toggle_fill` | Crop to fill window | `C` |
| `video.rotate_cw` | Rotate 90° | `Ctrl+R` |
| `video.flip_horizontal` / `video.flip_vertical` | Mirror / flip | — |
| `video.zoom_in` / `video.zoom_out` | Zoom ± | `=` / `-`, `Ctrl+WheelUp` / `Ctrl+WheelDown` |
| `video.reset_transform` | Reset zoom, pan, rotation, flip | `Shift+Backspace` |
| `video.toggle_deband` | Debanding on/off | — |
| `video.cycle_tone_mapping` | Next tone-mapping curve | — |

### Window and interface

| Action ID | Label | Default keys |
|---|---|---|
| `window.toggle_fullscreen` | Fullscreen | `F`, `F11`, `MouseLeftDouble` |
| `window.toggle_pip` | Picture-in-picture | `Ctrl+P` |
| `window.toggle_zen` | Zen mode (hide everything) | `Z`, `MouseMiddle` |
| `window.toggle_always_on_top` | Always on top | `T` |
| `window.minimize` | Minimize | — |
| `ui.escape` | Back out (see chain below) | `Escape` |
| `ui.toggle_sidebar` | Show sidebar | `\` |
| `ui.sidebar_subtitles` / `_audio` / `_video` / `_playback` / `_bookmarks` | Open sidebar tab | `Alt+1` / `Alt+2` / `Alt+3` / `Alt+4` / `Alt+5` |
| `ui.toggle_inspector` | Stream info | `I` |
| `ui.toggle_library` | Library | `Ctrl+L` |
| `ui.open_settings` | Settings | `Ctrl+,` |
| `ui.show_shortcuts` | Keyboard shortcuts | `F1`, `Shift+/` |
| `ui.command_palette` | Command palette | `Ctrl+Shift+P` |
| `ui.context_menu` | Context menu | `MouseRight`, `ContextMenu`, `Shift+F10` |
| `app.open_file` | Open file… | `Ctrl+O` |
| `app.open_folder` | Open folder… | `Ctrl+Shift+O` |
| `app.open_url` | Open URL… | `Ctrl+N` |
| `app.paste_and_play` | Play path/URL from clipboard | `Ctrl+V` |
| `app.reveal_current` | Show current file in file manager | — |
| `presets.save_current` | Save current settings as preset… | `Ctrl+S` |
| `presets.update_current` | Update applied preset | — |
| `app.quit` | Quit | `Ctrl+Q` |
| `mpv.command` | Raw mpv command (custom bindings only, §3.4) | — |

**`ui.escape` chain** — the first applicable step runs, then stops:
close menu/popover → return keyboard focus to the player → close settings / shortcuts / palette →
close library drawer → leave Zen → leave PiP → leave fullscreen. It never closes the dock and never pauses.
A hover tooltip also closes on Escape but does not use up the key.

### Context-scoped actions

Active only while focus is inside that panel; they take precedence over global bindings on the same key.

| Context | Action ID | Label | Default keys |
|---|---|---|---|
| `queue` | `queue.play_selected` | Play | `Enter` |
| `queue` | `queue.remove_selected` | Remove | `Delete` |
| `queue` | `queue.move_selected_up` / `_down` | Move up / down | `Alt+Up` / `Alt+Down` |
| `queue` | `queue.select_all` | Select all | `Ctrl+A` |
| `library` | `library.play_selected` | Play now | `Enter` |
| `library` | `library.play_next_selected` | Play next | `Shift+Enter` |
| `library` | `library.enqueue_selected` | Add to queue | `Ctrl+Enter` |
| `library` | `library.focus_search` | Search | `Ctrl+F`, `/` |
| `library` | `library.go_up` | Up one level | `Backspace` |
| `bookmarks` | `bookmarks.rename_selected` | Rename | `F2` |
| `bookmarks` | `bookmarks.remove_selected` | Delete | `Delete` |
| `settings` | `settings.focus_search` | Search settings | `Ctrl+F`, `/` |

### Built-in alternative keymaps

`keybinds.toml` → `base = "vlc"` or `"mpv"` swaps the whole default layer for a faithful transcription of that
player's stock bindings, mapped onto the same action IDs (actions with no equivalent stay unbound). Shipped as
`resources/keymaps/vlc.toml` and `resources/keymaps/mpv.toml` (Phase 10). `base = "none"` starts empty.

---

## 3. Key notation and `keybinds.toml`

### 3.1 Notation

```
binding   := [modifier "+"]* key
modifier  := "Ctrl" | "Alt" | "Shift" | "Meta"        (always written in this order)
```

Keys name **physical positions on a US layout** (from `KeyboardEvent.code`), so bindings don't move when the
OS layout changes: `A`–`Z`, `0`–`9`, `F1`–`F24`, `Space`, `Enter`, `Escape`, `Tab`, `Backspace`, `Delete`, `Insert`,
`Home`, `End`, `PageUp`, `PageDown`, `Up`, `Down`, `Left`, `Right`, `ContextMenu`, and punctuation by its unshifted
US glyph: `` ` `` `-` `=` `[` `]` `\` `;` `'` `,` `.` `/`. Numpad: `Num0`–`Num9`, `NumAdd`, `NumSubtract`, `NumMultiply`, `NumDivide`, `NumEnter`, `NumDecimal`.

| Kind | Tokens |
|---|---|
| Mouse buttons | `MouseLeft`, `MouseLeftDouble`, `MouseMiddle`, `MouseRight`, `MouseBack`, `MouseForward` |
| Wheel | `WheelUp`, `WheelDown`, `WheelLeft`, `WheelRight` |
| Media keys | `MediaPlayPause`, `MediaStop`, `MediaTrackNext`, `MediaTrackPrevious` (delivered by the OS even when unfocused; `MediaPlayPause` → `player.toggle_pause` is built in). They reach vihpluh only through the OS media controls (SMTC / MPRIS), which map them to fixed actions; rebinding them in `keybinds.toml` has no effect. |

Rules:

- Mouse and wheel bindings apply **only over the video area** (never over bars, panels, or menus).
- `MouseLeft` fires immediately on click; a double-click therefore toggles pause twice (net zero) and then runs `MouseLeftDouble` — no click delay.
- Single strokes only; no chords/sequences in v1.
- `Tab` and `Shift+Tab` are reserved for focus traversal and cannot be bound. `Meta`-only combinations are discouraged (the OS usually owns them).
- The hotkey editor displays keys with the *current layout's* glyph (via the Keyboard Layout Map API when the webview provides it, else the US name).

### 3.2 Resolution

1. Normalise the event to a binding string (`keybinds/normalize.ts`). Ignore pure-modifier presses and auto-repeat for non-repeatable actions (toggles); allow repeat for seek/volume/speed/delay/zoom steps.
2. If focus is in a text input: only bindings containing `Ctrl` or `Alt`, plus `Escape`, are considered.
3. If a control has keyboard focus (`:focus-visible`), its native keys (`Space`, `Enter`, arrows, `Home`/`End`) are left to it.
4. Look up the binding in the **active context** (`queue`, `library`, `bookmarks`, `settings`), then in `global`. First hit wins.
5. Check the action's `isEnabled()`; if disabled, do nothing (no OSD, no beep).
6. Dispatch. If chrome is hidden, show the OSD flash for the action (DESIGN_SYSTEM §8.9).

### 3.3 `keybinds.toml` format

The file layers on top of the chosen base keymap. An action listed here **replaces** that action's default keys entirely.

```toml
schema = 1
base   = "vihpluh"                 # "vihpluh" | "vlc" | "mpv" | "none"

[bind]                             # global context:  "action.id" = ["Key", …]
"player.toggle_pause"      = ["Space", "K", "MouseLeft"]
"window.toggle_zen"        = ["Z"]                 # drop the middle-click default
"player.seek_back_medium"  = []                    # unbind
"ui.toggle_sidebar"        = ["`"]

[context.queue.bind]               # per-context overrides
"queue.remove_selected"    = ["Delete", "Backspace"]
```

Conflict rule when loading: if two actions in the same context claim one key, the **user file wins over the base**;
between two user entries the later line wins, and a warning toast names both actions.

### 3.4 Custom bindings (parameterised)

For one-off needs that don't deserve a built-in action. Evaluated before `[bind]`.

```toml
[[custom]]
keys   = ["Ctrl+Shift+Right"]
action = "player.seek_relative"
args   = { seconds = 300 }

[[custom]]
keys   = ["F9"]
action = "player.set_speed"
args   = { value = 1.75 }

[[custom]]
keys   = ["Ctrl+D"]
label  = "Toggle deinterlace"       # shown in the OSD and the shortcuts sheet
action = "mpv.command"
args   = { command = ["cycle", "deinterlace"] }
```

Parameterised actions available to `[[custom]]` only:

| Action | Args |
|---|---|
| `player.seek_relative` | `{ seconds: number, exact?: bool }` |
| `player.seek_absolute` | `{ seconds: number }` or `{ percent: number }` |
| `player.set_speed` | `{ value: number }` |
| `player.set_volume` | `{ value: number }` |
| `audio.set_delay` / `subtitles.set_delay` | `{ ms: number }` |
| `audio.set_equalizer_preset` | `{ id: string }` |
| `video.set_aspect` | `{ value: string }` |
| `presets.apply` | `{ id: string }` |
| `player.set_sleep_timer` | `{ minutes: number }` |
| `mpv.command` | `{ command: string[] }` — escape hatch; passed to mpv verbatim |

---

## 4. Hotkey editor and shortcut OSD badge

**Settings → Controls** is the visual editor for everything in §2–§3.

```
┌ Controls ─────────────────────────────────────────────────────────────────┐
│ Keymap  [ vihpluh ▾ ]     🔍 Search actions or press a key…   [⌨ Find by key] │
│                                                          [Reset all…]     │
│ PLAYBACK                                                                  │
│ Play / Pause                         [Space] [K] [Click]            [+]   │
│ Seek back 5 s                        [←] [Shift+Wheel↓]             [+]   │
│ • Seek forward 10 s                  [;]                        ⟲   [+]   │  ← modified (accent dot)
│ Next frame                           [.]                            [+]   │
│ …                                                                         │
│ CUSTOM                                                                    │
│ Toggle deinterlace  (mpv: cycle deinterlace)   [Ctrl+D]            [✕]    │
└───────────────────────────────────────────────────────────────────────────┘
```

| Behaviour | Spec |
|---|---|
| Layout | Actions grouped as in §2, one 32 px row each: label · key caps · `+`. Context-scoped groups carry a chip ("Queue", "Library"). |
| Search | Filters by label and action ID. **Find by key**: toggle it, press any combination, and the list filters to whatever that key triggers. |
| Add | `+` (or Enter on the row) enters capture: the row shows "Press a key… (Esc to cancel)". The next non-modifier key/mouse/wheel event becomes the binding. |
| Edit / remove | Click a key cap → capture replaces it. Hover a cap → `✕` removes it. `Backspace` on a focused cap removes it. |
| Conflicts | If the captured key is already bound in the same context, an inline bar appears: "`Ctrl+B` is used by **Add bookmark here**. [Replace] [Keep both off] [Cancel]". Nothing changes until chosen. Reserved keys (`Tab`, `Shift+Tab`) are refused with a reason. |
| Reset | ⟲ per row (only when modified); "Reset all…" asks once. |
| Persistence | Each change is written to `keybinds.toml` immediately via `keybinds_set`; only differences from the base are stored. |
| Keymap select | Switches `base`; user overrides are kept and re-layered. |

**Shortcuts sheet** (`F1`): a read-only two-column overlay of every bound action in the current keymap, searchable, generated from the same data.

**Shortcut OSD badge** (`interface.osd_shortcut_badge`, off by default): when an action is triggered by a key,
the OSD shows the key cap that fired it — e.g. `Subtitle delay  +50 ms  [H]`. Useful for learning a keymap and for confirming
what a key did. Actions that otherwise show no OSD get a minimal `label [key]` flash while this is on. Mouse-triggered actions show no badge.

---

## 5. `config.toml` reference

Dotted keys below are `[section] key`. **Label** is the text shown in Settings; **Tooltip** is the ⓘ text.
Types: `bool`, `int`, `float`, `string`, `enum(a|b)`, `list<string>`, `color` (`#RRGGBB`).
Settings marked ⟳ apply after restart; everything else applies live.

### 5.1 General

| Key | Type | Default | Label | Tooltip |
|---|---|---|---|---|
| `general.restore_session` | bool | `true` | Reopen last session | Bring back the queue and position from last time, paused. |
| `general.single_instance` ⟳ | bool | `true` | Use a single window | Files opened from the file manager go to the window that's already open. |
| `general.open_behavior` | enum(`replace`\|`enqueue`\|`play_next`) | `replace` | When opening files while something is playing | Replace the queue, add to its end, or play right after the current item. |
| `general.pause_on_minimize` | bool | `false` | Pause when minimized | Pauses video (not music) when the window is minimized. |
| `general.media_keys` ⟳ | bool | `true` | Media keys and system media controls | Respond to play/pause/next keys and show what's playing in the OS media overlay. |
| `general.screenshot_dir` | string | `""` | Screenshot folder | Empty = your Pictures folder, in "vihpluh". *(With `--home`, `VIHPLUH_HOME`, or portable mode, empty means `<home>/screenshots` instead.)* |
| `general.screenshot_format` | enum(`png`\|`jpg`\|`webp`) | `png` | Screenshot format | PNG is lossless; JPG and WebP are smaller. |
| `general.screenshot_template` | string | `"%F-%wH.%wM.%wS"` | Screenshot file name | mpv template: %F = file name, %wH.%wM.%wS = timestamp. |
| `general.log_level` | enum(`error`\|`warn`\|`info`\|`debug`\|`trace`) | `warn` | Log detail | Higher levels help diagnose problems but write more to disk. |

### 5.2 Interface

| Key | Type | Default | Label | Tooltip |
|---|---|---|---|---|
| `interface.theme` | enum(`dark`\|`light`\|`system`) | `dark` | Theme | "System" follows your OS setting. |
| `interface.accent` | color | `"#FF6B00"` | Accent colour | Used for progress, selection, and active controls. |
| `interface.ui_scale` | float 0.8–1.5 | `1.0` | Interface size | Scales all controls and text. Doesn't affect video or subtitles. |
| `interface.window_decorations` ⟳ | enum(`custom`\|`native`) | `custom` | Title bar | Custom blends into the player; native uses your OS window frame. |
| `interface.autohide_delay_ms` | int 500–10000 | `2000` | Hide controls after | How long the mouse must be still before the bars hide. |
| `interface.autohide_in_windowed` | bool | `true` | Auto-hide controls when not fullscreen | Off keeps the bars visible in a window. |
| `interface.sidebar_mode` | enum(`auto`\|`dock`\|`overlay`) | `auto` | Side panels | Dock shrinks the video beside the panel; overlay floats the panel on top. Auto docks in a window and overlays in fullscreen. |
| `interface.sidebar_width` | int 320–480 | `360` | Side panel width | Also adjustable by dragging the panel edge. |
| `interface.seek_preview` | bool | `true` | Thumbnail previews on the seek bar | Shows the frame under the pointer. Local files only. |
| `interface.seek_waveform` | bool | `false` | Waveform seek bar | Draws the audio waveform in the seek bar so you can see quiet and loud parts. |
| `interface.seek_chapter_marks` | bool | `true` | Chapter marks on the seek bar | Splits the bar at chapter boundaries. |
| `interface.time_display` | enum(`total`\|`remaining`) | `total` | Right-hand time | Total length or time left. Click the time to switch. |
| `interface.osd_enabled` | bool | `true` | On-screen feedback | Briefly shows volume, seek position, and similar when controls are hidden. |
| `interface.osd_shortcut_badge` | bool | `false` | Show the shortcut I pressed | Adds the key you pressed to the on-screen feedback. |
| `interface.osd_position` | enum(`top-left`\|`top-center`\|`top-right`) | `top-center` | Feedback position | Where on-screen feedback appears. |
| `interface.osd_duration_ms` | int 300–5000 | `1200` | Feedback duration | How long on-screen feedback stays visible. |
| `interface.tooltip_shortcuts` | bool | `true` | Show shortcuts in tooltips | |
| `interface.reduce_motion` | enum(`system`\|`on`\|`off`) | `system` | Reduce motion | Turns off interface animation. |
| `interface.pip_width` | int 240–960 | `480` | Picture-in-picture width | Height follows the video's shape. |
| `interface.pip_corner` | enum(`top-left`\|`top-right`\|`bottom-left`\|`bottom-right`) | `bottom-right` | Picture-in-picture corner | Not available on Wayland, where the compositor places windows. |

### 5.3 Controls

Shortcuts themselves are edited in the hotkey editor (§4) and stored in `keybinds.toml`. These keys set step sizes and pointer behaviour.

| Key | Type | Default | Label | Tooltip |
|---|---|---|---|---|
| `controls.seek_small_s` | float | `5` | Small seek step | Seconds for ← / →. |
| `controls.seek_medium_s` | float | `10` | Medium seek step | Seconds for J / L. |
| `controls.seek_large_s` | float | `60` | Large seek step | Seconds for Ctrl+← / Ctrl+→. |
| `controls.precise_seek` | bool | `true` | Land exactly where I seek | Off jumps to the nearest keyframe: faster on very large files, less exact. |
| `controls.volume_step` | float | `5` | Volume step | Percent per key press or wheel notch. |
| `controls.speed_step` | float | `0.1` | Speed step | |
| `controls.sub_delay_step_ms` | int | `50` | Subtitle delay step | Milliseconds per key press. |
| `controls.audio_delay_step_ms` | int | `50` | Audio delay step | Milliseconds per key press. |
| `controls.sub_size_step` | float | `0.05` | Subtitle size step | Change in scale per key press. |
| `controls.zoom_step` | float | `0.1` | Zoom step | |
| `controls.drag_moves_window` | bool | `true` | Drag the video to move the window | |
| `controls.wheel_invert` | bool | `false` | Reverse scroll direction | Swaps what wheel-up and wheel-down do. |

### 5.4 Playback

| Key | Type | Default | Label | Tooltip |
|---|---|---|---|---|
| `playback.resume` | enum(`always`\|`ask`\|`never`) | `always` | Continue where I left off | "Ask" starts from the beginning and offers to resume. |
| `playback.remember_per_file` | bool | `true` | Remember tracks and delays per file | Restores the audio track, subtitle track, and sync you last used for each file. |
| `playback.auto_queue_folder` | bool | `true` | Queue the rest of the folder | Opening one file lines up the others in its folder so Next plays the next episode. |
| `playback.at_queue_end` | enum(`hold`\|`stop`\|`quit`) | `hold` | When the queue finishes | Hold keeps the last frame on screen; stop returns to the start screen. |
| `playback.autoplay` | bool | `true` | Start playing when a file opens | |
| `playback.default_speed` | float 0.25–4 | `1.0` | Default speed | |
| `playback.pitch_correction` | bool | `true` | Keep pitch when changing speed | Off makes voices higher or lower as speed changes. |
| `playback.sleep_action` | enum(`pause`\|`quit`) | `pause` | When the sleep timer ends | |
| `playback.finished_percent` | int 80–100 | `95` | Count as finished at | Past this point a file is marked watched and won't resume. |
| `playback.presets_enabled` | bool | `true` | Apply presets automatically | Presets are saved from the bar at the bottom of the side panel. |

The **Presets** manager (list, enable/disable, edit match rule, delete, reveal file) is a sub-page of this category.

### 5.5 Video

| Key | Type | Default | Label | Tooltip | mpv |
|---|---|---|---|---|---|
| `video.hwdec` | enum(`auto`\|`off`) or string | `auto` | Hardware decoding | Uses your graphics card to decode video. Turn off only to troubleshoot. A custom mpv value can be typed in the config file. | `hwdec` (ARCHITECTURE §5.4) |
| `video.gpu_api` ⟳ | enum(`auto`\|`d3d11`\|`vulkan`\|`opengl`) | `auto` | Graphics API (Windows) | Auto uses Direct3D 11, the most reliable choice with HDR. | `gpu-api` |
| `video.quality` | enum(`fast`\|`balanced`\|`high`) | `balanced` | Scaling quality | High looks sharper when upscaling but needs a stronger graphics card. | `profile=fast` / default / `profile=high-quality` |
| `video.tone_mapping` | string | `auto` | HDR tone mapping | How HDR video is fitted to a non-HDR screen. Auto picks a good curve. | `tone-mapping` |
| `video.hdr_passthrough` | enum(`auto`\|`off`) | `auto` | HDR passthrough | Sends HDR untouched to an HDR display. Windows only. | `target-colorspace-hint` |
| `video.target_peak` | `"auto"` or int (nits) | `"auto"` | Display peak brightness | Set if tone-mapped HDR looks too dark or too bright. | `target-peak` |
| `video.deband` | bool | `false` | Debanding | Smooths colour banding in gradients. | `deband` |
| `video.interpolation` | bool | `false` | Motion interpolation | Smooths motion by blending frames to match your display. | `interpolation` + `video-sync=display-resample` |
| `video.sync` | enum(`audio`\|`display-resample`) | `audio` | Timing source | Display sync reduces judder but slightly resamples audio. | `video-sync` |
| `video.default_aspect` | string | `auto` | Default aspect ratio | | `video-aspect-override` |
| `video.linux_force_xwayland` ⟳ | bool | `false` | Run through XWayland (Linux) | Compatibility switch if video misbehaves under Wayland. | sets `GDK_BACKEND=x11` |

### 5.6 Audio

| Key | Type | Default | Label | Tooltip | mpv |
|---|---|---|---|---|---|
| `audio.device` | string | `auto` | Output device | Auto follows the system default. | `audio-device` |
| `audio.preferred_languages` | list<string> | `["eng","en"]` | Preferred audio languages | First available language in this list is chosen. Use codes like eng, jpn. | `alang` |
| `audio.remember_volume` | bool | `true` | Remember volume | | — |
| `audio.default_volume` | float 0–150 | `100` | Starting volume | Used when volume isn't remembered. | `volume` |
| `audio.max_volume` | int 100–150 | `130` | Maximum volume | Above 100 % boosts quiet audio and can distort. | `volume-max` |
| `audio.normalize` | bool | `false` | Volume normalisation | Evens out loud and quiet moments. | `af` dynaudnorm |
| `audio.channels` | enum(`auto`\|`stereo`\|`5.1`\|`7.1`) | `auto` | Speaker layout | Auto matches your output device. | `audio-channels` |
| `audio.passthrough` | list<string> | `[]` | Pass through to receiver | Formats sent undecoded to an AV receiver: ac3, eac3, dts, dts-hd, truehd. | `audio-spdif` |
| `audio.exclusive` | bool | `false` | Exclusive output | Bypasses the system mixer. Other apps go silent while playing. | `audio-exclusive` |
| `audio.equalizer_enabled` | bool | `false` | Equalizer | | — |
| `audio.equalizer_preset` | string | `"flat"` | Equalizer preset | | — |
| `audio.gapless` | enum(`weak`\|`yes`\|`no`) | `weak` | Gapless playback | Removes the gap between tracks of the same album. | `gapless-audio` |
| `audio.replaygain` | enum(`off`\|`track`\|`album`) | `off` | ReplayGain | Uses loudness tags in music files to level volume. | `replaygain` |

### 5.7 Subtitles

| Key | Type | Default | Label | Tooltip | mpv |
|---|---|---|---|---|---|
| `subtitles.preferred_languages` | list<string> | `["eng","en"]` | Preferred subtitle languages | First available language in this list is chosen. | `slang` |
| `subtitles.show_by_default` | bool | `true` | Show subtitles by default | Off shows only forced subtitles (signs and foreign dialogue). | `sid` / `subs-fallback` |
| `subtitles.forced_only_when_audio_matches` | bool | `true` | Only forced subtitles when I understand the audio | If the audio is in one of your preferred audio languages, full subtitles stay off. | `subs-with-matching-audio` |
| `subtitles.auto_load` | enum(`off`\|`exact`\|`fuzzy`\|`all`) | `fuzzy` | Load matching subtitle files | Exact needs the same file name; fuzzy accepts similar names; all loads every subtitle in the folder. | `sub-auto` |
| `subtitles.extra_dirs` | list<string> | `["Subs","Subtitles","subs"]` | Subtitle subfolders | Folder names next to the video to search for subtitles. | `sub-file-paths` |
| `subtitles.font` | string | `""` | Font | Empty uses the system sans-serif. | `sub-font` |
| `subtitles.font_size` | float 10–120 | `42` | Font size | Exact size; presets S/M/L/XL in the side panel scale it. | `sub-font-size` |
| `subtitles.scale` | float 0.5–3 | `1.0` | Size multiplier | | `sub-scale` |
| `subtitles.bold` | bool | `false` | Bold | | `sub-bold` |
| `subtitles.color` | color | `"#FFFFFF"` | Text colour | | `sub-color` |
| `subtitles.outline_color` | color | `"#000000"` | Outline colour | | `sub-outline-color` |
| `subtitles.outline_size` | float 0–10 | `3.0` | Outline thickness | | `sub-outline-size` |
| `subtitles.shadow_offset` | float 0–10 | `0` | Shadow | | `sub-shadow-offset` |
| `subtitles.background` | enum(`none`\|`box`) | `none` | Background | A box improves legibility over busy scenes. | `sub-border-style`, `sub-back-color` |
| `subtitles.position` | int 0–100 | `100` | Vertical position | 100 = bottom. Lower moves subtitles up. | `sub-pos` |
| `subtitles.margin_y` | int 0–200 | `30` | Bottom margin | | `sub-margin-y` |
| `subtitles.ass_override` | enum(`no`\|`scale`\|`force`\|`strip`) | `scale` | Styled (ASS) subtitles | Keep their styling; keep it but apply my size; override with my style; or strip to plain text. | `sub-ass-override` |
| `subtitles.fix_timing` | bool | `false` | Close small gaps between lines | Removes flicker between consecutive lines. | `sub-fix-timing` |
| `subtitles.encoding` | string | `auto` | Text encoding | Change only if characters look garbled. | `sub-codepage` |

### 5.8 Library

Library and pinned folders are managed in the Library drawer and in this page (list with add/remove/kind), stored in the database.

| Key | Type | Default | Label | Tooltip |
|---|---|---|---|---|
| `library.track_history` | bool | `true` | Remember what I played | Needed for Recent and for continuing where you left off. Off writes no history. |
| `library.scan_on_startup` | bool | `true` | Check folders for changes at startup | |
| `library.watch_folders` | bool | `true` | Watch folders while running | Picks up new files without a rescan. |
| `library.group_series` | bool | `true` | Group episodes into series | Uses file names to group seasons and episodes. |
| `library.extra_extensions` | list<string> | `[]` | Additional file types | Extensions to treat as media, without the dot. |
| `library.ignore_patterns` | list<string> | `["**/.*","**/*sample*"]` | Ignore | Paths matching these patterns are skipped. |
| `library.recent_limit` | int 0–500 | `50` | Recent items to keep | |

### 5.9 Network

| Key | Type | Default | Label | Tooltip | mpv |
|---|---|---|---|---|---|
| `network.use_ytdlp` | bool | `true` | Play web page links with yt-dlp | Needs yt-dlp installed. Lets you paste video page addresses, not just direct streams. | `ytdl` |
| `network.ytdlp_format` | string | `"bestvideo[height<=?2160]+bestaudio/best"` | Preferred stream quality | yt-dlp format expression. | `ytdl-format` |
| `network.user_agent` | string | `""` | Browser identity | Some streams need a specific user agent. Empty uses the default. | `user-agent` |
| `network.proxy` | string | `""` | Proxy | For example http://127.0.0.1:8080. | `http-proxy` |
| `network.cache_secs` | int 1–600 | `30` | Stream buffer | Seconds of stream to buffer ahead. | `cache-secs` |
| `network.timeout_s` | int 1–120 | `20` | Connection timeout | | `network-timeout` |
| `network.auto_reconnect` | bool | `true` | Reconnect dropped streams | | `stream-lavf-o=reconnect…` |
| `network.hls_bitrate` | enum(`max`\|`min`) or int | `max` | Adaptive stream quality | | `hls-bitrate` |
| `network.load_remote_artwork` | bool | `false` | Load channel logos | Downloads images referenced by stream lists. Off keeps vihpluh fully offline unless you open a stream. | — |

### 5.10 Performance

| Key | Type | Default | Label | Tooltip | mpv |
|---|---|---|---|---|---|
| `performance.demuxer_cache_mb` | int 32–4096 | `512` | Read-ahead memory | Memory for buffering ahead. Larger helps high-bitrate files on slow or network drives. | `demuxer-max-bytes` |
| `performance.demuxer_back_cache_mb` | int 0–2048 | `128` | Seek-back memory | Memory kept behind the playhead so short rewinds are instant. | `demuxer-max-back-bytes` |
| `performance.readahead_secs` | int 1–120 | `20` | Read-ahead time | | `demuxer-readahead-secs` |
| `performance.software_threads` | int 0–64 | `0` | Software decoding threads | 0 = automatic. | `vd-lavc-threads` |
| `performance.thumbnail_hwdec` | string | `"auto-copy"` | Decoder for previews | Use "no" if previews cause stutter. | thumbnail handle `hwdec` |
| `performance.waveform` | enum(`on_demand`\|`always`\|`off`) | `on_demand` | Analyse audio for waveforms | On demand analyses a file the first time a waveform is shown. | — |
| `performance.cache_limit_mb` | int 64–16384 | `1024` | Disk cache limit | Space for thumbnails and waveforms. Oldest are removed first. | — |
| `performance.webkit_dmabuf` ⟳ | enum(`auto`\|`on`\|`off`) | `auto` | Interface GPU buffers (Linux) | Compatibility switch for blank or garbled interface on some NVIDIA setups. | sets `WEBKIT_DISABLE_DMABUF_RENDERER` |

A **Clear cache** button and current cache size sit at the bottom of this page.

### 5.11 Advanced — file only, no UI

```toml
[mpv]
load_user_config = false        # true: also read the user's own mpv.conf (never input.conf)

[mpv.options]                   # raw mpv options, applied last; override anything above
# scale = "ewa_lanczossharp"
# glsl-shaders = "~~/shaders/FSRCNNX_x2_8-0-4-1.glsl"

[env]                           # environment variables set before toolkit init (Linux troubleshooting)
# WEBKIT_DISABLE_COMPOSITING_MODE = "1"
```

`[mpv.options]` and `[env]` are unsupported power-user territory: values are passed through unvalidated and logged at startup.

### 5.12 `eq-presets.toml`

```toml
schema = 1

[[preset]]
id     = "dialogue"
name   = "Dialogue"
mode   = "graphic10"                       # "graphic10" | "parametric"
preamp = -2.0
gains  = [-2, -2, -1, 0, 2, 4, 4, 2, 0, -1]   # dB for 31,62,125,250,500,1k,2k,4k,8k,16k Hz

[[preset]]
id     = "my-headphones"
name   = "HD 600"
mode   = "parametric"
preamp = -4.5
bands  = [
  { type = "lowshelf",  freq = 105,  gain = 5.5, q = 0.70 },
  { type = "peak",      freq = 3300, gain = -2.0, q = 2.00 },
  { type = "highshelf", freq = 10000, gain = 1.5, q = 0.70 },
]
```

Built-ins (in `resources/eq-presets.toml`): `flat`, `bass`, `treble`, `dialogue`, `night`, `loudness`, `rock`, `classical`, `electronic`.
A user preset with a built-in's `id` overrides it.

---

## 6. Settings UI rules

1. **One schema.** `src/features/settings/schema.ts` lists every key from §5.1–§5.10 with `{ path, category, label, tooltip, control, keywords, restart?, platforms? }`.
   A unit test fails if a key in the Rust `Config` struct has no schema entry (or vice versa), or if a label/tooltip is missing.
2. **Ten flat categories**, exactly: General · Interface · Controls · Playback · Video · Audio · Subtitles · Library · Network · Performance. No nesting deeper than one sub-page (Presets under Playback). No "Advanced" toggle, no "Show all settings".
3. **Labels** are plain English, ≤ 6 words, describe the outcome, and never contain an mpv option name. **Tooltips** answer "what does this do and when would I change it" in ≤ 2 sentences.
4. **Controls by type:** `bool` → toggle; `enum` ≤ 4 options → segmented control, more → select; bounded number → slider + exact field; `list<string>` → chip input; `color` → swatch + picker; path → field + Browse.
5. **Search** matches label, tooltip, keywords, and the dotted key (so pasting `subtitles.font_size` finds it).
6. **Platform gating:** settings that don't apply to the current platform/session (per `capabilities`) are hidden, not disabled.
7. Every row can be reset individually; each page has "Reset this page". Changed-from-default rows are marked.
8. Writes go through `config_set` one key at a time, preserve comments in the file, and are echoed back by `config://changed`.

---

## 7. Worked examples

**A minimal personal `config.toml`** (only what differs from defaults):

```toml
schema = 1

[interface]
accent = "#FF6B00"
seek_waveform = true
osd_shortcut_badge = true

[audio]
preferred_languages = ["jpn", "ja", "eng"]

[subtitles]
preferred_languages = ["eng", "en"]
font = "Gandhi Sans"
font_size = 46

[performance]
demuxer_cache_mb = 1024

[mpv.options]
dither-depth = "auto"
```

**A minimal personal `keybinds.toml`:**

```toml
schema = 1
base = "vihpluh"

[bind]
"player.seek_back_medium"    = ["J", "Num4"]
"player.seek_forward_medium" = ["L", "Num6"]
"window.toggle_zen"          = ["Z"]

[[custom]]
keys   = ["Ctrl+Shift+Right"]
action = "player.seek_relative"
args   = { seconds = 85 }            # skip a typical opening
```

**Adding a new setting (checklist for contributors):**
1. Add the field + default to `config/schema.rs` and `resources/default.toml`.
2. Add its row to the right table in §5 (label + tooltip written for a non-expert).
3. Add the schema entry in `src/features/settings/schema.ts`.
4. Wire it: on `config://changed` the owning module applies it live (or mark it ⟳).
5. `pnpm gen:bindings`, `pnpm check`, `cargo test` — the schema-parity test must pass.

**Adding a new action:** register it in `src/actions/definitions/<domain>.ts` (id, label, icon, `run`, `isEnabled`, `repeatable`),
add its row to §2, add a default key in `resources/keymaps/vihpluh.toml` only if it deserves one, and surface it in a menu only if it's an essential.
