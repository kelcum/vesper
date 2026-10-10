# Contributing to Vesper

Thanks for helping. Vesper's promise is that it never fakes anything: no
invented Fast Flags, no performance claims nobody measured, no buttons that
don't work. Contributions are judged against that first.

## Development setup

You need Windows 10 or 11 and:

- [Rust](https://rustup.rs) (stable, 1.90+) with the MSVC toolchain
- [Node.js](https://nodejs.org) 24+
- WebView2 (preinstalled on Windows 11)

```bash
npm ci
npm run app:dev
```

`app:dev` starts Vite on port 1420 and the Tauri app with hot reload.

The repository path can get long on Windows. If Rust builds fail with path
errors, point Cargo at a short target folder:

```bash
set CARGO_TARGET_DIR=C:\vesper-target
```

## Project structure

```
src/                 Svelte 5 interface
  pages/             one file per page (Home, Profiles, Fast Flags, …)
  components/        ui/ primitives, shell/, flags/, profiles/
  lib/               typed API, store, formatting and flag logic
  styles/tokens.css  design tokens
src-tauri/
  src/
    roblox/          detection, launching, logs, frame-rate cap, deep links
    flags/           catalog, validation, applying, verification
    perf/            system info, GPU counters, frame timing (ETW), monitor
    discord/         IPC client and presence manager
    mods.rs          cursor, emote wheel, font, sound customisation
    profiles.rs config.rs backup.rs cleanup.rs diagnostics.rs updates.rs
    session.rs       one launch, end to end
    commands.rs customize.rs   commands the interface calls
  resources/flags/catalog.json  the Fast Flag catalog
  examples/probe.rs  prints what Vesper detects on your PC
website/vesper/      the xan.gripe/vesper page
scripts/             brand assets, release helpers, site build
tests/               interface unit tests (Vitest)
```

## Checks

Everything CI runs, locally:

```bash
npm run check          # svelte-check (warnings fail)
npm test               # Vitest
npm run format:check   # Prettier
cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

`cargo run --example probe` prints detection results, versions and a live
performance sample without changing anything.

## The Fast Flag catalog

`src-tauri/resources/flags/catalog.json` only lists flags on Roblox's
published allowlist. When you add or change an entry:

- Cite a source for every claim (`references`) and keep `reviewed` current.
- Mark `behavior` as `uncertain` unless independent sources agree, and say so
  in the text ("reported", "isn't documented").
- Only fill `robloxDefault` with a value Roblox actually publishes.
- Never state a frame-rate or performance gain.

`cargo test` enforces the catalog's structure and that it matches the
allowlist exactly. If Roblox changes the allowlist, update both together.

## Coding standards

- Match the surrounding code. Rust is formatted with `rustfmt`, TypeScript and
  Svelte with Prettier.
- User-facing errors are `Problem`s: what happened, why, and what to do.
  Never surface a raw error string.
- Never hold a handle to the Roblox process, never inject anything into it,
  and never read cookies or tokens.
- Log through `log::` macros; messages are redacted automatically, but don't
  log anything sensitive in the first place.
- Tests should exercise behaviour, not just that functions exist. Prefer
  real files in a temp directory over mocks.

## Pull requests

- One topic per pull request. Describe what changed, why, and how you tested it.
- Include screenshots for interface changes.
- Update `CHANGELOG.md` under "Unreleased".
- Make sure every check above passes.

## Commits

Write commit messages in the imperative ("Add emote wheel presets"), with a
short first line and detail below if it helps a reviewer.

## Reporting bugs

Use the issue templates. Attach diagnostics from Settings → Advanced → Copy
diagnostics. They contain no secrets and no Windows user name. Security
problems go through [SECURITY.md](SECURITY.md), not public issues.

## Releasing (maintainers)

1. Bump the version in `package.json`, `src-tauri/Cargo.toml` and
   `src-tauri/tauri.conf.json`, and move the "Unreleased" changelog entries
   under the new version.
2. Push a tag such as `v0.2.0`. The Release workflow builds the signed
   installer, a portable zip, `latest.json` and `SHA256SUMS.txt` into a
   **draft** GitHub release. It needs the repository secrets
   `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
3. Test the draft's installer, then stage it on the website:

   ```bash
   gh release download v0.2.0 -D release-files
   node scripts/site/publish.mjs release-files ../kelcum.github.io
   ```

   This checks every file against `SHA256SUMS.txt` and the update signature,
   then copies the page, downloads and update manifests into the Pages clone.
   It never commits or pushes: review the clone and push it yourself, then
   commit `website/vesper/release.json` here and publish the draft release.
