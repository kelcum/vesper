// Runs Vesper's real interface in a browser on sample data, for reviewing and
// screenshotting screens without a backend. Nothing here can reach Roblox.
//
//   npm run dev, then open http://localhost:1420/dev/preview/#home
//   (#home, #profiles, #flags, #customize, #performance, #settings, #about;
//    add ?running to show Roblox as running)

import catalog from '../../src-tauri/resources/flags/catalog.json';
import sampleFont from '../../assets/fonts/Geist-SemiBold.ttf?url';
import pkg from '../../package.json';
import cursorRing from '../../src/assets/mods/cursor-ring.png';
import defaults from './defaults.json';

const running = location.search.includes('running');
const minutesAgo = (m: number) => new Date(Date.now() - m * 60_000).toISOString();
const version = '0.742.0.7421053';
const versionHash = 'version-cec3ad5889b447cf';

const settings = structuredClone(defaults.settings) as Record<string, any>;
settings.discord.richPresence = true;
settings.lastLaunch = {
  at: minutesAgo(12),
  robloxVersion: version,
  profile: 'balanced',
  target: 'app',
  outcome: 'started',
};
let profiles = structuredClone(defaults.profiles) as Record<string, any>;

const install = {
  id: `official:${versionHash}`,
  source: 'official',
  versionHash,
  version,
  location: `%LOCALAPPDATA%\\Roblox\\Versions\\${versionHash}`,
  registered: true,
  managedBy: null,
  modified: minutesAgo(600),
  hasFlagsFile: true,
};
const detection = {
  installs: [install],
  preferred: install.id,
  protocol: { owner: 'Roblox', command: `"%LOCALAPPDATA%\\Roblox\\Versions\\${versionHash}\\RobloxPlayerBeta.exe" %1` },
  storeAppPresent: false,
  overrideProblem: null,
};
const published = {
  values: { DFIntCSGLevelOfDetailSwitchingDistance: '250', DFIntCSGLevelOfDetailSwitchingDistanceL12: '500' },
  fetchedAt: minutesAgo(30),
};
const system = {
  os: 'Windows 11 Home 25H2',
  cpu: 'AMD Ryzen 7 7800X3D 8-Core Processor',
  logicalProcessors: 16,
  memoryTotal: 34_359_738_368,
  gpus: [{ name: 'NVIDIA GeForce RTX 4070', dedicatedMemory: 12_878_610_432, vendorId: 4318 }],
  displayHz: 144,
};
const verification = {
  flags: {
    FFlagHandleAltEnterFullscreenManually: { verdict: 'accepted', robloxVersion: version, checkedAt: minutesAgo(12) },
  },
  lastReport: {
    robloxVersion: version,
    checkedAt: minutesAgo(12),
    accepted: ['FFlagHandleAltEnterFullscreenManually'],
    denied: [],
    notRead: [],
    complete: true,
  },
};
const dusk = (layer: string) => `/src-tauri/resources/mods/wheels/dusk/${layer}.png`;
const modsView = {
  config: {
    cursor: { source: { kind: 'preset', id: 'ring' }, scale: 115, color: '#a596ff' },
    emoteWheel: { source: { kind: 'preset', id: 'dusk' }, accent: '#a596ff', look: { dim: 30, dividers: true } },
    font: { name: "Fishstrap's font" },
    deathSound: null,
    customFiles: false,
  },
  cursorPresets: [
    { id: 'dot', label: 'Vesper dot' },
    { id: 'crosshair', label: 'Crosshair' },
    { id: 'ring', label: 'Ring' },
    { id: 'plus', label: 'Plus' },
    { id: 'arrow', label: 'Arrow' },
  ],
  wheelPresets: [
    { id: 'dusk', label: 'Dusk' },
    { id: 'glass', label: 'Glass' },
    { id: 'minimal', label: 'Minimal' },
  ],
  customDir: '%APPDATA%\\Vesper\\mods\\custom',
  customFileCount: 0,
  cursorScale: [30, 150],
  maxWheelDim: 80,
  launcherFonts: ['Fishstrap'],
  fontName: 'Geist SemiBold',
  lastReport: null,
  robloxRunning: running,
  appliedNow: false,
  previews: {
    cursor: cursorRing,
    wheelBackground: dusk('CircleBackground'),
    wheelRing: dusk('SegmentedCircle'),
    wheelGradient: null,
    wheelLine: null,
    font: sampleFont,
  },
};

const bootstrap = () => ({
  version: pkg.version,
  portable: false,
  settings,
  profiles,
  detection,
  catalog,
  verification,
  system,
  presence: { state: 'connected' },
  discordConfigured: true,
  update: { state: 'upToDate', checkedAt: minutesAgo(3) },
  launch: running ? { state: 'running', pid: 4242 } : { state: 'idle' },
  robloxRunning: running,
  latestRoblox: { version, versionHash, fetchedAt: minutesAgo(30) },
  published,
  notices: [],
});

