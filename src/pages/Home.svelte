<script lang="ts">
  import { ArrowRight, CircleCheck, FolderOpen, Play, RefreshCw, ShieldCheck, TriangleAlert, Download } from '@lucide/svelte';
  import { onDestroy, onMount } from 'svelte';
  import Badge from '../components/ui/Badge.svelte';
  import Button from '../components/ui/Button.svelte';
  import Card from '../components/ui/Card.svelte';
  import ProblemView from '../components/ui/ProblemView.svelte';
  import Select from '../components/ui/Select.svelte';
  import Sparkline from '../components/ui/Sparkline.svelte';
  import Toggle from '../components/ui/Toggle.svelte';
  import { api } from '../lib/api';
  import { bytes, compareVersions, fpsCapShort, minorVersion, percent, priorityLabel, relativeTime, sourceLabel } from '../lib/format';
  import { app } from '../lib/store.svelte';
  import type { LatestVersion, LaunchTarget } from '../lib/types';

  let target = $state('');
  let parsed = $state<LaunchTarget | null>(null);
  let parseError = $state<string | null>(null);
  let latest = $state<LatestVersion | null>(app.boot?.latestRoblox ?? null);

  onMount(() => {
    api.perfWatch('home', true);
    api.robloxLatest(false).then((l) => (latest = l)).catch(() => {});
  });
  onDestroy(() => {
    api.perfWatch('home', false);
  });

  // Validate the join field as the user types (debounced).
  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const value = target;
    clearTimeout(timer);
    if (!value.trim()) {
      parsed = null;
      parseError = null;
      return;
    }
    timer = setTimeout(async () => {
      try {
        parsed = await api.robloxParseTarget(value);
        parseError = null;
      } catch (e) {
        parsed = null;
        parseError = (e as { problem?: { what: string } }).problem?.what ?? 'Not a Roblox link.';
      }
    }, 220);
  });

  const busy = $derived(['preparing', 'starting', 'updating'].includes(app.launch.state));
  const install = $derived(app.install);
  const outdated = $derived(
    !!(install?.version && latest && compareVersions(latest.version, install.version) > 0),
  );
  const profile = $derived(app.activeProfile);
  const flagCount = $derived(profile ? Object.keys(profile.flags).length : 0);
  const report = $derived(app.lastReport);
  const perf = $derived(app.latestPerf);

  const launchLabel = $derived.by(() => {
    switch (app.launch.state) {
      case 'preparing':
        return app.launch.step + '…';
      case 'starting':
        return 'Starting Roblox…';
      case 'updating':
        return 'Roblox is updating…';
      default:
        return app.robloxRunning ? 'Launch another' : parsed?.kind === 'experience' ? 'Join game' : 'Launch Roblox';
    }
  });

  const targetHint = $derived.by(() => {
    if (parseError) return parseError;
    if (!parsed) return 'Optional: paste a game link or place ID to join it directly.';
    if (parsed.kind === 'shareLink') return 'Share links open in your browser, which hands them to Roblox.';
    if (parsed.kind === 'experience') {
      const extras = [parsed.linkCode && 'private server', parsed.gameInstanceId && 'specific server'].filter(Boolean);
      return `Place ${parsed.placeId}${extras.length ? ' · ' + extras.join(', ') : ''}`;
    }
    return 'Opens the Roblox app.';
  });

  async function launch() {
    if (parseError) return;
    await app.attempt(() => api.robloxLaunch(target));
  }

  function setProfile(id: string) {
    app.attempt(() => api.profilesSetActive(id)).then((s) => app.setProfiles(s));
  }

  const series = (pick: (s: NonNullable<typeof perf>) => number | null) => app.perf.map((s) => pick(s));
</script>

