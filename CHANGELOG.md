# Changelog

What changed in each release of Chyk. Release notes on GitHub and the changelog on
chyk.zgv2.dev are both built from this file, so add your entry here before tagging.

Format: a `## <version> — <YYYY-MM-DD>` heading per release (or `## Unreleased`), then
`### Added`, `### Changed` and `### Fixed` lists. Plain text and `code` only, no links.

## Unreleased

### Added
- Edit with Chyk in the right-click menu for videos in Windows Explorer, including the main Windows 11 menu.

### Changed
- The start screen no longer lists recent files.
- The Fedora and openSUSE package uses the system ffmpeg instead of bringing its own.
- When ffmpeg has no working H.264 encoder, export offers only a lossless copy and says why.

### Fixed
- The export chip shows a second encoding pass instead of staying at 100%.

## 1.3.2 — 2026-11-11

### Added
- The ffmpeg log window has Chyk's own window buttons and edges on Linux and Windows.

### Changed
- The timeline reuses filmstrips it has already made, so zooming back is instant.

### Fixed
- Update checks no longer give up on slow name lookups.
- Cleanup on Windows keeps update downloads and thumbnails.
- The dashed drop box has round corners, and Cancel is centered on the save dialog.
- Key cap letters are centered.

## 1.3.1 — 2026-11-11

### Fixed
- The log window has no stray title bar on macOS.
- The app icon's film is centered, with the scissors hanging below.

## 1.3.0 — 2026-11-11

### Added
- Every action has a key. Buttons and tooltips show it, and `?` opens a searchable sheet of all shortcuts.
- Undo and redo for trim, crop, Strip and speed.
- A status bar that says whether an export will be a lossless copy or a re-encode, with progress and time left.
- A toast when a file is saved, with a button to show it in the file manager.
- Save the current frame as a PNG.
- An ffmpeg log window with every command Chyk ran and what it printed.
- A time ruler above the trim bar, and labeled IN, OUT and LEN fields.
- Right-click the trim bar to set the start or end, fit the selection or save that frame.
- Closing with unsaved edits lists exactly what changed.
- Recent files on the start screen.

### Changed
- The export settings are laid out as rows, with a note on what each quality does.

### Fixed
- Paths with spaces open correctly in Explorer.
- Frame file names keep a literal `%`.

## 1.2.6 — 2026-11-10

### Fixed
- Quality settings now change the file on Macs that use VideoToolbox, and size estimates are measured.
- Boxes in the header and the playback row have even heights.

## 1.2.5 — 2026-11-10

### Added
- Export at 4K, 2K, 1080p, 720p, 480p, 360p or 144p, with a size estimate for each.

### Fixed
- The Dock progress bar keeps moving while the window is hidden.

## 1.2.4 — 2026-11-06

### Added
- Drag and drop on Wayland.

### Fixed
- Strip no longer rescans endlessly at the end of a trim.
- Chyk no longer touches X11 when running under Wayland.

## 1.2.3 — 2026-11-06

### Changed
- Open dialogs show only video files.

## 1.2.2 — 2026-11-06

### Fixed
- The update button stays responsive and compact.
- Strip only scans the trimmed part of the video.

## 1.2.1 — 2026-11-06

### Added
- Update from inside the app: one click downloads, another restarts on the new version.
- A macOS build for Intel Macs.

## 1.2.0 — 2026-11-06

### Added
- Export progress on the app icon, and a sound when an export finishes in the background.

### Changed
- Glint is now called Chyk.
- Release files no longer have the version in their name, so download links always get the latest.

### Fixed
- Switching from Crop to Strip no longer counts a crop you never touched.

## 1.1.1 — 2026-11-05

### Fixed
- Long filter graphs work with ffmpeg 6.

## 1.1.0 — 2026-11-05

### Changed
- Strip only looks inside the crop.
- Tool bars moved into the header.
- Exports are faster.

## 1.0.1 — 2026-11-05

### Added
- Linux packages for Debian, Ubuntu and Fedora, and a PKGBUILD for Arch.
- A macOS disk image with drag-to-Applications.

### Changed
- Licensed under GPL-3.0.

## 1.0.0 — 2026-11-04

### Added
- Strip: finds the stretches where nothing moves and cuts them out, with ignore zones for clocks and cursors.
- Compress: the smallest file for sharing.
- Ukrainian.
- File info: container, codecs, frame rate, color and tracks.

### Changed
- The new Phosphor look.

## 0.4.0 — 2026-10-23

### Added
- A toolbar at the top, and a trim selection you can slide.

### Changed
- A terminal-green look.

## 0.3.0 — 2026-10-14

### Added
- Type IN, OUT and length to the millisecond.
- Skip 5 seconds back and forward.

## 0.2.1 — 2026-10-14

### Fixed
- The app icon on Windows.

## 0.2.0 — 2026-10-14

### Added
- Export options.
- Trim handles outside the filmstrip.
- Windows as a single .exe.

## 0.1.0 — 2026-10-13

### Added
- First release: trim, crop and speed.
