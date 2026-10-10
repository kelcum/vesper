<script lang="ts">
  import { Cpu, Gauge, MemoryStick, Monitor, RotateCcw, Timer, MonitorCog } from '@lucide/svelte';
  import { onDestroy, onMount } from 'svelte';
  import Badge from '../components/ui/Badge.svelte';
  import Button from '../components/ui/Button.svelte';
  import Card from '../components/ui/Card.svelte';
  import Sparkline from '../components/ui/Sparkline.svelte';
  import Toggle from '../components/ui/Toggle.svelte';
  import { api } from '../lib/api';
  import { bytes, fpsCapLabel, percent, priorityLabel } from '../lib/format';
  import { app } from '../lib/store.svelte';
  import type { FpsCapStatus, PerfSample } from '../lib/types';

  let cap = $state<FpsCapStatus | null>(null);

  onMount(() => {
    api.perfWatch('performance', true);
    api
      .fpsCapStatus()
      .then((c) => (cap = c))
      .catch(() => {});
  });
  onDestroy(() => {
    api.perfWatch('performance', false);
  });

  const s = $derived(app.latestPerf);
  const r = $derived(s?.roblox ?? null);
  const frames = $derived(r?.frames ?? null);
  const sys = $derived(app.boot!.system);
  const profile = $derived(app.activeProfile);
  const series = (pick: (x: PerfSample) => number | null | undefined) => app.perf.map((x) => pick(x) ?? null);

  function capText(v: number | null | undefined) {
    if (v == null) return 'not set';
    if (v === -1) return 'Roblox default';
    if (v >= 9999) return 'Unlimited (9999)';
    return `${v} FPS`;
  }

  async function restoreCap() {
    const c = await app.attempt(() => api.fpsCapRestore(), "Restored Roblox's frame-rate setting");
    if (c) cap = c;
  }
</script>

<header class="head">
  <div>
    <p class="eyebrow">Performance</p>
    <h1>Measured, not promised.</h1>
    <p class="muted lede">
      Live numbers from Windows. Vesper never opens a handle to Roblox to read them, and shows a dash rather than a
      guess when something can't be measured.
    </p>
  </div>
  <div class="row">
    <span class="small muted">Live monitoring</span>
    <Toggle
      label="Live monitoring"
      checked={app.settings.performance.monitoring}
      onchange={(v) => app.saveSettings({ performance: { monitoring: v } })}
    />
  </div>
</header>

