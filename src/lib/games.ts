import type { Settings, Visit } from './types';

export function gameKey(placeId: number, universeId: number | null | undefined): string {
  return universeId ? `universe:${universeId}` : `place:${placeId}`;
}

export function profileForGame(
  settings: Pick<Settings, 'games'>,
  placeId: number,
  universeId: number | null | undefined,
): string | null {
  const byUniverse = universeId ? settings.games.profiles[`universe:${universeId}`] : undefined;
  return byUniverse ?? settings.games.profiles[`place:${placeId}`] ?? null;
}

export interface GameTotal {
  key: string;
  placeId: number;
  universeId: number | null;
  name: string | null;
  creator: string | null;
  iconUrl: string | null;
  totalMs: number;
  visits: number;
  lastJoinedAt: string;
}

export function totalsByGame(visits: Visit[], now: Date = new Date()): GameTotal[] {
  const totals = new Map<string, GameTotal>();
  for (const v of visits) {
    const key = v.universeId ? `universe:${v.universeId}` : `place:${v.placeId}`;
    const end = v.leftAt ? new Date(v.leftAt).getTime() : now.getTime();
    const ms = Math.max(0, end - new Date(v.joinedAt).getTime());
    const t = totals.get(key);
    if (t) {
      t.totalMs += ms;
      t.visits += 1;
      if (v.joinedAt > t.lastJoinedAt) {
        t.lastJoinedAt = v.joinedAt;
        t.name = v.name ?? t.name;
        t.creator = v.creator ?? t.creator;
        t.iconUrl = v.iconUrl ?? t.iconUrl;
      }
    } else {
      totals.set(key, {
        key,
        placeId: v.placeId,
        universeId: v.universeId,
        name: v.name,
        creator: v.creator,
        iconUrl: v.iconUrl,
        totalMs: ms,
        visits: 1,
        lastJoinedAt: v.joinedAt,
      });
    }
  }
  return [...totals.values()].sort((a, b) => b.totalMs - a.totalMs);
}

export function matchesQuery(v: Visit, query: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  return [v.name, v.creator, v.location, String(v.placeId)].some((f) => f?.toLowerCase().includes(q));
}

export function hotkeyLabel(hotkey: string | null | undefined): string {
  if (!hotkey) return 'None';
  return hotkey
    .split('+')
    .map((part) => {
      if (part.startsWith('Key') && part.length === 4) return part.slice(3);
      if (part.startsWith('Digit') && part.length === 6) return part.slice(5);
      if (part === 'CmdOrCtrl' || part === 'CommandOrControl') return 'Ctrl';
      if (part === 'Super') return 'Win';
      return part;
    })
    .join(' + ');
}
