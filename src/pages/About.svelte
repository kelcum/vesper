<script lang="ts">
  import { ExternalLink, Globe, MessageCircle, ShieldCheck } from '@lucide/svelte';
  import iconUrl from '../../assets/branding/icon.svg?url';
  import Button from '../components/ui/Button.svelte';
  import Card from '../components/ui/Card.svelte';
  import { api } from '../lib/api';
  import { computer, osName } from '../lib/platform';
  import { app } from '../lib/store.svelte';

  const SITE = 'https://xan.gripe/vesper';
  const CONTACT = 'https://xan.gripe/k3/#contact';

  const credits = [
    { name: 'Tauri', what: 'app framework', license: 'MIT / Apache-2.0', url: 'https://tauri.app' },
    { name: 'Svelte', what: 'interface', license: 'MIT', url: 'https://svelte.dev' },
    {
      name: 'Geist and Geist Mono',
      what: 'typefaces by Vercel',
      license: 'SIL OFL 1.1',
      url: 'https://github.com/vercel/geist-font',
    },
    { name: 'Lucide', what: 'icons', license: 'ISC', url: 'https://lucide.dev' },
    {
      name: 'Rust libraries',
      what: 'image, reqwest, notify, windows-rs and more',
      license: 'MIT / Apache-2.0',
      url: 'https://crates.io',
    },
  ];
</script>

<section class="hero glass">
  <img src={iconUrl} alt="Vesper logo" width="96" height="96" />
  <div>
    <h1>Vesper</h1>
    <p class="tag">Roblox, tuned and verified.</p>
    <p class="muted">Version {app.boot?.version}{app.boot?.portable ? ' · portable' : ''}</p>
  </div>
  <div class="links">
    <Button icon={Globe} onclick={() => api.openUrl(SITE)}>Website</Button>
    <Button variant="ghost" icon={MessageCircle} onclick={() => api.openUrl(CONTACT)}>Contact</Button>
  </div>
</section>

<div class="grid">
  <Card title="What Vesper is">
    <p class="muted">
      A launcher for Roblox on Windows and macOS. It applies the Fast Flags Roblox still accepts, sets Roblox's own
      frame-rate limit, lets you customise the cursor, emote wheel, font and sounds, and shows real performance numbers.
      After each launch it reads Roblox's log to confirm what Roblox actually accepted.
    </p>
    <p class="muted">
      Vesper starts the official Roblox client. It doesn't modify Roblox's code, inject anything into the game, or touch
      its anti-cheat.
    </p>
  </Card>

  <Card title="Privacy" icon={ShieldCheck}>
    <ul class="muted list">
      <li>No telemetry, analytics or accounts.</li>
      <li>Never reads Roblox cookies, tokens or passwords.</li>
      <li>
        Talks to <span class="mono">clientsettingscdn.roblox.com</span> (Roblox's public version and flag values),
        Roblox's public game APIs (game names and icons) and <span class="mono">xan.gripe</span> (update checks). All anonymous.
      </li>
      <li>
        Only for features you turn on: RoValra's public server list (server picking and server cities) and cloud
        endpoints for the ping test.
      </li>
      <li>Discord Rich Presence goes to the Discord app on your {computer}, and only when you turn it on.</li>
      <li>Logs and diagnostics remove your {osName} user name before they're written.</li>
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
      Flag descriptions cite Roblox's DevForum announcement and the public presets of Bloxstrap and Fishstrap (both
      MIT). No code from either project is included.
    </p>
  </Card>

  <Card title="License">
    <p class="muted">Vesper is released under the MIT License. Copyright © 2026 The Vesper Authors.</p>
    <p class="faint small">
      Vesper isn't affiliated with, endorsed by, or connected to Roblox Corporation. Roblox is a trademark of Roblox
      Corporation.
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
    background: radial-gradient(120% 140% at 0% 0%, rgba(149, 132, 255, 0.18), transparent 55%), var(--surface);
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