<header class="head">
  <div>
    <p class="eyebrow">Home</p>
    <h1>Good to see you.</h1>
  </div>
  {#if app.update.state === 'available'}
    <Button icon={Download} onclick={() => app.go('settings', 'updates')}>Vesper {app.update.version} is ready</Button>
  {/if}
</header>

<section class="hero glass">
  <div class="launch">
    <div class="launch-row">
      <Button variant="primary" size="lg" icon={Play} loading={busy} disabled={!install || !!parseError} onclick={launch}>
        {launchLabel}
      </Button>
      <div class="join">
        <input
          class="input"
          class:invalid={!!parseError}
          placeholder="Game link or place ID"
          aria-label="Game link or place ID"
          aria-describedby="target-hint"
          bind:value={target}
          onkeydown={(e) => e.key === 'Enter' && !busy && launch()}
          spellcheck="false"
        />
        <p id="target-hint" class="hint" class:bad={!!parseError}>{targetHint}</p>
      </div>
    </div>

    {#if app.launch.state === 'failed'}
      <div class="failure">
        <ProblemView problem={app.launch.problem} compact onretry={launch} />
        <Button size="sm" variant="ghost" onclick={() => api.robloxDismissLaunch()}>Dismiss</Button>
      </div>
    {:else if app.robloxRunning}
      <p class="state ok"><CircleCheck size={15} aria-hidden="true" /> Roblox is running. Flag and frame-rate changes apply the next time it starts.</p>
    {:else if install && !busy}
      <p class="state">
        Ready to start Roblox {install.version ?? ''} with the {profile?.name ?? ''} profile{flagCount ? `, applying ${flagCount} flag${flagCount === 1 ? '' : 's'}` : ''}.
      </p>
    {/if}
    {#if !install}
      <p class="state bad"><TriangleAlert size={15} aria-hidden="true" /> Roblox isn't installed for this Windows account. <button class="link" onclick={() => api.openUrl('https://www.roblox.com/download')}>Get Roblox</button></p>
    {/if}
  </div>

  <div class="profile">
    <p class="eyebrow">Profile</p>
    {#if profile}
      <Select
        label="Active profile"
        value={profile.id}
        options={app.profiles.profiles.map((p) => ({ value: p.id, label: p.name }))}
        onchange={setProfile}
      />
      <div class="chips">
        <Badge tone="accent">{fpsCapShort(profile.fpsCap, app.boot?.system.displayHz)}</Badge>
        <Badge>{flagCount} flag{flagCount === 1 ? '' : 's'}</Badge>
        {#if profile.priority !== 'normal'}<Badge>{priorityLabel(profile.priority)}</Badge>{/if}
      </div>
      <button class="link small" onclick={() => app.go('profiles')}>Edit profile <ArrowRight size={13} aria-hidden="true" /></button>
    {/if}
  </div>
</section>

<div class="grid">
  <Card title="Roblox" subtitle={install ? sourceLabel(install.source) : 'Not detected'}>
    {#snippet actions()}
      <Button size="sm" variant="ghost" icon={RefreshCw} label="Detect again" title="Detect again" onclick={() => api.robloxDetect().then((d) => (app.detection = d))} />
      {#if install}<Button size="sm" variant="ghost" icon={FolderOpen} label="Open Roblox folder" title="Open folder" onclick={() => api.openFolder('roblox')} />{/if}
    {/snippet}
    {#if install}
      <div class="kv">
        <span>Version</span><span class="mono">{install.version ?? 'unknown'}</span>
        <span>Status</span>
        <span>
          {#if !latest}<span class="faint">Not checked</span>
          {:else if outdated}<Badge tone="warn" title="Roblox updates itself when it next starts.">Update available · {minorVersion(latest.version)}</Badge>
          {:else}<Badge tone="ok">Up to date</Badge>{/if}
        </span>
        <span>Last launch</span>
        <span>{relativeTime(app.settings.lastLaunch?.at)}</span>
      </div>
      {#if app.detection.protocol && app.detection.protocol.owner !== 'Roblox' && app.detection.protocol.owner !== 'Vesper'}
        <p class="note">Game links from the website open in {app.detection.protocol.owner}. Launch from Vesper to use your profile.</p>
      {/if}
    {:else}
      <p class="muted">Install Roblox, open it once, then press Detect again.</p>
    {/if}
  </Card>

  <Card title="Last check" subtitle="What Roblox said about your flags">
    {#if report}
      <p class="big">
        {#if report.denied.length === 0}
          <ShieldCheck size={18} aria-hidden="true" class="okc" /> {report.accepted.length} accepted
        {:else}
          <TriangleAlert size={18} aria-hidden="true" class="warnc" /> {report.denied.length} denied, {report.accepted.length} accepted
        {/if}
      </p>
      <p class="muted small">Roblox {report.robloxVersion ?? '?'} · {relativeTime(report.checkedAt)}{report.complete ? '' : ' · log incomplete'}</p>
      {#if report.notRead.length}<p class="note">{report.notRead.length} flag(s) weren't read, likely because a different Roblox copy was launched.</p>{/if}
      <button class="link small" onclick={() => app.go('flags')}>See flags <ArrowRight size={13} aria-hidden="true" /></button>
    {:else}
      <p class="muted">Launch Roblox with flags set and Vesper will read Roblox's own log to confirm which ones it accepted.</p>
    {/if}
  </Card>

  <Card title="Performance" subtitle={app.robloxRunning ? 'Roblox' : 'This PC'}>
    {#snippet actions()}
      <Button size="sm" variant="ghost" icon={ArrowRight} label="Open performance" onclick={() => app.go('performance')} />
    {/snippet}
    {#if perf}
      {#if app.robloxRunning && perf.roblox}
        <div class="kv">
          <span>FPS</span><span class="mono">{perf.roblox.frames ? perf.roblox.frames.fps.toFixed(0) : '—'}</span>
          <span>CPU</span><span class="mono">{percent(perf.roblox.cpuPercent, 1)}</span>
          <span>Memory</span><span class="mono">{bytes(perf.roblox.workingSet)}</span>
        </div>
        <Sparkline label="Roblox CPU usage" values={series((s) => s.roblox?.cpuPercent ?? null)} max={100} />
      {:else}
        <div class="kv">
          <span>CPU</span><span class="mono">{percent(perf.system.cpuPercent)}</span>
          <span>Memory</span><span class="mono">{bytes(perf.system.memoryUsed)} / {bytes(perf.system.memoryTotal)}</span>
          <span>GPU</span><span class="mono">{percent(perf.system.gpuPercent)}</span>
        </div>
        <Sparkline label="CPU usage" values={series((s) => s.system.cpuPercent)} max={100} />
      {/if}
    {:else if app.settings.performance.monitoring}
      <p class="muted">Collecting the first sample…</p>
    {:else}
      <p class="muted">Monitoring is off. Turn it on in Settings → Performance.</p>
    {/if}
  </Card>
</div>

<section class="quick glass">
  <div class="q">
    <div><div class="ql">Discord Rich Presence</div><div class="qh">Show “Using Vesper” on your profile</div></div>
    <Toggle
      label="Discord Rich Presence"
      checked={app.settings.discord.richPresence}
      disabled={!app.boot?.discordConfigured}
      onchange={(v) => app.saveSettings({ discord: { richPresence: v } })}
    />
  </div>
  <div class="q">
    <div><div class="ql">Keep flags applied</div><div class="qh">Re-apply after Roblox updates itself</div></div>
    <Toggle
      label="Keep flags applied"
      checked={app.settings.roblox.flagMode === 'keepApplied'}
      onchange={(v) => app.saveSettings({ roblox: { flagMode: v ? 'keepApplied' : 'applyOnLaunch' } })}
    />
  </div>
  <div class="q">
    <div><div class="ql">Frame-rate limit</div><div class="qh">Use the profile's limit</div></div>
    <Toggle
      label="Apply frame-rate limit"
      checked={app.settings.roblox.applyFpsCap}
      onchange={(v) => app.saveSettings({ roblox: { applyFpsCap: v } })}
    />
  </div>
</section>

<style>
  .head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: var(--space-4);
    margin: var(--space-2) 0 var(--space-5);
  }
  h1 {
    font-size: var(--text-3xl);
    margin-top: 4px;
  }
  .hero {
    display: grid;
    grid-template-columns: 1fr 260px;
    gap: var(--space-6);
    padding: var(--space-6);
    border-radius: var(--radius-xl);
    background:
      radial-gradient(120% 140% at 0% 0%, rgba(149, 132, 255, 0.16), transparent 50%),
      var(--surface);
  }
  .launch-row {
    display: flex;
    align-items: flex-start;
    gap: var(--space-4);
    flex-wrap: wrap;
  }
  .join {
    flex: 1;
    min-width: 240px;
  }
  .input {
    width: 100%;
    height: 46px;
    padding: 0 14px;
    border-radius: var(--radius-lg);
    border: 1px solid var(--border);
    background: var(--surface-2);
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    transition: border-color var(--dur-fast);
  }
  .input:focus-visible {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 22%, transparent);
  }
  .input.invalid {
    border-color: color-mix(in srgb, var(--bad) 60%, transparent);
  }
  .hint {
    margin-top: 6px;
    font-size: var(--text-sm);
    color: var(--text-3);
  }
  .hint.bad {
    color: var(--bad);
  }
  .failure {
    margin-top: var(--space-5);
    padding: var(--space-4);
    border-radius: var(--radius-md);
    background: color-mix(in srgb, var(--bad) 7%, transparent);
    border: 1px solid color-mix(in srgb, var(--bad) 22%, transparent);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    align-items: flex-start;
  }
  .state {
    margin-top: var(--space-4);
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-2);
    font-size: var(--text-sm);
  }
  .state.ok :global(svg) {
    color: var(--ok);
  }
  .state.bad :global(svg) {
    color: var(--bad);
  }
  .profile {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding-left: var(--space-6);
    border-left: 1px solid var(--border);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--space-4);
    margin-top: var(--space-4);
  }
  .kv {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 7px var(--space-4);
    align-items: center;
    margin-bottom: var(--space-3);
  }
  .kv > span:nth-child(odd) {
    color: var(--text-3);
    font-size: var(--text-sm);
  }
  .big {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--text-lg);
    font-weight: 600;
  }
  .big :global(.okc) {
    color: var(--ok);
  }
  .big :global(.warnc) {
    color: var(--warn);
  }
  .note {
    margin-top: var(--space-3);
    font-size: var(--text-sm);
    color: var(--text-3);
  }
  .link {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 0;
    margin-top: var(--space-3);
    border: 0;
    background: none;
    color: var(--accent-strong);
    font-weight: 550;
    cursor: pointer;
  }
  .link:hover {
    text-decoration: underline;
  }
  .state .link {
    margin: 0;
  }
  .quick {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    margin-top: var(--space-4);
    padding: var(--space-2) var(--space-5);
  }
  .q {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    padding: var(--space-4) var(--space-4) var(--space-4) 0;
  }
  .q + .q {
    padding-left: var(--space-5);
    border-left: 1px solid var(--border);
  }
  .ql {
    font-weight: 550;
  }
  .qh {
    font-size: var(--text-sm);
    color: var(--text-3);
  }
  @media (max-width: 1060px) {
    .hero {
      grid-template-columns: 1fr;
    }
    .profile {
      padding-left: 0;
      border-left: 0;
      border-top: 1px solid var(--border);
      padding-top: var(--space-4);
    }
    .grid,
    .quick {
      grid-template-columns: 1fr;
    }
    .q + .q {
      padding-left: 0;
      border-left: 0;
      border-top: 1px solid var(--border);
    }
  }
</style>
