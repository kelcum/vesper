# Changelog

All notable changes to Vesper are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and Vesper uses
[semantic versioning](https://semver.org).

## [Unreleased]

## [0.3.3] - 2026-10-17

### Changed

- Shaders come already compiled with Vesper, so it no longer loads
  Microsoft's shader compiler while it runs.
- Customize and the website explain what to do if Windows Security wrongly
  removes Vesper when shaders run, with a link to report the mistake to
  Microsoft.

## [0.3.2] - 2026-10-17

### Removed

- The Avatar page. It didn't work reliably, so it's gone rather than half
  working. When 0.3.2 starts, it deletes the browser data that page kept on
  your computer, including the Roblox sign-in made there. To end that session
  on Roblox's side too, use “Log out of all other sessions” in Roblox's
  security settings.

## [0.3.1] - 2026-10-17

### Added

- “Hide completely” in Settings → After launching Roblox: Vesper leaves the
  taskbar and the tray while Roblox runs. It comes back when Roblox closes,
  or when you open Vesper again.

### Changed

- Shaders run only while you're in a game, not on Roblox's home screen and
  menus, and stay on when you click another window. Windows you put in front
  of Roblox stay in front of it.
- Customize explains where a custom font and sky show: Roblox's menus and the
  games that use Roblox's built-in fonts and default sky. Games that bring
  their own fonts or sky keep them.
- Discord: if you turned Rich Presence on before 0.3, it doesn't start
  showing the game you're playing until you switch that on.

### Fixed

- Avatar: Roblox's page could stay on top of Vesper's avatar tools (and other
  pages) after leaving Roblox's editor, and could show up blank.
- Avatar: the Outfits tab kept reloading when you had no saved outfits, items
  you own more than once appeared twice in Inventory, and signing in could
  queue up repeated checks.
- Recent games: servers from before Vesper opened were recorded as played
  until Vesper started, and rejoining a server kept its old start time, which
  could also mix up the days.
- Restarting Roblox could miss that it had closed, so the old game stayed
  shown during the relaunch.
- Shaders: the status could read “off” after going straight to another game,
  and the grade didn't show over a Roblox window set to always on top.
- Server picking could mix up two cities with the same name in different
  countries.
- A sky that couldn't be encoded completely could leave one side of the
  previous sky in place; now nothing changes and Vesper says so.
- Ending a Roblox that didn't close only tried until the first failure, and
  could also end a Roblox that started meanwhile.

## [0.3.0] - 2026-10-17

### Added

- Now playing: Home shows the game you're in (name, creator, icon, how long
  you've played and, if you turn it on, which city the server is in), with
  buttons to copy a link that joins your exact server, open the game's page,
  or rejoin. Vesper reads this from Roblox's own log, so it works however the
  game was started.
- Recent games: a new page lists the games and servers you've played, with
  the server's city, how long you played, and buttons to join the same server
  again, play the game, or copy the link. It's kept only on your computer;
  turn it off or clear it any time.
- Server picking: games you join from Vesper can go to the lowest-ping
  server, or to a region you choose (NA East, Europe West, Asia and more, or a
  single city). Settings → Servers measures your ping to every Roblox region.
  Server locations come from RoValra's public database; only the game's place
  ID is sent. If a region has no open server, Vesper says so and picks the
  lowest-ping one.
- Discord: new options to show the game you're playing with its icon and your
  play time, a “Join server” button that opens Roblox in your server (public
  servers only), and a “Roblox profile” button.
- Avatar (Windows): change your avatar inside Vesper. Wear and take off
  accessories, wear anything you own by its ID (Vesper adds no limits of its
  own; Roblox's rules decide), browse your inventory, change body colours,
  scale and R6/R15, and switch outfits. Roblox's own editor is a click away.
  You sign in on Roblox's real login page; Vesper never reads your password
  or session.
- Shaders (Windows): colour grading for everything you see in Roblox, with
  seven looks (Vivid, Cinematic, Crisp, Golden hour, Moonlight, Soft glow,
  Noir) and sliders for brightness, contrast, saturation, warmth, sharpening,
  glow, film tone and vignette. Nothing is loaded into Roblox: Vesper grades a
  capture of Roblox's window, as screen recorders do, and lays it over the
  game, so it's safe with Roblox's anti-cheat. It costs about one frame of
  delay. Switch it from the tray too.
- Custom skies: five built-in skies (Midnight, Sunset, Dawn, Overcast, Void),
  or your own 360° panorama or six pictures, in every game that doesn't set
  its own sky.

### Fixed

- Launching while Roblox is open did nothing, because Roblox only runs once.
  The button now says “Restart and rejoin”: Vesper closes Roblox (asking
  first, ending it only if it doesn't respond), applies your profile and
  customisations, and joins the same server again. The tray has it too.

## [0.2.0] - 2026-10-13

### Added

- macOS support. Vesper runs on macOS 11 or later (Apple silicon and
  Intel): it finds Roblox.app, launches Roblox and games, applies Fast Flags,
  customisations and the frame-rate limit, checks Roblox's log, re-applies
  everything after Roblox updates itself, shows live CPU and memory use, and
  keeps Discord Rich Presence. It uses macOS's own window buttons and lives in
  the menu bar when its window is closed. macOS doesn't let other apps count
  Roblox's frames, read per-app GPU use or change Roblox's priority, so Vesper
  says so instead of showing a number.

### Fixed

- Launching Roblox no longer takes Vesper off the taskbar when "Minimize to
  tray" is on. "Minimize to the taskbar" now always means the taskbar; only
  "Hide in the tray" leaves it.
- The frame-rate limit card no longer cuts off its warning about a read-only
  Roblox settings file; it explains the lock on its own line. "Restore
  original" explains a locked file instead of failing with a generic error.
- "Remove flags" (Settings → Backups & recovery) found nothing to remove on
  case-sensitive drives.

## [0.1.2] - 2026-10-11

### Fixed

- Custom fonts: Roblox's font lists are now changed the way Bloxstrap and
  Fishstrap change them, swapping only the font file and keeping the rest of
  each list exactly as Roblox wrote it.
- When Roblox updates itself while Vesper is open, customisations are applied
  again once the update has finished unpacking, so new textures and fonts
  aren't missed.
- The section list in Settings follows the section you scroll to.

### Changed

- The Font card shows the font's own name and a line of text in it, and has
  a "Find free fonts" button. Web fonts (.woff) get a clear explanation.
- Fast Flags dropdowns match the rest of the app, and the emote wheel hint
  matches what's chosen.

## [0.1.1] - 2026-10-11

### Added

- "When Roblox closes" (Settings → General): if Vesper minimized or hid
  itself to make room for Roblox, it comes back on screen once the game
  closes. It can also stay where it is, or quit.
- The tray menu and tooltip show whether Roblox is running and which
  profile is active.
- A "New profile" tile at the end of the profiles grid.

### Changed

- The About page links to the website and the contact page; problem
  messages point there too.

## [0.1.0] - 2026-10-11

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
  Your own picture becomes a round emote wheel with Roblox-style slots,
  adjustable darkening and optional dividers; cursor styles can be resized
  and recoloured. Both are previewed on a Roblox-style baseplate. A custom
  font set in Fishstrap or Bloxstrap can be brought over in one click (or
  with `Vesper.exe --import-font Fishstrap`).
- Keeping flags and customisations applied when Roblox updates itself.
- Performance page: system and Roblox CPU, memory and GPU, and frame rate and
  frame timing from Direct3D present events when Windows permits.
- Discord Rich Presence with a "Get Vesper" button, built in, with its own
  artwork and a status badge that follows the "show whether Roblox is
  running" setting.
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
