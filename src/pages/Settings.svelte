<script lang="ts">
  import {
    Archive,
    Copy,
    Download,
    FolderOpen,
    ImagePlus,
    RefreshCw,
    RotateCcw,
    Trash,
    Upload,
    FileText,
    Gauge,
  } from '@lucide/svelte';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { onMount, tick } from 'svelte';
  import iconUrl from '../../assets/branding/icon.svg?url';
  import Badge from '../components/ui/Badge.svelte';
  import Button from '../components/ui/Button.svelte';
  import Card from '../components/ui/Card.svelte';
  import Dialog from '../components/ui/Dialog.svelte';
  import Field from '../components/ui/Field.svelte';
  import Segmented from '../components/ui/Segmented.svelte';
  import Select from '../components/ui/Select.svelte';
  import Toggle from '../components/ui/Toggle.svelte';
  import { api } from '../lib/api';
  import { invalidateBackground } from '../lib/appearance';
  import { bytes, relativeTime, sourceLabel } from '../lib/format';
  import { isMac, osName, trayName } from '../lib/platform';
  import { app } from '../lib/store.svelte';
  import type { BackupInfo, CityPing, LogEntry, Option } from '../lib/types';
  import GameIcon from '../components/games/GameIcon.svelte';
  import ServerPicker from '../components/games/ServerPicker.svelte';

  const sections = [
    ['general', 'General'],
    ['appearance', 'Appearance'],
    ['performance', 'Performance'],
    ['roblox', 'Roblox'],
    ['discord', 'Discord'],
    ['servers', 'Servers'],
    ['updates', 'Updates'],
    ['recovery', 'Backups & recovery'],
    ['advanced', 'Advanced'],
  ] as const;

  const s = $derived(app.settings);
  let backups = $state<BackupInfo[]>([]);
  let logs = $state<LogEntry[]>([]);
  let diagnostics = $state<string | null>(null);
  let confirm = $state<{ title: string; body: string; action: string; run: () => Promise<void> } | null>(null);
  let confirmOpen = $state(false);
  let current = $state<string>('general');

  // A section picked from the list stays highlighted until the next scroll by
  // hand, even if it's too far down to reach the top.
  let pinned = false;

  onMount(async () => {
    backups = (await api.backupsList().catch(() => [])) ?? [];
    if (app.settingsSection) {
      await tick();
      jump(app.settingsSection);
    }
  });

  onMount(() => {
    const scroller = document.getElementById('s-general')?.closest('.scroll');
    if (!scroller) return;
    const follow = () => {
      if (pinned) return;
      const top = scroller.getBoundingClientRect().top + 120;
      let found: string = sections[0][0];
      for (const [id] of sections) {
        const el = document.getElementById(`s-${id}`);
        if (el && el.getBoundingClientRect().top <= top) found = id;
      }
      if (scroller.scrollTop + scroller.clientHeight >= scroller.scrollHeight - 4) found = sections.at(-1)![0];
      current = found;
    };
    const unpin = () => (pinned = false);
    const byHand = ['wheel', 'touchmove', 'keydown', 'pointerdown'] as const;
    scroller.addEventListener('scroll', follow, { passive: true });
    for (const e of byHand) scroller.addEventListener(e, unpin, { passive: true });
    return () => {
      scroller.removeEventListener('scroll', follow);
      for (const e of byHand) scroller.removeEventListener(e, unpin);
    };
  });

  function jump(id: string) {
    current = id;
    pinned = true;
    document.getElementById(`s-${id}`)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }

  function ask(title: string, body: string, action: string, run: () => Promise<void>) {
    confirm = { title, body, action, run };
    confirmOpen = true;
  }

  async function chooseInstall() {
    const dir = await open({
      directory: true,
      multiple: false,
      title: isMac
        ? 'Choose the folder that holds Roblox.app'
        : 'Choose the Roblox Versions folder or a version folder',
    });
    if (typeof dir === 'string') await app.saveSettings({ roblox: { installDir: dir } });
  }

  async function createBackup() {
    const b = await app.attempt(() => api.backupsCreate(), 'Backup created');
    if (b) backups = [b, ...backups];
  }

  async function restore(id: string) {
    const r = await app.attempt(
      () => api.backupsRestore(id),
      'Restored. Your previous setup was saved as a backup first.',
    );
    if (r) {
      app.settings = r.settings;
      app.setProfiles(r.profiles);
      backups = await api.backupsList();
    }
  }

  async function exportBackup(id: string) {
    const path = await save({
      defaultPath: `vesper-backup-${id}.json`,
      filters: [{ name: 'Vesper backup', extensions: ['json'] }],
    });
    if (path) await app.attempt(() => api.backupsExport(id, path), 'Backup exported');
  }

  async function importBackup() {
    const path = await open({ multiple: false, filters: [{ name: 'Vesper backup', extensions: ['json'] }] });
    if (typeof path !== 'string') return;
    ask(
      'Restore this backup?',
      'Your current settings and profiles will be replaced. Vesper saves them as a backup first.',
      'Restore',
      async () => {
        const r = await app.attempt(() => api.backupsImport(path), 'Backup restored');
        if (r) {
          app.settings = r.settings;
          app.setProfiles(r.profiles);
          backups = await api.backupsList();
        }
      },
    );
  }

  async function loadLogs() {
    logs = await api.logsTail(200).catch(() => []);
  }

  async function showDiagnostics() {
    diagnostics = (await app.attempt(() => api.diagnosticsText())) ?? null;
  }

  const presenceText = $derived.by(() => {
    switch (app.presence.state) {
      case 'connected':
        return { tone: 'ok' as const, label: 'Connected to Discord' };
      case 'connecting':
        return { tone: 'info' as const, label: 'Connecting…' };
      case 'discordNotRunning':
        return { tone: 'neutral' as const, label: "Discord isn't running. Vesper will connect when it starts." };
      case 'rejected':
        return { tone: 'bad' as const, label: `Discord refused the connection: ${app.presence.detail}` };
      case 'error':
        return { tone: 'bad' as const, label: app.presence.detail };
      case 'notConfigured':
        return { tone: 'warn' as const, label: 'This build has no Discord application ID.' };
      default:
        return { tone: 'neutral' as const, label: 'Off' };
    }
  });

  // ------------------------------------------------ servers
  let regions = $state<Option[]>([]);
  let pings = $state<CityPing[] | null>(null);
  let pinging = $state(false);
  onMount(async () => {
    regions = (await api.serversRegions().catch(() => [])) ?? [];
  });
  async function testPing() {
    pinging = true;
    const r = await app.attempt(() => api.serversScan(null, true));
    if (r) pings = r;
    pinging = false;
  }
  /** Best city per region, in region order. */
  const byRegion = $derived.by(() => {
    if (!pings) return [];
    return regions
      .map((r) => ({ region: r, cities: pings!.filter((c) => c.region === r.id) }))
      .filter((g) => g.cities.length);
  });
  const regionChoice = $derived(s.servers.region);
  const regionOptions = $derived.by(() => {
    const list = regions.map((r) => ({ value: r.id, label: r.label }));
    for (const c of pings ?? []) list.push({ value: c.key, label: `${c.city} (${c.regionLabel})` });
    if (regionChoice.startsWith('city:') && !list.some((o) => o.value === regionChoice)) {
      list.push({ value: regionChoice, label: regionChoice.split(':').slice(2).join(':') });
    }
    return list;
  });

  const gamePreview = $derived(app.nowPlaying?.game && s.discord.showGame ? app.nowPlaying : null);
  const presenceButtons = $derived.by(() => {
    const b: string[] = [];
    if (gamePreview && s.discord.joinButton && gamePreview.joinUrl) b.push('Join server');
    if (s.discord.profileButton && app.player) b.push('Roblox profile');
    if (b.length < 2) b.push('Get Vesper');
    return b;
  });

  const presencePreview = $derived.by(() => {
    const status = app.robloxRunning ? 'Playing Roblox' : 'In the launcher';
    const profile = app.activeProfile?.name;
    if (s.discord.showRobloxStatus && s.discord.showProfile && profile) return `${status} · ${profile} profile`;
    if (s.discord.showRobloxStatus) return status;
    if (s.discord.showProfile && profile) return `${profile} profile`;
    return null;
  });

  const reasonLabel = {
    manual: 'Manual',
    beforeRestore: 'Before restore',
    beforeImport: 'Before import',
    beforeReset: 'Before reset',
  };
