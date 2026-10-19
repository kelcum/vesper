<script lang="ts">
  import { Activity, Flag, History, House, Info, Layers, Palette, Settings } from '@lucide/svelte';
  import iconUrl from '../../../assets/branding/icon.svg?url';
  import { app, type PageId } from '../../lib/store.svelte';

  const nav: { id: PageId; label: string; icon: typeof House }[] = [
    { id: 'home', label: 'Home', icon: House },
    { id: 'recent', label: 'Recent', icon: History },
    { id: 'profiles', label: 'Profiles', icon: Layers },
    { id: 'flags', label: 'Fast Flags', icon: Flag },
    { id: 'customize', label: 'Customize', icon: Palette },
    { id: 'performance', label: 'Performance', icon: Activity },
    { id: 'settings', label: 'Settings', icon: Settings },
    { id: 'about', label: 'About', icon: Info },
  ];

  const status = $derived.by(() => {
    if (app.robloxRunning) return { tone: 'ok', label: 'Roblox running' };
    if (!app.install) return { tone: 'bad', label: 'Roblox not found' };
    return { tone: 'idle', label: 'Roblox ready' };
  });

  function onkeydown(e: KeyboardEvent) {
    const i = nav.findIndex((n) => n.id === app.page);
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      app.go(nav[(i + 1) % nav.length].id);
      focusActive();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      app.go(nav[(i - 1 + nav.length) % nav.length].id);
      focusActive();
    }
  }

  function focusActive() {
    queueMicrotask(() => document.querySelector<HTMLElement>('.nav [aria-current="page"]')?.focus());
  }
</script>

<aside class="sidebar">
  <div class="brand" data-tauri-drag-region>
    <img src={iconUrl} alt="" width="28" height="28" />
    <span class="word">vesper</span>
  </div>

  <nav class="nav" aria-label="Main">
    <ul role="list">
      {#each nav as item (item.id)}
        {@const Icon = item.icon}
        <li>
          <button
            class:active={app.page === item.id}
            aria-current={app.page === item.id ? 'page' : undefined}
            title={item.label}
            {onkeydown}
            onclick={() => app.go(item.id)}
          >
            <Icon size={17} strokeWidth={1.8} aria-hidden="true" />
            <span class="label">{item.label}</span>
          </button>
        </li>
      {/each}
    </ul>
  </nav>

  <div class="foot">
    <div class="status {status.tone}" title={app.install?.version ? `Roblox ${app.install.version}` : status.label}>
      <span class="dot" aria-hidden="true"></span>
      <span class="label">{status.label}</span>
    </div>
    {#if app.activeProfile}
      <button class="profile" title="Active profile" onclick={() => app.go('profiles')}>
        <span class="label">{app.activeProfile.name}</span>
      </button>
    {/if}
  </div>
</aside>

<style>
  .sidebar {
    width: 216px;
    flex: none;
    display: flex;
    flex-direction: column;
    padding: 10px 12px 14px;
    border-right: 1px solid var(--border);
    background: color-mix(in srgb, var(--surface) 60%, transparent);
    transition: width var(--dur-slow) var(--ease-out);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 46px;
    padding: 0 8px;
    margin-bottom: 10px;
  }
  :global(:root[data-platform='macos']) .brand {
    height: 74px;
    padding-top: 28px;
  }
  .brand img {
    border-radius: 8px;
    box-shadow: 0 4px 14px -6px rgba(123, 105, 240, 0.7);
  }
  .word {
    font-weight: 650;
    font-size: 18px;
    letter-spacing: -0.03em;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .nav button {
    position: relative;
    width: 100%;
    display: flex;
    align-items: center;
    gap: 11px;
    height: 36px;
    padding: 0 10px;
    border: 0;
    border-radius: 10px;
    background: transparent;
    color: var(--text-2);
    font-weight: 520;
    cursor: pointer;
    transition:
      background var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out);
  }
  .nav button:hover {
    background: var(--surface-2);
    color: var(--text);
  }
  .nav button.active {
    background: var(--surface-3);
    color: var(--text);
    box-shadow: inset 0 1px 0 var(--highlight);
  }
  .nav button.active::before {
    content: '';
    position: absolute;
    left: -12px;
    top: 9px;
    bottom: 9px;
    width: 3px;
    border-radius: 0 3px 3px 0;
    background: var(--accent-gradient);
  }
  .nav button.active :global(svg) {
    color: var(--accent-strong);
  }
  .foot {
    margin-top: auto;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 0 6px;
  }
  .status {
    display: flex;
    align-items: center;
    gap: 9px;
    font-size: var(--text-sm);
    color: var(--text-2);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex: none;
    background: var(--text-3);
  }
  .ok .dot {
    background: var(--ok);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--ok) 22%, transparent);
  }
  .bad .dot {
    background: var(--bad);
  }
  .profile {
    border: 0;
    background: transparent;
    padding: 0 0 0 17px;
    text-align: left;
    font-size: var(--text-xs);
    color: var(--text-3);
    cursor: pointer;
  }
  .profile:hover {
    color: var(--text);
  }

  @media (max-width: 1060px) {
    .sidebar {
      width: 66px;
    }
    .label,
    .word {
      display: none;
    }
    .nav button {
      justify-content: center;
      padding: 0;
    }
    .status {
      justify-content: center;
    }
  }
</style>
