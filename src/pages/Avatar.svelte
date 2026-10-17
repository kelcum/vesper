<script lang="ts">
  import {
    ExternalLink,
    LogIn,
    LogOut,
    Plus,
    RefreshCw,
    Shirt,
    ShieldCheck,
    Sparkles,
    UserRound,
    X,
  } from '@lucide/svelte';
  import { onDestroy, onMount, tick } from 'svelte';
  import Badge from '../components/ui/Badge.svelte';
  import Button from '../components/ui/Button.svelte';
  import Card from '../components/ui/Card.svelte';
  import Segmented from '../components/ui/Segmented.svelte';
  import Select from '../components/ui/Select.svelte';
  import { api, toProblem, type Bounds } from '../lib/api';
  import {
    avatar,
    BODY_PARTS,
    CATEGORIES,
    parseAssetId,
    type Avatar as AvatarData,
    type AvatarAsset,
    type BodyColors,
    type InventoryItem,
    type Me,
    type Outfit,
    type Rules,
    type Scales,
  } from '../lib/avatar';
  import { app } from '../lib/store.svelte';

  const REMEMBER = 'vesper.avatar.used';
  type Mode = 'tools' | 'roblox';

  let started = $state(false);
  let mode = $state<Mode>('tools');
  let me = $state<Me | null>(null);
  let signedIn = $state<boolean | null>(null);
  let loading = $state(false);
  let working = $state<string | null>(null);
  let data = $state<AvatarData | null>(null);
  let rules = $state<Rules | null>(null);
  let render = $state<string | null>(null);
  let rendering = $state(false);
  let thumbs = $state<Map<number, string>>(new Map());
  let tab = $state<'wearing' | 'inventory' | 'body' | 'outfits'>('wearing');
  let wearInput = $state('');
  let frame = $state<HTMLDivElement | null>(null);

  // Inventory
  let category = $state(CATEGORIES[0].id);
  let items = $state<InventoryItem[]>([]);
  let cursor = $state<string | null>(null);
  let itemsLoading = $state(false);
  // Outfits
  let outfits = $state<Outfit[]>([]);
  let outfitThumbs = $state<Map<number, string>>(new Map());
  // Body
  let colors = $state<BodyColors | null>(null);
  let scales = $state<Scales | null>(null);

  const worn = $derived(new Set((data?.assets ?? []).map((a) => a.id)));
  const typeLimit = (typeId: number) => rules?.wearableAssetTypes.find((t) => t.id === typeId)?.maxNumber;

  function remembered() {
    try {
      return localStorage.getItem(REMEMBER) === '1';
    } catch {
      return false;
    }
  }

  onMount(() => {
    if (remembered()) start();
  });

  onDestroy(() => {
    window.removeEventListener('resize', place);
    scroller?.removeEventListener('scroll', place);
    observer?.disconnect();
    clearInterval(signInPoll);
    api.avatarClose().catch(() => {});
  });

  // ------------------------------------------------ the docked Roblox page

  let scroller: Element | null = null;
  let observer: ResizeObserver | null = null;

  function bounds(): Bounds {
    const r = frame?.getBoundingClientRect();
    const k = window.devicePixelRatio || 1;
    if (!r) return { x: 0, y: 0, width: 1, height: 1 };
    return {
      x: Math.round(r.left * k),
      y: Math.round(r.top * k),
      width: Math.round(r.width * k),
      height: Math.round(r.height * k),
    };
  }

  /** Keeps the Roblox page over its frame, or hidden when it shouldn't show. */
  function place() {
    const show = started && mode === 'roblox' && !app.problem && !app.closePrompt && app.page === 'avatar';
    if (show) api.avatarBounds(bounds()).catch(() => {});
    else api.avatarHide().catch(() => {});
  }

  $effect(() => {
    // Re-place when anything that affects visibility changes.
    void [mode, app.problem, app.closePrompt, started];
    tick().then(place);
  });

  $effect(() => {
    if (!frame) return;
    scroller = frame.closest('.scroll');
    scroller?.addEventListener('scroll', place, { passive: true });
    window.addEventListener('resize', place);
    observer = new ResizeObserver(place);
    observer.observe(frame);
    return () => {
      scroller?.removeEventListener('scroll', place);
      window.removeEventListener('resize', place);
      observer?.disconnect();
    };
  });

  async function show(page: 'editor' | 'login') {
    mode = 'roblox';
    await tick();
    await app.attempt(() => api.avatarOpen(bounds(), page, true));
  }

  // ------------------------------------------------ signing in

  let signInPoll: ReturnType<typeof setInterval> | undefined;

  async function start() {
    started = true;
    try {
      localStorage.setItem(REMEMBER, '1');
    } catch {
      /* only a convenience */
    }
    loading = true;
    const opened = await app.attempt(() => api.avatarOpen(bounds(), 'bridge', false));
    if (opened === undefined) {
      loading = false;
      return;
    }
    await checkSignIn(true);
    loading = false;
  }

  /** Asks Roblox who is signed in. Waits for the page to load first. */
  async function checkSignIn(wait: boolean) {
    for (let i = 0; i < (wait ? 30 : 1); i++) {
      try {
        me = await avatar.me();
        signedIn = true;
        await loadAll();
        return;
      } catch (e) {
        const p = toProblem(e);
        if (p.code === 'avatar.signed_out') {
          signedIn = false;
          me = null;
          return;
        }
        if (!wait || !/loading|isn't open/.test(p.what)) {
          if (wait) app.problem = p;
          return;
        }
        await new Promise((r) => setTimeout(r, 500));
      }
    }
  }

  async function signIn() {
    await show('login');
    clearInterval(signInPoll);
    signInPoll = setInterval(async () => {
      if (signedIn) return clearInterval(signInPoll);
      await checkSignIn(false);
      if (signedIn) {
        clearInterval(signInPoll);
        app.toast(`Signed in as @${me?.name}`, 'ok');
        mode = 'tools';
      }
    }, 3000);
  }

  async function signOut() {
    working = 'signout';
    await app.attempt(() => api.avatarSignOut(), 'Signed out of Roblox in Vesper');
    working = null;
    signedIn = false;
    me = null;
    data = null;
    render = null;
    started = false;
    mode = 'tools';
    try {
      localStorage.removeItem(REMEMBER);
    } catch {
      /* only a convenience */
    }
  }

  // ------------------------------------------------ loading

  async function loadAll() {
    const [d, r] = await Promise.all([
      app.attempt(() => avatar.get()),
      rules ? rules : app.attempt(() => avatar.rules()),
    ]);
    if (r) rules = r;
    if (d) setData(d);
    refreshRender();
  }

  function setData(d: AvatarData) {
    data = d;
    colors = { ...d.bodyColor3s };
    scales = { ...d.scales };
    loadThumbs(d.assets.map((a) => a.id));
  }

  async function loadThumbs(ids: number[]) {
    const missing = ids.filter((id) => !thumbs.has(id));
    if (!missing.length) return;
    const got = await avatar.assetThumbs(missing).catch(() => null);
    if (got) thumbs = new Map([...thumbs, ...got]);
  }

  /** Roblox redraws the avatar after a change; waits until the new picture is ready. */
  async function refreshRender(redraw = false) {
    if (!me) return;
    rendering = true;
    const before = render;
    if (redraw) await avatar.redraw().catch(() => {});
    for (let i = 0; i < 12; i++) {
      const url = await avatar.render(me.id).catch(() => null);
      if (url && (!redraw || url !== before || i >= 6)) {
        render = url;
        break;
      }
      await new Promise((r) => setTimeout(r, 1200));
    }
    rendering = false;
  }

  /** Runs a change, then reloads the avatar and its picture. */
  async function change(key: string, fn: () => Promise<unknown>, done?: string) {
    working = key;
    const ok = await app.attempt(fn);
    working = null;
    if (ok === undefined) return false;
    const d = await app.attempt(() => avatar.get());
    if (d) setData(d);
    if (done) app.toast(done, 'ok');
    refreshRender(true);
    return true;
  }

  // ------------------------------------------------ wearing

  async function wearList(next: AvatarAsset[], added?: { id: number; name: string; typeId: number }) {
    working = 'wear';
    try {
      let r = await avatar.wear(next);
      let note: string | null = null;
      if (added && r.invalidAssetIds?.includes(added.id)) {
        // Roblox's limit for this kind of item: swap out the oldest one of the same kind.
        const same = (data?.assets ?? []).filter((a) => a.assetType.id === added.typeId);
        if (same.length) {
          const swapped = next.filter((a) => a.id !== same[0].id);
          r = await avatar.wear(swapped);
          if (!r.invalidAssetIds?.includes(added.id))
            note = `Swapped ${same[0].name} for ${added.name}; Roblox allows ${typeLimit(added.typeId) ?? 'a limited number'} of these.`;
        }
      }
      if (added && r.invalidAssetIds?.includes(added.id)) {
        app.toast(`Roblox didn't accept ${added.name}. You need to own it, and it has to be wearable.`, 'warn');
      } else if (note) {
        app.toast(note, 'info');
      } else if (added) {
        app.toast(`Wearing ${added.name}`, 'ok');
      }
    } catch (e) {
      app.problem = toProblem(e);
    }
    working = null;
    const d = await app.attempt(() => avatar.get());
    if (d) setData(d);
    refreshRender(true);
  }

  async function addAsset(id: number) {
    if (!data) return;
    if (worn.has(id)) return app.toast("You're already wearing that.", 'info');
    const info = await app.attempt(() => avatar.details(id));
    if (!info) return;
    await wearList([...data.assets, { id, name: info.Name, assetType: { id: info.AssetTypeId, name: '' } }], {
      id,
      name: info.Name,
      typeId: info.AssetTypeId,
    });
  }

  async function wearById() {
    const id = parseAssetId(wearInput);
    if (!id) return app.toast('Paste an item ID or a link to an item on roblox.com.', 'warn');
    await addAsset(id);
    wearInput = '';
  }

  function removeAsset(a: AvatarAsset) {
    if (!data) return;
    wearList(data.assets.filter((x) => x.id !== a.id));
  }

  function toggleItem(item: InventoryItem) {
    if (!data) return;
    const current = data.assets.find((a) => a.id === item.assetId);
    if (current) removeAsset(current);
    else addAsset(item.assetId);
  }

  // ------------------------------------------------ inventory & outfits

  async function loadItems(more = false) {
    if (!me) return;
    const cat = CATEGORIES.find((c) => c.id === category) ?? CATEGORIES[0];
    itemsLoading = true;
    const r = await app.attempt(() => avatar.inventory(me!.id, cat.types, more ? (cursor ?? undefined) : undefined));
    itemsLoading = false;
    if (!r) return;
    items = more ? [...items, ...r.data] : r.data;
    cursor = r.nextPageCursor;
    loadThumbs(r.data.map((i) => i.assetId));
  }

  async function loadOutfits() {
    if (!me) return;
    itemsLoading = true;
    const r = await app.attempt(() => avatar.outfits(me!.id));
    itemsLoading = false;
    if (!r) return;
    outfits = r.data;
    const t = await avatar.outfitThumbs(r.data.map((o) => o.id)).catch(() => null);
    if (t) outfitThumbs = t;
  }

  $effect(() => {
    if (!signedIn || !me) return;
    if (tab === 'inventory') {
      void category;
      loadItems();
    } else if (tab === 'outfits' && !outfits.length) {
      loadOutfits();
    }
  });

  async function wearOutfit(o: Outfit) {
    await change(
      'outfit',
      async () => {
        const d = await avatar.outfitDetails(o.id);
        await avatar.setType(d.playerAvatarType);
        await avatar.setColorIds(d.bodyColors);
        await avatar.setScales({ ...d.scale });
        const r = await avatar.wear(d.assets);
        if (r.invalidAssetIds?.length)
          app.toast(`${r.invalidAssetIds.length} item(s) in that outfit couldn't be worn.`, 'warn');
      },
      `Wearing ${o.name}`,
    );
  }

  // ------------------------------------------------ body

  function setAllColors(hex: string) {
    if (!colors) return;
    const h = hex.replace('#', '').toUpperCase();
    const next = { ...colors };
    for (const p of BODY_PARTS) next[p.key] = h;
    colors = next;
    saveColors();
  }

  function saveColors() {
    if (colors) change('colors', () => avatar.setColors(colors!));
  }

  let scaleTimer: ReturnType<typeof setTimeout> | undefined;
  function saveScales() {
    clearTimeout(scaleTimer);
    scaleTimer = setTimeout(() => {
      if (scales) change('scales', () => avatar.setScales(scales!));
    }, 500);
  }

  const SCALE_LABELS: { key: 'height' | 'width' | 'head' | 'proportion' | 'bodyType'; label: string }[] = [
    { key: 'height', label: 'Height' },
    { key: 'width', label: 'Width' },
    { key: 'head', label: 'Head' },
    { key: 'bodyType', label: 'Body type' },
    { key: 'proportion', label: 'Proportions' },
  ];
