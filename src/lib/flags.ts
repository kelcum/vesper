import type { Catalog, FlagCheck, FlagInfo, FlagSet, Issue } from './types';
import { computer } from './platform';

export const CATEGORY_LABELS: Record<string, string> = {
  rendering: 'Rendering',
  graphics: 'Graphics',
  performance: 'Performance',
  networking: 'Networking',
  physics: 'Physics',
  ui: 'UI',
  debug: 'Debug',
  experimental: 'Experimental',
  engine: 'Engine',
  misc: 'Miscellaneous',
};

export type Show = 'all' | 'set' | 'issues';

export interface FlagRow {
  name: string;
  info: FlagInfo | null;
  value: string | null;
  check: FlagCheck | null;
  issues: Issue[];
  custom: boolean;
  ignoredReason: string | null;
}

export function buildRows(
  catalog: Catalog,
  flags: FlagSet,
  checks: Record<string, FlagCheck>,
  issues: Issue[],
): FlagRow[] {
  const rows: FlagRow[] = catalog.flags.map((info) => ({
    name: info.name,
    info,
    value: flags[info.name] ?? null,
    check: checks[info.name] ?? null,
    issues: issues.filter((i) => i.flag === info.name),
    custom: false,
    ignoredReason: null,
  }));
  for (const [name, value] of Object.entries(flags)) {
    if (catalog.flags.some((f) => f.name === name)) continue;
    rows.push({
      name,
      info: null,
      value,
      check: checks[name] ?? null,
      issues: issues.filter((i) => i.flag === name),
      custom: true,
      ignoredReason: catalog.knownIgnored.find((k) => k.name === name)?.reason ?? null,
    });
  }
  return rows;
}

export function categoriesOf(row: FlagRow): string[] {
  return row.info?.categories ?? ['misc'];
}

export function filterRows(rows: FlagRow[], query: string, category: string | null, show: Show): FlagRow[] {
  const q = query.trim().toLowerCase();
  const words = q.split(/\s+/).filter(Boolean);
  return rows.filter((r) => {
    if (category && !categoriesOf(r).includes(category)) return false;
    if (show === 'set' && r.value === null) return false;
    if (show === 'issues' && r.issues.length === 0 && !r.ignoredReason && r.check?.verdict !== 'denied') return false;
    if (words.length === 0) return true;
    const hay = [r.name, r.info?.title, r.info?.summary, ...(r.info?.categories ?? [])]
      .filter(Boolean)
      .join(' ')
      .toLowerCase();
    return words.every((w) => hay.includes(w));
  });
}

export function categoryCounts(rows: FlagRow[]): Record<string, number> {
  const out: Record<string, number> = {};
  for (const key of Object.keys(CATEGORY_LABELS)) out[key] = 0;
  for (const r of rows) for (const c of categoriesOf(r)) out[c] = (out[c] ?? 0) + 1;
  return out;
}

export type Tone = 'ok' | 'warn' | 'bad' | 'info' | 'neutral';

export function verificationBadge(row: FlagRow): { label: string; tone: Tone; hint: string } {
  if (row.check?.verdict === 'denied') {
    return {
      label: 'Denied by Roblox',
      tone: 'bad',
      hint: `Roblox ${row.check.robloxVersion ?? ''} refused this flag at its last launch.`.replace('  ', ' '),
    };
  }
  if (row.check?.verdict === 'accepted') {
    return {
      label: `Accepted · ${shortVersion(row.check.robloxVersion)}`,
      tone: 'ok',
      hint: `Roblox ${row.check.robloxVersion ?? ''} read this flag and didn't deny it.`,
    };
  }
  if (row.ignoredReason) return { label: 'Ignored by Roblox', tone: 'bad', hint: row.ignoredReason };
  if (row.custom) {
    return {
      label: 'Not on allowlist',
      tone: 'warn',
      hint: "Roblox ignores local flags that aren't on its allowlist. Vesper will confirm after a launch.",
    };
  }
  const obs = row.info?.observations?.[0];
  if (obs) {
    return {
      label: `Allowlisted · seen ${shortVersion(obs.robloxVersion)}`,
      tone: 'info',
      hint: `On Roblox's allowlist. ${obs.method} (Roblox ${obs.robloxVersion}, ${obs.date}). Launch with it set to verify on your ${computer}.`,
    };
  }
  return {
    label: 'Allowlisted · not verified',
    tone: 'neutral',
    hint: `On Roblox's published allowlist. Not yet confirmed on your ${computer}; launch with it set to verify.`,
  };
}

function shortVersion(v: string | null | undefined): string {
  if (!v) return '?';
  const p = v.split('.');
  return p.length >= 2 ? `${p[0]}.${p[1]}` : v;
}

export function kindOf(name: string): 'bool' | 'int' | 'string' | null {
  const m = /^(?:DF|SF|F)(Flag|Int|Log|String)[A-Za-z0-9_]+$/.exec(name);
  if (!m) return null;
  return m[1] === 'Flag' ? 'bool' : m[1] === 'String' ? 'string' : 'int';
}

export function normalizeValue(name: string, raw: string): { value?: string; error?: string } {
  const kind = kindOf(name);
  if (!kind) return { error: 'Flag names start with FFlag, DFFlag, FInt, DFInt, FString, DFString, FLog or DFLog.' };
  const v = raw.trim();
  if (kind === 'bool') {
    if (/^true$/i.test(v)) return { value: 'True' };
    if (/^false$/i.test(v)) return { value: 'False' };
    return { error: 'Use True or False.' };
  }
  if (kind === 'int') {
    if (!/^-?\d+$/.test(v)) return { error: 'Use a whole number.' };
    const n = Number(v);
    if (n < -2147483648 || n > 2147483647) return { error: 'That number is too large.' };
    return { value: String(n) };
  }
  if (raw.length > 1024) return { error: 'Text values are at most 1024 characters.' };
  return { value: raw };
}

export function parseFlagJson(text: string): { flags?: FlagSet; error?: string } {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    return { error: "That isn't valid JSON." };
  }
  if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed))
    return { error: 'Expected a JSON object of flags.' };
  const flags: FlagSet = {};
  for (const [k, v] of Object.entries(parsed as Record<string, unknown>)) {
    if (typeof v === 'boolean') flags[k] = v ? 'True' : 'False';
    else if (typeof v === 'number' || typeof v === 'string') flags[k] = String(v);
    else return { error: `${k} has a value that isn't text, a number or true/false.` };
  }
  return { flags };
}
