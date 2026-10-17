// Runs Vesper's real interface in a browser on sample data, for reviewing and
// screenshotting screens without a backend. Nothing here can reach Roblox.
//
//   npm run dev, then open http://localhost:1420/dev/preview/#home
//   (#home, #profiles, #flags, #customize, #performance, #settings, #about;
//    add ?running to show Roblox as running, ?platform=macos for the Mac
//    interface, ?locked for a read-only Roblox settings file, and
//    ?focus=<css selector> to scroll a section into view (&click to press a
//    button first), ?still to skip
//    transitions for screenshots; #recent and
//    #avatar show the newer pages, the avatar signed in as a made-up player)

import catalog from '../../src-tauri/resources/flags/catalog.json';
import sampleFont from '../../assets/fonts/Geist-SemiBold.ttf?url';
import pkg from '../../package.json';
import cursorRing from '../../src/assets/mods/cursor-ring.png';
import defaults from './defaults.json';
import skySunset from './sky-sunset.png';

const running = location.search.includes('running');
const mac = new URLSearchParams(location.search).get('platform') === 'macos';
const minutesAgo = (m: number) => new Date(Date.now() - m * 60_000).toISOString();
const version = '0.742.0.7421053';
const versionHash = 'version-cec3ad5889b447cf';

const settings = structuredClone(defaults.settings) as Record<string, any>;
settings.discord = {
  ...settings.discord,
  richPresence: true,
  showGame: true,
  showPlayTime: true,
  joinButton: true,
  profileButton: true,
};
settings.servers = { pick: 'lowestPing', region: 'us-east' };
settings.general = { ...settings.general, afterLaunch: 'hidden' };
settings.games = { history: true, locateServers: true };
settings.shaders = {
  enabled: true,
  preset: 'cinematic',
  params: {
    exposure: 0,
    contrast: 1.12,
    saturation: 0.92,
    vibrance: 0,
    temperature: 0.12,
    tint: 0.04,
    sharpen: 0,
    vignette: 0.35,
    bloom: 0.15,
    tonemap: 0.7,
  },
};
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
  location: mac ? '/Applications/Roblox.app/Contents/MacOS' : `%LOCALAPPDATA%\\Roblox\\Versions\\${versionHash}`,
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
const system = mac
  ? {
      platform: 'macos',
      os: 'macOS 15.1 (build 24B83)',
      cpu: 'Apple M3 Pro',
      logicalProcessors: 12,
      memoryTotal: 19_327_352_832,
      gpus: [{ name: 'Apple M3 Pro GPU', dedicatedMemory: 0, vendorId: 4203 }],
      displayHz: 120,
    }
  : {
      platform: 'windows',
      os: 'Windows 11 Pro 24H2',
      cpu: 'Intel(R) Core(TM) i5-13600K',
      logicalProcessors: 20,
      memoryTotal: 34_359_738_368,
      gpus: [{ name: 'NVIDIA GeForce RTX 3060', dedicatedMemory: 12_878_610_432, vendorId: 4318 }],
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
    sky: { source: { kind: 'preset', id: 'sunset' }, layout: 'panorama' },
    customFiles: false,
  },
  cursorPresets: [
    { id: 'dot', label: 'Vesper dot' },
    { id: 'crosshair', label: 'Crosshair' },
    { id: 'ring', label: 'Ring' },
    { id: 'plus', label: 'Plus' },
    { id: 'arrow', label: 'Arrow' },
  ],
  skyPresets: [
    { id: 'midnight', label: 'Midnight' },
    { id: 'sunset', label: 'Sunset' },
    { id: 'dawn', label: 'Dawn' },
    { id: 'overcast', label: 'Overcast' },
    { id: 'void', label: 'Void' },
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
    sky: skySunset,
  },
};

const icon = (seed: string) =>
  `data:image/svg+xml,${encodeURIComponent(`<svg xmlns='http://www.w3.org/2000/svg' width='64' height='64'><defs><linearGradient id='g' x1='0' y1='0' x2='1' y2='1'><stop offset='0' stop-color='${seed}'/><stop offset='1' stop-color='#1b1b2a'/></linearGradient></defs><rect width='64' height='64' fill='url(#g)'/></svg>`)}`;
