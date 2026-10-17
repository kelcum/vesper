// Typed wrappers for Vesper's backend commands.
// Every failure is thrown as a ProblemError carrying a user-facing Problem.

import { invoke } from '@tauri-apps/api/core';
import type {
  BackupInfo,
  Bootstrap,
  CleanupSummary,
  Detection,
  FlagSet,
  FpsCapStatus,
  InstalledFlags,
  Issue,
  LatestVersion,
  LaunchTarget,
  LogEntry,
  ModsView,
  Problem,
  Profile,
  ProfilePatch,
  ProfileStore,
  PublishedValues,
  RestoreOutcome,
  Settings,
  SettingsPatch,
  SystemInfo,
  UpdateState,
  Appearance,
  PresenceStatus,
  NowPlaying,
  Player,
  History,
  CityPing,
  Option,
  ShaderParams,
  ShaderStatus,
} from './types';

/** Physical pixels inside Vesper's window. */
export interface Bounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export class ProblemError extends Error {
  constructor(public problem: Problem) {
    super(problem.what || problem.title);
  }
}

export function toProblem(err: unknown): Problem {
  if (err instanceof ProblemError) return err.problem;
  if (err && typeof err === 'object' && 'title' in err && 'what' in err) return err as Problem;
  return {
    code: 'unexpected',
    title: 'Something went wrong',
    what: 'Vesper hit an unexpected error.',
    why: 'This is probably a bug.',
    fix: 'Try again. If it keeps happening, copy the details and send them through xan.gripe/k3.',
    details: String(err),
    actions: [{ kind: 'openLogs' }],
  };
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (err) {
    throw new ProblemError(toProblem(err));
  }
}

