import { describe, expect, it } from 'vitest';
import catalogJson from '../src-tauri/resources/flags/catalog.json';
import {
  buildRows,
  categoryCounts,
  filterRows,
  kindOf,
  normalizeValue,
  parseFlagJson,
  verificationBadge,
} from '../src/lib/flags';
import { bytes, compareVersions, fpsCapLabel, minorVersion, percent, relativeTime } from '../src/lib/format';
import type { Catalog } from '../src/lib/types';

const catalog = catalogJson as unknown as Catalog;

describe('format', () => {
  it('formats bytes and percentages', () => {
    expect(bytes(0)).toBe('0 B');
    expect(bytes(1536)).toBe('1.5 KB');
    expect(bytes(16 * 1024 ** 3)).toBe('16.0 GB');
    expect(bytes(null)).toBe('—');
    expect(percent(12.345, 1)).toBe('12.3%');
    expect(percent(undefined)).toBe('—');
  });

  it('describes relative times', () => {
    const now = new Date('2026-10-10T12:00:00Z');
    expect(relativeTime('2026-10-10T11:59:50Z', now)).toBe('just now');
    expect(relativeTime('2026-10-10T11:50:00Z', now)).toBe('10 min ago');
    expect(relativeTime('2026-10-10T09:00:00Z', now)).toBe('3 h ago');
    expect(relativeTime('2026-10-08T12:00:00Z', now)).toBe('2 days ago');
    expect(relativeTime(null, now)).toBe('never');
  });

  it('labels frame-rate caps without promising frame rates', () => {
    expect(fpsCapLabel(null)).toBe("Roblox's setting");
    expect(fpsCapLabel({ mode: 'matchDisplay' }, 144)).toBe('Match display (144 FPS)');
    expect(fpsCapLabel({ mode: 'fixed', value: 240 })).toBe('240 FPS');
    expect(fpsCapLabel({ mode: 'unlimited' })).toBe('Unlimited');
  });

  it('compares Roblox versions numerically', () => {
    expect(compareVersions('0.741.0.7411058', '0.740.0.7400927')).toBeGreaterThan(0);
    expect(compareVersions('0.99.0.1', '0.741.0.1')).toBeLessThan(0);
    expect(compareVersions('0.741.0.1', '0.741.0.1')).toBe(0);
    expect(minorVersion('0.742.0.7421053')).toBe('0.742');
  });
});

describe('flag rows', () => {
  const rows = buildRows(
    catalog,
    { FIntDebugForceMSAASamples: '4', DFIntTaskSchedulerTargetFps: '240', FFlagMyCustom: 'True' },
    {
      FFlagHandleAltEnterFullscreenManually: {
        verdict: 'accepted',
        robloxVersion: '0.742.0.7421053',
        checkedAt: '2026-10-10T00:00:00Z',
      },
    },
    [],
  );

  it('includes every catalog flag plus custom ones', () => {
    expect(rows.filter((r) => !r.custom)).toHaveLength(catalog.flags.length);
    expect(rows.filter((r) => r.custom).map((r) => r.name)).toEqual(['DFIntTaskSchedulerTargetFps', 'FFlagMyCustom']);
  });

  it('flags known-ignored entries with their reason', () => {
    const fps = rows.find((r) => r.name === 'DFIntTaskSchedulerTargetFps')!;
    expect(fps.ignoredReason).toMatch(/allowlist/);
    expect(verificationBadge(fps).tone).toBe('bad');
  });

  it('never claims verification it does not have', () => {
    const msaa = rows.find((r) => r.name === 'FIntDebugForceMSAASamples')!;
    expect(verificationBadge(msaa).label).toBe('Allowlisted · not verified');
    const alt = rows.find((r) => r.name === 'FFlagHandleAltEnterFullscreenManually')!;
    expect(verificationBadge(alt)).toMatchObject({ tone: 'ok', label: 'Accepted · 0.742' });
    const custom = rows.find((r) => r.name === 'FFlagMyCustom')!;
    expect(verificationBadge(custom).label).toBe('Not on allowlist');
  });

  it('searches titles, names and categories with every word', () => {
    expect(filterRows(rows, 'anti aliasing', null, 'all').map((r) => r.name)).toEqual(['FIntDebugForceMSAASamples']);
    expect(filterRows(rows, 'vulkan', null, 'all').map((r) => r.name)).toEqual(['FFlagDebugGraphicsPreferVulkan']);
    expect(filterRows(rows, '', 'engine', 'all').length).toBe(3);
    expect(
      filterRows(rows, '', null, 'set')
        .map((r) => r.name)
        .sort(),
    ).toEqual(['DFIntTaskSchedulerTargetFps', 'FFlagMyCustom', 'FIntDebugForceMSAASamples'].sort());
  });

  it('counts categories including empty ones', () => {
    const c = categoryCounts(rows);
    expect(c.networking).toBe(0);
    expect(c.physics).toBe(0);
    expect(c.rendering).toBeGreaterThan(0);
  });
});

describe('flag values', () => {
  it('infers types from prefixes like Roblox does', () => {
    expect(kindOf('FFlagDebugSkyGray')).toBe('bool');
    expect(kindOf('DFIntTextureQualityOverride')).toBe('int');
    expect(kindOf('FStringX')).toBe('string');
    expect(kindOf('Nope')).toBeNull();
  });

  it('normalises values', () => {
    expect(normalizeValue('FFlagX', ' true ')).toEqual({ value: 'True' });
    expect(normalizeValue('FIntX', '042')).toEqual({ value: '42' });
    expect(normalizeValue('FIntX', '1.5').error).toBeTruthy();
    expect(normalizeValue('Bad', '1').error).toBeTruthy();
  });

  it('parses pasted flag JSON', () => {
    expect(parseFlagJson('{"FFlagA": true, "FIntB": 3}')).toEqual({ flags: { FFlagA: 'True', FIntB: '3' } });
    expect(parseFlagJson('[1]').error).toBeTruthy();
    expect(parseFlagJson('{oops').error).toBeTruthy();
  });
});