</script>

<header class="head">
  <p class="eyebrow">Settings</p>
  <h1>Settings</h1>
</header>

<div class="layout">
  <nav class="toc" aria-label="Settings sections">
    {#each sections as [id, label] (id)}
      <button class:active={current === id} onclick={() => jump(id)}>{label}</button>
    {/each}
  </nav>

  <div class="sections">
    <Card id="s-general" title="General">
      <Field
        label={isMac ? 'Open at login' : 'Start with Windows'}
        hint="Vesper starts in the {trayName} when you sign in."
      >
        <Toggle
          label={isMac ? 'Open at login' : 'Start with Windows'}
          checked={s.general.startWithWindows}
          onchange={(v) => app.saveSettings({ general: { startWithWindows: v } })}
        />
      </Field>
      <Field
        label="Start hidden"
        hint="When starting at sign-in, stay in the {trayName} instead of opening the window."
      >
        <Toggle
          label="Start hidden"
          checked={s.general.startMinimized}
          onchange={(v) => app.saveSettings({ general: { startMinimized: v } })}
        />
      </Field>
      {#if !isMac}
        <Field
          label="Minimize to {trayName}"
          hint={isMac
            ? 'Minimizing hides the window to the menu bar instead of the Dock.'
            : 'Minimizing it yourself hides the window to the notification area instead of the taskbar.'}
        >
          <Toggle
            label="Minimize to {trayName}"
            checked={s.general.minimizeToTray}
            onchange={(v) => app.saveSettings({ general: { minimizeToTray: v } })}
          />
        </Field>
      {/if}
      <Field label="When I close the window">
        <Segmented
          label="Close behavior"
          value={s.general.closeBehavior}
          options={[
            { value: 'tray', label: `Keep in ${trayName}` },
            { value: 'quit', label: 'Quit Vesper' },
          ]}
          onchange={(v) => app.saveSettings({ general: { closeBehavior: v } })}
        />
      </Field>
      <Field
        label="After launching Roblox"
        hint={s.general.afterLaunch === 'tray'
          ? isMac
            ? 'Vesper hides its window while Roblox runs. Open it from the menu bar icon or the Dock.'
            : 'Vesper leaves the taskbar while Roblox runs. Open it from the tray icon.'
          : s.general.afterLaunch === 'hidden'
            ? isMac
              ? 'Vesper hides its window and its menu bar icon while Roblox runs. Click it in the Dock to bring it back.'
              : 'Vesper leaves the taskbar and the tray while Roblox runs. Open Vesper again from the Start menu to bring it back.'
            : undefined}
      >
        <Select
          label="After launching Roblox"
          value={s.general.afterLaunch}
          options={[
            { value: 'stay', label: 'Keep Vesper open' },
            { value: 'minimize', label: isMac ? 'Minimize to the Dock' : 'Minimize to the taskbar' },
            { value: 'tray', label: `Hide in the ${trayName}` },
            { value: 'hidden', label: 'Hide completely' },
          ]}
          onchange={(v) => app.saveSettings({ general: { afterLaunch: v } })}
        />
      </Field>
      {#if s.general.afterLaunch !== 'stay'}
        <Field label="When Roblox closes" hint="After Vesper minimized or hid itself to make room for Roblox.">
          <Select
            label="When Roblox closes"
            value={s.general.whenRobloxCloses}
            options={[
              { value: 'show', label: 'Bring Vesper back' },
              {
                value: 'stay',
                label: s.general.afterLaunch === 'hidden' ? `Leave it in the ${trayName}` : 'Leave it where it is',
              },
              { value: 'quit', label: 'Quit Vesper' },
            ]}
            onchange={(v) => app.saveSettings({ general: { whenRobloxCloses: v } })}
          />
        </Field>
      {/if}
      <Field label="Launch Roblox when Vesper starts">
        <Toggle
          label="Launch Roblox on start"
          checked={s.general.launchRobloxOnStart}
          onchange={(v) => app.saveSettings({ general: { launchRobloxOnStart: v } })}
        />
      </Field>
      <Field label="Default profile" hint="Used when Vesper starts.">
        <Select
          label="Default profile"
          value={app.profiles.default}
          options={app.profiles.profiles.map((p) => ({ value: p.id, label: p.name }))}
          onchange={(id) => app.attempt(() => api.profilesSetDefault(id)).then((st) => app.setProfiles(st))}
        />
      </Field>
    </Card>

    <Card id="s-appearance" title="Appearance">
      <Field label="Theme">
        <Segmented
          label="Theme"
          value={s.appearance.theme}
          options={[
            { value: 'dark', label: 'Dark' },
            { value: 'light', label: 'Light' },
            { value: 'system', label: 'System' },
          ]}
          onchange={(v) => app.saveSettings({ appearance: { theme: v } })}
        />
      </Field>
      <Field
        label="Transparency"
        hint={isMac
          ? "Glass lets macOS's window material show through."
          : "Glass uses Windows 11's Mica material behind the window."}
      >
        <Segmented
          label="Transparency"
          value={s.appearance.transparency}
          options={[
            { value: 'full', label: 'Glass' },
            { value: 'reduced', label: 'Subtle' },
            { value: 'off', label: 'Solid' },
          ]}
          onchange={(v) => app.saveSettings({ appearance: { transparency: v } })}
        />
      </Field>
      <Field
        label="Animations"
        hint={isMac
          ? 'System follows “Reduce motion” in macOS Accessibility settings.'
          : "System follows Windows' “Animation effects” setting."}
      >
        <Segmented
          label="Animations"
          value={s.appearance.motion}
          options={[
            { value: 'system', label: 'System' },
            { value: 'full', label: 'Full' },
            { value: 'reduced', label: 'Reduced' },
            { value: 'off', label: 'Off' },
          ]}
          onchange={(v) => app.saveSettings({ appearance: { motion: v } })}
        />
      </Field>
      <Field label="Density">
        <Segmented
          label="Density"
          value={s.appearance.density}
          options={[
            { value: 'comfortable', label: 'Comfortable' },
            { value: 'compact', label: 'Compact' },
          ]}
          onchange={(v) => app.saveSettings({ appearance: { density: v } })}
        />
      </Field>
      <Field label="Interface size">
        <Segmented
          label="Interface size"
          value={String(s.appearance.scale)}
          options={['90', '100', '110', '125', '150'].map((v) => ({ value: v, label: `${v}%` }))}
          onchange={(v) => app.saveSettings({ appearance: { scale: Number(v) } })}
        />
      </Field>
      <Field label="Window background" hint="An image behind Vesper. Adjust darkening and blur on the Customize page.">
        <Button
          size="sm"
          icon={ImagePlus}
          onclick={async () => {
            const p = await open({
              multiple: false,
              filters: [{ name: 'Image', extensions: ['png', 'jpg', 'jpeg', 'webp'] }],
            });
            if (typeof p === 'string') {
              const st = await app.attempt(() => api.backgroundSet(p));
              if (st) {
                invalidateBackground();
                app.settings = st;
              }
            }
          }}>Choose…</Button
        >
        {#if s.appearance.background}
          <Button
            size="sm"
            variant="ghost"
            onclick={async () => {
              const st = await app.attempt(() => api.backgroundClear());
              if (st) {
                invalidateBackground();
                app.settings = st;
              }
            }}>Remove</Button
          >
        {/if}
      </Field>
    </Card>

    <Card id="s-performance" title="Performance">
      <Field label="Live monitoring" hint="Samples only while the dashboard or Performance page is visible.">
        <Toggle
          label="Live monitoring"
          checked={s.performance.monitoring}
          onchange={(v) => app.saveSettings({ performance: { monitoring: v } })}
        />
      </Field>
      <Field label="Sample every">
        <Segmented
          label="Sample interval"
          value={String(s.performance.sampleMs)}
          options={[
            { value: '500', label: '0.5 s' },
            { value: '1000', label: '1 s' },
            { value: '2000', label: '2 s' },
          ]}
          onchange={(v) => app.saveSettings({ performance: { sampleMs: Number(v) } })}
        />
      </Field>
      {#if !isMac}
        <Field
          label="Frame-rate counter"
          hint="Counts Direct3D present events. Needs administrator rights or the Performance Log Users group."
        >
          <Toggle
            label="Frame-rate counter"
            checked={s.performance.fpsCounter}
            onchange={(v) => app.saveSettings({ performance: { fpsCounter: v } })}
          />
        </Field>
      {/if}
    </Card>

    <Card id="s-roblox" title="Roblox">
      <Field
        label="Roblox in use"
        hint={app.install
          ? `${sourceLabel(app.install.source)} · ${app.install.version ?? '?'} · ${app.install.location}`
          : 'Not found'}
        stacked={false}
      >
        <Button size="sm" icon={FolderOpen} onclick={chooseInstall}>{isMac ? 'Choose…' : 'Choose folder…'}</Button>
        {#if s.roblox.installDir}<Button
            size="sm"
            variant="ghost"
            onclick={() => app.saveSettings({ roblox: { installDir: null } })}>Automatic</Button
          >{/if}
      </Field>
      {#if app.detection.overrideProblem}<p class="warn small">{app.detection.overrideProblem}</p>{/if}
      {#if app.detection.installs.length > 1}
        <details class="installs">
          <summary>{app.detection.installs.length} Roblox copies found</summary>
          {#each app.detection.installs as i (i.id)}
            <div class="install">
              <span>{sourceLabel(i.source)}{i.registered ? ' · registered' : ''}</span>
              <span class="mono small">{i.version ?? '?'}</span>
              <span class="faint small path">{i.location}</span>
              {#if i.id === app.detection.preferred}<Badge tone="accent">In use</Badge>{/if}
            </div>
          {/each}
        </details>
      {/if}
      {#if app.detection.storeAppPresent}<p class="faint small">
          The Microsoft Store edition is also installed. Its files are sealed, so Vesper can't launch or tune it.
        </p>{/if}
      <Field label="Fast Flags">
        <Select
          label="Fast Flag handling"
          value={s.roblox.flagMode}
          options={[
            { value: 'applyOnLaunch', label: 'Apply when Vesper launches Roblox' },
            { value: 'keepApplied', label: 'Also keep them applied after Roblox updates' },
            { value: 'off', label: "Don't manage Fast Flags" },
          ]}
          onchange={(v) => app.saveSettings({ roblox: { flagMode: v } })}
        />
      </Field>
      <Field
        label="Apply frame-rate limits"
        hint="Writes the profile's limit into Roblox's own settings before launch."
      >
        <Toggle
          label="Apply frame-rate limits"
          checked={s.roblox.applyFpsCap}
          onchange={(v) => app.saveSettings({ roblox: { applyFpsCap: v } })}
        />
      </Field>
      <Field label="Check flags after launch" hint="Reads Roblox's log to see which flags it accepted.">
        <Toggle
          label="Check flags after launch"
          checked={s.roblox.verifyAfterLaunch}
          onchange={(v) => app.saveSettings({ roblox: { verifyAfterLaunch: v } })}
        />
      </Field>
      <Field
        label="Check for Roblox updates"
        hint="Asks Roblox's public version service whether a newer client exists."
      >
        <Toggle
          label="Check for Roblox updates"
          checked={s.roblox.checkRobloxUpdates}
          onchange={(v) => app.saveSettings({ roblox: { checkRobloxUpdates: v } })}
        />
      </Field>
      {#if app.detection.protocol}
        <p class="faint small">
          Game links from roblox.com open in {app.detection.protocol.owner}. Vesper doesn't take over website links.
        </p>
      {/if}
    </Card>

    <Card id="s-discord" title="Discord">
      <Field
        label="Rich Presence"
        hint="Shows what you’re up to in Vesper and Roblox on your Discord profile. The options below choose what’s included."
      >
        <Toggle
          label="Rich Presence"
          checked={s.discord.richPresence}
          disabled={!app.boot?.discordConfigured}
          onchange={(v) => app.saveSettings({ discord: { richPresence: v } })}
        />
      </Field>
      {#if !app.boot?.discordConfigured}
        <p class="warn small">
          This build of Vesper wasn't given a Discord application ID, so Rich Presence can't connect.
        </p>
      {/if}
      <Field label="Show the active profile">
        <Toggle
          label="Show the active profile"
          checked={s.discord.showProfile}
          onchange={(v) => app.saveSettings({ discord: { showProfile: v } })}
        />
      </Field>
      <Field label="Show whether Roblox is running">
        <Toggle
          label="Show Roblox status"
          checked={s.discord.showRobloxStatus}
          onchange={(v) => app.saveSettings({ discord: { showRobloxStatus: v } })}
        />
      </Field>
      <Field label="Show the game you're playing" hint="The game's name, creator and icon, while you're in a game.">
        <Toggle
          label="Show the game"
          checked={s.discord.showGame}
          onchange={(v) => app.saveSettings({ discord: { showGame: v } })}
        />
      </Field>
      <Field label="Show play time" hint="How long you've been in the current server.">
        <Toggle
          label="Show play time"
          checked={s.discord.showPlayTime}
          disabled={!s.discord.showGame}
          onchange={(v) => app.saveSettings({ discord: { showPlayTime: v } })}
        />
      </Field>
      <Field
        label="“Join server” button"
        hint="Lets friends open Roblox in your exact server. Only for public servers; private and reserved servers never get it."
      >
        <Toggle
          label="Join server button"
          checked={s.discord.joinButton}
          disabled={!s.discord.showGame}
          onchange={(v) => app.saveSettings({ discord: { joinButton: v } })}
        />
      </Field>
      <Field
        label="“Roblox profile” button"
        hint={app.player
          ? `Links to ${app.player.info ? '@' + app.player.info.name : 'your profile'} on roblox.com.`
          : 'Links to your Roblox profile. Vesper learns which account it is the first time you join a game.'}
      >
        <Toggle
          label="Roblox profile button"
          checked={s.discord.profileButton}
          onchange={(v) => app.saveSettings({ discord: { profileButton: v } })}
        />
      </Field>
      <div class="presence">
        <div class="pv glass">
          <span class="pv-art">
            {#if gamePreview}
              <GameIcon url={gamePreview.game?.iconUrl} size={56} />
            {:else}
              <img src={iconUrl} alt="" width="56" height="56" />
            {/if}
            {#if s.discord.showRobloxStatus}<span
                class="pv-badge"
                class:playing={app.robloxRunning}
                title={app.robloxRunning ? 'Playing Roblox' : 'In the launcher'}
              ></span>{/if}
          </span>
          <div>
            <div class="pv-name">Vesper</div>
            {#if gamePreview}
              <div class="pv-line">{gamePreview.game?.name}</div>
              <div class="pv-line faint">by {gamePreview.game?.creator}</div>
            {:else}
              <div class="pv-line">Using Vesper</div>
              {#if presencePreview}<div class="pv-line faint">{presencePreview}</div>{/if}
            {/if}
            {#each presenceButtons as b (b)}<div class="pv-btn">{b}</div>{/each}
          </div>
        </div>
        <div>
          <Badge tone={presenceText.tone} dot>{s.discord.richPresence ? presenceText.label : 'Off'}</Badge>
          <p class="faint small">
            What friends see. Discord hides buttons on your own profile. Nothing else (paths, account details, server
            addresses) is shared. If Discord is closed, Vesper connects when it opens.
          </p>
        </div>
      </div>
    </Card>

    <Card id="s-servers" title="Servers" subtitle="For games you join from Vesper">
      <Field
        label="Which server to join"
        hint="Roblox normally picks a server for you. Vesper can pick the one with the lowest ping, or one in a region you choose."
      >
        <ServerPicker />
      </Field>
      {#if s.servers.pick === 'region'}
        <Field
          label="Region or city"
          hint="If the game has no open server there, Vesper picks the lowest-ping one and says so."
        >
          <Select
            label="Region or city"
            value={regionChoice}
            options={regionOptions}
            onchange={(v) => app.saveSettings({ servers: { region: v } })}
          />
        </Field>
      {/if}
      <div class="pings">
        <div class="pings-head">
          <div>
            <div class="label">Your ping to Roblox's regions</div>
            <p class="faint small">
              Measured with a quick connection to a cloud data centre in or next to each city; cities without one are
              estimated from distance.
            </p>
          </div>
          <Button size="sm" icon={Gauge} loading={pinging} onclick={testPing}
            >{pings ? 'Test again' : 'Test ping'}</Button
          >
        </div>
        {#if byRegion.length}
          <div class="ping-grid">
            {#each byRegion as g (g.region.id)}
              {@const best = g.cities[0]}
              <div class="ping glass">
                <div class="ping-region">{g.region.label}</div>
                <div
                  class="ping-ms"
                  class:good={(best.ms ?? 999) < 80}
                  class:ok={(best.ms ?? 999) >= 80 && (best.ms ?? 999) < 150}
                >
                  {best.ms ?? '—'}<span> ms</span>
                </div>
                <div class="ping-cities faint">
                  {#each g.cities as c (c.key)}<span title={c.measured ? 'Measured' : 'Estimated from distance'}
                      >{c.city} {c.ms ?? '—'}{c.measured ? '' : '*'}</span
                    >{/each}
                </div>
              </div>
            {/each}
          </div>
          <p class="faint small">* estimated from distance.</p>
        {/if}
      </div>
      <p class="faint small">
        Server locations come from RoValra's public database (apis.rovalra.com): Vesper sends the game's place ID and
        nothing about you. Vesper then joins a server Roblox lists as open right now. Games opened from the website use
        Roblox's own matchmaking.
      </p>
    </Card>

    <Card id="s-updates" title="Updates">
      <Field
        label="Vesper {app.boot?.version}"
        hint={app.update.state === 'upToDate'
          ? `Up to date · checked ${relativeTime(app.update.checkedAt)}`
          : app.update.state === 'failed'
            ? app.update.message
            : app.update.state === 'available'
              ? `Version ${app.update.version} is available`
              : 'Updates are signed and verified before they install.'}
      >
        {#if app.update.state === 'available'}
          <Button variant="primary" size="sm" icon={Download} onclick={() => app.attempt(() => api.updatesInstall())}
            >Install and restart</Button
          >
        {:else if app.update.state === 'downloading'}
          <Badge tone="info"
            >Downloading {app.update.total
              ? Math.round((app.update.downloaded / app.update.total) * 100) + '%'
              : bytes(app.update.downloaded)}</Badge
          >
        {:else if app.update.state === 'installing'}
          <Badge tone="info">Installing…</Badge>
        {:else}
          <Button
            size="sm"
            icon={RefreshCw}
            loading={app.update.state === 'checking'}
            onclick={() => app.attempt(() => api.updatesCheck())}>Check now</Button
          >
        {/if}
      </Field>
      {#if app.update.state === 'available' && app.update.notes}
        <pre class="notes">{app.update.notes}</pre>
      {/if}
      <Field label="Check automatically" hint="Checks once when Vesper starts. Installing always needs your click.">
        <Toggle
          label="Check automatically"
          checked={s.updates.autoCheck}
          onchange={(v) => app.saveSettings({ updates: { autoCheck: v } })}
        />
      </Field>
      <Field label="Channel">
        <Segmented
          label="Update channel"
          value={s.updates.channel}
          options={[
            { value: 'stable', label: 'Stable' },
            { value: 'beta', label: 'Beta' },
          ]}
          onchange={(v) => app.saveSettings({ updates: { channel: v } })}
        />
      </Field>
    </Card>

    <Card id="s-recovery" title="Backups & recovery" subtitle="Settings and profiles (including their Fast Flags)">
      {#snippet actions()}
        <Button size="sm" icon={Upload} onclick={importBackup}>Import</Button>
        <Button size="sm" variant="primary" icon={Archive} onclick={createBackup}>Back up now</Button>
      {/snippet}
      {#if backups.length === 0}
        <p class="muted small">
          No backups yet. Vesper also makes one automatically before anything replaces your setup.
        </p>
      {:else}
        <div class="backups">
          {#each backups.slice(0, 12) as b (b.id)}
            <div class="backup">
              <span>{new Date(b.createdAt).toLocaleString()}</span>
              <Badge>{reasonLabel[b.reason]}</Badge>
              <span class="faint small">{b.profileCount} profiles · v{b.appVersion}</span>
              <span class="spacer"></span>
              <Button
                size="sm"
                variant="ghost"
                icon={RotateCcw}
                onclick={() =>
                  ask(
                    'Restore this backup?',
                    'Your current settings and profiles will be replaced. Vesper saves them as a backup first.',
                    'Restore',
                    () => restore(b.id),
                  )}>Restore</Button
              >
              <Button
                size="sm"
                variant="ghost"
                icon={Download}
                label="Export"
                title="Export"
                onclick={() => exportBackup(b.id)}
              />
              <Button
                size="sm"
                variant="ghost"
                icon={Trash}
                label="Delete"
                title="Delete"
                onclick={() =>
                  ask('Delete this backup?', "This can't be undone.", 'Delete', async () => {
                    await app.attempt(() => api.backupsDelete(b.id));
                    backups = backups.filter((x) => x.id !== b.id);
                  })}
              />
            </div>
          {/each}
        </div>
      {/if}

      <h4 class="sub">If Roblox misbehaves</h4>
      <Field
        label="Remove Vesper's Fast Flags from Roblox"
        hint="Roblox uses its defaults until your next launch from Vesper. Files Vesper didn't write are left alone."
      >
        <Button
          size="sm"
          onclick={() =>
            ask(
              "Remove Vesper's flags from Roblox?",
              'Your profiles keep their flags; only the file in Roblox is removed.',
              'Remove',
              async () => {
                const r = await app.attempt(() => api.flagsRemoveApplied(false));
                if (r)
                  app.toast(
                    `Removed ${r.removed} file(s)${r.keptForeign ? `, kept ${r.keptForeign} not written by Vesper` : ''}.`,
                    'ok',
                  );
              },
            )}>Remove flags</Button
        >
      </Field>
      <Field label="Undo all customisations" hint="Restores Roblox's original cursor, emote wheel, font and sounds.">
        <Button
          size="sm"
          onclick={() =>
            ask('Undo all customisations?', "Roblox's original files are put back.", 'Undo all', async () => {
              await app.attempt(() => api.modsClear('all'), 'Customisations removed');
            })}>Undo all</Button
        >
      </Field>
      <Field label="Restore Roblox's frame-rate setting" hint="Puts back the value from before Vesper changed it.">
        <Button
          size="sm"
          disabled={app.robloxRunning}
          onclick={() => app.attempt(() => api.fpsCapRestore(), 'Frame-rate setting restored')}>Restore</Button
        >
      </Field>
      <Field label="Reset Vesper's settings" hint="Profiles return to the built-ins. A backup is made first.">
        <Button
          size="sm"
          variant="danger"
          onclick={() =>
            ask(
              "Reset Vesper's settings?",
              'Settings and profiles go back to defaults. A backup is made first, so you can undo this.',
              'Reset',
              async () => {
                const r = await app.attempt(() => api.recoveryResetSettings(), 'Settings reset');
                if (r) {
                  app.settings = r.settings;
                  app.setProfiles(r.profiles);
                  backups = await api.backupsList();
                }
              },
            )}>Reset settings</Button
        >
      </Field>
      <Field label="Configuration folder">
        <Button size="sm" variant="ghost" icon={FolderOpen} onclick={() => api.openFolder('config')}>Open</Button>
      </Field>
    </Card>

    <Card id="s-advanced" title="Advanced">
      <Field label="Log detail">
        <Select
          label="Log detail"
          value={s.advanced.logLevel}
          options={[
            { value: 'error', label: 'Errors only' },
            { value: 'warn', label: 'Warnings' },
            { value: 'info', label: 'Normal' },
            { value: 'debug', label: 'Detailed' },
          ]}
          onchange={(v) => app.saveSettings({ advanced: { logLevel: v } })}
        />
      </Field>
      <Field label="Logs" hint="Secrets and your {osName} user name are removed before anything is written.">
        <Button size="sm" icon={FileText} onclick={loadLogs}>Show recent</Button>
        <Button size="sm" variant="ghost" icon={FolderOpen} onclick={() => api.openFolder('logs')}>Open folder</Button>
      </Field>
      {#if logs.length}
        <pre class="log">{#each logs as l (l.ts + l.msg)}<span class="lv-{l.level.toLowerCase()}"
              >{l.ts.slice(11, 19)} {l.level.padEnd(5)} {l.area} {l.msg}</span
            >
          {/each}</pre>
      {/if}
      <Field
        label="Diagnostics"
        hint="A summary to paste when you report a problem. It never includes secrets or your user name."
      >
        <Button
          size="sm"
          icon={Copy}
          onclick={async () => {
            const t = await app.attempt(() => api.diagnosticsText());
            if (t) {
              await navigator.clipboard.writeText(t);
              app.toast('Diagnostics copied', 'ok');
            }
          }}>Copy diagnostics</Button
        >
        <Button size="sm" variant="ghost" onclick={showDiagnostics}>Preview</Button>
      </Field>
      {#if diagnostics}<pre class="log">{diagnostics}</pre>{/if}
    </Card>
  </div>
</div>

<Dialog bind:open={confirmOpen} title={confirm?.title ?? ''} description={confirm?.body}>
  {#snippet actions()}
    <Button variant="ghost" onclick={() => (confirmOpen = false)}>Cancel</Button>
    <Button
      variant={confirm?.action === 'Delete' || confirm?.action === 'Reset' ? 'danger' : 'primary'}
      onclick={async () => {
        confirmOpen = false;
        await confirm?.run();
      }}>{confirm?.action}</Button
    >
  {/snippet}
</Dialog>

<style>
  .head {
    margin: var(--space-2) 0 var(--space-6);
  }
  h1 {
    font-size: var(--text-3xl);
    margin-top: 4px;
  }
  .layout {
    display: grid;
    grid-template-columns: 180px 1fr;
    gap: var(--space-6);
    align-items: start;
  }
  .toc {
    position: sticky;
    top: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .toc button {
    text-align: left;
    height: 32px;
    padding: 0 10px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--text-2);
    cursor: pointer;
  }
  .toc button:hover {
    color: var(--text);
    background: var(--surface-2);
  }
  .toc button.active {
    color: var(--text);
    background: var(--surface-3);
  }
  .sections {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    min-width: 0;
  }
  .sections :global(.card) {
    scroll-margin-top: 12px;
  }
  .warn {
    color: var(--warn);
  }
  .installs {
    margin: var(--space-2) 0;
    font-size: var(--text-sm);
  }
  summary {
    cursor: pointer;
    color: var(--text-2);
  }
  .install {
    display: grid;
    grid-template-columns: 170px 130px 1fr auto;
    gap: var(--space-3);
    align-items: center;
    padding: 6px 0;
  }
  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pings {
    padding: var(--space-4) 0;
    border-top: 1px solid var(--border);
  }
  .pings-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--space-4);
  }
  .pings-head .label {
    font-weight: 600;
  }
  .ping-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
    gap: var(--space-2);
    margin-top: var(--space-3);
  }
  .ping {
    padding: var(--space-3);
    border-radius: var(--radius-md);
  }
  .ping-region {
    font-size: var(--text-sm);
    font-weight: 600;
  }
  .ping-ms {
    margin: 2px 0;
    font-size: var(--text-xl);
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    color: var(--bad);
  }
  .ping-ms.ok {
    color: var(--warn);
  }
  .ping-ms.good {
    color: var(--ok);
  }
  .ping-ms span {
    font-size: var(--text-sm);
    font-weight: 500;
    color: var(--text-3);
  }
  .ping-cities {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 8px;
    font-size: var(--text-xs);
  }
  .presence {
    display: flex;
    align-items: center;
    gap: var(--space-5);
    padding-top: var(--space-4);
    border-top: 1px solid var(--border);
  }
  .presence p {
    margin-top: 8px;
    max-width: 44ch;
  }
  .pv {
    display: flex;
    gap: 12px;
    padding: 12px;
    width: 280px;
    flex: none;
    border-radius: var(--radius-md);
  }
  .pv-art {
    position: relative;
    flex: none;
    width: 56px;
    height: 56px;
  }
  .pv img {
    border-radius: 10px;
  }
  /* The small image Discord shows in the corner (assets "idle" / "playing"). */
  .pv-badge {
    position: absolute;
    right: -5px;
    bottom: -5px;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    box-shadow: 0 0 0 3px var(--bg-elevated);
    background:
      radial-gradient(circle, #f7f5ff 0 22%, rgba(201, 189, 255, 0.45) 24%, transparent 62%),
      linear-gradient(#0c0b1a, #221d47);
  }
  .pv-badge.playing {
    background: linear-gradient(#4fe0a8, #22a879);
  }
  .pv-badge.playing::after {
    content: '';
    position: absolute;
    inset: 6px 5px 6px 8px;
    background: #fff;
    clip-path: polygon(0 0, 100% 50%, 0 100%);
  }
  .pv-name {
    font-weight: 650;
  }
  .pv-line {
    font-size: var(--text-sm);
  }
  .pv-btn {
    margin-top: 8px;
    padding: 5px 10px;
    border-radius: 6px;
    background: var(--surface-3);
    font-size: var(--text-sm);
    text-align: center;
  }
  .notes,
  .log {
    max-height: 260px;
    overflow: auto;
    margin: 0 0 var(--space-3);
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: var(--surface);
    border: 1px solid var(--border);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    white-space: pre-wrap;
    word-break: break-word;
    color: var(--text-2);
  }
  .log span {
    display: block;
  }
  .lv-warn {
    color: var(--warn);
  }
  .lv-error {
    color: var(--bad);
  }
  .backups {
    display: flex;
    flex-direction: column;
  }
  .backup {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: 8px 0;
    border-top: 1px solid var(--border);
    font-size: var(--text-sm);
  }
  .backup:first-child {
    border-top: 0;
  }
  .sub {
    margin: var(--space-6) 0 var(--space-2);
    font-size: var(--text-md);
  }
  @media (max-width: 1060px) {
    .layout {
      grid-template-columns: 1fr;
    }
    .toc {
      display: none;
    }
    .presence {
      flex-direction: column;
      align-items: flex-start;
    }
  }
</style>
