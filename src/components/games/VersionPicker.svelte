<script lang="ts">
  import { Download, RefreshCw, RotateCcw, TriangleAlert } from '@lucide/svelte';
  import Badge from '../ui/Badge.svelte';
  import Button from '../ui/Button.svelte';
  import Dialog from '../ui/Dialog.svelte';
  import ProblemView from '../ui/ProblemView.svelte';
  import { api, toProblem } from '../../lib/api';
  import { bytes, relativeTime } from '../../lib/format';
  import { app } from '../../lib/store.svelte';
  import type { Problem, RobloxBuild, RobloxBuildList } from '../../lib/types';

  interface Props {
    open: boolean;
  }
  let { open = $bindable() }: Props = $props();

  let list = $state<RobloxBuildList | null>(null);
  let loading = $state(false);
  let loadError = $state<Problem | null>(null);
  let lastTried = $state<RobloxBuild | null>(null);

  const status = $derived(app.builds);
  const working = $derived(status.state === 'working' ? status : null);
  const failed = $derived(status.state === 'failed' ? status.problem : null);
  const pinned = $derived(app.settings.roblox.pinnedBuild);
  const percent = $derived(working && working.total > 0 ? Math.round((working.done / working.total) * 100) : 0);

  async function load(refresh = false) {
    loading = true;
    loadError = null;
    try {
      list = await api.buildsList(refresh);
    } catch (err) {
      loadError = toProblem(err);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    if (open && !list && !loading && !loadError) load();
  });

  function back(n: number) {
    return n === 1 ? '1 version back' : `${n} versions back`;
  }

  async function choose(build: RobloxBuild) {
    lastTried = build;
    await app.attempt(() => api.buildsInstall(build.hash, build.version));
  }

  async function retry() {
    await api.buildsDismiss();
    if (lastTried) await choose(lastTried);
  }

  async function latest() {
    const s = await app.attempt(() => api.buildsUseLatest());
    if (s) {
      app.settings = s;
      app.toast('Back on your normal Roblox.', 'ok');
    }
  }
</script>

<Dialog
  bind:open
  title="Roblox version"
  description="Use an older Roblox from Vesper. Your normal Roblox install isn't changed, so you can go back any time."
  width={560}
>
  <p class="warning">
    <TriangleAlert size={15} aria-hidden="true" />
    <span>
      Roblox only lets recent versions join games. Older ones get error 280 or open to a blank window, and the cutoff
      moves up as Roblox updates. The closer to the latest, the better the odds.
    </span>
  </p>

  {#if working}
    <div class="progress" role="status">
      <div class="progress-head">
        <span>Getting Roblox <span class="mono">{working.version}</span></span>
        <span class="faint">{bytes(working.done)} of {working.total ? bytes(working.total) : '…'}</span>
      </div>
      <div class="bar"><div class="fill" style:width="{percent}%"></div></div>
      <Button size="sm" variant="ghost" onclick={() => api.buildsCancel()}>Cancel</Button>
    </div>
  {:else if failed}
    <ProblemView problem={failed} compact onretry={retry} />
  {:else if loadError}
    <ProblemView problem={loadError} compact onretry={() => load(true)} />
  {:else if loading && !list}
    <p class="faint center">Looking up older versions…</p>
  {:else if list}
    {#if pinned}
      <div class="row current">
        <div class="info">
          <span class="name">Your normal Roblox</span>
          <span class="faint small">{list.base ? `Roblox ${list.base}` : 'Updates itself as usual'}</span>
        </div>
        <Button size="sm" variant="primary" icon={RotateCcw} onclick={latest}>Back to latest</Button>
      </div>
    {/if}
    {#if list.builds.length}
      <ul role="list">
        {#each list.builds as b (b.hash)}
          <li class="row">
            <div class="info">
              <span class="name"
                ><span class="mono">{b.version}</span>
                {#if pinned === b.hash}<Badge tone="ok" dot>In use</Badge>{/if}</span
              >
              <span class="faint small"
                >{back(b.versionsBack)}{b.released ? ` · released ${relativeTime(b.released)}` : ''}</span
              >
            </div>
            {#if b.tooOld}
              <Badge tone="warn" title="Roblox refused this version or a newer one on this PC.">Too old</Badge>
            {:else if pinned !== b.hash}
              <Button size="sm" icon={Download} onclick={() => choose(b)}>Use this</Button>
            {/if}
          </li>
        {/each}
      </ul>
      <p class="faint small">
        Downloads about 230 MB from Roblox, and only applies when you launch from Vesper. Game links on the website keep
        opening your normal Roblox.
      </p>
    {:else}
      <p class="faint center">
        {list.partial ? "Vesper couldn't reach the list of older versions." : 'No older versions found right now.'}
      </p>
    {/if}
    {#if list.partial}
      <p class="faint small">Only Roblox's current version could be checked. Try again in a little while.</p>
    {/if}
  {/if}

  {#snippet actions()}
    {#if !working}
      <Button variant="ghost" icon={RefreshCw} {loading} onclick={() => load(true)}>Refresh</Button>
    {/if}
    <Button onclick={() => (open = false)}>Close</Button>
  {/snippet}
</Dialog>

<style>
  .warning {
    display: flex;
    gap: 9px;
    margin: 0 0 14px;
    padding: 10px 12px;
    border-radius: 10px;
    font-size: var(--text-sm);
    line-height: 1.45;
    color: var(--text-2);
    background: color-mix(in srgb, var(--warn) 10%, transparent);
    border: 1px solid color-mix(in srgb, var(--warn) 26%, transparent);
  }
  .warning :global(svg) {
    flex: none;
    margin-top: 2px;
    color: var(--warn);
  }
  ul {
    list-style: none;
    margin: 0 0 12px;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 290px;
    overflow-y: auto;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 9px 10px;
    border-radius: 10px;
  }
  .row:hover {
    background: var(--surface-2);
  }
  .current {
    margin-bottom: 8px;
    background: var(--surface-2);
  }
  .info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .name {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 560;
  }
  .mono {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
  }
  .faint {
    color: var(--text-3);
  }
  .small {
    font-size: var(--text-xs);
  }
  p.small {
    margin: 0 0 4px;
    line-height: 1.45;
  }
  .center {
    margin: 22px 0;
    text-align: center;
  }
  .progress {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 10px;
    padding: 6px 0;
  }
  .progress-head {
    width: 100%;
    display: flex;
    justify-content: space-between;
    gap: 12px;
    font-size: var(--text-sm);
  }
  .bar {
    width: 100%;
    height: 6px;
    border-radius: 3px;
    overflow: hidden;
    background: var(--surface-3);
  }
  .fill {
    height: 100%;
    border-radius: 3px;
    background: var(--accent-gradient);
    transition: width var(--dur-fast) var(--ease-out);
  }
</style>
