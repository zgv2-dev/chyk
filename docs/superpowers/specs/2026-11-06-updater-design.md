# Updater design

Date: 2026-11-06. Status: agreed in conversation, waiting for sign-off on this document.

## Why

Chyk ships fixes often. A copy downloaded once should not sit on old bugs forever, but the user
stays in charge: nothing downloads or changes until they click, and nothing interrupts them with a
dialog.

## What the user sees

One button in the header, directly left of Export (on the start screen: at the right end of the
title bar). It is exactly as tall as Export (34 px) and has two lines: a label and the version tag
under it. It walks through three states:

1. **Update available** / `v1.2.1`. Click: download the new build in the background. On a Linux
   .deb, .rpm or AUR install the click opens the release page in the browser instead (see
   "Install kinds").
2. **Downloading**: the button becomes a progress chip, same height: download icon, version, bar,
   percent, and an × that cancels (back to state 1, partial file deleted).
3. **Update now** / `v1.2.1`, a brighter look. Click: swap in the new version and reopen, with the
   same video and edits restored. No confirmation: nothing is lost.

"Later" is implicit. Once a download has finished, quitting Chyk swaps in the new version silently,
without asking. The user already agreed by downloading. The next launch is the new version.

Mockups (header and start screen): `.superpowers/brainstorm/*/content/updater-header.html`.

## Goals

- Check on every launch, quietly. Offline, rate-limited or GitHub down: no button, no error.
- Never download or install without a click on the button.
- Keep a finished download. If Chyk closes before the swap (a crash, say), the next launch finds
  it and opens straight into state 3, with no second download. If a newer release appears, the old
  download is deleted and the button goes back to state 1.
- "Update now" restores the session: file, trim, crop, aspect preset, speed, Strip on/off, padding
  and ignore zones, export settings (format, quality, precise, mute), playhead, and whether the
  edits count as exported (so the unexported-changes prompt still behaves).

## Non-goals and anti-goals

- No dialogs, no nag on every launch, no "skip this version".
- No silent background download, no automatic install.
- No new crates. HTTP goes through the system `curl`, run like ffmpeg (`std::process::Command`).
- No update checks in debug builds (`cargo run`).

## Install kinds

| Kind | How it's detected | Download | Swap |
|------|-------------------|----------|------|
| macOS app | `current_exe` is `<X>.app/Contents/MacOS/chyk` | `Chyk-macos-<arm64\|x86_64>.dmg` | mount the DMG read-only (`hdiutil attach -nobrowse`), copy its `Chyk.app` next to the current bundle with `ditto`, `codesign --verify --deep` it, rename the old bundle aside, rename the new one into place, delete the old one, detach |
| Windows exe | release build, any path | `Chyk-windows-x64.exe` | rename the running exe to `<name>.old.exe`, move the download into its place; on failure rename back. The next launch deletes `*.old.exe` |
| Linux AppImage | `$APPIMAGE` is set | `Chyk-linux-x86_64.AppImage` | copy to `$APPIMAGE.new`, `chmod 755`, rename over `$APPIMAGE` |
| Linux package | `current_exe` under `/usr/` | none | state 1 opens `https://github.com/zgv2-dev/chyk/releases/latest` |
| Anything else | | none | no button |

Downloads come from `https://github.com/zgv2-dev/chyk/releases/latest/download/<name>`. Release
file names carry no version (`SPEC.md`, Packaging).

## How it works

- **Check** (background thread at startup): `curl -sSI` on `releases/latest`, read the `location`
  header (`…/releases/tag/vX.Y.Z`), and compare with `CARGO_PKG_VERSION` numerically. If newer,
  `HEAD` this system's download URL as well: a release without a file for this system shows no
  button, and the `content-length` is kept for the progress bar. Newer and present means state 1,
  or state 3 if the cache already holds that version. No `curl` on the system means no button.
- **Cache**: `~/Library/Caches/Chyk/update/` (macOS), `%LOCALAPPDATA%\Chyk\update\` (Windows,
  next to the existing bundle dir), `$XDG_CACHE_HOME/chyk/update/` or `~/.cache/chyk/update/`
  (Linux). The file is named with the tag, e.g. `v1.2.1-Chyk-macos-arm64.dmg`. Anything else in the
  folder is deleted when a check finds a different latest version, or when the running version is
  already the latest.
- **Download**: `curl -fL -o <file>.part <url>`, as a child process the × can kill. The total size
  comes from the check's `HEAD`; the UI polls the `.part` file size for progress. On success it is
  renamed to the final name. A failure deletes the `.part` and shows a status message.
- **Update now**: the swap runs in-process (a running app can be renamed or replaced on all three
  systems). On success: write the session file to the cache folder, start the new copy with
  `--restore <session file>` (macOS: `open -n <bundle> --args …`; elsewhere spawn the exe or
  AppImage), then close without the unexported-changes prompt. On failure: show the error in the
  status line, keep running the old version, keep the download.
- **On quit**: after `eframe::run_native` returns in `main`, if a download is ready and no swap
  happened yet, do the swap. Failures are silent and the download stays for the next launch.
- **Restore**: `--restore <file>` replaces the file argument. The session file is plain
  `key = value` lines, no serde. Values are applied once the video has loaded, as `loaded()`
  already does for a fresh file. A Strip scan is not saved; Strip rescans, or uses its cache. The
  session file is deleted after it is read.
- **Export running**: "Update now" is disabled (dimmed, with a tooltip) until the export ends.
  Quitting during an export already asks first; the swap runs only after Chyk really exits.

## Text

New i18n keys in `assets/i18n/en.lang` and `uk.lang`: the button labels, the tooltip shown while
an export blocks "Update now", and the status messages for a failed download or swap. The version
tag is not translated.

## Code layout (builder's call within the 200-line limit)

- `src/update/`: `mod.rs` (state machine, check, version compare), `download.rs`, `swap.rs`
  (one function per install kind), `paths.rs` (cache folder, install kind).
- `src/app/session.rs`: write and read the session file.
- The header button lives with the other header widgets, built from the same helpers as Export.

## Tests

Unit tests: version compare (`1.10.0 > 1.9.9`, a `v` prefix, malformed tags), tag parsing from a
`location` header, session file round trip, asset name per install kind and CPU, and cache cleanup
deciding which files to delete. The swap and curl paths are checked by hand on macOS. Windows and
AppImage are checked once a release exists.

## Calls I made (not discussed)

- curl instead of an HTTP crate, and the redirect instead of the JSON API (no JSON parsing). The
  downside: no checksum check beyond HTTPS. The DMG gets `codesign --verify`; the exe and AppImage
  rely on `curl -f` and the `.part` rename.
- If the network is down but the cache holds a newer download, state 3 still shows.
- Restoring the session also restores export settings and the playhead.
- Cancel (×) deletes the partial file. Resuming downloads is not supported.
- The CPU in the asset name comes from the build target (`cfg!(target_arch)`), so an Intel build
  running under Rosetta keeps getting Intel builds.
- Windows: each new exe unpacks its libraries to a new `%LOCALAPPDATA%\Chyk\<build-id>` folder. Old
  folders are left alone, as with a manual update today. Cleaning them up is a separate change.
- curl downloads carry no "downloaded from the internet" mark, so neither Gatekeeper nor SmartScreen
  warns on the swapped-in version.
