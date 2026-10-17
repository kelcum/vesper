import type { FpsCap, InstallSource, Priority } from './types';

export function bytes(n: number | null | undefined, digits = 1): string {
  if (n == null || !Number.isFinite(n) || n < 0) return '—';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let v = n;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(i === 0 ? 0 : digits)} ${units[i]}`;
}

export function percent(n: number | null | undefined, digits = 0): string {
  if (n == null || !Number.isFinite(n)) return '—';
  return `${n.toFixed(digits)}%`;
}

/** "just now", "5 min ago", "3 h ago", "2 days ago", or a date. */
export function relativeTime(iso: string | null | undefined, now: Date = new Date()): string {
  if (!iso) return 'never';
  const t = new Date(iso);
  if (Number.isNaN(t.getTime())) return 'unknown';
  const s = Math.round((now.getTime() - t.getTime()) / 1000);
  if (s < 45) return 'just now';
  const m = Math.round(s / 60);
  if (m < 60) return `${m} min ago`;
  const h = Math.round(m / 60);
  if (h < 24) return `${h} h ago`;
  const d = Math.round(h / 24);
  if (d < 7) return `${d} day${d === 1 ? '' : 's'} ago`;
  return t.toLocaleDateString(undefined, { day: 'numeric', month: 'short', year: 'numeric' });
}

export function shortDate(iso: string | null | undefined): string {
  if (!iso) return '—';
  const t = new Date(iso);
  if (Number.isNaN(t.getTime())) return '—';
  return t.toLocaleDateString(undefined, { day: 'numeric', month: 'short' });
}

export const FPS_PRESETS = [30, 60, 75, 120, 144, 165, 240, 360, 480] as const;

export function fpsCapLabel(cap: FpsCap | null | undefined, displayHz?: number | null): string {
  if (!cap) return "Roblox's setting";
  switch (cap.mode) {
    case 'robloxDefault':
      return 'Roblox default';
    case 'unlimited':
      return 'Unlimited';
    case 'matchDisplay':
      return displayHz ? `Match display (${displayHz} FPS)` : 'Match display';
    case 'fixed':
      return `${cap.value} FPS`;
  }
}

/** Short form for chips. */
export function fpsCapShort(cap: FpsCap | null | undefined, displayHz?: number | null): string {
  if (!cap) return 'FPS unchanged';
  if (cap.mode === 'matchDisplay') return displayHz ? `${displayHz} FPS` : 'Display FPS';
  return fpsCapLabel(cap, displayHz);
}

export function priorityLabel(p: Priority): string {
  return { normal: 'Normal priority', aboveNormal: 'Above-normal priority', high: 'High priority' }[p];
}

export function sourceLabel(s: InstallSource): string {
  return {
    official: 'Official install',
    allUsers: 'Roblox (all users)',
    fishstrap: 'Fishstrap copy',
    bloxstrap: 'Bloxstrap copy',
    custom: 'Chosen folder',
  }[s];
}

/** Compares Roblox versions like 0.741.0.7411058. Returns <0, 0 or >0. */
export function compareVersions(a: string, b: string): number {
  const pa = a.split(/[.,]/).map((x) => parseInt(x.trim(), 10) || 0);
  const pb = b.split(/[.,]/).map((x) => parseInt(x.trim(), 10) || 0);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const d = (pa[i] ?? 0) - (pb[i] ?? 0);
    if (d !== 0) return d;
  }
  return 0;
}

/** "0.741" from "0.741.0.7411058" for compact display. */
export function minorVersion(v: string | null | undefined): string {
  if (!v) return '?';
  const p = v.split('.');
  return p.length >= 2 ? `${p[0]}.${p[1]}` : v;
}

/** "1 h 12 min", "4 min", "less than a minute" */
export function duration(ms: number): string {
  const min = Math.floor(ms / 60000);
  if (min < 1) return 'less than a minute';
  if (min < 60) return `${min} min`;
  const h = Math.floor(min / 60);
  return min % 60 ? `${h} h ${min % 60} min` : `${h} h`;
}

/** "Private server", "Reserved server", or null for public servers. */
export function serverKindLabel(kind: 'public' | 'private' | 'reserved'): string | null {
  return kind === 'private' ? 'Private server' : kind === 'reserved' ? 'Reserved server' : null;
}