const nowPlaying = running
  ? {
      placeId: 1100000001,
      universeId: 2100000001,
      jobId: '0d6a1c3e-4b7f-4e21-9a55-3f0c2d8e1b11',
      kind: 'public',
      joinedAt: minutesAgo(23),
      game: {
        universeId: 2100000001,
        rootPlaceId: 1100000001,
        name: 'Skyline Obby',
        creator: 'Nimbus Studio',
        iconUrl: icon('#e35d6a'),
      },
      location: 'Ashburn, US',
      joinUrl:
        'https://www.roblox.com/games/start?placeId=1100000001&gameInstanceId=0d6a1c3e-4b7f-4e21-9a55-3f0c2d8e1b11',
      pageUrl: 'https://www.roblox.com/games/1100000001',
    }
  : null;
const history = {
  visits: [
    ...(nowPlaying
      ? [{ ...nowPlaying, name: 'Skyline Obby', creator: 'Nimbus Studio', iconUrl: icon('#e35d6a'), leftAt: null }]
      : []),
    {
      placeId: 1100000002,
      universeId: 2100000002,
      jobId: '3f8b2a61-6c0d-4e9f-8a1b-5d7e2c9f0a33',
      kind: 'public',
      name: 'Harbor Heist',
      creator: 'Quayside Games',
      iconUrl: icon('#3b7ae8'),
      location: 'New York City, US',
      joinedAt: minutesAgo(180),
      leftAt: minutesAgo(118),
    },
    {
      placeId: 1100000003,
      universeId: 2100000003,
      jobId: '9a4d7e12-2b6f-4c80-9e3a-1f5b8c7d6e44',
      kind: 'private',
      name: 'Pet Meadow',
      creator: 'Clover Lab',
      iconUrl: icon('#f2c94c'),
      location: 'Chicago, US',
      joinedAt: minutesAgo(60 * 27),
      leftAt: minutesAgo(60 * 26),
    },
    {
      placeId: 1100000004,
      universeId: 2100000004,
      jobId: '5e1c9b80-7a2d-4f63-8b4e-0c6d3a9f2b55',
      kind: 'public',
      name: 'Sunset Town 🏡RP',
      creator: 'Porchlight',
      iconUrl: icon('#3fbf6f'),
      location: 'Ashburn, US',
      joinedAt: minutesAgo(60 * 50),
      leftAt: minutesAgo(60 * 49),
    },
  ],
};
const regions = [
  { id: 'us-east', label: 'NA East' },
  { id: 'us-central', label: 'NA Central' },
  { id: 'us-west', label: 'NA West' },
  { id: 'latam', label: 'Latin America' },
  { id: 'eu-west', label: 'Europe West' },
  { id: 'eu-east', label: 'Europe East & Middle East' },
  { id: 'asia', label: 'Asia' },
  { id: 'oceania', label: 'Oceania' },
  { id: 'africa', label: 'Africa' },
];
const city = (cityName: string, cc: string, region: string, ms: number, measured = true) => ({
  key: `city:${cc}:${cityName}`,
  city: cityName,
  countryCode: cc,
  region,
  regionLabel: regions.find((r) => r.id === region)!.label,
  ms,
  measured,
  servers: 0,
});
const pings = [
  // A made-up player on the US east coast.
  city('Ashburn', 'US', 'us-east', 14),
  city('New York City', 'US', 'us-east', 19, false),
  city('Chicago', 'US', 'us-central', 31, false),
  city('Dallas', 'US', 'us-central', 44, false),
  city('Los Angeles', 'US', 'us-west', 68, false),
  city('San Jose', 'US', 'us-west', 71),
  city('São Paulo', 'BR', 'latam', 128),
  city('London', 'GB', 'eu-west', 81),
  city('Amsterdam', 'NL', 'eu-west', 88, false),
  city('Frankfurt am Main', 'DE', 'eu-west', 92),
  city('Warsaw', 'PL', 'eu-east', 112, false),
  city('Tokyo', 'JP', 'asia', 162),
  city('Singapore', 'SG', 'asia', 231),
  city('Sydney', 'AU', 'oceania', 214),
  city('Cape Town', 'ZA', 'africa', 238),
];

// ---------------------------------------------------------------- avatar (a made-up player)

const svg = (w: number, h: number, body: string) =>
  `data:image/svg+xml,${encodeURIComponent(`<svg xmlns='http://www.w3.org/2000/svg' width='${w}' height='${h}' viewBox='0 0 ${w} ${h}'>${body}</svg>`)}`;
