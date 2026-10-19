const browser = typeof window !== 'undefined';
const forced = import.meta.env.DEV && browser ? new URLSearchParams(location.search).get('platform') : null;

export const isMac = forced ? forced === 'macos' : browser && /Macintosh|Mac OS X/.test(navigator.userAgent);

export const osName = isMac ? 'macOS' : 'Windows';

export const computer = isMac ? 'Mac' : 'PC';

export const trayName = isMac ? 'menu bar' : 'tray';

export const modKey = isMac ? '⌘' : 'Ctrl+';

if (browser) document.documentElement.dataset.platform = isMac ? 'macos' : 'windows';
