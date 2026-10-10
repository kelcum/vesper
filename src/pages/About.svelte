<script lang="ts">
  import { Bug, ExternalLink, Globe, ShieldCheck } from '@lucide/svelte';
  import iconUrl from '../../assets/branding/icon.svg?url';
  import Button from '../components/ui/Button.svelte';
  import Card from '../components/ui/Card.svelte';
  import { api } from '../lib/api';
  import { app } from '../lib/store.svelte';

  const SITE = 'https://xan.gripe/vesper';
  const REPO = 'https://github.com/kelcum/vesper';

  const credits = [
    { name: 'Tauri', what: 'app framework', license: 'MIT / Apache-2.0', url: 'https://tauri.app' },
    { name: 'Svelte', what: 'interface', license: 'MIT', url: 'https://svelte.dev' },
    { name: 'Geist and Geist Mono', what: 'typefaces by Vercel', license: 'SIL OFL 1.1', url: 'https://github.com/vercel/geist-font' },
    { name: 'Lucide', what: 'icons', license: 'ISC', url: 'https://lucide.dev' },
    { name: 'Rust libraries', what: 'image, reqwest, notify, windows-rs and more', license: 'MIT / Apache-2.0', url: 'https://crates.io' },
  ];
</script>

<section class="hero glass">
  <img src={iconUrl} alt="Vesper logo" width="96" height="96" />
  <div>
    <h1>vesper</h1>
    <p class="tag">Roblox, tuned and verified.</p>
    <p class="muted">Version {app.boot?.version}{app.boot?.portable ? ' · portable' : ''}</p>
  </div>
  <div class="links">
    <Button icon={Globe} onclick={() => api.openUrl(SITE)}>Website</Button>
    <Button onclick={() => api.openUrl(REPO)}>
      <span class="gh">
        <svg viewBox="0 0 16 16" width="15" height="15" aria-hidden="true"><path fill="currentColor" d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"/></svg>
        GitHub
      </span>
    </Button>
    <Button variant="ghost" icon={Bug} onclick={() => api.openUrl(`${REPO}/issues/new/choose`)}>Report a bug</Button>
  </div>
</section>

<div class="grid">
  <Card title="What Vesper is">
    <p class="muted">
      A launcher for Roblox on Windows. It applies the Fast Flags Roblox still accepts, sets Roblox's own frame-rate limit, lets you
      customise the cursor, emote wheel, font and sounds, and shows real performance numbers. After each launch it reads Roblox's log
      to confirm what Roblox actually accepted.
    </p>
    <p class="muted">
      Vesper starts the official Roblox client. It doesn't modify Roblox's code, inject anything into the game, or touch its anti-cheat.
    </p>
  </Card>

  <Card title="Privacy" icon={ShieldCheck}>
    <ul class="muted list">
      <li>No telemetry, analytics or accounts.</li>
      <li>Never reads Roblox cookies, tokens or passwords.</li>
      <li>Talks to <span class="mono">clientsettingscdn.roblox.com</span> (Roblox's public version and flag values) and <span class="mono">xan.gripe</span> (update checks). Both anonymous.</li>
      <li>Discord Rich Presence goes to the Discord app on your PC, and only when you turn it on.</li>
      <li>Logs and diagnostics remove your Windows user name before they're written.</li>
    </ul>
  </Card>

  <Card title="Credits">
    <ul class="credits">
      {#each credits as c (c.name)}
        <li>
          <button class="credit" onclick={() => api.openUrl(c.url)}>
            <span class="cn">{c.name}</span> <span class="faint">{c.what} · {c.license}</span>
            <ExternalLink size={12} aria-hidden="true" />
          </button>
        </li>
      {/each}
    </ul>
    <p class="faint small">
      Flag descriptions cite Roblox's DevForum announcement and the public presets of Bloxstrap and Fishstrap (both MIT). No code
      from either project is included.
    </p>
  </Card>

  <Card title="License">
    <p class="muted">Vesper is released under the MIT License. Copyright © 2026 The Vesper Authors.</p>
    <p class="faint small">
      Vesper isn't affiliated with, endorsed by, or connected to Roblox Corporation. Roblox is a trademark of Roblox Corporation.
    </p>
  </Card>
</div>

<style>
  .hero {
    display: flex;
    align-items: center;
    gap: var(--space-6);
    padding: var(--space-6);
    margin-top: var(--space-4);
    border-radius: var(--radius-xl);
    background:
      radial-gradient(120% 140% at 0% 0%, rgba(149, 132, 255, 0.18), transparent 55%),
      var(--surface);
  }
  .hero img {
    border-radius: 22px;
    box-shadow: 0 16px 40px -16px rgba(123, 105, 240, 0.8);
  }
  h1 {
    font-size: 40px;
    letter-spacing: -0.04em;
  }
  .tag {
    font-size: var(--text-lg);
    margin: 2px 0 6px;
  }
  .links {
    margin-left: auto;
    display: flex;
    gap: var(--space-2);
    flex-wrap: wrap;
  }
  .gh {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-4);
    margin-top: var(--space-4);
  }
  .grid :global(.card p + p) {
    margin-top: var(--space-3);
  }
  .list {
    margin: 0;
    padding-left: 18px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .credits {
    list-style: none;
    padding: 0;
    margin: 0 0 var(--space-3);
  }
  .credit {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 0;
    border: 0;
    background: none;
    color: var(--text);
    cursor: pointer;
    text-align: left;
  }
  .credit:hover .cn {
    text-decoration: underline;
  }
  .cn {
    font-weight: 550;
  }
  @media (max-width: 1060px) {
    .hero {
      flex-direction: column;
      align-items: flex-start;
    }
    .links {
      margin-left: 0;
    }
    .grid {
      grid-template-columns: 1fr;
    }
  }
</style>
