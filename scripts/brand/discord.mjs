// Renders the art for Vesper's Discord application.
//
//   node scripts/brand/discord.mjs
//
// Output: assets/branding/discord/, all 1024×1024 (Discord's recommended size)
//   app-icon.png   General Information → App Icon
//   vesper.png     Rich Presence → Art Assets, name "vesper"   (large image)
//   playing.png    Rich Presence → Art Assets, name "playing"  (small badge: Roblox is running)
//   idle.png       Rich Presence → Art Assets, name "idle"     (small badge: in the launcher)
//
// The asset names must match the keys in src-tauri/src/discord/presence.rs.
// Discord crops the large image to a rounded square and the small one to a
// circle, so both are drawn full bleed.

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

// Small badges are shown at about 30 px, so each is one bold shape on a strong colour.
const badges = {
  // A rounded play triangle on mint.
  playing: `
    <defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#4fe0a8"/><stop offset="1" stop-color="#22a879"/></linearGradient></defs>
    <rect width="1024" height="1024" fill="url(#g)"/>
    <path d="M412 300 Q380 282 380 318 V706 Q380 742 412 724 L734 536 Q764 512 734 488 Z"
      fill="#ffffff" stroke="#ffffff" stroke-width="44" stroke-linejoin="round"/>`,
  // The evening star from the logo, over the dusk sky.
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