</script>

<header class="head">
  <div>
    <p class="eyebrow">Avatar</p>
    <h1>Your avatar</h1>
  </div>
  {#if started}
    <div class="toolbar">
      <Segmented
        label="Editor"
        size="sm"
        value={mode}
        options={[
          { value: 'tools', label: 'Vesper tools' },
          { value: 'roblox', label: "Roblox's editor" },
        ]}
        onchange={(v) => (v === 'roblox' ? show('editor') : (mode = 'tools'))}
      />
      {#if mode === 'roblox'}
        <Button
          size="sm"
          variant="ghost"
          icon={RefreshCw}
          label="Reload"
          title="Reload Roblox's page"
          onclick={() => api.avatarReload()}
        />
      {/if}
      <Button
        size="sm"
        variant="ghost"
        icon={ExternalLink}
        label="Open in browser"
        title="Open Roblox's avatar editor in your browser"
        onclick={() => api.openUrl('https://www.roblox.com/my/avatar')}
      />
      {#if signedIn}
        <Button size="sm" variant="ghost" icon={LogOut} loading={working === 'signout'} onclick={signOut}
          >Sign out</Button
        >
      {/if}
    </div>
  {/if}
</header>

{#if !started}
  <section class="intro glass">
    <div class="intro-art"><UserRound size={40} strokeWidth={1.4} aria-hidden="true" /></div>
    <div class="intro-copy">
      <h2>Change your avatar without leaving Vesper</h2>
      <p class="muted">
        Wear and remove accessories, try anything you own by ID, change body colours, scale and body type, and switch
        outfits. Roblox's own editor is one click away too.
      </p>
      <ul class="facts">
        <li>
          <ShieldCheck size={15} aria-hidden="true" /> You sign in on Roblox's real login page, inside Vesper. Your password
          and session stay with that page; Vesper's code never reads them.
        </li>
        <li>
          <Sparkles size={15} aria-hidden="true" /> Vesper adds no limits of its own. Roblox decides what can be worn together,
          and Vesper shows its answer.
        </li>
      </ul>
      <Button variant="primary" icon={LogIn} {loading} onclick={start}>Open the avatar editor</Button>
    </div>
  </section>
{:else}
  <div class="frame-wrap" class:hidden={mode !== 'roblox'}>
    <div class="frame" bind:this={frame}>
      <span class="faint small">Loading Roblox…</span>
    </div>
    <p class="faint xs frame-note">This is roblox.com. Links to other sites open in your browser.</p>
  </div>

  {#if mode === 'tools'}
    {#if loading || signedIn === null}
      <Card><p class="muted">Connecting to Roblox…</p></Card>
    {:else if !signedIn}
      <section class="intro glass">
        <div class="intro-art"><LogIn size={36} strokeWidth={1.4} aria-hidden="true" /></div>
        <div class="intro-copy">
          <h2>Sign in to Roblox</h2>
          <p class="muted">
            Roblox's login page opens here. After you sign in, Vesper's tools appear. The sign-in is remembered on this
            PC until you sign out.
          </p>
          <Button variant="primary" icon={LogIn} onclick={signIn}>Sign in</Button>
        </div>
      </section>
    {:else}
      <div class="tools">
        <aside class="render glass">
          <div class="render-img" class:busy={rendering}>
            {#if render}<img src={render} alt="Your avatar" />{:else}<UserRound
                size={64}
                strokeWidth={1}
                aria-hidden="true"
              />{/if}
          </div>
          <div class="who">
            <strong>{me?.displayName}</strong>
            <span class="faint">@{me?.name}</span>
          </div>
          {#if data}
            <Segmented
              label="Body type"
              size="sm"
              value={data.playerAvatarType}
              options={[
                { value: 'R15', label: 'R15' },
                { value: 'R6', label: 'R6' },
              ]}
              onchange={(v) => change('type', () => avatar.setType(v), `Switched to ${v}`)}
            />
          {/if}
          <Button size="sm" variant="ghost" icon={RefreshCw} loading={rendering} onclick={() => refreshRender(true)}
            >Redraw picture</Button
          >
        </aside>

        <div class="panel">
          <Segmented
            label="Avatar section"
            value={tab}
            options={[
              { value: 'wearing', label: 'Wearing' },
              { value: 'inventory', label: 'Inventory' },
              { value: 'body', label: 'Body' },
              { value: 'outfits', label: 'Outfits' },
            ]}
            onchange={(v) => (tab = v)}
          />

          {#if tab === 'wearing'}
            <div class="wear-by-id">
              <input
                class="input"
                placeholder="Item ID or roblox.com item link"
                aria-label="Item ID or link"
                bind:value={wearInput}
                onkeydown={(e) => e.key === 'Enter' && wearById()}
                spellcheck="false"
              />
              <Button icon={Plus} loading={working === 'wear'} disabled={!wearInput.trim()} onclick={wearById}
                >Wear</Button
              >
            </div>
            <p class="faint xs">
              Roblox's own editor lets you wear 3 hats and one of each other accessory. Vesper doesn't add limits: it
              asks Roblox directly, and if Roblox says no, Vesper tells you (or swaps out an item of the same kind).
            </p>
            {#if data?.assets.length}
              <ul class="items" role="list">
                {#each data.assets as a (a.id)}
                  <li class="item glass">
                    <span class="thumb"
                      >{#if thumbs.get(a.id)}<img src={thumbs.get(a.id)} alt="" />{:else}<Shirt
                          size={20}
                          aria-hidden="true"
                        />{/if}</span
                    >
                    <span class="item-name" title={a.name}>{a.name}</span>
                    <span class="faint xs">{a.assetType.name}</span>
                    <button
                      class="remove"
                      title="Take off"
                      aria-label="Take off {a.name}"
                      disabled={!!working}
                      onclick={() => removeAsset(a)}><X size={14} aria-hidden="true" /></button
                    >
                  </li>
                {/each}
              </ul>
            {:else}
              <p class="muted">You're not wearing anything right now.</p>
            {/if}
          {:else if tab === 'inventory'}
            <div class="inv-head">
              <Select
                label="Category"
                value={category}
                options={CATEGORIES.map((c) => ({ value: c.id, label: c.label }))}
                onchange={(v) => (category = v)}
              />
              <span class="faint small">Click an item to put it on or take it off.</span>
            </div>
            {#if itemsLoading && !items.length}
              <p class="muted">Loading your items…</p>
            {:else if !items.length}
              <p class="muted">Nothing in this category.</p>
            {/if}
            <ul class="items grid" role="list">
              {#each items as it (it.assetId)}
                <li>
                  <button
                    class="item tile-item glass"
                    class:on={worn.has(it.assetId)}
                    disabled={!!working}
                    onclick={() => toggleItem(it)}
                    title={it.assetName}
                  >
                    <span class="thumb big"
                      >{#if thumbs.get(it.assetId)}<img src={thumbs.get(it.assetId)} alt="" />{:else}<Shirt
                          size={22}
                          aria-hidden="true"
                        />{/if}</span
                    >
                    <span class="item-name">{it.assetName}</span>
                    {#if worn.has(it.assetId)}<Badge tone="accent">Wearing</Badge>{/if}
                  </button>
                </li>
              {/each}
            </ul>
            {#if cursor}
              <Button size="sm" variant="ghost" loading={itemsLoading} onclick={() => loadItems(true)}>Load more</Button
              >
            {/if}
          {:else if tab === 'body'}
            {#if colors}
              <h3 class="sub">Skin tone</h3>
              <div class="palette">
                {#each rules?.bodyColorsPalette ?? [] as c (c.brickColorId)}
                  <button
                    class="swatch"
                    style:--c={c.hexColor}
                    title={c.name}
                    aria-label={c.name}
                    onclick={() => setAllColors(c.hexColor)}
                  ></button>
                {/each}
              </div>
              <h3 class="sub">Each part</h3>
              <div class="parts">
                {#each BODY_PARTS as p (p.key)}
                  <label class="part">
                    <input
                      type="color"
                      value={'#' + colors[p.key]}
                      onchange={(e) => {
                        colors = {
                          ...colors!,
                          [p.key]: (e.currentTarget as HTMLInputElement).value.slice(1).toUpperCase(),
                        };
                        saveColors();
                      }}
                    />
                    <span>{p.label}</span>
                  </label>
                {/each}
              </div>
            {/if}
            {#if scales && rules && data?.playerAvatarType === 'R15'}
              <h3 class="sub">Scale</h3>
              <div class="scales">
                {#each SCALE_LABELS as sl (sl.key)}
                  {@const r = rules.scales[sl.key]}
                  <label class="scale">
                    <span>{sl.label}</span>
                    <input
                      type="range"
                      min={r.min}
                      max={r.max}
                      step={r.increment}
                      bind:value={scales[sl.key]}
                      oninput={saveScales}
                    />
                    <output>{Math.round(scales[sl.key] * 100)}%</output>
                  </label>
                {/each}
              </div>
            {:else if data?.playerAvatarType === 'R6'}
              <p class="faint small">Scaling only applies to R15 avatars.</p>
            {/if}
          {:else}
            {#if itemsLoading && !outfits.length}<p class="muted">Loading outfits…</p>{/if}
            {#if !itemsLoading && !outfits.length}<p class="muted">You haven't saved any outfits yet.</p>{/if}
            <ul class="items grid" role="list">
              {#each outfits as o (o.id)}
                <li>
                  <button
                    class="item tile-item glass"
                    disabled={!!working}
                    onclick={() => wearOutfit(o)}
                    title="Wear {o.name}"
                  >
                    <span class="thumb big"
                      >{#if outfitThumbs.get(o.id)}<img src={outfitThumbs.get(o.id)} alt="" />{:else}<UserRound
                          size={22}
                          aria-hidden="true"
                        />{/if}</span
                    >
                    <span class="item-name">{o.name}</span>
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
        </div>
      </div>
    {/if}
  {/if}
{/if}

<style>
  .head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: var(--space-4);
    margin-bottom: var(--space-5);
  }
  .eyebrow {
    margin: 0;
    font-size: var(--text-xs);
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  h1 {
    margin: 4px 0 0;
    font-size: var(--text-2xl);
    letter-spacing: var(--tracking-tight);
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .intro {
    display: flex;
    gap: var(--space-6);
    padding: var(--space-6);
    border-radius: var(--radius-xl);
  }
  .intro-art {
    display: grid;
    place-items: center;
    flex: none;
    width: 88px;
    height: 88px;
    border-radius: 24px;
    color: var(--accent-strong);
    background: color-mix(in srgb, var(--accent) 16%, transparent);
  }
  .intro-copy {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-3);
    max-width: 640px;
  }
  .intro h2 {
    margin: 0;
    font-size: var(--text-xl);
  }
  .intro p {
    margin: 0;
  }
  .facts {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin: 0 0 var(--space-2);
    padding: 0;
    list-style: none;
    font-size: var(--text-sm);
    color: var(--text-2);
  }
  .facts li {
    display: flex;
    gap: var(--space-2);
  }
  .facts :global(svg) {
    flex: none;
    margin-top: 2px;
    color: var(--accent-strong);
  }
  .frame-wrap.hidden {
    display: none;
  }
  .frame {
    display: grid;
    place-items: center;
    height: calc(100vh - 210px);
    min-height: 360px;
    border-radius: var(--radius-lg);
    border: 1px solid var(--border);
    background: #121216;
  }
  .frame-note {
    margin: var(--space-2) 0 0;
  }
  .tools {
    display: grid;
    grid-template-columns: 280px minmax(0, 1fr);
    gap: var(--space-5);
    align-items: start;
  }
  .render {
    position: sticky;
    top: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-4);
    border-radius: var(--radius-lg);
  }
  .render-img {
    display: grid;
    place-items: center;
    width: 100%;
    aspect-ratio: 1;
    border-radius: var(--radius-md);
    color: var(--text-3);
    background: radial-gradient(circle at 50% 40%, color-mix(in srgb, var(--accent) 18%, transparent), transparent 70%);
    transition: opacity var(--dur) var(--ease-out);
  }
  .render-img.busy {
    opacity: 0.55;
  }
  .render-img img {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
  .who {
    display: flex;
    flex-direction: column;
    align-items: center;
  }
  .panel {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    min-width: 0;
  }
  .wear-by-id {
    display: flex;
    gap: var(--space-2);
  }
  .wear-by-id .input {
    flex: 1;
  }
  .input {
    height: 38px;
    padding: 0 12px;
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text);
    font-family: var(--font-mono);
    font-size: var(--text-sm);
  }
  .input:focus-visible {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 22%, transparent);
  }
  .items {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
  }
  .items.grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(124px, 1fr));
  }
  .item {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: 8px 10px;
    border-radius: var(--radius-md);
  }
  .item-name {
    flex: 1;
    min-width: 0;
    font-weight: 550;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .thumb {
    display: grid;
    place-items: center;
    flex: none;
    width: 44px;
    height: 44px;
    overflow: hidden;
    border-radius: 9px;
    color: var(--text-3);
    background: var(--surface-3);
  }
  .thumb.big {
    width: 84px;
    height: 84px;
  }
  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
  .remove {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--text-3);
    cursor: pointer;
  }
  .remove:hover:not(:disabled) {
    color: var(--bad);
    background: color-mix(in srgb, var(--bad) 14%, transparent);
  }
  .tile-item {
    width: 100%;
    flex-direction: column;
    gap: 6px;
    padding: 10px;
    border: 1px solid transparent;
    color: var(--text);
    font: inherit;
    font-size: var(--text-xs);
    cursor: pointer;
    text-align: center;
  }
  .tile-item .item-name {
    width: 100%;
  }
  .tile-item:hover:not(:disabled) {
    border-color: var(--border-strong);
  }
  .tile-item.on {
    border-color: color-mix(in srgb, var(--accent) 60%, transparent);
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }
  .tile-item:disabled {
    opacity: 0.6;
    cursor: progress;
  }
  .inv-head {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    flex-wrap: wrap;
  }
  .sub {
    margin: var(--space-2) 0 0;
    font-size: var(--text-sm);
    color: var(--text-2);
  }
  .palette {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .swatch {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    border: 2px solid var(--border-strong);
    background: var(--c);
    cursor: pointer;
    transition: transform var(--dur-fast) var(--ease-spring);
  }
  .swatch:hover {
    transform: scale(1.12);
  }
  .parts {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
    gap: var(--space-2);
  }
  .part {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--text-sm);
  }
  .part input {
    width: 34px;
    height: 28px;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: none;
    cursor: pointer;
  }
  .scales {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    max-width: 520px;
  }
  .scale {
    display: grid;
    grid-template-columns: 110px 1fr 52px;
    align-items: center;
    gap: var(--space-3);
    font-size: var(--text-sm);
  }
  .scale output {
    text-align: right;
    font-variant-numeric: tabular-nums;
    color: var(--text-2);
  }
  @media (max-width: 1000px) {
    .tools {
      grid-template-columns: 1fr;
    }
    .render {
      position: static;
    }
  }
</style>
