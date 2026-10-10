<script lang="ts">
  import {
    Copy,
    FolderOpen,
    ImagePlus,
    LifeBuoy,
    MousePointer2,
    RotateCcw,
    Type,
    Volume2,
    FolderTree,
    Image as ImageIcon,
    Paintbrush,
  } from '@lucide/svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { onMount } from 'svelte';
  import Badge from '../components/ui/Badge.svelte';
  import Button from '../components/ui/Button.svelte';
  import Card from '../components/ui/Card.svelte';
  import Slider from '../components/ui/Slider.svelte';
  import Toggle from '../components/ui/Toggle.svelte';
  import { api } from '../lib/api';
  import { invalidateBackground } from '../lib/appearance';
  import { app } from '../lib/store.svelte';
  import type { ModsView } from '../lib/types';
  import studioScript from '../../docs/studio/ColorPresets.client.lua?raw';

  const thumbs = import.meta.glob('../assets/mods/*.png', { eager: true, query: '?url', import: 'default' }) as Record<
    string,
    string
  >;
  const thumb = (name: string) => thumbs[`../assets/mods/${name}.png`];

  let view = $state<ModsView | null>(null);
  let busy = $state<string | null>(null);
  let cursorScale = $state(80);

  const ACCENTS = [
    { value: null, label: "Roblox's white" },
    { value: '#a596ff', label: 'Lavender' },
    { value: '#7aa2ff', label: 'Blue' },
    { value: '#5fd3a6', label: 'Mint' },
    { value: '#ff8ccf', label: 'Pink' },
    { value: '#f2c46d', label: 'Gold' },
    { value: '#ff6b6b', label: 'Red' },
  ];

  onMount(load);

  async function load() {
    view = (await app.attempt(() => api.modsGet())) ?? view;
    if (view?.config.cursor?.source.kind === 'file') cursorScale = view.config.cursor.scale;
  }

  async function run(key: string, fn: () => Promise<ModsView>) {
    busy = key;
    const v = await app.attempt(fn);
    busy = null;
    if (v) {
      view = v;
      const r = v.lastReport;
      if (r?.missing.length)
        app.toast(`${r.missing.length} file(s) aren't in this Roblox version, so they were skipped.`, 'warn');
      else if (r?.skippedForeign.length)
        app.toast(
          `${r.skippedForeign.length} file(s) were changed by another program, so Vesper left them alone.`,
          'warn',
        );
      else
        app.toast(
          v.appliedNow ? 'Applied to Roblox' : v.robloxRunning ? 'Saved. Applies next time Roblox starts.' : 'Saved',
          'ok',
        );
    }
  }

  async function pick(name: string, extensions: string[]) {
    const p = await open({ multiple: false, filters: [{ name, extensions }] });
    return typeof p === 'string' ? p : null;
  }

  async function cursorFromFile() {
    const path = await pick('Image', ['png', 'jpg', 'jpeg', 'webp']);
    if (path) run('cursor', () => api.modsSetCursor({ path, scale: cursorScale }));
  }

  function cursorPreset(id: string) {
    run('cursor', () => api.modsSetCursor({ preset: id }));
  }

  async function wheelFromFile() {
    const path = await pick('Image', ['png', 'jpg', 'jpeg', 'webp']);
    if (path) run('wheel', () => api.modsSetWheel({ path, accent: view?.config.emoteWheel?.accent ?? null }));
  }

  function wheelPreset(id: string | null) {
    const accent = view?.config.emoteWheel?.accent ?? null;
    if (id === null) {
      if (accent) run('wheel', () => api.modsSetWheel({ accent }));
      else run('wheel', () => api.modsClear('emoteWheel'));
    } else run('wheel', () => api.modsSetWheel({ preset: id, accent }));
  }

  function wheelAccent(accent: string | null) {
    const src = view?.config.emoteWheel?.source;
    if (!src && !accent) return run('wheel', () => api.modsClear('emoteWheel'));
    if (src?.kind === 'file') return run('wheel', () => api.modsSetWheel({ accent, reuse: true }));
    run('wheel', () => api.modsSetWheel({ preset: src?.kind === 'preset' ? src.id : undefined, accent }));
  }

  async function chooseBackground() {
    const path = await pick('Image', ['png', 'jpg', 'jpeg', 'webp']);
    if (!path) return;
    busy = 'bg';
    const s = await app.attempt(() => api.backgroundSet(path));
    busy = null;
    if (s) {
      invalidateBackground();
      app.settings = s;
      app.toast('Background updated', 'ok');
    }
  }

  const cfg = $derived(view?.config);
  const cursorSel = $derived(
    cfg?.cursor ? (cfg.cursor.source.kind === 'preset' ? cfg.cursor.source.id : 'file') : null,
  );
  const wheelSel = $derived(
    cfg?.emoteWheel?.source ? (cfg.emoteWheel.source.kind === 'preset' ? cfg.emoteWheel.source.id : 'file') : null,
  );
  const accent = $derived(cfg?.emoteWheel?.accent ?? null);
