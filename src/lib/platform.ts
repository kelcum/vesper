// Which system Vesper runs on, for window chrome and wording.
//
// Known before the backend answers (the title bar and startup errors need it):
// WebView2 on Windows reports "Windows" in its user agent, WebKit on macOS
// "Macintosh". The preview (dev/preview) can show macOS with ?platform=macos.

const browser = typeof window !== 'undefined';
const forced = import.meta.env.DEV && browser ? new URLSearchParams(location.search).get('platform') : null;

export const isMac = forced ? forced === 'macos' : browser && /Macintosh|Mac OS X/.test(navigator.userAgent);

/** The system's name, for sentences like "Live numbers from macOS". */
export const osName = isMac ? 'macOS' : 'Windows';

/** What people call this computer: "this Mac" or "this PC". */
export const computer = isMac ? 'Mac' : 'PC';

/** Where Vesper's icon lives while its window is closed. */
export const trayName = isMac ? 'menu bar' : 'tray';

/** The modifier key for keyboard shortcuts. */
export const modKey = isMac ? '⌘' : 'Ctrl+';

if (browser) document.documentElement.dataset.platform = isMac ? 'macos' : 'windows';
