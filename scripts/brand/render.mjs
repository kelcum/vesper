// Renders every Vesper brand asset from scripts/brand/mark.mjs.
//
//   node scripts/brand/render.mjs
//
// Outputs (committed to the repo, so builds never need this script):
//   assets/branding/*            logo, icon, wordmark, social preview, presence art
//   src-tauri/icons/*            application + tray icons
//   src-tauri/installer/*.bmp    NSIS installer artwork
//   website/vesper/assets/brand/ favicon + Open Graph image
//
// The wordmark is converted to outlines with `usvg` (cargo install usvg)
// so the logo files never depend on an installed font.

import { Resvg } from '@resvg/resvg-js';
import { execFileSync } from 'node:child_process';
import { mkdirSync, readFileSync, writeFileSync, existsSync, rmSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { markBody, markSvg, detailFor } from './mark.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const fontsDir = join(root, 'assets/fonts');
const fontFiles = [
  'Geist-Regular.ttf',
  'Geist-Medium.ttf',
  'Geist-SemiBold.ttf',
  'GeistMono-Regular.ttf',
  'GeistMono-Medium.ttf',
].map((f) => join(fontsDir, f));

const out = (p) => {
  const full = join(root, p);
  mkdirSync(dirname(full), { recursive: true });
  return full;
};

const ink = { light: '#f2f0fa', dark: '#15131f', muted: '#b8b4cc' };

function png(svg, width, height) {
  const r = new Resvg(svg, {
    fitTo: height ? { mode: 'height', value: height } : { mode: 'width', value: width },
    font: { fontFiles, loadSystemFonts: false, defaultFontFamily: 'Geist' },
  });
  return r.render().asPng();
}

function rgba(svg, width) {
  const r = new Resvg(svg, {
    fitTo: { mode: 'width', value: width },
    font: { fontFiles, loadSystemFonts: false, defaultFontFamily: 'Geist' },
  });
  const img = r.render();
  return { w: img.width, h: img.height, px: img.pixels };
}

// ---------------------------------------------------------------- ICO / BMP

function ico(entries) {
  // entries: [{ size, png: Buffer }] – PNG-compressed entries (Vista+).
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(entries.length, 4);
  const dir = Buffer.alloc(16 * entries.length);
  let offset = 6 + dir.length;
  entries.forEach((e, i) => {
    const o = i * 16;
    dir.writeUInt8(e.size >= 256 ? 0 : e.size, o);
    dir.writeUInt8(e.size >= 256 ? 0 : e.size, o + 1);
    dir.writeUInt8(0, o + 2);
    dir.writeUInt8(0, o + 3);
    dir.writeUInt16LE(1, o + 4);
    dir.writeUInt16LE(32, o + 6);
    dir.writeUInt32LE(e.png.length, o + 8);
    dir.writeUInt32LE(offset, o + 12);
    offset += e.png.length;
  });
  return Buffer.concat([header, dir, ...entries.map((e) => e.png)]);
}

function bmp24({ w, h, px }, bg = [255, 255, 255]) {
  // Composites RGBA over `bg` and writes a bottom-up 24-bit BMP (NSIS needs BMP).
  const rowSize = Math.ceil((w * 3) / 4) * 4;
  const data = Buffer.alloc(rowSize * h);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const i = (y * w + x) * 4;
      const a = px[i + 3] / 255;
      const o = (h - 1 - y) * rowSize + x * 3;
      data[o] = Math.round(px[i + 2] * a + bg[2] * (1 - a));
      data[o + 1] = Math.round(px[i + 1] * a + bg[1] * (1 - a));
      data[o + 2] = Math.round(px[i] * a + bg[0] * (1 - a));
    }
  }
  const file = Buffer.alloc(54);
  file.write('BM', 0);
  file.writeUInt32LE(54 + data.length, 2);
  file.writeUInt32LE(54, 10);
  file.writeUInt32LE(40, 14);
  file.writeInt32LE(w, 18);
  file.writeInt32LE(h, 22);
  file.writeUInt16LE(1, 26);
  file.writeUInt16LE(24, 28);
  file.writeUInt32LE(data.length, 34);
  file.writeInt32LE(2835, 38);
  file.writeInt32LE(2835, 42);
  return Buffer.concat([file, data]);
}