</script>

<header class="head">
  <div>
    <p class="eyebrow">Customize</p>
    <h1>Make Roblox yours.</h1>
    <p class="muted lede">
      Swap Roblox's cursor, emote wheel, font and death sound. Vesper backs up every original file and puts it back when
      you switch something off. Changes take effect the next time Roblox starts.
    </p>
  </div>
  {#if view?.robloxRunning}<Badge tone="warn" dot>Roblox is running · changes apply next launch</Badge>{/if}
</header>

{#if !app.install}
  <p class="note glass">Install Roblox first; customisations are applied to its files.</p>
{/if}

<div class="grid">
  <!-- Cursor -->
  <Card title="Cursor" subtitle="The arrow you aim with in game" icon={MousePointer2}>
    {#snippet actions()}
      {#if cfg?.cursor}<Button
          size="sm"
          variant="ghost"
          icon={RotateCcw}
          onclick={() => run('cursor', () => api.modsClear('cursor'))}>Reset</Button
        >{/if}
    {/snippet}
    <div class="preview cursor-preview">
      {#if view?.previews.cursor}<img src={view.previews.cursor} alt="Current cursor" width="64" height="64" />{/if}
      <span class="crosshair" aria-hidden="true"></span>
    </div>
    <div class="tiles">
      <button
        class="tile"
        class:sel={cursorSel === null}
        onclick={() => run('cursor', () => api.modsClear('cursor'))}
        disabled={busy === 'cursor'}
      >
        <span class="tile-art roblox">R</span><span>Roblox</span>
      </button>
      {#each view?.cursorPresets ?? [] as p (p.id)}
        <button
          class="tile"
          class:sel={cursorSel === p.id}
          onclick={() => cursorPreset(p.id)}
          disabled={busy === 'cursor'}
        >
          <img class="tile-art" src={thumb(`cursor-${p.id}`)} alt="" width="40" height="40" /><span>{p.label}</span>
        </button>
      {/each}
      <button class="tile" class:sel={cursorSel === 'file'} onclick={cursorFromFile} disabled={busy === 'cursor'}>
        <span class="tile-art"><ImagePlus size={20} aria-hidden="true" /></span><span>Your image…</span>
      </button>
    </div>
    <div class="field">
      <span class="small muted">Size of your own image</span>
      <Slider
        label="Custom cursor size"
        min={30}
        max={100}
        step={5}
        unit="%"
        bind:value={cursorScale}
        onchange={(v) =>
          cfg?.cursor?.source.kind === 'file' && run('cursor', () => api.modsSetCursor({ scale: v, reuse: true }))}
      />
    </div>
    <p class="faint xs">
      Roblox draws the cursor centred on the mouse, so the middle of your image is where you aim. Images are fitted into
      64×64.
    </p>
  </Card>

  <!-- Emote wheel -->
  <Card title="Emote wheel" subtitle="The radial menu on the period (.) key" icon={LifeBuoy}>
    {#snippet actions()}
      {#if cfg?.emoteWheel}<Button
          size="sm"
          variant="ghost"
          icon={RotateCcw}
          onclick={() => run('wheel', () => api.modsClear('emoteWheel'))}>Reset</Button
        >{/if}
    {/snippet}
    <div class="preview wheel-preview" style:--accent-c={accent ?? '#ffffff'}>
      {#if view?.previews.wheelBackground}<img src={view.previews.wheelBackground} alt="" />{/if}
      {#if view?.previews.wheelRing}<img src={view.previews.wheelRing} alt="Current emote wheel" />{/if}
      <span class="highlight" aria-hidden="true"></span>
    </div>
    <div class="tiles">
      <button class="tile" class:sel={wheelSel === null} onclick={() => wheelPreset(null)} disabled={busy === 'wheel'}>
        <span class="tile-art roblox">R</span><span>Roblox</span>
      </button>
      {#each view?.wheelPresets ?? [] as p (p.id)}
        <button
          class="tile"
          class:sel={wheelSel === p.id}
          onclick={() => wheelPreset(p.id)}
          disabled={busy === 'wheel'}
        >
          <img class="tile-art" src={thumb(`wheel-${p.id}`)} alt="" width="40" height="40" /><span>{p.label}</span>
        </button>
      {/each}
      <button class="tile" class:sel={wheelSel === 'file'} onclick={wheelFromFile} disabled={busy === 'wheel'}>
        <span class="tile-art"><ImagePlus size={20} aria-hidden="true" /></span><span>Your image…</span>
      </button>
    </div>
    <div class="field">
      <span class="small muted">Highlight colour</span>
      <div class="swatches" role="radiogroup" aria-label="Highlight colour">
        {#each ACCENTS as a (a.label)}
          <button
            role="radio"
            aria-checked={accent === a.value}
            aria-label={a.label}
            title={a.label}
            class="swatch"
            class:sel={accent === a.value}
            style:--c={a.value ?? '#ffffff'}
            onclick={() => wheelAccent(a.value)}
          ></button>
        {/each}
      </div>
    </div>
    <p class="faint xs">
      Your image replaces the ring with dividers; square images work best. Vesper makes every size Roblox uses (small
      screens, PC, TV).
    </p>
  </Card>

  <!-- Font -->
  <Card title="Font" subtitle="Replaces the text font across Roblox's interface" icon={Type}>
    {#snippet actions()}
      {#if cfg?.font}<Button
          size="sm"
          variant="ghost"
          icon={RotateCcw}
          onclick={() => run('font', () => api.modsClear('font'))}>Reset</Button
        >{/if}
    {/snippet}
    <p class="current">{cfg?.font ? cfg.font.name : "Roblox's own fonts"}</p>
    <Button
      icon={FolderOpen}
      loading={busy === 'font'}
      onclick={async () => {
        const p = await pick('Font', ['ttf', 'otf', 'ttc']);
        if (p) run('font', () => api.modsSetFont(p));
      }}>Choose font…</Button
    >
    <p class="faint xs">TrueType or OpenType. Games that load their own fonts from Roblox's servers aren't affected.</p>
  </Card>

  <!-- Death sound -->
  <Card title="Death sound" subtitle="Plays when your character resets or dies" icon={Volume2}>
    {#snippet actions()}
      {#if cfg?.deathSound}<Button
          size="sm"
          variant="ghost"
          icon={RotateCcw}
          onclick={() => run('sound', () => api.modsClear('deathSound'))}>Reset</Button
        >{/if}
    {/snippet}
    <p class="current">{cfg?.deathSound ? cfg.deathSound.name : "Roblox's default"}</p>
    <Button
      icon={FolderOpen}
      loading={busy === 'sound'}
      onclick={async () => {
        const p = await pick('Sound', ['ogg', 'mp3']);
        if (p) run('sound', () => api.modsSetSound(p));
      }}>Choose sound…</Button
    >
    <p class="faint xs">.ogg works best. Games with their own death sounds keep them.</p>
  </Card>

  <!-- Window background -->
  <Card title="Vesper background" subtitle="An image behind Vesper's window" icon={ImageIcon}>
    {#snippet actions()}
      {#if app.settings.appearance.background}
        <Button
          size="sm"
          variant="ghost"
          icon={RotateCcw}
          onclick={async () => {
            const s = await app.attempt(() => api.backgroundClear());
            if (s) {
              invalidateBackground();
              app.settings = s;
            }
          }}>Remove</Button
        >
      {/if}
    {/snippet}
    <Button icon={ImagePlus} loading={busy === 'bg'} onclick={chooseBackground}
      >{app.settings.appearance.background ? 'Change image…' : 'Choose image…'}</Button
    >
    {#if app.settings.appearance.background}
      <div class="field">
        <span class="small muted">Darken</span>
        <Slider
          label="Background dim"
          min={0}
          max={90}
          step={5}
          unit="%"
          value={app.settings.appearance.backgroundDim}
          onchange={(v) => app.saveSettings({ appearance: { backgroundDim: v } })}
        />
      </div>
      <div class="field">
        <span class="small muted">Blur</span>
        <Slider
          label="Background blur"
          min={0}
          max={40}
          step={2}
          unit="px"
          value={app.settings.appearance.backgroundBlur}
          onchange={(v) => app.saveSettings({ appearance: { backgroundBlur: v } })}
        />
      </div>
    {/if}
    <p class="faint xs">
      Stored as a resized copy in Vesper's settings folder, so large photos don't slow the window down.
    </p>
  </Card>

  <!-- Custom files -->
  <Card title="Custom files" subtitle="Anything else, in Roblox's own folder layout" icon={FolderTree}>
    {#snippet actions()}
      <Toggle
        label="Apply custom files"
        checked={!!cfg?.customFiles}
        onchange={(v) => run('custom', () => api.modsSetCustom(v))}
      />
    {/snippet}
    <p class="muted small">
      Put files in the folder below using the same paths as inside Roblox's version folder, e.g.
      <span class="mono">content\textures\…</span>. {view?.customFileCount ?? 0} file(s) there now.
    </p>
    <Button icon={FolderOpen} onclick={() => api.modsOpenCustomFolder()}>Open folder</Button>
  </Card>
</div>

<section class="shaders glass">
  <div class="row top">
    <span class="ico"><Paintbrush size={16} aria-hidden="true" /></span>
    <h3>Shaders and colour filters</h3>
  </div>
  <p class="muted">
    Roblox doesn't support post-processing shaders from outside the game, and none of its allowlisted Fast Flags do
    colour grading. Tools that inject shaders or scripts into Roblox are detected by its anti-cheat and can get accounts
    banned, so Vesper doesn't do that.
  </p>
  <p class="muted">
    In experiences you build yourself, colour grading is a normal Roblox feature. Vesper includes your colour presets
    (Pink, Purple, Blue, Red, Green, Warm, Cold, Saturated, Night) as a ready-to-use Studio script.
  </p>
  <div class="row">
    <Button
      icon={Copy}
      onclick={async () => {
        await navigator.clipboard.writeText(studioScript);
        app.toast('Studio script copied. Paste it into a LocalScript in StarterPlayerScripts.', 'ok');
      }}>Copy Studio script</Button
    >
  </div>
</section>

{#if view?.lastReport && (view.lastReport.errors.length || view.lastReport.missing.length)}
  <section class="report glass">
    <h3>Last apply</h3>
    {#each view.lastReport.missing as m (m)}<p class="small warn">
        Not in this Roblox version: <span class="mono">{m}</span>
      </p>{/each}
    {#each view.lastReport.errors as e (e)}<p class="small bad">{e}</p>{/each}
  </section>
{/if}

<style>
  .head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: var(--space-4);
    margin: var(--space-2) 0 var(--space-6);
  }
  h1 {
    font-size: var(--text-3xl);
    margin-top: 4px;
  }
  .lede {
    margin-top: var(--space-2);
    max-width: 64ch;
  }
  .note {
    padding: var(--space-4);
    margin-bottom: var(--space-4);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: var(--space-4);
  }
  .grid :global(.card) {
    gap: var(--space-3);
  }
  .preview {
    position: relative;
    display: grid;
    place-items: center;
    height: 150px;
    border-radius: var(--radius-md);
    background: linear-gradient(135deg, #6f8fa8 0%, #9fb7a2 55%, #c9b98f 100%);
    overflow: hidden;
    border: 1px solid var(--border);
  }
  .cursor-preview img {
    image-rendering: auto;
    width: 64px;
    height: 64px;
    z-index: 1;
  }
  .crosshair {
    position: absolute;
    left: 50%;
    top: 50%;
    width: 7px;
    height: 7px;
    margin: -3px 0 0 -3px;
    border-radius: 50%;
    background: rgba(255, 60, 60, 0.85);
    z-index: 2;
    opacity: 0;
  }
  .cursor-preview:hover .crosshair {
    opacity: 1;
  }
  .wheel-preview {
    height: 190px;
    background: linear-gradient(135deg, #52687a, #7d8f74);
  }
  .wheel-preview img {
    position: absolute;
    width: 170px;
    height: 170px;
  }
  .highlight {
    position: absolute;
    width: 170px;
    height: 170px;
    border-radius: 50%;
    background: conic-gradient(
      from -22.5deg,
      color-mix(in srgb, var(--accent-c) 55%, transparent) 0 45deg,
      transparent 45deg
    );
    mask: radial-gradient(circle, transparent 46%, #000 47%, #000 98%, transparent 99%);
    opacity: 0.75;
    pointer-events: none;
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 6px;
  }
  .tile {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 9px 6px;
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text-2);
    font-size: var(--text-xs);
    font-weight: 550;
    cursor: pointer;
    transition: all var(--dur-fast) var(--ease-out);
  }
  .tile:hover:not(:disabled) {
    color: var(--text);
    border-color: var(--border-strong);
    transform: translateY(-1px);
  }
  .tile.sel {
    color: var(--text);
    border-color: color-mix(in srgb, var(--accent) 60%, transparent);
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }
  .tile:disabled {
    opacity: 0.6;
    cursor: progress;
  }
  .tile-art {
    width: 40px;
    height: 40px;
    display: grid;
    place-items: center;
    border-radius: 10px;
    background: linear-gradient(135deg, #5d7180, #8a9a80);
    color: #fff;
  }
  .tile-art.roblox {
    font-weight: 800;
    background: linear-gradient(135deg, #3a3d45, #26282e);
  }
  .field {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    flex-wrap: wrap;
  }
  .swatches {
    display: flex;
    gap: 7px;
  }
  .swatch {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    border: 2px solid var(--border-strong);
    background: var(--c);
    cursor: pointer;
    transition: transform var(--dur-fast) var(--ease-spring);
  }
  .swatch:hover {
    transform: scale(1.12);
  }
  .swatch.sel {
    box-shadow:
      0 0 0 2px var(--bg),
      0 0 0 4px var(--accent-strong);
  }
  .current {
    font-weight: 600;
  }
  .shaders,
  .report {
    margin-top: var(--space-4);
    padding: var(--space-5);
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .shaders p {
    max-width: 78ch;
  }
  .top {
    gap: var(--space-3);
  }
  .ico {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 9px;
    color: var(--accent-strong);
    background: color-mix(in srgb, var(--accent) 16%, transparent);
  }
  .warn {
    color: var(--warn);
  }
  .bad {
    color: var(--bad);
  }
  @media (max-width: 1060px) {
    .grid {
      grid-template-columns: 1fr;
    }
  }
</style>
