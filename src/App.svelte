<script lang="ts">
  import { fly } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import Sidebar from './components/shell/Sidebar.svelte';
  import TitleBar from './components/shell/TitleBar.svelte';
  import Button from './components/ui/Button.svelte';
  import Dialog from './components/ui/Dialog.svelte';
  import ProblemView from './components/ui/ProblemView.svelte';
  import Toasts from './components/ui/Toasts.svelte';
  import { api } from './lib/api';
  import { isMac, trayName } from './lib/platform';
  import { app } from './lib/store.svelte';
  import About from './pages/About.svelte';
  import Recent from './pages/Recent.svelte';
  import Customize from './pages/Customize.svelte';
  import FastFlags from './pages/FastFlags.svelte';
  import FirstRun from './pages/FirstRun.svelte';
  import Home from './pages/Home.svelte';
  import Performance from './pages/Performance.svelte';
  import Profiles from './pages/Profiles.svelte';
  import Settings from './pages/Settings.svelte';

  const pages = {
    home: Home,
    recent: Recent,
    profiles: Profiles,
    flags: FastFlags,
    customize: Customize,
    performance: Performance,
    settings: Settings,
    about: About,
  };

  let problemOpen = $state(false);
  $effect(() => {
    problemOpen = app.problem !== null;
  });

  let rememberTray = $state(true);

  async function keepInTray() {
    app.closePrompt = false;
    if (rememberTray) await app.saveSettings({ general: { trayHintShown: true } });
    await api.appHide();
  }

  async function quitInstead() {
    app.closePrompt = false;
    await app.saveSettings({ general: { closeBehavior: 'quit', trayHintShown: true } });
    await api.appQuit();
  }

  // Ctrl+1…7 (⌘1…7 on macOS) jumps between pages.
  function onkeydown(e: KeyboardEvent) {
    if (!(isMac ? e.metaKey : e.ctrlKey) || e.altKey) return;
    const order = Object.keys(pages) as (keyof typeof pages)[];
    const n = Number(e.key);
    if (n >= 1 && n <= order.length) {
      e.preventDefault();
      app.go(order[n - 1]);
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="backdrop" aria-hidden="true"></div>

{#if !app.ready}
  <div class="splash" aria-busy="true" aria-label="Loading Vesper"></div>
{:else if app.bootError}
  <div class="fatal">
    <TitleBar />
    <div class="fatal-body glass">
      <ProblemView problem={app.bootError} onretry={() => location.reload()} />
    </div>
  </div>
{:else if !app.settings.firstRunComplete}
  <FirstRun />
{:else}
  <div class="shell">
    <Sidebar />
    <main>
      <TitleBar />
      <div class="scroll">
        {#key app.page}
          {@const Page = pages[app.page]}
          <div class="page" in:fly={{ y: 8, duration: 220, easing: cubicOut }}>
            <Page />
          </div>
        {/key}
      </div>
    </main>
  </div>
{/if}

<Toasts />

<Dialog bind:open={problemOpen} title="Something went wrong" width={520} onclose={() => (app.problem = null)}>
  {#if app.problem}
    <ProblemView problem={app.problem} />
  {/if}
  {#snippet actions()}
    <Button variant="primary" onclick={() => (app.problem = null)}>Close</Button>
  {/snippet}
</Dialog>

<Dialog
  bind:open={app.closePrompt}
  title="Vesper keeps running in the {trayName}"
  description={isMac
    ? 'Closing the window keeps Vesper in the menu bar, so Discord Rich Presence and kept-applied flags keep working. Quit from the menu bar icon or with ⌘Q.'
    : 'Closing the window keeps Vesper in the notification area, so Discord Rich Presence and kept-applied flags keep working. Right-click the tray icon to quit.'}
>
  <label class="remember">
    <input type="checkbox" bind:checked={rememberTray} />
    Don't show this again
  </label>
  {#snippet actions()}
    <Button variant="ghost" onclick={quitInstead}>Quit when I close</Button>
    <Button variant="primary" onclick={keepInTray}>Keep running in {trayName}</Button>
  {/snippet}
</Dialog>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: -1;
    background: var(--bg-gradient);
  }
  /* Glass: Windows' Mica or macOS's window material shows through faintly; the dusk tint stays in charge. */
  :global(:root[data-transparency='full']) .backdrop {
    background:
      radial-gradient(120% 90% at 85% -10%, rgba(123, 105, 240, 0.2), transparent 55%),
      radial-gradient(90% 70% at -10% 110%, rgba(93, 125, 255, 0.12), transparent 60%),
      color-mix(in srgb, var(--bg) 82%, transparent);
  }
  :global(:root[data-custom-bg='on']) .backdrop {
    background: var(--custom-bg) center / cover no-repeat;
    filter: blur(var(--bg-blur));
    transform: scale(1.06);
  }
  :global(:root[data-custom-bg='on']) .backdrop::after {
    content: '';
    position: absolute;
    inset: 0;
    background: color-mix(in srgb, var(--bg) calc(var(--bg-dim) * 100%), transparent);
  }
  .splash {
    height: 100%;
  }
  .shell {
    display: flex;
    height: 100%;
  }
  main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .scroll {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    scrollbar-gutter: stable;
  }
  .page {
    max-width: 1180px;
    margin: 0 auto;
    padding: 4px var(--space-8) var(--space-10);
  }
  .fatal {
    height: 100%;
    display: flex;
    flex-direction: column;
  }
  .fatal-body {
    margin: auto;
    max-width: 560px;
    padding: var(--space-6);
  }
  .remember {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-2);
    font-size: var(--text-sm);
  }
  .remember input {
    accent-color: var(--accent);
  }
  @media (max-width: 1060px) {
    .page {
      padding: 4px var(--space-5) var(--space-8);
    }
  }
</style>
