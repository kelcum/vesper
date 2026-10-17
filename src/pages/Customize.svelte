<script lang="ts">
  import {
    Copy,
    ExternalLink,
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
    CloudSun,
    Images,
  } from '@lucide/svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { onMount } from 'svelte';
  import Badge from '../components/ui/Badge.svelte';
  import Button from '../components/ui/Button.svelte';
  import Card from '../components/ui/Card.svelte';
  import Slider from '../components/ui/Slider.svelte';
  import Toggle from '../components/ui/Toggle.svelte';
  import RobloxScene from '../components/customize/RobloxScene.svelte';
  import WheelPreview from '../components/customize/WheelPreview.svelte';
  import { api } from '../lib/api';
  import { invalidateBackground } from '../lib/appearance';
  import { isMac } from '../lib/platform';
  import { app } from '../lib/store.svelte';
  import type { ModsView, Option, ShaderParams } from '../lib/types';
  import studioScript from '../../docs/studio/ColorPresets.client.lua?raw';

  const thumbs = import.meta.glob('../assets/mods/*.png', { eager: true, query: '?url', import: 'default' }) as Record<
    string,
    string
  >;
  const thumb = (name: string) => thumbs[`../assets/mods/${name}.png`];

  let view = $state<ModsView | null>(null);
  // The chosen font, loaded into the page so the card can show text in it.
  let fontFamily = $state<string | null>(null);
  let fontLoads = 0;
  $effect(() => {
    const url = view?.previews.font;
    fontFamily = null;
    if (!url) return;
    const face = new FontFace(`vesper-font-preview-${++fontLoads}`, `url(${url})`);
    let current = true;
    face
      .load()
      .then((f) => {
        if (!current) return;
        document.fonts.add(f);
        fontFamily = f.family;
      })
      .catch(() => {});
    return () => {
      current = false;
      document.fonts.delete(face);
    };
  });
  let busy = $state<string | null>(null);
  let cursorScale = $state(100);
  let wheelDim = $state(30);
  let wheelDividers = $state(true);
  /** Where the preview cursor is drawn, in px inside the scene; null = centred. */
  let tryAt = $state<{ x: number; y: number } | null>(null);

  const COLOURS = [
    { value: '#a596ff', label: 'Lavender' },
    { value: '#7aa2ff', label: 'Blue' },
    { value: '#5fd3a6', label: 'Mint' },
    { value: '#ff8ccf', label: 'Pink' },
    { value: '#f2c46d', label: 'Gold' },
    { value: '#ff6b6b', label: 'Red' },
  ];
  const ACCENTS = [{ value: null, label: "Roblox's white" }, ...COLOURS];
  const CURSOR_COLOURS = [{ value: null, label: 'Original colours' }, ...COLOURS];

  onMount(load);

  // ------------------------------------------------ sky
  const SKY_ART: Record<string, string> = {
    midnight: 'linear-gradient(#04050d, #111739 60%, #1d2550)',
    sunset: 'linear-gradient(#2c2f6b, #d8667a 55%, #ffa05a)',
    dawn: 'linear-gradient(#7ea6e0, #c7d6f0 55%, #ffd8c2)',
    overcast: 'linear-gradient(#858d98, #a6adb7 55%, #c6cbd2)',
    void: '#000',
  };
  const skySel = $derived.by(() => {
    const sky = view?.config.sky;
    if (!sky) return null;
    if (sky.source.kind === 'preset') return sky.source.id;
    return sky.layout === 'faces' ? 'faces' : 'panorama';
  });
  async function skyFromPanorama() {
    const path = await pick('360° picture', ['png', 'jpg', 'jpeg', 'webp']);
    if (path) run('sky', () => api.modsSetSky({ panorama: path }));
  }
  async function skyFromFaces() {
    const p = await open({
      multiple: true,
      filters: [{ name: 'Six pictures', extensions: ['png', 'jpg', 'jpeg', 'webp'] }],
    });
    const faces = Array.isArray(p) ? p : typeof p === 'string' ? [p] : [];
    if (faces.length) run('sky', () => api.modsSetSky({ faces }));
  }

  // ------------------------------------------------ shaders
  let shaderLooks = $state<Option[]>([]);
  onMount(async () => {
    const info = await api.shadersInfo().catch(() => null);
    if (info) {
      shaderLooks = info.presets;
      app.shaders = info.status;
    }
  });
  const shaders = $derived(app.settings.shaders);
  /** Sliders work in whole numbers; the settings keep the shader's units. */
  const SLIDERS: {
    key: keyof ShaderParams;
    label: string;
    min: number;
    max: number;
    scale: number;
    offset?: number;
    unit: string;
  }[] = [
    { key: 'exposure', label: 'Brightness', min: -100, max: 100, scale: 100, unit: '' },
    { key: 'contrast', label: 'Contrast', min: 50, max: 150, scale: 100, unit: '%' },
    { key: 'saturation', label: 'Saturation', min: 0, max: 200, scale: 100, unit: '%' },
    { key: 'vibrance', label: 'Vibrance', min: -100, max: 100, scale: 100, unit: '' },
    { key: 'temperature', label: 'Warmth', min: -100, max: 100, scale: 100, unit: '' },
    { key: 'tint', label: 'Tint', min: -100, max: 100, scale: 100, unit: '' },
    { key: 'sharpen', label: 'Sharpen', min: 0, max: 100, scale: 100, unit: '' },
    { key: 'bloom', label: 'Glow', min: 0, max: 100, scale: 100, unit: '' },
    { key: 'tonemap', label: 'Film tone', min: 0, max: 100, scale: 100, unit: '' },
    { key: 'vignette', label: 'Vignette', min: 0, max: 100, scale: 100, unit: '' },
  ];
  let sliderValues = $state<Record<string, number>>({});
  $effect(() => {
    const p = shaders.params;
    const next: Record<string, number> = {};
    for (const sl of SLIDERS) next[sl.key] = Math.round(p[sl.key] * sl.scale);
    sliderValues = next;
  });
  function setParam(key: keyof ShaderParams, v: number, scale: number) {
    const params = { ...shaders.params, [key]: v / scale };
    app.saveSettings({ shaders: { params, preset: null } });
  }
  async function shaderLook(id: string) {
    const params = await app.attempt(() => api.shadersPreset(id));
    if (params) app.saveSettings({ shaders: { params, preset: id, enabled: true } });
  }
  const shaderBadge = $derived.by(() => {
    if (!shaders.enabled) return { tone: 'neutral' as const, label: 'Off' };
    switch (app.shaders.state) {
      case 'running':
        return { tone: 'ok' as const, label: 'On in Roblox' };
      case 'failed':
        return { tone: 'bad' as const, label: "Couldn't start" };
      default:
        // They run inside games only, not on Roblox's home screen.
        if (!app.robloxRunning) return { tone: 'info' as const, label: 'Waiting for Roblox' };
        return { tone: 'info' as const, label: app.nowPlaying ? 'Starting…' : 'Waiting for a game' };
    }
  });
  /** A rough stand-in for the grade, using the browser's own filters, for the preview. */
  const previewFilter = $derived.by(() => {
    const p = shaders.params;
    const warm = Math.max(p.temperature, 0);
    const cool = Math.max(-p.temperature, 0);
    return [
      `brightness(${Math.pow(2, p.exposure).toFixed(3)})`,
      `contrast(${p.contrast.toFixed(3)})`,
      `saturate(${(p.saturation * (1 + p.vibrance * 0.4)).toFixed(3)})`,
      warm ? `sepia(${(warm * 0.35).toFixed(3)})` : '',
      cool ? `hue-rotate(${(-cool * 14).toFixed(1)}deg)` : '',
    ].join(' ');
  });

  async function load() {
    view = (await app.attempt(() => api.modsGet())) ?? view;
    syncControls();
  }

  function syncControls() {
    if (view?.config.cursor) cursorScale = view.config.cursor.scale;
    const look = view?.config.emoteWheel?.look;
    if (look) {
      wheelDim = look.dim;
      wheelDividers = look.dividers;
    }
  }

  async function run(key: string, fn: () => Promise<ModsView>) {
    busy = key;
    const v = await app.attempt(fn);
    busy = null;
    if (v) {
      view = v;
      syncControls();
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
    if (path) run('cursor', () => api.modsSetCursor({ path, scale: Math.min(cursorScale, 100) }));
  }

  function cursorPreset(id: string) {
    run('cursor', () => api.modsSetCursor({ preset: id, scale: cursorScale, color: cfg?.cursor?.color ?? null }));
  }

  function cursorRedraw(change: { scale?: number; color?: string | null }) {
    if (!cfg?.cursor) return;
    const color = 'color' in change ? change.color : cfg.cursor.color;
    run('cursor', () => api.modsSetCursor({ scale: change.scale ?? cursorScale, color, reuse: true }));
  }

  function tryCursor(e: PointerEvent) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    tryAt = { x: e.clientX - r.left, y: e.clientY - r.top };
  }

  async function wheelFromFile() {
    const path = await pick('Image', ['png', 'jpg', 'jpeg', 'webp']);
    if (path)
      run('wheel', () =>
        api.modsSetWheel({
          path,
          accent: view?.config.emoteWheel?.accent ?? null,
          dim: wheelDim,
          dividers: wheelDividers,
        }),
      );
  }

  function wheelLook(change: { dim?: number; dividers?: boolean }) {
    run('wheel', () => api.modsSetWheel({ ...change, accent: cfg?.emoteWheel?.accent ?? null, reuse: true }));
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
  const cursorColor = $derived(cfg?.cursor?.color ?? null);
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
    <RobloxScene height={170} hideCursor onpointermove={tryCursor} onpointerleave={() => (tryAt = null)}>
      {#if view?.previews.cursor}
        <img
          class="try-cursor"
          src={view.previews.cursor}
          alt="Current cursor"
          width="64"
          height="64"
          style:left={tryAt ? `${tryAt.x}px` : '50%'}
          style:top={tryAt ? `${tryAt.y}px` : '50%'}
        />
      {/if}
      {#if !tryAt}<span class="scene-hint">Move your mouse here to try it</span>{/if}
    </RobloxScene>
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
        {#if cursorSel === 'file' && view?.previews.cursor}
          <img class="tile-art" src={view.previews.cursor} alt="" width="40" height="40" />
        {:else}
          <span class="tile-art"><ImagePlus size={20} aria-hidden="true" /></span>
        {/if}
        <span>{cursorSel === 'file' ? 'Change image…' : 'Your image…'}</span>
      </button>
    </div>
    {#if cfg?.cursor}
      <div class="field">
        <span class="small muted">Size</span>
        <Slider
          label="Cursor size"
          min={view?.cursorScale[0] ?? 30}
          max={view?.cursorScale[1] ?? 150}
          step={5}
          unit="%"
          bind:value={cursorScale}
          onchange={(v) => cursorRedraw({ scale: v })}
        />
      </div>
      {#if cfg.cursor.source.kind === 'preset'}
        <div class="field">
          <span class="small muted">Colour</span>
          <div class="swatches" role="radiogroup" aria-label="Cursor colour">
            {#each CURSOR_COLOURS as c (c.label)}
              <button
                role="radio"
                aria-checked={cursorColor === c.value}
                aria-label={c.label}
                title={c.label}
                class="swatch"
                class:sel={cursorColor === c.value}
                class:original={c.value === null}
                style:--c={c.value ?? '#ffffff'}
                onclick={() => cursorRedraw({ color: c.value })}
              ></button>
            {/each}
          </div>
        </div>
      {/if}
    {/if}
    <p class="faint xs">
      Roblox draws the cursor centred on the mouse, so the middle of your image is where you aim. Images are fitted into
      Roblox's 64×64 cursor; above 100% the edges are cut off.
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
    <RobloxScene height={250}>
      <WheelPreview
        background={view?.previews.wheelBackground ?? null}
        ring={view?.previews.wheelRing ?? null}
        gradient={view?.previews.wheelGradient ?? null}
        line={view?.previews.wheelLine ?? null}
      />
    </RobloxScene>
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
        {#if wheelSel === 'file' && view?.previews.wheelRing}
          <img class="tile-art round" src={view.previews.wheelRing} alt="" width="40" height="40" />
        {:else}
          <span class="tile-art"><ImagePlus size={20} aria-hidden="true" /></span>
        {/if}
        <span>{wheelSel === 'file' ? 'Change image…' : 'Your image…'}</span>
      </button>
    </div>
    {#if wheelSel === 'file'}
      <div class="field">
        <span class="small muted">Darken picture</span>
        <Slider
          label="Darken picture"
          min={0}
          max={view?.maxWheelDim ?? 80}
          step={5}
          unit="%"
          bind:value={wheelDim}
          onchange={(v) => wheelLook({ dim: v })}
        />
      </div>
      <div class="field">
        <span class="small muted">Lines between slots</span>
        <Toggle label="Lines between slots" bind:checked={wheelDividers} onchange={(v) => wheelLook({ dividers: v })} />
      </div>
    {/if}
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
      {#if cfg?.emoteWheel?.source?.kind === 'file'}
        Your picture is cut into a circle from its middle and darkened a little so emotes stay readable.
      {:else}
        Choose “Your image…” to put a picture behind your emotes.
      {/if}
      Point at the preview to see a slot highlighted. Vesper makes every size Roblox uses (small screens, PC, TV).
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
    {#if cfg?.font}
      <div class="font-sample" style:font-family={fontFamily ? `"${fontFamily}", var(--font-sans)` : null}>
        <span class="aa" aria-hidden="true">Aa</span>
        <span class="lines">
          <span>The quick brown fox jumps over the lazy dog</span>
          <span class="digits">0123456789 · Select an Emote · Leave game?</span>
        </span>
      </div>
      <p class="current">
        {view?.fontName ?? cfg.font.name}
        {#if view?.fontName && view.fontName !== cfg.font.name}<span class="faint from">· {cfg.font.name}</span>{/if}
      </p>
    {:else}
      <p class="current">Roblox's own fonts</p>
    {/if}
    <div class="row-buttons">
      <Button
        icon={FolderOpen}
        loading={busy === 'font'}
        onclick={async () => {
          const p = await pick('Font', ['ttf', 'otf', 'ttc']);
          if (p) run('font', () => api.modsSetFont(p));
        }}>Choose font…</Button
      >
      {#each view?.launcherFonts ?? [] as l (l)}
        {#if cfg?.font?.name !== `${l}'s font`}
          <Button variant="ghost" icon={Copy} onclick={() => run('font', () => api.modsUseLauncherFont(l))}
            >Use {l}'s font</Button
          >
        {/if}
      {/each}
      <Button variant="ghost" icon={ExternalLink} onclick={() => api.openUrl('https://fonts.google.com')}
        >Find free fonts</Button
      >
    </div>
    <p class="faint xs">
      A .ttf or .otf file. Roblox uses this one file for every weight, so pick the exact style you want (for example
      SemiBold), not a variable font. It shows in Roblox's menus and in games that use Roblox's built-in fonts; games
      that bring their own fonts keep them. Restart Roblox to see a change.
    </p>
  </Card>

  <!-- Sky -->
  <Card title="Sky" subtitle="The skybox in games that don't set their own" icon={CloudSun}>
    {#snippet actions()}
      {#if cfg?.sky}<Button
          size="sm"
          variant="ghost"
          icon={RotateCcw}
          onclick={() => run('sky', () => api.modsClear('sky'))}>Reset</Button
        >{/if}
    {/snippet}
    <div class="sky-view">
      {#if view?.previews.sky}
        <img src={view.previews.sky} alt="The sky all the way around the horizon" />
      {:else}
        <span class="faint small">Roblox's sky preview isn't available.</span>
      {/if}
      {#if busy === 'sky'}<span class="sky-busy">Building the sky…</span>{/if}
    </div>
    <div class="tiles">
      <button
        class="tile"
        class:sel={skySel === null}
        onclick={() => run('sky', () => api.modsClear('sky'))}
        disabled={busy === 'sky'}
      >
        <span class="tile-art roblox">R</span><span>Roblox</span>
      </button>
      {#each view?.skyPresets ?? [] as p (p.id)}
        <button
          class="tile"
          class:sel={skySel === p.id}
          onclick={() => run('sky', () => api.modsSetSky({ preset: p.id }))}
          disabled={busy === 'sky'}
        >
          <span class="tile-art" style:background={SKY_ART[p.id]}></span><span>{p.label}</span>
        </button>
      {/each}
      <button class="tile" class:sel={skySel === 'panorama'} onclick={skyFromPanorama} disabled={busy === 'sky'}>
        <span class="tile-art"><ImagePlus size={20} aria-hidden="true" /></span><span>360° picture…</span>
      </button>
      <button class="tile" class:sel={skySel === 'faces'} onclick={skyFromFaces} disabled={busy === 'sky'}>
        <span class="tile-art"><Images size={20} aria-hidden="true" /></span><span>Six pictures…</span>
      </button>
    </div>
    {#if cfg?.sky?.source.kind === 'file'}<p class="current">{cfg.sky.source.name}</p>{/if}
    <div class="row-buttons">
      <Button variant="ghost" icon={ExternalLink} onclick={() => api.openUrl('https://polyhaven.com/hdris')}
        >Find free 360° skies</Button
      >
    </div>
    <p class="faint xs">
      A 360° picture is a panorama twice as wide as it is tall (on Poly Haven, download the “Tonemapped JPG”). Six
      pictures are named Back, Down, Front, Left, Right and Up, like a Roblox Sky. This replaces Roblox's default sky,
      so it shows in games that use it. Many popular games place their own Sky, and theirs wins there. Restart Roblox to
      see a change.
    </p>
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
      {#if isMac}
        Put files in the folder below using the same paths as inside Roblox.app/Contents/Resources, e.g.
        <span class="mono">content/textures/…</span>.
      {:else}
        Put files in the folder below using the same paths as inside Roblox's version folder, e.g.
        <span class="mono">content\textures\…</span>.
      {/if}
      {view?.customFileCount ?? 0} file(s) there now.
    </p>
    <Button icon={FolderOpen} onclick={() => api.modsOpenCustomFolder()}>Open folder</Button>
  </Card>
</div>

{#if !isMac}<section class="shaders glass">
    <div class="row top">
      <span class="ico"><Paintbrush size={16} aria-hidden="true" /></span>
      <div class="sh-titles">
        <h3>Shaders</h3>
        <p class="faint small">Colour grading for the games you play in Roblox</p>
      </div>
      <Badge tone={shaderBadge.tone} dot>{shaderBadge.label}</Badge>
      <Toggle
        label="Shaders"
        checked={shaders.enabled}
        onchange={(v) => app.saveSettings({ shaders: { enabled: v } })}
      />
    </div>
    {#if app.shaders.state === 'failed' && shaders.enabled}
      <p class="small bad">{app.shaders.detail}</p>
    {/if}
    <div class="sh-body">
      <div class="sh-preview">
        <div class="sh-scene" style:filter={previewFilter}>
          <RobloxScene height={190}>
            <div class="sh-parts" aria-hidden="true">
              <span style:--c="#e8473b" style:--h="58px"></span>
              <span style:--c="#3b7ae8" style:--h="88px"></span>
              <span style:--c="#f2c94c" style:--h="42px"></span>
              <span style:--c="#3fbf6f" style:--h="70px"></span>
              <span style:--c="#f7f7f7" style:--h="30px"></span>
            </div>
          </RobloxScene>
          {#if shaders.params.vignette > 0}<span class="sh-vignette" style:opacity={shaders.params.vignette}
            ></span>{/if}
        </div>
        <p class="faint xs">Preview is approximate; the real grade runs on your graphics card.</p>
        <div class="tiles looks">
          {#each shaderLooks as l (l.id)}
            <button class="tile" class:sel={shaders.preset === l.id} onclick={() => shaderLook(l.id)}>
              <span>{l.label}</span>
            </button>
          {/each}
        </div>
      </div>
      <div class="sh-sliders">
        {#each SLIDERS as sl (sl.key)}
          <div class="sh-row">
            <span class="sh-label">{sl.label}</span>
            <Slider
              label={sl.label}
              min={sl.min}
              max={sl.max}
              unit={sl.unit}
              bind:value={sliderValues[sl.key]}
              onchange={(v) => setParam(sl.key, v, sl.scale)}
            />
          </div>
        {/each}
      </div>
    </div>
    <p class="muted small">
      Vesper never loads anything into Roblox; its anti-cheat blocks tools that do (ReShade included). Instead it works
      like a screen recorder: Windows hands over each frame Roblox draws, your graphics card grades it, and Vesper shows
      the result in a click-through layer exactly over Roblox's window. That adds about one frame of delay. It runs
      while you're in a game (Roblox's home screen and menus stay as they are) and stays on when you click another
      window; windows you put in front of Roblox stay in front. Turn it on or off from the tray icon too.
    </p>
    <p class="muted small">
      Windows Security can mistake this for something harmful, because capturing another app's window is also what
      spyware does, and remove Vesper. If it does, open Windows Security → Virus &amp; threat protection → Protection
      history, choose Vesper and restore it, then tell Microsoft it was a mistake.
    </p>
    <div class="row-buttons">
      <Button
        variant="ghost"
        icon={ExternalLink}
        onclick={() => api.openUrl('https://www.microsoft.com/wdsi/filesubmission')}
        >Report a mistake to Microsoft</Button
      >
    </div>
    <details class="studio">
      <summary class="small">Making your own game? Copy these looks as a Studio script</summary>
      <div class="row">
        <Button
          size="sm"
          icon={Copy}
          onclick={async () => {
            await navigator.clipboard.writeText(studioScript);
            app.toast('Studio script copied. Paste it into a LocalScript in StarterPlayerScripts.', 'ok');
          }}>Copy Studio script</Button
        >
      </div>
    </details>
  </section>{/if}

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
  .try-cursor {
    position: absolute;
    width: 64px;
    height: 64px;
    transform: translate(-50%, -50%);
    pointer-events: none;
    z-index: 1;
  }
  .scene-hint {
    position: absolute;
    bottom: 10px;
    padding: 3px 9px;
    border-radius: 999px;
    font-size: var(--text-xs);
    color: #fff;
    background: rgba(10, 10, 16, 0.5);
    pointer-events: none;
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(72px, 1fr));
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
  .tile-art.round {
    border-radius: 50%;
    object-fit: cover;
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
  .swatch.original {
    background: conic-gradient(#fff 0 25%, #c9bdff 0 50%, #fff 0 75%, #c9bdff 0);
  }
  .swatch.sel {
    box-shadow:
      0 0 0 2px var(--bg),
      0 0 0 4px var(--accent-strong);
  }
  .current {
    font-weight: 600;
  }
  .current .from {
    font-weight: 400;
  }
  .font-sample {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-4);
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    background:
      linear-gradient(180deg, color-mix(in srgb, var(--accent) 10%, transparent), transparent), var(--surface-2);
    min-width: 0;
  }
  .font-sample .aa {
    font-size: 44px;
    line-height: 1;
    color: var(--text);
  }
  .font-sample .lines {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .font-sample .lines span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .font-sample .digits {
    color: var(--text-2);
    font-size: var(--text-sm);
  }
  .row-buttons {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
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
    max-width: 90ch;
  }
  .sh-titles {
    flex: 1;
  }
  .sh-titles h3 {
    margin: 0;
  }
  .sh-titles p {
    margin: 0;
  }
  .sh-body {
    display: grid;
    grid-template-columns: minmax(260px, 0.9fr) minmax(0, 1.1fr);
    gap: var(--space-5);
  }
  @media (max-width: 1060px) {
    .sh-body {
      grid-template-columns: 1fr;
    }
  }
  .sh-preview {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .sh-scene {
    position: relative;
    border-radius: var(--radius-md);
    overflow: hidden;
  }
  .sh-parts {
    position: absolute;
    inset: auto 0 34px 0;
    display: flex;
    align-items: flex-end;
    justify-content: center;
    gap: 14px;
  }
  .sh-parts span {
    width: 34px;
    height: var(--h);
    border-radius: 4px;
    background: linear-gradient(90deg, var(--c), color-mix(in srgb, var(--c) 70%, #000));
    box-shadow: 0 6px 10px rgba(0, 0, 0, 0.25);
  }
  .sh-vignette {
    position: absolute;
    inset: 0;
    pointer-events: none;
    background: radial-gradient(ellipse at center, transparent 45%, rgba(0, 0, 0, 0.85));
  }
  .looks {
    grid-template-columns: repeat(auto-fill, minmax(92px, 1fr));
  }
  .looks .tile {
    padding: 8px 6px;
  }
  .sh-sliders {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--space-2) var(--space-5);
    align-content: start;
  }
  @media (max-width: 1260px) {
    .sh-sliders {
      grid-template-columns: 1fr;
    }
  }
  .sh-row {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .sh-row :global(.slider) {
    min-width: 0;
  }
  .sh-label {
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text-2);
  }
  .studio summary {
    cursor: pointer;
    color: var(--text-3);
  }
  .studio .row {
    margin-top: var(--space-2);
  }
  .sky-view {
    position: relative;
    display: grid;
    place-items: center;
    aspect-ratio: 4 / 1;
    overflow: hidden;
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    background: var(--surface-3);
  }
  .sky-view img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .sky-busy {
    position: absolute;
    inset: auto 8px 8px auto;
    padding: 3px 8px;
    border-radius: var(--radius-pill);
    font-size: var(--text-xs);
    background: rgba(0, 0, 0, 0.6);
    color: #fff;
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