// ---------------------------------------------------------------- wordmark

function hasUsvg() {
  try {
    execFileSync('usvg', ['--version'], { stdio: 'ignore' });
    return true;
  } catch {
    return false;
  }
}

/** Converts a text-bearing SVG to outlines with usvg. */
function outline(svg) {
  const tmpIn = out('.brand-tmp/in.svg');
  const tmpOut = out('.brand-tmp/out.svg');
  writeFileSync(tmpIn, svg);
  execFileSync('usvg', [
    '--skip-system-fonts',
    '--use-fonts-dir',
    fontsDir,
    '--text-rendering',
    'geometricPrecision',
    tmpIn,
    tmpOut,
  ]);
  const result = readFileSync(tmpOut, 'utf8');
  rmSync(join(root, '.brand-tmp'), { recursive: true, force: true });
  return result;
}

// Lockup metrics (in px of the source canvas).
const WM = {
  fontSize: 200,
  baseline: 196,
  tracking: -7,
  markR: 92,
  gap: 46,
};

function wordmarkSource(color) {
  const markCx = WM.markR + 8;
  // Centre the mark on the x-height band of the lowercase word.
  const xHeight = 0.53 * WM.fontSize;
  const markCy = WM.baseline - xHeight / 2;
  const scale = (WM.markR * 2) / 1024;
  const textX = markCx + WM.markR + WM.gap;
  return `<svg xmlns="http://www.w3.org/2000/svg" width="980" height="300" viewBox="0 0 980 300">
  <g transform="translate(${markCx - WM.markR} ${markCy - WM.markR}) scale(${scale})">
    ${markBody({ detail: 'medium', container: 'circle', id: 'wm' })}
  </g>
  <text x="${textX}" y="${WM.baseline}" font-family="Geist" font-weight="600"
    font-size="${WM.fontSize}" letter-spacing="${WM.tracking}" fill="${color}">vesper</text>
</svg>`;
}

/**
 * Crops an SVG to its visible pixels. Geometric bounding boxes include
 * clipped-away shapes (the full planet circle), so scan rendered alpha instead.
 */
function cropToContent(svg, pad = 6) {
  const img = new Resvg(svg, { font: { fontFiles, loadSystemFonts: false } }).render();
  const { width: w, height: h, pixels } = img;
  let x0 = w;
  let y0 = h;
  let x1 = -1;
  let y1 = -1;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      if (pixels[(y * w + x) * 4 + 3] > 2) {
        if (x < x0) x0 = x;
        if (x > x1) x1 = x;
        if (y < y0) y0 = y;
        if (y > y1) y1 = y;
      }
    }
  }
  if (x1 < 0) return svg;
  const vx = x0 - pad;
  const vy = y0 - pad;
  const vw = x1 - x0 + 1 + pad * 2;
  const vh = y1 - y0 + 1 + pad * 2;
  return svg.replace(
    /<svg[^>]*>/,
    `<svg xmlns="http://www.w3.org/2000/svg" width="${vw}" height="${vh}" viewBox="${vx} ${vy} ${vw} ${vh}">`,
  );
}

// ---------------------------------------------------------------- scenes