const avatarRender = svg(
  420,
  420,
  `<ellipse cx='210' cy='388' rx='92' ry='12' fill='#000' opacity='.25'/>` +
    `<rect x='160' y='300' width='42' height='80' rx='8' fill='#2a2b44'/><rect x='218' y='300' width='42' height='80' rx='8' fill='#2a2b44'/>` +
    `<rect x='154' y='368' width='52' height='18' rx='7' fill='#f4f4f8'/><rect x='214' y='368' width='52' height='18' rx='7' fill='#f4f4f8'/>` +
    `<rect x='150' y='184' width='120' height='124' rx='16' fill='#a596ff'/><rect x='190' y='184' width='40' height='54' rx='6' fill='#8b7bf0'/>` +
    `<rect x='112' y='190' width='36' height='108' rx='12' fill='#a596ff'/><rect x='272' y='190' width='36' height='108' rx='12' fill='#a596ff'/>` +
    `<rect x='114' y='290' width='32' height='26' rx='10' fill='#f1c7a5'/><rect x='274' y='290' width='32' height='26' rx='10' fill='#f1c7a5'/>` +
    `<rect x='164' y='86' width='92' height='92' rx='22' fill='#f1c7a5'/>` +
    `<path d='M158 120 q0-52 52-52 q52 0 52 52 l0 6 q-10-30-52-30 q-42 0-52 30z' fill='#3a2f5e'/>` +
    `<circle cx='192' cy='134' r='6' fill='#1b1b2a'/><circle cx='228' cy='134' r='6' fill='#1b1b2a'/>` +
    `<path d='M196 156 q14 10 28 0' stroke='#1b1b2a' stroke-width='4' fill='none' stroke-linecap='round'/>` +
    `<path d='M154 128 q0-66 56-66 q56 0 56 66' stroke='#22223a' stroke-width='9' fill='none'/>` +
    `<rect x='144' y='118' width='20' height='34' rx='8' fill='#22223a'/><rect x='256' y='118' width='20' height='34' rx='8' fill='#22223a'/>`,
);
const thumb = (body: string) => svg(150, 150, `<rect width='150' height='150' rx='18' fill='#262640'/>${body}`);
const avatarItems = [
  {
    id: 9000000101,
    name: 'Midnight Waves',
    type: { id: 41, name: 'HairAccessory' },
    art: thumb(`<path d='M30 90 q0-60 45-60 q45 0 45 60 l0 20 q-12-34-45-34 q-33 0-45 34z' fill='#3a2f5e'/>`),
  },
  {
    id: 9000000102,
    name: 'Night Shift Headphones',
    type: { id: 8, name: 'Hat' },
    art: thumb(
      `<path d='M35 90 q0-55 40-55 q40 0 40 55' stroke='#a596ff' stroke-width='10' fill='none'/><rect x='24' y='80' width='24' height='38' rx='9' fill='#a596ff'/><rect x='102' y='80' width='24' height='38' rx='9' fill='#a596ff'/>`,
    ),
  },
  {
    id: 9000000103,
    name: 'Lavender Hoodie',
    type: { id: 67, name: 'JacketAccessory' },
    art: thumb(
      `<path d='M45 40 h60 l25 30 -16 14 -9-9 v55 h-60 v-55 l-9 9 -16-14z' fill='#a596ff'/><rect x='62' y='40' width='26' height='30' rx='6' fill='#8b7bf0'/>`,
    ),
  },
  {
    id: 9000000104,
    name: 'Dusk Joggers',
    type: { id: 12, name: 'Pants' },
    art: thumb(`<path d='M50 30 h50 l6 90 h-22 l-9-60 -9 60 h-22z' fill='#2f3150'/>`),
  },
  {
    id: 9000000105,
    name: 'Cloud Runners',
    type: { id: 70, name: 'LeftShoeAccessory' },
    art: thumb(`<path d='M30 95 q4-28 30-28 l22 10 q30 4 40 18 v12 h-92z' fill='#f4f4f8'/>`),
  },
  {
    id: 9000000106,
    name: 'Starlight Backpack',
    type: { id: 46, name: 'BackAccessory' },
    art: thumb(
      `<rect x='42' y='36' width='66' height='84' rx='18' fill='#5fd3a6'/><rect x='56' y='72' width='38' height='28' rx='8' fill='#46b48b'/>`,
    ),
  },
];
const wearing = new Set(avatarItems.slice(0, 5).map((i) => i.id));
const asset = (i: (typeof avatarItems)[number]) => ({ id: i.id, name: i.name, assetType: i.type });
const skin = 'F1C7A5';
function avatarApi({ url }: { url: string }) {
  const u = new URL(url);
  const path = u.pathname;
  if (path === '/v1/users/authenticated') return { id: 1, name: 'player', displayName: 'Player' };
  if (path === '/v2/avatar/avatar')
    return {
      scales: { height: 1, width: 1, head: 1, depth: 1, proportion: 0, bodyType: 0 },
      playerAvatarType: 'R15',
      bodyColor3s: {
        headColor3: skin,
        torsoColor3: skin,
        rightArmColor3: skin,
        leftArmColor3: skin,
        rightLegColor3: skin,
        leftLegColor3: skin,
      },
      assets: avatarItems.filter((i) => wearing.has(i.id)).map(asset),
    };
  if (path === '/v1/avatar-rules')
    return {
      scales: {
        height: { min: 0.9, max: 1.05, increment: 0.01 },
        width: { min: 0.7, max: 1, increment: 0.01 },
        head: { min: 0.95, max: 1, increment: 0.01 },
        proportion: { min: 0, max: 1, increment: 0.01 },
        bodyType: { min: 0, max: 1, increment: 0.01 },
      },
      wearableAssetTypes: [{ id: 8, name: 'Hat', maxNumber: 3 }],
      bodyColorsPalette: ['F1C7A5', 'E0A57E', 'C58C5E', '8D5A3B', '5C3A25', 'F8E1CF'].map((hex, i) => ({
        brickColorId: i + 1,
        hexColor: `#${hex}`,
        name: `Skin ${i + 1}`,
      })),
    };
  if (path === '/v1/users/avatar') return { data: [{ targetId: 1, state: 'Completed', imageUrl: avatarRender }] };
  if (path === '/v1/assets') {
    const ids = (u.searchParams.get('assetIds') ?? '').split(',').map(Number);
    return {
      data: avatarItems
        .filter((i) => ids.includes(i.id))
        .map((i) => ({ targetId: i.id, state: 'Completed', imageUrl: i.art })),
    };
  }
  if (path.includes('/inventory'))
    return { data: avatarItems.map((i) => ({ assetId: i.id, assetName: i.name })), nextPageCursor: null };
  if (path.includes('/outfits')) return { data: [] };
  return {};
}
if (location.hash === '#avatar') {
  try {
    localStorage.setItem('vesper.avatar.used', '1');
  } catch {
    /* preview only */
  }
}

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
  fps_cap_status: () => ({
    current: 144,
    original: -1,
    displayHz: 144,
    error: null,
    locked: location.search.includes('locked'),
  }),
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
  playing_get: () => ({
    now: nowPlaying,
    player: {
      id: 1,
      info: { id: 1, name: 'player', displayName: 'Player' },
      profileUrl: 'https://www.roblox.com/users/1/profile',
    },
  }),
  history_get: () => history,
  avatar_api: avatarApi,
  servers_regions: () => regions,
  servers_scan: async () => (await new Promise((r) => setTimeout(r, 600)), pings),
  shaders_info: () => ({
    status: { state: running ? 'running' : 'waiting' },
    presets: [
      { id: 'vivid', label: 'Vivid' },
      { id: 'cinematic', label: 'Cinematic' },
      { id: 'crisp', label: 'Crisp' },
      { id: 'warm', label: 'Golden hour' },
      { id: 'cool', label: 'Moonlight' },
      { id: 'soft', label: 'Soft glow' },
      { id: 'noir', label: 'Noir' },
    ],
  }),
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
    return h ? structuredClone(await h(args ?? {})) : null;
  },
  convertFileSrc: (p: string) => p,
};
(window as any).__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };

// ?still: transitions finish at once, so screenshots never catch a page halfway in.
if (location.search.includes('still')) {
  const animate = Element.prototype.animate;
  Element.prototype.animate = function (keyframes, options) {
    const o = typeof options === 'number' ? {} : (options ?? {});
    return animate.call(this, keyframes, { ...o, duration: 0, delay: 0 });
  };
  const style = document.createElement('style');
  style.textContent = '*, *::before, *::after { transition: none !important; animation: none !important; }';
  document.head.append(style);
}

await import('../../src/main');
const { app } = await import('../../src/lib/store.svelte');
const page = location.hash.slice(1);
if (page) setTimeout(() => app.go(page as never), 300);
const query = new URLSearchParams(location.search);
// &click=<css selector> presses a button first (say, the ping test).
const click = query.get('click');
if (click) setTimeout(() => document.querySelector<HTMLElement>(click)?.click(), 700);
const focus = query.get('focus');
if (focus)
  setTimeout(() => {
    const el = document.querySelector(focus);
    el?.scrollIntoView({ block: 'start' });
    // Clear of the window's title bar, or further down with &offset=<px>.
    el?.closest('.scroll')?.scrollBy(0, Number(query.get('offset') ?? -64));
  }, 1800);