export const api = {
  bootstrap: () => call<Bootstrap>('bootstrap'),
  noticesTake: () => call<string[]>('notices_take'),
  appReady: () => call<void>('app_ready'),
  appHide: () => call<void>('app_hide'),
  appQuit: () => call<void>('app_quit'),

  settingsUpdate: (patch: SettingsPatch) => call<Settings>('settings_update', { patch }),
  firstRunFinish: (choices: { appearance: Appearance; profile: string; richPresence: boolean }) =>
    call<Bootstrap>('first_run_finish', { choices }),

  robloxDetect: () => call<Detection>('roblox_detect'),
  robloxParseTarget: (input: string) => call<LaunchTarget>('roblox_parse_target', { input }),
  robloxLaunch: (input?: string) => call<void>('roblox_launch', { input: input ?? null }),
  robloxDismissLaunch: () => call<void>('roblox_dismiss_launch'),
  robloxRestart: (input: string | null, rejoin: boolean) => call<void>('roblox_restart', { input, rejoin }),

  playingGet: () => call<{ now: NowPlaying | null; player: Player | null }>('playing_get'),
  historyGet: () => call<History>('history_get'),
  historyClear: () => call<History>('history_clear'),
  historyRemove: (jobId: string) => call<History>('history_remove', { jobId }),
  serversRegions: () => call<Option[]>('servers_regions'),
  serversScan: (input: string | null, refresh = false) => call<CityPing[]>('servers_scan', { input, refresh }),
  shadersInfo: () => call<{ status: ShaderStatus; presets: Option[] }>('shaders_info'),
  shadersPreset: (id: string) => call<ShaderParams>('shaders_preset', { id }),

  avatarOpen: (bounds: Bounds, page: 'editor' | 'bridge' | 'login' | null, visible: boolean) =>
    call<void>('avatar_open', { bounds, page, visible }),
  avatarBounds: (bounds: Bounds) => call<void>('avatar_bounds', { bounds }),
  avatarHide: () => call<void>('avatar_hide'),
  avatarClose: () => call<void>('avatar_close'),
  avatarReload: () => call<void>('avatar_reload'),
  avatarApi: <T = unknown>(method: 'GET' | 'POST' | 'PATCH' | 'DELETE', url: string, body?: unknown) =>
    call<T>('avatar_api', { method, url, body: body ?? null }),
  avatarSignOut: () => call<void>('avatar_sign_out'),
  robloxLatest: (refresh = false) => call<LatestVersion | null>('roblox_latest', { refresh }),

  flagsValidate: (flags: FlagSet) => call<Issue[]>('flags_validate', { flags }),
  flagsPublished: (refresh = false) => call<PublishedValues | null>('flags_published', { refresh }),
  flagsInstalled: () => call<InstalledFlags>('flags_installed'),
  flagsRemoveApplied: (force: boolean) => call<CleanupSummary>('flags_remove_applied', { force }),

  profilesGet: () => call<ProfileStore>('profiles_get'),
  profilesSetActive: (id: string) => call<ProfileStore>('profiles_set_active', { id }),
  profilesSetDefault: (id: string) => call<ProfileStore>('profiles_set_default', { id }),
  profilesCreate: (name: string, from?: string) => call<ProfileStore>('profiles_create', { name, from: from ?? null }),
  profilesDuplicate: (id: string) => call<ProfileStore>('profiles_duplicate', { id }),
  profilesUpdate: (id: string, patch: ProfilePatch) => call<ProfileStore>('profiles_update', { id, patch }),
  profilesDelete: (id: string) => call<ProfileStore>('profiles_delete', { id }),
  profilesReset: (id: string) => call<ProfileStore>('profiles_reset', { id }),
  profilesExport: (id: string, path: string) => call<void>('profiles_export', { id, path }),
  profilesImport: (path: string) =>
    call<{ store: ProfileStore; profile: Profile; issues: Issue[] }>('profiles_import', { path }),

  fpsCapStatus: () => call<FpsCapStatus>('fps_cap_status'),
  fpsCapRestore: () => call<FpsCapStatus>('fps_cap_restore'),

  perfWatch: (source: string, active: boolean) => call<void>('perf_watch', { source, active }),
  systemInfo: () => call<SystemInfo>('system_info'),

  backupsList: () => call<BackupInfo[]>('backups_list'),
  backupsCreate: () => call<BackupInfo>('backups_create'),
  backupsRestore: (id: string) => call<RestoreOutcome>('backups_restore', { id }),
  backupsDelete: (id: string) => call<void>('backups_delete', { id }),
  backupsExport: (id: string, path: string) => call<void>('backups_export', { id, path }),
  backupsImport: (path: string) => call<RestoreOutcome>('backups_import', { path }),
  recoveryResetSettings: () => call<RestoreOutcome>('recovery_reset_settings'),

  openFolder: (kind: 'config' | 'logs' | 'backups' | 'roblox' | 'robloxLogs') => call<void>('open_folder', { kind }),
  openUrl: (url: string) => call<void>('open_url', { url }),
  diagnosticsText: () => call<string>('diagnostics_text'),
  logsTail: (limit = 200) => call<LogEntry[]>('logs_tail', { limit }),

  discordStatus: () => call<{ status: PresenceStatus; configured: boolean }>('discord_status'),
  updatesCheck: () => call<UpdateState>('updates_check'),
  updatesInstall: () => call<void>('updates_install'),

  modsGet: () => call<ModsView>('mods_get'),
  modsSetCursor: (args: { preset?: string; path?: string; scale?: number; color?: string | null; reuse?: boolean }) =>
    call<ModsView>('mods_set_cursor', {
      preset: args.preset ?? null,
      path: args.path ?? null,
      scale: args.scale ?? null,
      color: args.color ?? null,
      reuse: args.reuse ?? null,
    }),
  modsSetWheel: (args: {
    preset?: string;
    path?: string;
    accent?: string | null;
    dim?: number;
    dividers?: boolean;
    reuse?: boolean;
  }) =>
    call<ModsView>('mods_set_wheel', {
      preset: args.preset ?? null,
      path: args.path ?? null,
      accent: args.accent ?? null,
      dim: args.dim ?? null,
      dividers: args.dividers ?? null,
      reuse: args.reuse ?? null,
    }),
  modsSetFont: (path: string) => call<ModsView>('mods_set_font', { path }),
  modsSetSound: (path: string) => call<ModsView>('mods_set_sound', { path }),
  modsSetSky: (args: { preset?: string; panorama?: string; faces?: string[] }) =>
    call<ModsView>('mods_set_sky', {
      preset: args.preset ?? null,
      panorama: args.panorama ?? null,
      faces: args.faces ?? null,
    }),
  modsUseLauncherFont: (launcher: string) => call<ModsView>('mods_use_launcher_font', { launcher }),
  modsSetCustom: (enabled: boolean) => call<ModsView>('mods_set_custom', { enabled }),
  modsClear: (kind: 'cursor' | 'emoteWheel' | 'font' | 'deathSound' | 'sky' | 'customFiles' | 'all') =>
    call<ModsView>('mods_clear', { kind }),
  modsOpenCustomFolder: () => call<void>('mods_open_custom_folder'),

  backgroundSet: (path: string) => call<Settings>('appearance_set_background', { path }),
  backgroundClear: () => call<Settings>('appearance_clear_background'),
  backgroundData: () => call<string | null>('appearance_background_data'),
};
