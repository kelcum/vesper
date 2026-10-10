<script lang="ts">
  import { ArrowLeft, ArrowRight, CircleCheck, FolderOpen, Play, RefreshCw, TriangleAlert } from '@lucide/svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import { fly } from 'svelte/transition';
  import iconUrl from '../../assets/branding/icon.svg?url';
  import TitleBar from '../components/shell/TitleBar.svelte';
  import Badge from '../components/ui/Badge.svelte';
  import Button from '../components/ui/Button.svelte';
  import Segmented from '../components/ui/Segmented.svelte';
  import Toggle from '../components/ui/Toggle.svelte';
  import { api } from '../lib/api';
  import { applyAppearance } from '../lib/appearance';
  import { fpsCapShort, sourceLabel } from '../lib/format';
  import { app } from '../lib/store.svelte';
  import type { Appearance } from '../lib/types';

  const steps = ['Welcome', 'Roblox', 'Appearance', 'Profile', 'Discord', 'Done'];
  let step = $state(0);
  let dir = $state(1);
  let detecting = $state(false);
  let appearance = $state<Appearance>({ ...app.settings.appearance });
  let profile = $state(app.profiles.default);
  let presence = $state(!!app.boot?.discordConfigured);
  let finishing = $state(false);

  const builtins = $derived(app.profiles.profiles.filter((p) => p.builtin && p.builtin !== 'custom'));
  const install = $derived(app.install);

  function go(n: number) {
    dir = n > step ? 1 : -1;
    step = Math.max(0, Math.min(steps.length - 1, n));
  }

  async function detect() {
    detecting = true;
    const d = await app.attempt(() => api.robloxDetect());
    if (d) app.detection = d;
    detecting = false;
  }

  async function chooseFolder() {
    const d = await open({ directory: true, multiple: false });
    if (typeof d === 'string') {
      await app.saveSettings({ roblox: { installDir: d } });
      await detect();
    }
  }

  function preview(patch: Partial<Appearance>) {
    appearance = { ...appearance, ...patch };
    applyAppearance(appearance);
  }

  async function finish(launch: boolean) {
    finishing = true;
    const b = await app.attempt(() =>
      api.firstRunFinish({ appearance, profile, richPresence: presence && !!app.boot?.discordConfigured }),
    );
    finishing = false;
    if (!b) return;
    app.applyBoot(b);
    if (launch) app.attempt(() => api.robloxLaunch());
  }
</script>