function merge(target: Record<string, any>, patch: Record<string, any>) {
  for (const [k, v] of Object.entries(patch)) {
    if (v && typeof v === 'object' && !Array.isArray(v) && target[k] && typeof target[k] === 'object')
      merge(target[k], v);
    else target[k] = v;
  }
  return target;
}

// ---------------------------------------------------------------- events

const callbacks = new Map<number, (payload: unknown) => void>();
const listeners = new Map<string, number[]>();
let nextId = 1;
function emit(event: string, payload: unknown) {
  for (const id of listeners.get(event) ?? []) callbacks.get(id)?.({ event, id, payload });
}

let cpu = 18;
setInterval(() => {
  cpu = Math.max(4, Math.min(60, cpu + (Math.random() - 0.5) * 8));
  emit('perf://sample', {
    at: Date.now(),
    system: {
      cpuPercent: cpu,
      memoryUsed: 14_600_000_000,
      memoryTotal: system.memoryTotal,
      gpuPercent: running ? 62 : 6,
    },
    roblox: running
      ? {
          pid: 4242,
          processes: 1,
          cpuPercent: cpu * 0.6,
          workingSet: 1_210_000_000,
          privateBytes: 1_480_000_000,
          gpuPercent: 58,
          gpuMemory: 1_900_000_000,
          frames: { fps: 143.6, frameTimeAvgMs: 6.96, frameTimeP99Ms: 9.8, frames: 1436 },
        }
      : null,
    gpuError: null,
    fpsStatus: running ? { state: 'measuring' } : { state: 'off' },
  });
}, 1000);

// ---------------------------------------------------------------- commands

const handlers: Record<string, (args: any) => unknown> = {
  bootstrap,
  first_run_finish: bootstrap,
  notices_take: () => [],
  settings_update: ({ patch }) => merge(settings, patch),
  roblox_detect: () => detection,
  roblox_latest: () => bootstrap().latestRoblox,
  flags_validate: () => [],
  flags_published: () => published,
  flags_installed: () => ({
    file: `%LOCALAPPDATA%\\Roblox\\Versions\\${versionHash}\\ClientSettings\\ClientAppSettings.json`,
    flags: { FFlagHandleAltEnterFullscreenManually: 'False' },
    error: null,
    writtenByVesper: true,
  }),
  profiles_get: () => profiles,
  profiles_set_active: ({ id }) => ((profiles = { ...profiles, active: id }), profiles),
  profiles_set_default: ({ id }) => ((profiles = { ...profiles, default: id }), profiles),
  fps_cap_status: () => ({ current: 144, original: -1, displayHz: 144, error: null, locked: false }),
  system_info: () => system,
  backups_list: () => [
    {
      id: 'b1',
      createdAt: minutesAgo(60 * 26),
      reason: 'manual',
      appVersion: '0.1.0',
      profileCount: 5,
      sizeBytes: 18_432,
    },
  ],
  logs_tail: () => [
    {
      ts: minutesAgo(12),
      level: 'INFO',
      area: 'session',
      msg: `Started Roblox ${version} (pid 4242, priority Normal)`,
    },
    {
      ts: minutesAgo(12),
      level: 'INFO',
      area: 'session',
      msg: `Roblox ${version} accepted 1 flag(s), denied 0, didn't read 0`,
    },
  ],
  discord_status: () => ({ status: { state: 'connected' }, configured: true }),
  diagnostics_text: () => 'Vesper 0.1.0 (preview)',
  updates_check: () => ({ state: 'upToDate', checkedAt: new Date().toISOString() }),
  mods_get: () => modsView,
  appearance_background_data: () => null,
  'plugin:event|listen': ({ event, handler }) => {
    listeners.set(event, [...(listeners.get(event) ?? []), handler]);
    return handler;
  },
};

(window as any).__TAURI_INTERNALS__ = {
  metadata: { currentWindow: { label: 'main' }, currentWebview: { windowLabel: 'main', label: 'main' } },
  transformCallback: (cb: (payload: unknown) => void) => {
    const id = nextId++;
    callbacks.set(id, cb);
    return id;
  },
  unregisterCallback: (id: number) => callbacks.delete(id),
  invoke: async (cmd: string, args: unknown) => {
    const h = handlers[cmd];
    // Anything else (window controls, saving, applying) quietly does nothing here.
    return h ? structuredClone(h(args ?? {})) : null;
  },
  convertFileSrc: (p: string) => p,
};
(window as any).__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };

await import('../../src/main');
const { app } = await import('../../src/lib/store.svelte');
const page = location.hash.slice(1);
if (page) setTimeout(() => app.go(page as never), 300);
