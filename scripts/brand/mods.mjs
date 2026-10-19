import { Resvg } from '@resvg/resvg-js';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const out = (p) => {
  const full = join(root, 'src-tauri/resources/mods', p);
  mkdirSync(dirname(full), { recursive: true });
  return full;
};
const render = (svg, w) => new Resvg(svg, { fitTo: { mode: 'width', value: w } }).render().asPng();

const cursors = {
  dot: `
    <circle cx="32" cy="32" r="10" fill="none" stroke="#c9bdff" stroke-opacity="0.42" stroke-width="1.6"/>
    <circle cx="32" cy="32" r="6.2" fill="#0a0914" fill-opacity="0.9"/>
    <circle cx="32" cy="32" r="4.4" fill="#f1edff"/>`,
  crosshair: (() => {
    const arms = [
      [32, 20, 32, 28],
      [32, 36, 32, 44],
      [20, 32, 28, 32],
      [36, 32, 44, 32],
    ];
    const lines = (stroke, width) =>
      arms
        .map(
          ([x1, y1, x2, y2]) =>
            `<line x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}" stroke="${stroke}" stroke-width="${width}" stroke-linecap="round"/>`,
        )
        .join('');
    return `${lines('#0a0914', 4.6)}${lines('#ffffff', 2.4)}
      <circle cx="32" cy="32" r="2.4" fill="#0a0914"/><circle cx="32" cy="32" r="1.4" fill="#c9bdff"/>`;
  })(),
  ring: `
    <circle cx="32" cy="32" r="9" fill="none" stroke="#0a0914" stroke-opacity="0.85" stroke-width="4.6"/>
    <circle cx="32" cy="32" r="9" fill="none" stroke="#ffffff" stroke-width="2.2"/>
    <circle cx="32" cy="32" r="2.2" fill="#0a0914"/><circle cx="32" cy="32" r="1.3" fill="#ffffff"/>`,
  plus: `
    <path d="M32 21 V43 M21 32 H43" stroke="#0a0914" stroke-width="5.4" stroke-linecap="round"/>
    <path d="M32 21 V43 M21 32 H43" stroke="#ffffff" stroke-width="2.6" stroke-linecap="round"/>`,
  arrow: `<path transform="translate(32 32) scale(1.15)"
      d="M0 0 L0 17.2 L4.3 13.3 L7.3 20.1 L10.2 18.8 L7.2 12.2 L12.8 12.2 Z"
      fill="#ffffff" stroke="#0a0914" stroke-width="1.7" stroke-linejoin="round" paint-order="stroke"/>`,
};

for (const [id, body] of Object.entries(cursors)) {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 64 64">${body}</svg>`;
  writeFileSync(out(`cursors/${id}.png`), render(svg, 64));
}

const S = 1920;
const C = S / 2;
const R = S / 2;
const INNER = 0.465 * R;

function dividers({ color, inner, outer, width, startOpacity }) {
  let defs = '';
  let lines = '';
  for (let k = 0; k < 8; k++) {
    const a = ((22.5 + 45 * k) * Math.PI) / 180;
    const x1 = C + Math.sin(a) * inner;
    const y1 = C - Math.cos(a) * inner;
    const x2 = C + Math.sin(a) * outer;
    const y2 = C - Math.cos(a) * outer;
    defs += `<linearGradient id="d${k}" gradientUnits="userSpaceOnUse" x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}">
      <stop offset="0" stop-color="${color}" stop-opacity="${startOpacity}"/>
      <stop offset="1" stop-color="${color}" stop-opacity="0"/></linearGradient>`;
    lines += `<line x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}" stroke="url(#d${k})" stroke-width="${width}" stroke-linecap="round"/>`;
  }
  return { defs, lines };
}