function socialPreview() {
  // 1280×640 – GitHub social preview and website Open Graph image.
  const W = 1280;
  const H = 640;
  const planetCy = 1980;
  const planetR = 1500;
  const horizon = planetCy - planetR;
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">
  <defs>
    <linearGradient id="sky" x1="0" y1="0" x2="0" y2="${horizon}" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#0a0915"/>
      <stop offset="1" stop-color="#1f1a42"/>
    </linearGradient>
    <radialGradient id="limb" cx="${W * 0.62}" cy="${horizon}" r="900" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#f1edff"/>
      <stop offset="0.35" stop-color="#a596ff"/>
      <stop offset="0.85" stop-color="#5d7dff" stop-opacity="0.35"/>
      <stop offset="1" stop-color="#5d7dff" stop-opacity="0.1"/>
    </radialGradient>
    <radialGradient id="atmo" cx="${W * 0.62}" cy="${horizon}" r="760" gradientUnits="userSpaceOnUse"
      gradientTransform="translate(0 ${horizon}) scale(1 0.34) translate(0 ${-horizon})">
      <stop offset="0" stop-color="#8f7dff" stop-opacity="0.45"/>
      <stop offset="1" stop-color="#6b72ff" stop-opacity="0"/>
    </radialGradient>
    <radialGradient id="halo" cx="1032" cy="168" r="120" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#c9bdff" stop-opacity="0.6"/>
      <stop offset="0.25" stop-color="#c9bdff" stop-opacity="0.16"/>
      <stop offset="1" stop-color="#c9bdff" stop-opacity="0"/>
    </radialGradient>
    <mask id="band" maskUnits="userSpaceOnUse" x="0" y="0" width="${W}" height="${H}">
      <circle cx="${W * 0.62}" cy="${planetCy}" r="${planetR}" fill="#fff"/>
      <circle cx="${W * 0.62}" cy="${planetCy + 520}" r="${planetR + 470}" fill="#000"/>
    </mask>
  </defs>
  <rect width="${W}" height="${H}" fill="url(#sky)"/>
  <rect width="${W}" height="${H}" fill="url(#atmo)"/>
  <circle cx="${W * 0.62}" cy="${planetCy}" r="${planetR}" fill="#08070f"/>
  <rect width="${W}" height="${H}" fill="url(#limb)" mask="url(#band)"/>
  <circle cx="1032" cy="168" r="120" fill="url(#halo)"/>
  <circle cx="1032" cy="168" r="12" fill="#f7f5ff"/>

  <g transform="translate(96 104) scale(${72 / 1024})">
    ${markBody({ detail: 'medium', container: 'squircle', id: 'sp' })}
  </g>
  <text x="188" y="152" font-family="Geist" font-weight="600" font-size="46" letter-spacing="-1.4" fill="${ink.light}">vesper</text>

  <text x="96" y="296" font-family="Geist" font-weight="600" font-size="76" letter-spacing="-2.6" fill="${ink.light}">Roblox, tuned</text>
  <text x="96" y="382" font-family="Geist" font-weight="600" font-size="76" letter-spacing="-2.6" fill="${ink.light}">and verified.</text>
  <text x="98" y="444" font-family="Geist" font-weight="400" font-size="27" letter-spacing="-0.2" fill="${ink.muted}">A fast, private Roblox launcher for Windows.</text>
  <text x="98" y="560" font-family="Geist Mono" font-weight="500" font-size="22" fill="#a596ff">xan.gripe/vesper</text>
</svg>`;
}

function installerSidebar() {
  // 164×314 welcome/finish panel. Rendered at 2× then downsampled by resvg.
  const W = 164;
  const H = 314;
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">
  <defs>
    <linearGradient id="sky" x1="0" y1="0" x2="0" y2="${H}" gradientUnits="userSpaceOnUse">
      <stop offset="0" stop-color="#0b0a17"/>
      <stop offset="0.72" stop-color="#211c45"/>
      <stop offset="1" stop-color="#0b0a17"/>
    </linearGradient>
  </defs>
  <rect width="${W}" height="${H}" fill="url(#sky)"/>
  <g transform="translate(34 64) scale(${96 / 1024})">
    ${markBody({ detail: 'medium', container: 'squircle', id: 'sb' })}
  </g>
  <text x="34" y="200" font-family="Geist" font-weight="600" font-size="30" letter-spacing="-0.9" fill="${ink.light}">vesper</text>
  <text x="35" y="224" font-family="Geist" font-weight="400" font-size="11.5" fill="${ink.muted}">Roblox, tuned and verified.</text>
</svg>`;
}

function installerHeader() {
  // 150×57 header bitmap. NSIS draws it on a white header strip.
  return `<svg xmlns="http://www.w3.org/2000/svg" width="150" height="57" viewBox="0 0 150 57">
  <rect width="150" height="57" fill="#ffffff"/>
  <g transform="translate(103 10.5) scale(${36 / 1024})">
    ${markBody({ detail: 'medium', container: 'squircle', id: 'hd' })}
  </g>
</svg>`;
}

// ---------------------------------------------------------------- main