{#if !app.settings.performance.monitoring}
  <p class="note glass">Monitoring is off, so no samples are being taken. Turn it on to see live numbers.</p>
{:else}
  <section class="tiles">
    <div class="tile glass">
      <div class="label"><Gauge size={15} aria-hidden="true" /> Frame rate</div>
      {#if !app.robloxRunning}
        <div class="value faint">—</div>
        <div class="sub">Start Roblox to measure</div>
      {:else if s?.fpsStatus.state === 'needsPermission'}
        <div class="value faint">—</div>
        <div class="sub">Needs Windows permission (see below)</div>
      {:else if s?.fpsStatus.state === 'unavailable'}
        <div class="value faint">—</div>
        <div class="sub">{s.fpsStatus.detail}</div>
      {:else if frames}
        <div class="value">{frames.fps.toFixed(0)}<span class="unit">FPS</span></div>
        <div class="sub">from Direct3D presents</div>
      {:else}
        <div class="value faint">—</div>
        <div class="sub">No frames yet (minimised, loading, or not Direct3D)</div>
      {/if}
    </div>
    <div class="tile glass">
      <div class="label"><Timer size={15} aria-hidden="true" /> Frame time</div>
      <div class="value">{frames ? frames.frameTimeAvgMs.toFixed(1) : '—'}<span class="unit">ms avg</span></div>
      <div class="sub">99th percentile {frames ? `${frames.frameTimeP99Ms.toFixed(1)} ms` : '—'}</div>
    </div>
    <div class="tile glass">
      <div class="label"><Cpu size={15} aria-hidden="true" /> Roblox CPU</div>
      <div class="value">{r ? percent(r.cpuPercent, 1) : '—'}</div>
      <div class="sub">of all {sys.logicalProcessors} logical processors</div>
    </div>
    <div class="tile glass">
      <div class="label"><MemoryStick size={15} aria-hidden="true" /> Roblox memory</div>
      <div class="value">{r ? bytes(r.workingSet) : '—'}</div>
      <div class="sub">{r ? `${bytes(r.privateBytes)} private` : 'Not running'}</div>
    </div>
    <div class="tile glass">
      <div class="label"><Monitor size={15} aria-hidden="true" /> Roblox GPU</div>
      <div class="value">{r?.gpuPercent != null ? percent(r.gpuPercent) : '—'}</div>
      <div class="sub">
        {r?.gpuMemory ? `${bytes(r.gpuMemory)} video memory` : s?.gpuError ? 'GPU counters unavailable' : 'Not running'}
      </div>
    </div>
  </section>

  <div class="charts">
    <Card title="Roblox" subtitle="Last two minutes">
      {#if app.robloxRunning}
        <div class="chart">
          <span class="small muted">Frame rate</span><Sparkline
            label="Frame rate history"
            values={series((x) => x.roblox?.frames?.fps)}
          />
        </div>
        <div class="chart">
          <span class="small muted">CPU</span><Sparkline
            label="Roblox CPU history"
            values={series((x) => x.roblox?.cpuPercent)}
            max={100}
            color="var(--blue-400)"
          />
        </div>
        <div class="chart">
          <span class="small muted">GPU</span><Sparkline
            label="Roblox GPU history"
            values={series((x) => x.roblox?.gpuPercent)}
            max={100}
            color="var(--ok)"
          />
        </div>
      {:else}
        <p class="muted">Roblox isn't running. Charts fill in once it starts.</p>
      {/if}
    </Card>
    <Card title="This PC" subtitle="Whole system">
      <div class="kv">
        <span>CPU</span><span class="mono">{percent(s?.system.cpuPercent)}</span>
        <span>Memory</span><span class="mono"
          >{s ? `${bytes(s.system.memoryUsed)} of ${bytes(s.system.memoryTotal)}` : '—'}</span
        >
        <span>GPU</span><span class="mono">{percent(s?.system.gpuPercent)}</span>
      </div>
      <div class="chart">
        <span class="small muted">CPU</span><Sparkline
          label="System CPU history"
          values={series((x) => x.system.cpuPercent)}
          max={100}
          color="var(--blue-400)"
        />
      </div>
      <div class="chart">
        <span class="small muted">GPU</span><Sparkline
          label="System GPU history"
          values={series((x) => x.system.gpuPercent)}
          max={100}
          color="var(--ok)"
        />
      </div>
    </Card>
  </div>
{/if}

{#if app.latestPerf?.fpsStatus.state === 'needsPermission'}
  <Card title="Turning on the frame-rate counter" icon={Gauge}>
    <p class="muted">
      Vesper measures frame rate by counting Direct3D present events, the same signal PresentMon uses. Windows only lets
      administrators and members of the <strong>Performance Log Users</strong> group read those events. Vesper doesn't change
      Windows security settings for you; if you want the counter:
    </p>
    <ol class="muted steps">
      <li>
        Open <span class="mono">Terminal (Admin)</span> and run
        <span class="mono">net localgroup "Performance Log Users" %USERNAME% /add</span>.
      </li>
      <li>Sign out of Windows and back in.</li>
      <li>Reopen Vesper. The counter starts the next time this page or the dashboard is open.</li>
    </ol>
    <p class="faint small">Roblox in Vulkan or OpenGL mode may not report frames this way.</p>
  </Card>
{/if}

<div class="charts">
  <Card title="Frame-rate limit" subtitle="Roblox's own setting, written before launch" icon={MonitorCog}>
    <div class="kv">
      <span>Profile</span><span>{fpsCapLabel(profile?.fpsCap, sys.displayHz)}</span>
      <span>In Roblox now</span><span class="mono">{cap?.error ? cap.error : capText(cap?.current)}</span>
      <span>Before Vesper</span><span class="mono">{cap?.original != null ? capText(cap.original) : 'unchanged'}</span>
      <span>Priority</span><span>{profile ? priorityLabel(profile.priority) : '—'}</span>
    </div>
    <div class="row">
      <Button size="sm" onclick={() => app.go('profiles')}>Change in profile</Button>
      {#if cap?.original != null}
        <Button size="sm" variant="ghost" icon={RotateCcw} onclick={restoreCap} disabled={app.robloxRunning}
          >Restore original</Button
        >
      {/if}
      {#if !app.settings.roblox.applyFpsCap}<Badge tone="warn">Applying limits is off in Settings</Badge>{/if}
      {#if cap?.locked}<Badge
          tone="warn"
          title="Another launcher's lock option usually does this. Vesper won't override it."
          >Roblox's settings file is read-only</Badge
        >{/if}
    </div>
  </Card>
  <Card title="Hardware">
    <div class="kv">
      <span>Windows</span><span>{sys.os}</span>
      <span>Processor</span><span>{sys.cpu}</span>
      {#each sys.gpus as g (g.name)}<span>Graphics</span><span>{g.name} · {bytes(g.dedicatedMemory, 0)}</span>{/each}
      <span>Memory</span><span>{bytes(sys.memoryTotal, 0)}</span>
      <span>Display</span><span>{sys.displayHz ? `${sys.displayHz} Hz` : 'unknown'}</span>
    </div>
  </Card>
</div>

<style>
  .head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: var(--space-4);
    margin: var(--space-2) 0 var(--space-6);
  }
  h1 {
    font-size: var(--text-3xl);
    margin-top: 4px;
  }
  .lede {
    margin-top: var(--space-2);
    max-width: 64ch;
  }
  .note {
    padding: var(--space-5);
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
    gap: var(--space-3);
  }
  .tile {
    padding: var(--space-4);
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .label {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-3);
    font-size: var(--text-sm);
  }
  .value {
    font-size: 28px;
    font-weight: 650;
    letter-spacing: -0.02em;
    font-variant-numeric: tabular-nums;
  }
  .unit {
    margin-left: 6px;
    font-size: var(--text-sm);
    font-weight: 500;
    color: var(--text-3);
  }
  .sub {
    color: var(--text-3);
    font-size: var(--text-xs);
  }
  .charts {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--space-4);
    margin-top: var(--space-4);
  }
  .chart {
    display: grid;
    grid-template-columns: 70px 1fr;
    align-items: center;
    gap: var(--space-3);
    margin-top: var(--space-2);
  }
  .kv {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 8px var(--space-4);
    margin-bottom: var(--space-3);
  }
  .kv > span:nth-child(odd) {
    color: var(--text-3);
    font-size: var(--text-sm);
  }
  .steps {
    margin: var(--space-3) 0;
    padding-left: 20px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  @media (max-width: 1060px) {
    .charts {
      grid-template-columns: 1fr;
    }
  }
</style>
