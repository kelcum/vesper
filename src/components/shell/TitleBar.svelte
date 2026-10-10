<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { onMount } from 'svelte';

  const win = getCurrentWindow();
  let maximized = $state(false);

  onMount(() => {
    win.isMaximized().then((m) => (maximized = m));
    const un = win.onResized(async () => (maximized = await win.isMaximized()));
    return () => {
      un.then((f) => f());
    };
  });
</script>

<div class="bar" data-tauri-drag-region>
  <div class="spacer" data-tauri-drag-region></div>
  <div class="controls">
    <button class="ctl" aria-label="Minimize" title="Minimize" onclick={() => win.minimize()}>
      <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1 5h8" /></svg>
    </button>
    <button
      class="ctl"
      aria-label={maximized ? 'Restore' : 'Maximize'}
      title={maximized ? 'Restore' : 'Maximize'}
      onclick={() => win.toggleMaximize()}
    >
      {#if maximized}
        <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M3 1h6v6M1 3h6v6H1z" /></svg>
      {:else}
        <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1 1h8v8H1z" /></svg>
      {/if}
    </button>
    <button class="ctl close" aria-label="Close" title="Close" onclick={() => win.close()}>
      <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1 1l8 8M9 1l-8 8" /></svg>
    </button>
  </div>
</div>

<style>
  .bar {
    display: flex;
    align-items: stretch;
    height: 38px;
    flex: none;
    user-select: none;
  }
  .spacer {
    flex: 1;
  }
  .controls {
    display: flex;
  }
  .ctl {
    width: 46px;
    border: 0;
    background: transparent;
    color: var(--text-2);
    display: grid;
    place-items: center;
    cursor: default;
    transition: background var(--dur-fast) var(--ease-out);
  }
  .ctl:hover {
    background: var(--surface-2);
    color: var(--text);
  }
  .ctl.close:hover {
    background: #c42b3a;
    color: #fff;
  }
  .ctl:focus-visible {
    border-radius: 0;
  }
  svg {
    width: 10px;
    height: 10px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1;
    shape-rendering: crispEdges;
  }
</style>
