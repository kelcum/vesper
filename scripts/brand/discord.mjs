import { Resvg } from '@resvg/resvg-js';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { markSvg, palette } from './mark.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const out = (name) => {
  const full = join(root, 'assets/branding/discord', name);
  mkdirSync(dirname(full), { recursive: true });
  return full;
};
const png = (svg) => new Resvg(svg, { fitTo: { mode: 'width', value: 1024 } }).render().asPng();
const doc = (body) =>
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024" width="1024" height="1024">${body}</svg>`;

const badges = {
  playing: `
    <defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#4fe0a8"/><stop offset="1" stop-color="#22a879"/></linearGradient></defs>
    <rect width="1024" height="1024" fill="url(#g)"/>
    <path d="M412 300 Q380 282 380 318 V706 Q380 742 412 724 L734 536 Q764 512 734 488 Z"
      fill="#ffffff" stroke="#ffffff" stroke-width="44" stroke-linejoin="round"/>`,
  idle: `
    <defs>
      <linearGradient id="g" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stop-color="${palette.skyTop}"/><stop offset="1" stop-color="${palette.skyHorizon}"/></linearGradient>
      <radialGradient id="h"><stop offset="0" stop-color="${palette.halo}" stop-opacity="0.55"/>
        <stop offset="1" stop-color="${palette.halo}" stop-opacity="0"/></radialGradient>
    </defs>
    <rect width="1024" height="1024" fill="url(#g)"/>
    <circle cx="512" cy="512" r="420" fill="url(#h)"/>
    <circle cx="512" cy="512" r="190" fill="${palette.star}"/>`,
};

writeFileSync(out('app-icon.png'), png(markSvg({ detail: 'full', container: 'square' })));
writeFileSync(out('vesper.png'), png(markSvg({ detail: 'medium', container: 'square' })));
for (const [name, body] of Object.entries(badges)) writeFileSync(out(`${name}.png`), png(doc(body)));
console.log('Discord art rendered.');
