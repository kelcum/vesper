<script lang="ts">
  import {
    Copy,
    ExternalLink,
    History as HistoryIcon,
    Layers,
    LogIn,
    MapPin,
    Pin,
    PinOff,
    Play,
    Search,
    Trash,
    Trophy,
    X,
  } from '@lucide/svelte';
  import Badge from '../components/ui/Badge.svelte';
  import Button from '../components/ui/Button.svelte';
  import Card from '../components/ui/Card.svelte';
  import Dialog from '../components/ui/Dialog.svelte';
  import Field from '../components/ui/Field.svelte';
  import Toggle from '../components/ui/Toggle.svelte';
  import GameIcon from '../components/games/GameIcon.svelte';
  import GameProfile from '../components/games/GameProfile.svelte';
  import NowPlaying from '../components/games/NowPlaying.svelte';
  import { api } from '../lib/api';
  import { duration, relativeTime, serverKindLabel } from '../lib/format';
  import { matchesQuery, profileForGame, totalsByGame } from '../lib/games';
  import { app } from '../lib/store.svelte';
  import type { Visit } from '../lib/types';

  const games = $derived(app.settings.games);
  const busy = $derived(['preparing', 'starting', 'updating'].includes(app.launch.state));
  let confirmClear = $state(false);
  let query = $state('');
  let profileTarget = $state<{ placeId: number; universeId: number | null; name: string | null } | null>(null);
  let profileOpen = $state(false);

  const shown = $derived(app.history.visits.filter((v) => matchesQuery(v, query)));
  const pinnedVisits = $derived(shown.filter((v) => v.pinned));

  const groups = $derived.by(() => {
    const out: { label: string; visits: Visit[] }[] = [];
    const today = new Date();
    const dayKey = (d: Date) => d.toDateString();
    const yesterday = new Date(today.getTime() - 86400000);
    for (const v of shown) {
      if (v.pinned) continue;
      const d = new Date(v.joinedAt);
      const label =
        dayKey(d) === dayKey(today)
          ? 'Today'
          : dayKey(d) === dayKey(yesterday)
            ? 'Yesterday'
            : d.toLocaleDateString(undefined, {
                day: 'numeric',
                month: 'short',
                year: d.getFullYear() === today.getFullYear() ? undefined : 'numeric',
              });
      const group = out.find((g) => g.label === label);
      if (group) group.visits.push(v);
      else out.push({ label, visits: [v] });
    }
    return out;
  });

  const mostPlayed = $derived(totalsByGame(app.history.visits).slice(0, 5));

  const joinUrl = (v: Visit) =>
    v.kind === 'public' ? `https://www.roblox.com/games/start?placeId=${v.placeId}&gameInstanceId=${v.jobId}` : null;

  const profileName = (v: Visit) => {
    const id = profileForGame(app.settings, v.placeId, v.universeId);
    return id ? (app.profiles.profiles.find((p) => p.id === id)?.name ?? null) : null;
  };

  async function launch(input: string) {
    if (app.robloxRunning) await app.attempt(() => api.robloxRestart(input, false));
    else await app.attempt(() => api.robloxLaunch(input));
    app.go('home');
  }

  async function copy(v: Visit) {
    const url = joinUrl(v);
    if (!url) return;
    try {
      await navigator.clipboard.writeText(url);
      app.toast('Server link copied.', 'ok');
    } catch {
      app.toast("Couldn't copy the link.", 'warn');
    }
  }

  async function remove(v: Visit) {
    const h = await app.attempt(() => api.historyRemove(v.jobId));
    if (h) app.history = h;
  }

  async function pin(v: Visit) {
    const h = await app.attempt(() => api.historyPin(v.jobId, !v.pinned));
    if (h) app.history = h;
  }

  function chooseProfile(v: { placeId: number; universeId: number | null; name: string | null }) {
    profileTarget = { placeId: v.placeId, universeId: v.universeId, name: v.name };
    profileOpen = true;
  }

  async function clear() {
    const h = await app.attempt(() => api.historyClear(), 'Recent games cleared');
    if (h) app.history = h;
    confirmClear = false;
  }

  const playedFor = (v: Visit) =>
    v.leftAt ? duration(new Date(v.leftAt).getTime() - new Date(v.joinedAt).getTime()) : null;
  const isCurrent = (v: Visit) => app.nowPlaying?.jobId === v.jobId;
</script>

