// TypeScript mirrors of the Rust types sent over IPC (serde, camelCase).

export type FlagSet = Record<string, string>;

// ---------------------------------------------------------------- settings

export type CloseBehavior = 'tray' | 'quit';
export type AfterLaunch = 'stay' | 'minimize' | 'tray';
export type WhenRobloxCloses = 'show' | 'stay' | 'quit';
export type Theme = 'system' | 'dark' | 'light';
export type Transparency = 'full' | 'reduced' | 'off';
export type Motion = 'system' | 'full' | 'reduced' | 'off';
export type Density = 'comfortable' | 'compact';
export type FlagMode = 'applyOnLaunch' | 'keepApplied' | 'off';
export type UpdateChannel = 'stable' | 'beta';

export interface Appearance {
  theme: Theme;
  transparency: Transparency;
  motion: Motion;
  density: Density;
  scale: number;
  background: string | null;
  backgroundDim: number;
  backgroundBlur: number;
}

export interface Settings {
  schema: number;
  firstRunComplete: boolean;
  general: {
    startWithWindows: boolean;
    startMinimized: boolean;
    minimizeToTray: boolean;
    closeBehavior: CloseBehavior;
    launchRobloxOnStart: boolean;
    afterLaunch: AfterLaunch;
    whenRobloxCloses: WhenRobloxCloses;
    trayHintShown: boolean;
  };
  appearance: Appearance;
  performance: { monitoring: boolean; sampleMs: number; fpsCounter: boolean };
  roblox: {
    installDir: string | null;
    flagMode: FlagMode;
    applyFpsCap: boolean;
    verifyAfterLaunch: boolean;
    checkRobloxUpdates: boolean;
  };
  discord: { richPresence: boolean; showProfile: boolean; showRobloxStatus: boolean };
  updates: { autoCheck: boolean; channel: UpdateChannel };
  advanced: { logLevel: string };
  lastLaunch: { at: string; robloxVersion: string | null; profile: string; target: string; outcome: string } | null;
  originalFpsCap: number | null;
}

/** A deep-partial settings patch. */
export type SettingsPatch = { [K in keyof Settings]?: Settings[K] extends object ? Partial<Settings[K]> : Settings[K] };

// ---------------------------------------------------------------- profiles

export type FpsCap =
  { mode: 'robloxDefault' } | { mode: 'matchDisplay' } | { mode: 'fixed'; value: number } | { mode: 'unlimited' };
export type Priority = 'normal' | 'aboveNormal' | 'high';
export type BuiltinId = 'competitive' | 'balanced' | 'quality' | 'potato' | 'custom';

export interface Profile {
  id: string;
  name: string;
  description: string;
  builtin: BuiltinId | null;
  flags: FlagSet;
  fpsCap: FpsCap | null;
  priority: Priority;
  affinity: number | null;
  createdAt: string;
  updatedAt: string;
}

export interface ProfileStore {
  active: string;
  default: string;
  profiles: Profile[];
}

export interface ProfilePatch {
  name?: string;
  description?: string;
  flags?: FlagSet;
  fpsCap?: FpsCap | null;
  priority?: Priority;
  affinity?: number | null;
}

// ---------------------------------------------------------------- flags

export type Risk = 'low' | 'medium' | 'high';
export type Behavior = 'established' | 'uncertain';
export type InputSpec =
  | { kind: 'toggle' }
  | { kind: 'choice'; options: { value: string; label: string }[] }
  | { kind: 'number'; min: number; max: number; step: number }
  | { kind: 'text' };

export interface FlagInfo {
  name: string;
  title: string;
  summary: string;
  details: string;
  categories: string[];
  risk: Risk;
  behavior: Behavior;
  input: InputSpec;
  robloxDefault: { value: string; source: string; observed: string } | null;
  status: 'active' | 'removal-announced';
  conflicts: { flag: string; when: string; reason: string }[];
  requires: { flag: string; when: string; reason: string }[];
  references: string[];
  observations: { robloxVersion: string; result: string; date: string; method: string }[];
}

export interface Catalog {
  schema: number;
  reviewed: string;
  allowlist: { title: string; url: string; publisher: string; effective: string; summary: string };
  sources: Record<string, { label: string; url: string }>;
  flags: FlagInfo[];
  knownIgnored: { name: string; reason: string; replacement?: string | null }[];
}

export type Severity = 'info' | 'warning' | 'error';
export type Fix = { kind: 'set'; flag: string; value: string } | { kind: 'remove'; flag: string };
export interface Issue {
  flag: string;
  severity: Severity;
  code: string;
  message: string;
  fix: Fix | null;
}

export interface LaunchReport {
  robloxVersion: string | null;
  checkedAt: string;
  accepted: string[];
  denied: string[];
  notRead: string[];
  complete: boolean;
}

export interface FlagCheck {
  verdict: 'accepted' | 'denied' | 'notRead';
  robloxVersion: string | null;
  checkedAt: string;
}

export interface VerificationStore {
  flags: Record<string, FlagCheck>;
  lastReport: LaunchReport | null;
}

export interface PublishedValues {
  values: FlagSet;
  fetchedAt: string;
}

export interface InstalledFlags {
  file: string | null;
  flags: FlagSet | null;
  error: string | null;
  writtenByVesper: boolean;
}

// ---------------------------------------------------------------- roblox

export type InstallSource = 'official' | 'allUsers' | 'fishstrap' | 'bloxstrap' | 'custom';

export interface RobloxInstall {
  id: string;
  source: InstallSource;
  versionHash: string;
  version: string | null;
  location: string;
  registered: boolean;
  managedBy: string | null;
  modified: string | null;
  hasFlagsFile: boolean;
}

