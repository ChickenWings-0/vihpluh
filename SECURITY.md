# Security and privacy

vihpluh is a local media player. It has no accounts, no server, and nothing to sign in to. This page
describes what it does and does not do with your data and the network, and how to report a problem.

## Supported versions

Only the latest release (currently `0.1.0`) and `main` receive fixes.

## Privacy model

- **No telemetry, no analytics, no crash reporting, no auto-updater, no update checks.** vihpluh never
  phones home. Updating means installing a newer release yourself.
- **Local-only storage.** Settings, presets, playlists, the library database, history, bookmarks,
  thumbnails, waveforms, and logs are files on your own disk, in the OS config, data, and cache folders,
  or under one folder in portable mode (see the [User Guide §1.3](docs/USER_GUIDE.md#13-where-vihpluh-keeps-its-files)).
  Deleting those folders removes everything vihpluh knows.
- **History is optional.** `library.track_history = false` writes no history at all (no Recent, no resume).
- **Logs** hold events and, at `debug` level or above, file paths. They never hold file contents.

## Network

vihpluh itself connects out only when you ask it to:

| Connection | When |
|---|---|
| A stream you open | Open URL, a stream-list channel, a URL argument, or a URL in a playlist. |
| A stream list | When you add an `.m3u` from an address, or press Refresh on it. Nothing refreshes by itself. |
| yt-dlp | Only for web page links, only if yt-dlp is installed and `network.use_ytdlp` is on. yt-dlp runs as its own program. |
| Channel logos | Never in this version (`network.load_remote_artwork` is off by default, and logos are not displayed). |

Requests for streams and lists go through mpv's network stack with your `network.user_agent` and
`network.proxy`. vihpluh contains no HTTP client of its own.

**The system webview.** The interface runs in the OS webview (WebView2 on Windows, WebKitGTK on Linux).
vihpluh starts WebView2 with background networking, component updates, reliability reports, and pings
switched off. Even so, on Windows the WebView2 runtime's own processes were seen opening a few
connections to Microsoft addresses and secure-DNS probes with no stream open. vihpluh's own process
had none. This belongs to the runtime, not vihpluh, and is listed among the known limitations in the
[changelog](CHANGELOG.md#known-limitations).

## Hardening

- **Bundled assets only.** The webview loads nothing remote: no remote fonts, scripts, or styles.
  Content Security Policy:

  ```
  default-src 'self';
  img-src 'self' data: blob: vthumb: http://vthumb.localhost;
  style-src 'self' 'unsafe-inline';
  connect-src ipc: http://ipc.localhost;
  object-src 'none'
  ```

- **Narrow IPC surface.** The interface reaches the core only through vihpluh's own typed commands. No
  shell, file-system, or HTTP plugin is enabled, and only the main window has capabilities.
- **mpv scripts are off.** mpv's Lua/JavaScript scripts are not loaded (its built-in ones are switched
  off one by one), so media files and playlists cannot run code. Your own `mpv.conf` is read only with
  `mpv.load_user_config = true`, and `input.conf` never.
- **Power-user escape hatches are local.** `[mpv.options]` in `config.toml` and `mpv.command` custom
  bindings in `keybinds.toml` pass values to mpv unvalidated, with your own privileges. They come only
  from files you edit, never from media content.
- **Writes are confined.** Config, preset, and playlist writes go only to the config folder. The
  exceptions are files you place yourself: M3U8 exports and screenshots. Edits are written atomically,
  and config migrations keep a `.bak`.

## Binary licences

Windows binaries bundle a libmpv build that contains GPL parts of mpv and FFmpeg, so those binaries are
distributed under GPL-3.0-or-later. Release builds are made by the public workflow in
`.github/workflows/release.yml` from tagged source. The pinned libmpv archive is checked against a
SHA-256 in `scripts/fetch-libmpv.ps1`.

## Reporting a vulnerability

Please **do not** open a public issue for a security problem.

1. Use GitHub's private reporting on this repository: **Security → Report a vulnerability**.
2. If that option is not available, open a regular issue that says only that you have a security
   report and asks for a private contact. Leave out the details.

Include the vihpluh version (`vihpluh --version`), your OS, and steps or a file that reproduces it.
This is a one-person project: expect an answer within about two weeks. A fix lands on `main` and in
the next release, with credit if you want it.

In scope: anything that lets a media file, playlist, stream list, subtitle, or stream run code, read or
write files outside vihpluh's own folders, or send data somewhere you did not ask for. Out of scope: bugs
in mpv, FFmpeg, WebView2, or WebKitGTK themselves (report those upstream), and options you set on
purpose in `[mpv.options]` or `[[custom]]` bindings.
