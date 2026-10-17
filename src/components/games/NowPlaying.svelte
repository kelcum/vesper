<script lang="ts">
  import { Copy, ExternalLink, MapPin, RotateCw } from '@lucide/svelte';
  import { onDestroy } from 'svelte';
  import Badge from '../ui/Badge.svelte';
  import Button from '../ui/Button.svelte';
  import GameIcon from './GameIcon.svelte';
  import { api } from '../../lib/api';
  import { duration, serverKindLabel } from '../../lib/format';
  import { app } from '../../lib/store.svelte';
  import type { NowPlaying } from '../../lib/types';

  interface Props {
    now: NowPlaying;
  }
  let { now }: Props = $props();

  let tick = $state(Date.now());
  const timer = setInterval(() => (tick = Date.now()), 15000);
  onDestroy(() => clearInterval(timer));

  const played = $derived(duration(tick - new Date(now.joinedAt).getTime()));
  const kind = $derived(serverKindLabel(now.kind));
  const busy = $derived(['preparing', 'starting', 'updating'].includes(app.launch.state));

  async function copyLink() {
    if (!now.joinUrl) return;
    try {
      await navigator.clipboard.writeText(now.joinUrl);
      app.toast('Server link copied. Anyone with Roblox can open it to join you.', 'ok');
    } catch {
      app.toast("Couldn't copy the link.", 'warn');
    }
  }
</script>

<div class="now">
  <GameIcon url={now.game?.iconUrl} size={64} alt="" />
  <div class="info">
    <p class="eyebrow">Now playing</p>
    <h3 class="name" title={now.game?.name}>{now.game?.name ?? `Place ${now.placeId}`}</h3>
    <div class="meta">
      {#if now.game}<span class="faint">by {now.game.creator}</span>{/if}
      <span class="faint">{played}</span>
      {#if now.location}
        <Badge title="Where this server is"><MapPin size={11} aria-hidden="true" /> {now.location}</Badge>
      {/if}
      {#if kind}<Badge tone="info">{kind}</Badge>{/if}
    </div>
  </div>
  <div class="actions">
    {#if now.joinUrl}
      <Button size="sm" icon={Copy} onclick={copyLink} title="Copy a link that joins this exact server"
        >Copy join link</Button
      >
    {/if}
    <Button size="sm" variant="ghost" icon={ExternalLink} onclick={() => api.openUrl(now.pageUrl)}>Game page</Button>
    <Button
      size="sm"
      variant="ghost"
      icon={RotateCw}
      disabled={busy}
      title={now.joinUrl
        ? 'Close Roblox and open it again in this same server'
        : 'Close Roblox and open this game again (this server can only be joined by invite)'}
      onclick={() => app.attempt(() => api.robloxRestart(null, true))}>{now.joinUrl ? 'Rejoin' : 'Restart'}</Button
    >
  </div>
</div>

<style>
  .now {
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: var(--space-4);
    border-radius: var(--radius-lg);
    border: 1px solid var(--border);
    background: color-mix(in srgb, var(--surface-2) 70%, transparent);
  }
  .info {
    flex: 1;
    min-width: 0;
  }
  .eyebrow {
    margin: 0;
    font-size: var(--text-xs);
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--accent-strong);
  }
  .name {
    margin: 2px 0 4px;
    font-size: var(--text-lg);
    font-weight: 650;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    font-size: var(--text-sm);
  }
  .faint {
    color: var(--text-3);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: var(--space-2);
    max-width: 46%;
  }
</style>