function main() {
  const brand = 'assets/branding';

  // Vector masters
  writeFileSync(out(`${brand}/logo.svg`), markSvg({ detail: 'full', container: 'circle' }));
  writeFileSync(out(`${brand}/icon.svg`), markSvg({ detail: 'full', container: 'squircle' }));
  writeFileSync(out(`${brand}/icon-small.svg`), markSvg({ detail: 'small', container: 'squircle' }));
  writeFileSync(out(`${brand}/mark-transparent.svg`), markSvg({ detail: 'full', container: 'none' }));

  // Raster masters
  writeFileSync(out(`${brand}/logo.png`), png(markSvg({ detail: 'full', container: 'circle' }), 1024));
  writeFileSync(out(`${brand}/icon.png`), png(markSvg({ detail: 'full', container: 'squircle' }), 1024));
  writeFileSync(out(`${brand}/avatar.png`), png(markSvg({ detail: 'medium', container: 'square' }), 1024));
  writeFileSync(out(`${brand}/presence.png`), png(markSvg({ detail: 'medium', container: 'square' }), 1024));

  // Multi-resolution ICO with optical sizes
  const icoSizes = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256];
  const icoEntries = icoSizes.map((size) => ({
    size,
    png: png(markSvg({ detail: detailFor(size), container: 'squircle' }), size),
  }));
  const icoBuf = ico(icoEntries);
  writeFileSync(out(`${brand}/icon.ico`), icoBuf);

  // Tauri icons
  writeFileSync(out('src-tauri/icons/icon.ico'), icoBuf);
  writeFileSync(out('src-tauri/icons/32x32.png'), png(markSvg({ detail: 'small', container: 'squircle' }), 32));
  writeFileSync(out('src-tauri/icons/128x128.png'), png(markSvg({ detail: 'full', container: 'squircle' }), 128));
  writeFileSync(out('src-tauri/icons/128x128@2x.png'), png(markSvg({ detail: 'full', container: 'squircle' }), 256));
  writeFileSync(out('src-tauri/icons/icon.png'), png(markSvg({ detail: 'full', container: 'squircle' }), 512));
  writeFileSync(out('src-tauri/icons/tray.png'), png(markSvg({ detail: 'small', container: 'squircle' }), 32));

  // Installer artwork
  writeFileSync(out('src-tauri/installer/sidebar.bmp'), bmp24(rgba(installerSidebar(), 164), [11, 10, 23]));
  writeFileSync(out('src-tauri/installer/header.bmp'), bmp24(rgba(installerHeader(), 150)));

  // Social preview
  const social = png(socialPreview(), 1280);
  writeFileSync(out(`${brand}/social-preview.png`), social);

  // Wordmark lockups (outlined text)
  if (hasUsvg()) {
    for (const [name, color] of [
      ['wordmark.svg', ink.light],
      ['wordmark-dark.svg', ink.dark],
    ]) {
      const outlined = cropToContent(outline(wordmarkSource(color)));
      writeFileSync(out(`${brand}/${name}`), outlined);
    }
    writeFileSync(out(`${brand}/wordmark.png`), png(readFileSync(out(`${brand}/wordmark.svg`), 'utf8'), 1200));
  } else if (!existsSync(out(`${brand}/wordmark.svg`))) {
    throw new Error('usvg not found: install it with `cargo install usvg` to build the wordmark.');
  } else {
    console.warn('usvg not found – keeping existing wordmark files.');
  }

  // Website brand files
  const site = 'website/vesper/assets/brand';
  writeFileSync(out(`${site}/favicon.svg`), markSvg({ detail: 'medium', container: 'squircle' }));
  writeFileSync(out(`${site}/favicon-32.png`), png(markSvg({ detail: 'small', container: 'squircle' }), 32));
  writeFileSync(out(`${site}/apple-touch-icon.png`), png(markSvg({ detail: 'full', container: 'square' }), 180));
  writeFileSync(out(`${site}/og.png`), social);
  writeFileSync(out(`${site}/presence.png`), png(markSvg({ detail: 'medium', container: 'square' }), 1024));
  writeFileSync(out(`${site}/logo.svg`), markSvg({ detail: 'full', container: 'circle' }));

  console.log('Brand assets rendered.');
}

main();