<header class="head">
  <div>
    <p class="eyebrow">Recent</p>
    <h1>Recent games</h1>
  </div>
  <div class="tools">
    {#if app.history.visits.length > 3}
      <label class="search">
        <Search size={14} aria-hidden="true" />
        <input type="search" placeholder="Search games" aria-label="Search recent games" bind:value={query} />
      </label>
    {/if}
    {#if app.history.visits.length}
      <Button variant="ghost" icon={Trash} onclick={() => (confirmClear = true)}>Clear history</Button>
    {/if}
  </div>
</header>

{#if app.nowPlaying}
  <div class="current"><NowPlaying now={app.nowPlaying} /></div>
{/if}

<div class="layout">
  <div class="list">
    {#if !games.history}
      <Card>
        <div class="empty">
          <HistoryIcon size={28} strokeWidth={1.5} aria-hidden="true" />
          <h3>History is off</h3>
          <p>Turn it on to keep a list of the games and servers you play, on this PC only.</p>
          <Button variant="primary" onclick={() => app.saveSettings({ games: { history: true } })}
            >Keep a history</Button
          >
        </div>
      </Card>
    {:else if !app.history.visits.length}
      <Card>
        <div class="empty">
          <HistoryIcon size={28} strokeWidth={1.5} aria-hidden="true" />
          <h3>Nothing here yet</h3>
          <p>
            Games you play show up here with the server you were in, wherever you start them from (Vesper, the Roblox
            app or the website).
          </p>
        </div>
      </Card>
    {:else if !shown.length}
      <Card>
        <div class="empty">
          <Search size={28} strokeWidth={1.5} aria-hidden="true" />
          <h3>No matches</h3>
          <p>Nothing in your recent games matches “{query}”.</p>
        </div>
      </Card>
    {/if}

    {#snippet row(v: Visit)}
      {@const url = joinUrl(v)}
      {@const kind = serverKindLabel(v.kind)}
      {@const assigned = profileName(v)}
      <li class="visit glass" class:pinned={v.pinned}>
        <GameIcon url={v.iconUrl} size={52} />
        <div class="info">
          <div class="title">
            <span class="name" title={v.name ?? ''}>{v.name ?? `Place ${v.placeId}`}</span>
            {#if isCurrent(v)}<Badge tone="ok" dot>Playing</Badge>{/if}
            {#if assigned}<Badge tone="info" title="Launches with this profile"
                ><Layers size={11} aria-hidden="true" /> {assigned}</Badge
              >{/if}
          </div>
          <div class="meta">
            {#if v.creator}<span>by {v.creator}</span>{/if}
            <span title={new Date(v.joinedAt).toLocaleString()}>{relativeTime(v.joinedAt)}</span>
            {#if playedFor(v)}<span>played {playedFor(v)}</span>{/if}
            {#if v.location}<Badge><MapPin size={11} aria-hidden="true" /> {v.location}</Badge>{/if}
            {#if kind}<Badge tone="info">{kind}</Badge>{/if}
          </div>
        </div>
        <div class="actions">
          {#if url && !isCurrent(v)}
            <Button
              size="sm"
              icon={LogIn}
              disabled={busy}
              title="Join this same server, if it's still open"
              onclick={() => launch(url)}>Join server</Button
            >
          {/if}
          <Button
            size="sm"
            variant="ghost"
            icon={Play}
            disabled={busy}
            title="Play this game in any server"
            onclick={() => launch(String(v.placeId))}>Play</Button
          >
          <Button
            size="sm"
            variant="ghost"
            icon={v.pinned ? PinOff : Pin}
            label={v.pinned ? 'Unpin' : 'Pin'}
            title={v.pinned ? 'Unpin' : 'Pin to the top'}
            onclick={() => pin(v)}
          />
          <Button
            size="sm"
            variant="ghost"
            icon={Layers}
            label="Profile for this game"
            title="Profile for this game"
            onclick={() => chooseProfile(v)}
          />
          {#if url}
            <Button
              size="sm"
              variant="ghost"
              icon={Copy}
              label="Copy join link"
              title="Copy join link"
              onclick={() => copy(v)}
            />
          {/if}
          <Button
            size="sm"
            variant="ghost"
            icon={ExternalLink}
            label="Game page"
            title="Open the game's page"
            onclick={() => api.openUrl(`https://www.roblox.com/games/${v.placeId}`)}
          />
          <Button
            size="sm"
            variant="ghost"
            icon={X}
            label="Remove"
            title="Remove from history"
            onclick={() => remove(v)}
          />
        </div>
      </li>
    {/snippet}

    {#if pinnedVisits.length}
      <section class="day">
        <h2>Pinned</h2>
        <ul role="list">
          {#each pinnedVisits as v (v.jobId)}
            {@render row(v)}
          {/each}
        </ul>
      </section>
    {/if}

    {#each groups as g (g.label)}
      <section class="day">
        <h2>{g.label}</h2>
        <ul role="list">
          {#each g.visits as v (v.jobId)}
            {@render row(v)}
          {/each}
        </ul>
      </section>
    {/each}
  </div>

  <aside>
    {#if mostPlayed.length}
      <Card title="Most played" subtitle="From the visits kept here">
        <ol class="most">
          {#each mostPlayed as g, i (g.key)}
            <li>
              <span class="rank">{i + 1}</span>
              <GameIcon url={g.iconUrl} size={34} />
              <div class="most-info">
                <span class="most-name" title={g.name ?? ''}>{g.name ?? `Place ${g.placeId}`}</span>
                <span class="faint">{duration(g.totalMs)} · {g.visits} visit{g.visits === 1 ? '' : 's'}</span>
              </div>
              <Button
                size="sm"
                variant="ghost"
                icon={Play}
                label="Play"
                title="Play this game"
                disabled={busy}
                onclick={() => launch(String(g.placeId))}
              />
            </li>
          {/each}
        </ol>
        <p class="note"><Trophy size={12} aria-hidden="true" /> Time counts from joining a server to leaving it.</p>
      </Card>
    {/if}
    <Card title="History">
      <Field label="Keep a history" hint="Saved on this PC only. Nothing is uploaded.">
        <Toggle
          label="Keep a history"
          checked={games.history}
          onchange={(v) => app.saveSettings({ games: { history: v } })}
        />
      </Field>
      <Field
        label="Show server locations"
        hint="Asks RoValra's public database which city a server is in. Only the server's address is sent."
      >
        <Toggle
          label="Show server locations"
          checked={games.locateServers}
          onchange={(v) => app.saveSettings({ games: { locateServers: v } })}
        />
      </Field>
      <p class="note">
        Servers close when everyone leaves, so an old “Join server” link may land you in a new server instead. Joining
        from here goes through Vesper, so your profile and customisations apply.
      </p>
    </Card>
  </aside>
</div>

{#if profileTarget}
  <GameProfile
    bind:open={profileOpen}
    placeId={profileTarget.placeId}
    universeId={profileTarget.universeId}
    name={profileTarget.name}
  />
{/if}

<Dialog
  bind:open={confirmClear}
  title="Clear recent games?"
  description="This removes all {app.history.visits.length} entries, pinned ones too. It can't be undone."
>
  {#snippet actions()}
    <Button variant="ghost" onclick={() => (confirmClear = false)}>Keep them</Button>
    <Button variant="danger" icon={Trash} onclick={clear}>Clear history</Button>
  {/snippet}
</Dialog>

<style>
  .head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: var(--space-4);
    margin-bottom: var(--space-5);
  }
  .tools {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 12px;
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text-3);
  }
  .search:focus-within {
    border-color: var(--border-strong);
    box-shadow: var(--focus);
  }
  .search input {
    width: 180px;
    border: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    outline: none;
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
  .current {
    margin-bottom: var(--space-5);
  }
  .layout {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 300px;
    gap: var(--space-5);
    align-items: start;
  }
  @media (max-width: 1080px) {
    .layout {
      grid-template-columns: 1fr;
    }
  }
  aside {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }
  .day + .day {
    margin-top: var(--space-5);
  }
  h2 {
    margin: 0 0 var(--space-2);
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text-2);
  }
  ul {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    margin: 0;
    padding: 0;
  }
  .visit {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3) var(--space-4);
    padding: var(--space-3) var(--space-4);
    border-radius: var(--radius-lg);
  }
  .visit.pinned {
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .info {
    flex: 1 1 240px;
    min-width: 0;
  }
  .title {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .name {
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-top: 3px;
    font-size: var(--text-sm);
    color: var(--text-3);
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 2px;
    flex: none;
    margin-left: auto;
  }
  .most {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .most li {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .rank {
    width: 18px;
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--text-3);
    text-align: center;
  }
  .most-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    font-size: var(--text-sm);
  }
  .most-name {
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .faint {
    color: var(--text-3);
    font-size: var(--text-xs);
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-6) var(--space-4);
    text-align: center;
    color: var(--text-3);
  }
  .empty h3 {
    margin: var(--space-2) 0 0;
    color: var(--text);
  }
  .empty p {
    max-width: 420px;
    margin: 0 0 var(--space-3);
  }
  .note {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: var(--space-3) 0 0;
    font-size: var(--text-sm);
    color: var(--text-3);
  }
</style>