const wheels = {
  dusk: {
    background: `
      <defs><radialGradient id="bg" cx="${C}" cy="${C}" r="${R}" gradientUnits="userSpaceOnUse">
        <stop offset="0" stop-color="#1c1838" stop-opacity="0.8"/>
        <stop offset="1" stop-color="#100e20" stop-opacity="0.88"/></radialGradient></defs>
      <circle cx="${C}" cy="${C}" r="${R * 0.995}" fill="url(#bg)"/>
      <circle cx="${C}" cy="${C}" r="${R * 0.99}" fill="none" stroke="#a596ff" stroke-opacity="0.38" stroke-width="${R * 0.006}"/>`,
    ring: () => {
      const d = dividers({
        color: '#c9bdff',
        inner: INNER + R * 0.015,
        outer: R * 0.97,
        width: R * 0.0062,
        startOpacity: 0.85,
      });
      return `<defs>${d.defs}</defs>${d.lines}
        <circle cx="${C}" cy="${C}" r="${INNER}" fill="#0d0c1a"/>
        <circle cx="${C}" cy="${C}" r="${INNER}" fill="none" stroke="#a596ff" stroke-opacity="0.6" stroke-width="${R * 0.006}"/>`;
    },
  },
  glass: {
    background: `
      <defs><linearGradient id="bg" x1="0" y1="0" x2="0" y2="${S}" gradientUnits="userSpaceOnUse">
        <stop offset="0" stop-color="#ffffff" stop-opacity="0.2"/>
        <stop offset="1" stop-color="#ffffff" stop-opacity="0.09"/></linearGradient></defs>
      <circle cx="${C}" cy="${C}" r="${R * 0.995}" fill="url(#bg)"/>
      <circle cx="${C}" cy="${C}" r="${R * 0.99}" fill="none" stroke="#ffffff" stroke-opacity="0.42" stroke-width="${R * 0.006}"/>`,
    ring: () => {
      const d = dividers({
        color: '#ffffff',
        inner: INNER + R * 0.015,
        outer: R * 0.97,
        width: R * 0.006,
        startOpacity: 0.6,
      });
      return `<defs>${d.defs}</defs>${d.lines}
        <circle cx="${C}" cy="${C}" r="${INNER}" fill="#0c0c14" fill-opacity="0.62"/>
        <circle cx="${C}" cy="${C}" r="${INNER}" fill="none" stroke="#ffffff" stroke-opacity="0.4" stroke-width="${R * 0.005}"/>`;
    },
  },
  minimal: {
    background: `<circle cx="${C}" cy="${C}" r="${R * 0.995}" fill="#0a0a0e" fill-opacity="0.58"/>`,
    ring: () => `<circle cx="${C}" cy="${C}" r="${INNER}" fill="#0a0a0e" fill-opacity="0.88"/>`,
  },
};

const photoOverlay = (withDividers) => {
  const d = dividers({
    color: '#ffffff',
    inner: INNER + R * 0.012,
    outer: R * 0.985,
    width: R * 0.0062,
    startOpacity: 0.75,
  });
  return `${withDividers ? `<defs>${d.defs}</defs>${d.lines}` : ''}
    <circle cx="${C}" cy="${C}" r="${R * 0.992}" fill="none" stroke="#ffffff" stroke-opacity="0.38" stroke-width="${R * 0.008}"/>
    <circle cx="${C}" cy="${C}" r="${INNER}" fill="#08080d" fill-opacity="0.46"/>
    <circle cx="${C}" cy="${C}" r="${INNER}" fill="none" stroke="#ffffff" stroke-opacity="0.32" stroke-width="${R * 0.006}"/>`;
};

for (const [id, w] of Object.entries(wheels)) {
  const doc = (body) =>
    `<svg xmlns="http://www.w3.org/2000/svg" width="${S}" height="${S}" viewBox="0 0 ${S} ${S}">${body}</svg>`;
  writeFileSync(out(`wheels/${id}/CircleBackground.png`), render(doc(w.background), S));
  writeFileSync(out(`wheels/${id}/SegmentedCircle.png`), render(doc(w.ring()), S));
}

{
  const doc = (body) =>
    `<svg xmlns="http://www.w3.org/2000/svg" width="${S}" height="${S}" viewBox="0 0 ${S} ${S}">${body}</svg>`;
  writeFileSync(out('wheels/photo/SegmentedCircle.png'), render(doc(photoOverlay(true)), S));
  writeFileSync(out('wheels/photo/SegmentedCircleClean.png'), render(doc(photoOverlay(false)), S));
}

const thumbs = (p) => {
  const full = join(root, 'src/assets/mods', p);
  mkdirSync(dirname(full), { recursive: true });
  return full;
};
for (const [id, body] of Object.entries(cursors)) {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 64 64">${body}</svg>`;
  writeFileSync(thumbs(`cursor-${id}.png`), render(svg, 64));
}
for (const [id, w] of Object.entries(wheels)) {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${S}" height="${S}" viewBox="0 0 ${S} ${S}">${w.background}${w.ring()}</svg>`;
  writeFileSync(thumbs(`wheel-${id}.png`), render(svg, 192));
}

console.log('Mod presets rendered.');