export interface Detection {
  installs: RobloxInstall[];
  preferred: string | null;
  protocol: { owner: string; command: string } | null;
  storeAppPresent: boolean;
  overrideProblem: string | null;
}

export interface LatestVersion {
  version: string;
  versionHash: string;
  fetchedAt: string;
}

export type LaunchTarget =
  | { kind: 'app' }
  | {
      kind: 'experience';
      placeId: number;
      gameInstanceId: string | null;
      linkCode: string | null;
      launchData: string | null;
    }
  | { kind: 'shareLink'; url: string };

export type ProblemAction =
  | { kind: 'openLogs' }
  | { kind: 'openConfigFolder' }
  | { kind: 'retry' }
  | { kind: 'openUrl'; value: string }
  | { kind: 'openSettings'; value: string };

export interface Problem {
  code: string;
  title: string;
  what: string;
  why: string;
  fix: string;
  details: string | null;
  actions: ProblemAction[];
}

export type LaunchStatus =
  | { state: 'idle' }
  | { state: 'preparing'; step: string }
  | { state: 'starting' }
  | { state: 'updating' }
  | { state: 'running'; pid: number }
  | { state: 'failed'; problem: Problem };

export interface FpsCapStatus {
  current: number | null;
  original: number | null;
  displayHz: number | null;
  error: string | null;
  locked: boolean;
}

// ---------------------------------------------------------------- performance

export interface SystemInfo {
  platform: 'windows' | 'macos' | 'linux';
  os: string;
  cpu: string;
  logicalProcessors: number;
  memoryTotal: number;
  gpus: { name: string; dedicatedMemory: number; vendorId: number }[];
  displayHz: number | null;
}

export interface FrameStats {
  fps: number;
  frameTimeAvgMs: number;
  frameTimeP99Ms: number;
  frames: number;
}

export type FpsStatus =
  { state: 'off' } | { state: 'measuring' } | { state: 'needsPermission' } | { state: 'unavailable'; detail: string };

export interface PerfSample {
  at: number;
  system: { cpuPercent: number | null; memoryUsed: number; memoryTotal: number; gpuPercent: number | null };
  roblox: {
    pid: number;
    processes: number;
    cpuPercent: number | null;
    workingSet: number;
    privateBytes: number;
    gpuPercent: number | null;
    gpuMemory: number | null;
    frames: FrameStats | null;
  } | null;
  gpuError: string | null;
  fpsStatus: FpsStatus;
}

// ---------------------------------------------------------------- discord, updates

export type PresenceStatus =
  | { state: 'disabled' }
  | { state: 'notConfigured' }
  | { state: 'connecting' }
  | { state: 'connected' }
  | { state: 'discordNotRunning' }
  | { state: 'rejected'; detail: string }
  | { state: 'error'; detail: string };

export type UpdateState =
  | { state: 'idle' }
  | { state: 'checking' }
  | { state: 'upToDate'; checkedAt: string }
  | { state: 'available'; version: string; notes: string | null; date: string | null }
  | { state: 'downloading'; downloaded: number; total: number | null }
  | { state: 'installing' }
  | { state: 'failed'; message: string };

// ---------------------------------------------------------------- backups, recovery

export type BackupReason = 'manual' | 'beforeRestore' | 'beforeImport' | 'beforeReset';
export interface BackupInfo {
  id: string;
  createdAt: string;
  reason: BackupReason;
  appVersion: string;
  profileCount: number;
  sizeBytes: number;
}
export interface RestoreOutcome {
  settings: Settings;
  profiles: ProfileStore;
  safetyBackup: BackupInfo;
}
export interface CleanupSummary {
  removed: number;
  keptForeign: number;
  alreadyGone: number;
}

// ---------------------------------------------------------------- customize

export interface WheelLook {
  dim: number;
  dividers: boolean;
}
export type ModSource = { kind: 'preset'; id: string } | { kind: 'file'; name: string };
export interface ModsConfig {
  cursor: { source: ModSource; scale: number; color: string | null } | null;
  emoteWheel: { source: ModSource | null; accent: string | null; look: WheelLook } | null;
  font: { name: string } | null;
  deathSound: { name: string } | null;
  customFiles: boolean;
}
export interface ModReport {
  written: number;
  unchanged: number;
  restored: number;
  missing: string[];
  skippedForeign: string[];
  errors: string[];
}
export interface ModsView {
  config: ModsConfig;
  cursorPresets: { id: string; label: string }[];
  wheelPresets: { id: string; label: string }[];
  customDir: string;
  customFileCount: number;
  cursorScale: [number, number];
  maxWheelDim: number;
  launcherFonts: string[];
  /** The name inside the chosen font file ("Poppins ExtraBold"). */
  fontName: string | null;
  lastReport: ModReport | null;
  robloxRunning: boolean;
  appliedNow: boolean;
  previews: {
    cursor: string | null;
    wheelBackground: string | null;
    wheelRing: string | null;
    wheelGradient: string | null;
    wheelLine: string | null;
    /** The chosen font file, for showing text in it. */
    font: string | null;
  };
}

// ---------------------------------------------------------------- bootstrap

export interface Bootstrap {
  version: string;
  portable: boolean;
  settings: Settings;
  profiles: ProfileStore;
  detection: Detection;
  catalog: Catalog;
  verification: VerificationStore;
  system: SystemInfo;
  presence: PresenceStatus;
  discordConfigured: boolean;
  update: UpdateState;
  launch: LaunchStatus;
  robloxRunning: boolean;
  latestRoblox: LatestVersion | null;
  published: PublishedValues | null;
  notices: string[];
}

export interface LogEntry {
  ts: string;
  level: string;
  area: string;
  msg: string;
}
