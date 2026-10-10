# Changelog

All notable changes to Vesper are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and Vesper uses
[semantic versioning](https://semver.org).

## [Unreleased]

## [0.1.0] - 2026-10-10

First release.

### Added

- Roblox detection: the registered install, older version folders, all-users
  installs, Fishstrap and Bloxstrap copies, a chosen folder, and the Microsoft
  Store edition (reported as unsupported). Versions are read from the
  executable, including Roblox's 7-digit build numbers.
- Launching the Roblox app or a game from a place ID, game link, private
  server link or `roblox://` link. Input is validated and the launch link is
  rebuilt from checked parts. Share links open in the browser.
- Launch handling for Roblox updating itself first, closing straight away, or
  not starting, each with an explanation and next steps.
- A Fast Flag catalog of the 18 flags on Roblox's allowlist, with sources,
  risk, confidence in the described behaviour, and Roblox's published values
  where Roblox publishes them.
- Verification: after each launch Vesper reads Roblox's own log and shows which
  flags Roblox accepted, denied or didn't read.
- Profiles: Competitive, Balanced, Quality, Potato and Custom, plus your own;
  duplicate, rename, delete, import, export, reset and set default.
- Frame-rate limit through Roblox's own setting (30–480, unlimited, custom or
  match the display), restored to the original on request or uninstall.
- Process priority and optional processor affinity per profile.
- Customisation: cursor and emote wheel (built-in styles or your own image,
  generated at every size Roblox uses, with a highlight colour), font, death
  sound and a custom files folder. Originals are backed up and restored.
- Keeping flags and customisations applied when Roblox updates itself.
- Performance page: system and Roblox CPU, memory and GPU, and frame rate and
  frame timing from Direct3D present events when Windows permits.
- Discord Rich Presence with a "Get Vesper" button, built in.
- Backups with history, import and export; recovery tools for flags,
  customisations, the frame-rate setting and Vesper's own settings.
- Diagnostics and structured logs with secrets and user names removed.
- System tray, start with Windows, close-to-tray, first-run setup.
- Signed updates from xan.gripe/vesper, installed only when you confirm.
- Windows installer with Start menu and desktop shortcuts and a clean
  uninstall that reverts Vesper's changes to Roblox.

### Security

- Vesper never reads Roblox cookies or tokens, never injects into Roblox, and
  never holds a handle to the Roblox process.

[Unreleased]: https://github.com/kelcum/vesper/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/kelcum/vesper/releases/tag/v0.1.0
