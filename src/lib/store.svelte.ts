// Global app state. Loaded once from `bootstrap`, then kept current by backend events.

import { listen } from '@tauri-apps/api/event';
import { api, toProblem } from './api';
import { applyAppearance } from './appearance';
import type {
  Bootstrap,
  Detection,
  LaunchReport,
  LaunchStatus,
  PerfSample,
  PresenceStatus,
  Problem,
  Profile,
  ProfileStore,
  Settings,
  SettingsPatch,
  UpdateState,
} from './types';

export type PageId = 'home' | 'profiles' | 'flags' | 'customize' | 'performance' | 'settings' | 'about';

export interface Toast {
  id: number;
  message: string;
  tone: 'ok' | 'info' | 'warn' | 'bad';
}

const PERF_HISTORY = 120;

class AppStore {
  ready = $state(false);
  bootError = $state<Problem | null>(null);
  boot = $state<Bootstrap | null>(null);

  settings = $state<Settings>(null as unknown as Settings);
  profiles = $state<ProfileStore>({ active: '', default: '', profiles: [] });
  detection = $state<Detection>({
    installs: [],
    preferred: null,
    protocol: null,
    storeAppPresent: false,
    overrideProblem: null,
  });
  launch = $state<LaunchStatus>({ state: 'idle' });
  robloxRunning = $state(false);
  presence = $state<PresenceStatus>({ state: 'disabled' });
  update = $state<UpdateState>({ state: 'idle' });
  perf = $state<PerfSample[]>([]);
  lastReport = $state<LaunchReport | null>(null);

  page = $state<PageId>('home');
  settingsSection = $state<string | null>(null);
  toasts = $state<Toast[]>([]);
  problem = $state<Problem | null>(null);
  closePrompt = $state(false);

  #toastId = 0;

  get activeProfile(): Profile | undefined {
    return this.profiles.profiles.find((p) => p.id === this.profiles.active);
  }

  get install() {
    return this.detection.installs.find((i) => i.id === this.detection.preferred) ?? null;
  }

  get latestPerf(): PerfSample | null {
    return this.perf.length ? this.perf[this.perf.length - 1] : null;
  }

  async init() {
    try {
      const b = await api.bootstrap();
      this.applyBoot(b);
      await this.subscribe();
      await applyAppearance(this.settings.appearance);
      this.ready = true;
      for (const n of b.notices) this.toast(n, 'info');
    } catch (err) {
      this.bootError = toProblem(err);
      this.ready = true;
    } finally {
      // Show the window only once the first frame is ready (no empty flash).
      requestAnimationFrame(() => api.appReady().catch(() => {}));
    }
  }

  applyBoot(b: Bootstrap) {
    this.boot = b;
    this.settings = b.settings;
    this.profiles = b.profiles;
    this.detection = b.detection;
    this.launch = b.launch;
    this.robloxRunning = b.robloxRunning;
    this.presence = b.presence;
    this.update = b.update;
    this.lastReport = b.verification.lastReport;
  }

  async subscribe() {
    await Promise.all([
      listen<{ running: boolean }>('roblox://running', (e) => (this.robloxRunning = e.payload.running)),
      listen<Detection>('roblox://detection', (e) => (this.detection = e.payload)),
      listen<LaunchStatus>('launch://status', (e) => this.onLaunch(e.payload)),
      listen<LaunchReport>('launch://verified', (e) => this.onVerified(e.payload)),
      listen<PerfSample>('perf://sample', (e) => {
        const next = this.perf.length >= PERF_HISTORY ? this.perf.slice(1) : this.perf.slice();
        next.push(e.payload);
        this.perf = next;
      }),
      listen<PresenceStatus>('discord://status', (e) => (this.presence = e.payload)),
      listen<UpdateState>('update://state', (e) => (this.update = e.payload)),
      listen<ProfileStore>('profiles://changed', (e) => (this.profiles = e.payload)),
      listen<Settings>('settings://changed', (e) => {
        this.settings = e.payload;
        applyAppearance(e.payload.appearance);
      }),
      listen('app://close-requested', () => this.onCloseRequested()),
      listen<string>('app://navigate', (e) => this.go(e.payload as PageId)),
    ]);
  }

  onLaunch(s: LaunchStatus) {
    const previous = this.launch.state;
    this.launch = s;
    if (s.state === 'running' && previous !== 'running') this.toast('Roblox is running', 'ok');
    if (s.state === 'running' || s.state === 'failed') {
      api
        .noticesTake()
        .then((ns) => ns.forEach((n) => this.toast(n, 'info')))
        .catch(() => {});
    }
  }

  onVerified(r: LaunchReport) {
    this.lastReport = r;
    if (this.boot) {
      const flags = { ...this.boot.verification.flags };
      for (const name of r.accepted)
        flags[name] = { verdict: 'accepted', robloxVersion: r.robloxVersion, checkedAt: r.checkedAt };
      for (const name of r.denied)
        flags[name] = { verdict: 'denied', robloxVersion: r.robloxVersion, checkedAt: r.checkedAt };
      this.boot.verification = { flags, lastReport: r };
    }
    if (r.denied.length) {
      this.toast(
        `Roblox denied ${r.denied.length} flag${r.denied.length === 1 ? '' : 's'}. See Fast Flags for details.`,
        'warn',
      );
    } else if (r.accepted.length) {
      this.toast(`Roblox accepted all ${r.accepted.length} flag${r.accepted.length === 1 ? '' : 's'}.`, 'ok');
    }
  }

  onCloseRequested() {
    if (this.settings.general.trayHintShown) api.appHide();
    else this.closePrompt = true;
  }

  go(page: PageId, section: string | null = null) {
    this.page = page;
    this.settingsSection = section;
  }

  toast(message: string, tone: Toast['tone'] = 'info') {
    const id = ++this.#toastId;
    this.toasts = [...this.toasts, { id, message, tone }].slice(-4);
    setTimeout(() => (this.toasts = this.toasts.filter((t) => t.id !== id)), tone === 'bad' ? 7000 : 4200);
  }

  /** Runs an action and shows its Problem if it fails. Returns undefined on failure. */
  async attempt<T>(fn: () => Promise<T>, success?: string): Promise<T | undefined> {
    try {
      const r = await fn();
      if (success) this.toast(success, 'ok');
      return r;
    } catch (err) {
      this.problem = toProblem(err);
      return undefined;
    }
  }

  async saveSettings(patch: SettingsPatch) {
    const s = await this.attempt(() => api.settingsUpdate(patch));
    if (s) this.settings = s;
    return s;
  }

  setProfiles(store: ProfileStore | undefined) {
    if (store) this.profiles = store;
  }
}

export const app = new AppStore();
