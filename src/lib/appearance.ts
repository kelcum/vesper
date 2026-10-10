// Applies appearance settings: theme, Mica, motion, density, UI scale, background.

import { getCurrentWebview } from '@tauri-apps/api/webview';
import { Effect, EffectState, getCurrentWindow } from '@tauri-apps/api/window';
import { api } from './api';
import type { Appearance } from './types';

const dark = window.matchMedia('(prefers-color-scheme: dark)');
const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)');
let current: Appearance | null = null;
let backgroundKey: string | null | undefined;

function resolvedTheme(a: Appearance): 'dark' | 'light' {
  if (a.theme === 'system') return dark.matches ? 'dark' : 'light';
  return a.theme;
}

function resolvedMotion(a: Appearance): 'full' | 'reduced' | 'off' {
  if (a.motion === 'system') return reducedMotion.matches ? 'reduced' : 'full';
  return a.motion;
}

export async function applyAppearance(a: Appearance) {
  current = a;
  const root = document.documentElement;
  const theme = resolvedTheme(a);
  root.dataset.theme = theme;
  root.dataset.motion = resolvedMotion(a);
  root.dataset.density = a.density;
  root.dataset.transparency = a.transparency;
  root.style.setProperty('--bg-dim', String(a.backgroundDim / 100));
  root.style.setProperty('--bg-blur', `${a.backgroundBlur}px`);

  const win = getCurrentWindow();
  try {
    await win.setTheme(a.theme === 'system' ? null : theme);
    if (a.transparency === 'full') {
      await win.setEffects({ effects: [Effect.Mica], state: EffectState.Active });
    } else {
      await win.clearEffects();
    }
  } catch {
    // Window effects need Windows 11; the CSS fallback covers older systems.
  }
  try {
    await getCurrentWebview().setZoom(a.scale / 100);
  } catch {
    /* zoom is a nicety */
  }

  if (backgroundKey !== a.background) {
    backgroundKey = a.background;
    const url = a.background ? await api.backgroundData().catch(() => null) : null;
    if (url) root.style.setProperty('--custom-bg', `url("${url}")`);
    else root.style.removeProperty('--custom-bg');
    root.dataset.customBg = url ? 'on' : 'off';
  }
}

/** Forces the background to reload (after choosing a new image with the same name). */
export function invalidateBackground() {
  backgroundKey = undefined;
}

for (const mq of [dark, reducedMotion]) {
  mq.addEventListener('change', () => {
    if (current) applyAppearance(current);
  });
}