<div class="first">
  <TitleBar />
  <div class="body">
    <ol class="dots" aria-label="Setup progress">
      {#each steps as s, i (s)}
        <li class:done={i < step} class:current={i === step} aria-current={i === step ? 'step' : undefined}>
          <span class="sr-only">{s}</span>
        </li>
      {/each}
    </ol>

    {#key step}
      <section class="panel glass" in:fly={{ x: 24 * dir, duration: 240 }}>
        {#if step === 0}
          <img class="logo" src={iconUrl} alt="" width="84" height="84" />
          <h1>Welcome to Vesper</h1>
          <p class="lead">
            A calm launcher for Roblox: the Fast Flags Roblox still accepts, a frame-rate limit, customisation and
            honest performance numbers.
          </p>
          <p class="muted small">No accounts, no telemetry. Vesper never touches your Roblox login.</p>
        {:else if step === 1}
          <h2>Finding Roblox</h2>
          {#if install}
            <div class="found">
              <CircleCheck size={22} aria-hidden="true" class="ok" />
              <div>
                <div class="strong">Roblox {install.version ?? ''}</div>
                <div class="muted small">{sourceLabel(install.source)} · {install.location}</div>
              </div>
            </div>
            {#if app.detection.installs.length > 1}<p class="muted small">
                {app.detection.installs.length - 1} other copies were found; Vesper uses the one Roblox registered.
              </p>{/if}
          {:else}
            <div class="found">
              <TriangleAlert size={22} aria-hidden="true" class="warn" />
              <div>
                <div class="strong">Roblox isn't installed for this account</div>
                <div class="muted small">
                  {app.detection.storeAppPresent
                    ? "Only the Microsoft Store edition is installed, and launchers can't use it."
                    : 'Install Roblox, open it once, then detect again.'}
                </div>
              </div>
            </div>
            <div class="row">
              <Button onclick={() => api.openUrl('https://www.roblox.com/download')}>Get Roblox</Button>
            </div>
          {/if}
          <div class="row">
            <Button size="sm" variant="ghost" icon={RefreshCw} loading={detecting} onclick={detect}>Detect again</Button
            >
            <Button size="sm" variant="ghost" icon={FolderOpen} onclick={chooseFolder}>It's somewhere else…</Button>
          </div>
        {:else if step === 2}
          <h2>How should Vesper look?</h2>
          <div class="opts">
            <span class="ol">Theme</span>
            <Segmented
              label="Theme"
              value={appearance.theme}
              options={[
                { value: 'dark', label: 'Dark' },
                { value: 'light', label: 'Light' },
                { value: 'system', label: 'System' },
              ]}
              onchange={(v) => preview({ theme: v })}
            />
            <span class="ol">Transparency</span>
            <Segmented
              label="Transparency"
              value={appearance.transparency}
              options={[
                { value: 'full', label: 'Glass' },
                { value: 'reduced', label: 'Subtle' },
                { value: 'off', label: 'Solid' },
              ]}
              onchange={(v) => preview({ transparency: v })}
            />
            <span class="ol">Animations</span>
            <Segmented
              label="Animations"
              value={appearance.motion}
              options={[
                { value: 'system', label: 'System' },
                { value: 'full', label: 'Full' },
                { value: 'reduced', label: 'Reduced' },
              ]}
              onchange={(v) => preview({ motion: v })}
            />
          </div>
          <p class="muted small">You can change these, add a background image and adjust the interface size later.</p>
        {:else if step === 3}
          <h2>Pick a starting profile</h2>
          <p class="muted small">
            Profiles change Fast Flags, the frame-rate limit and process priority. Switch any time.
          </p>
          <div class="profiles" role="radiogroup" aria-label="Starting profile">
            {#each builtins as p (p.id)}
              <button
                class="pcard"
                class:sel={profile === p.id}
                role="radio"
                aria-checked={profile === p.id}
                onclick={() => (profile = p.id)}
              >
                <span class="pn">{p.name}</span>
                <span class="pd">{p.description}</span>
                <Badge>{fpsCapShort(p.fpsCap, app.boot?.system.displayHz)}</Badge>
              </button>
            {/each}
          </div>
        {:else if step === 4}
          <h2>Discord Rich Presence</h2>
          <p class="lead">
            Show friends that you're using Vesper, with a button to get it. Nothing else about you or your games is
            shared.
          </p>
          <div class="row toggle-row">
            <Toggle
              label="Discord Rich Presence"
              checked={presence}
              disabled={!app.boot?.discordConfigured}
              onchange={(v) => (presence = v)}
            />
            <span>{presence ? 'On' : 'Off'}</span>
          </div>
          {#if !app.boot?.discordConfigured}<p class="muted small">
              This build has no Discord application ID, so Rich Presence is unavailable.
            </p>{/if}
        {:else}
          <img class="logo" src={iconUrl} alt="" width="72" height="72" />
          <h2>You're set</h2>
          <p class="lead">
            Vesper will apply the {app.profiles.profiles.find((p) => p.id === profile)?.name} profile each time it launches
            Roblox, then check Roblox's log to confirm what it accepted.
          </p>
          <div class="row">
            <Button
              variant="primary"
              size="lg"
              icon={Play}
              loading={finishing}
              disabled={!install}
              onclick={() => finish(true)}>Launch Roblox</Button
            >
            <Button size="lg" loading={finishing} onclick={() => finish(false)}>Open Vesper</Button>
          </div>
        {/if}
      </section>
    {/key}

    {#if step < steps.length - 1}
      <div class="nav">
        {#if step > 0}<Button variant="ghost" icon={ArrowLeft} onclick={() => go(step - 1)}>Back</Button>{:else}<span
          ></span>{/if}
        <Button variant="primary" onclick={() => go(step + 1)}>
          {step === 0 ? 'Get started' : 'Continue'}
          <ArrowRight size={15} aria-hidden="true" />
        </Button>
      </div>
    {/if}
  </div>
</div>

<style>
  .first {
    height: 100%;
    display: flex;
    flex-direction: column;
  }
  .body {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-5);
    padding: 0 var(--space-6) var(--space-10);
  }
  .dots {
    display: flex;
    gap: 8px;
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .dots li {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--surface-3);
    transition: all var(--dur) var(--ease-out);
  }
  .dots li.done {
    background: var(--accent);
  }
  .dots li.current {
    width: 24px;
    border-radius: 4px;
    background: var(--accent-gradient);
  }
  .panel {
    width: min(620px, 100%);
    min-height: 340px;
    padding: var(--space-8);
    border-radius: var(--radius-xl);
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    justify-content: center;
  }
  .logo {
    border-radius: 20px;
    box-shadow: 0 16px 40px -16px rgba(123, 105, 240, 0.8);
  }
  h1 {
    font-size: var(--text-3xl);
  }
  .lead {
    font-size: var(--text-lg);
    color: var(--text-2);
  }
  .found {
    display: flex;
    gap: var(--space-3);
    align-items: center;
    padding: var(--space-4);
    border-radius: var(--radius-md);
    background: var(--surface);
    border: 1px solid var(--border);
  }
  .found :global(.ok) {
    color: var(--ok);
  }
  .found :global(.warn) {
    color: var(--warn);
  }
  .strong {
    font-weight: 600;
  }
  .opts {
    display: grid;
    grid-template-columns: 120px auto;
    gap: var(--space-3);
    align-items: center;
    justify-content: start;
  }
  .ol {
    color: var(--text-2);
  }
  .profiles {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-3);
  }
  .pcard {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    padding: var(--space-4);
    text-align: left;
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    background: var(--surface);
    cursor: pointer;
    transition: all var(--dur-fast) var(--ease-out);
  }
  .pcard:hover {
    border-color: var(--border-strong);
  }
  .pcard.sel {
    border-color: color-mix(in srgb, var(--accent) 60%, transparent);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }
  .pn {
    font-weight: 600;
  }
  .pd {
    color: var(--text-2);
    font-size: var(--text-xs);
    line-height: 1.45;
  }
  .toggle-row {
    gap: var(--space-3);
  }
  .nav {
    width: min(620px, 100%);
    display: flex;
    justify-content: space-between;
  }
</style>
