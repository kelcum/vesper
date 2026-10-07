// Dev-only: renders a contact sheet of the mark at real sizes for review.
// Usage: node scripts/brand/preview.mjs <out.png>
import { Resvg } from '@resvg/resvg-js';
import { writeFileSync } from 'node:fs';
import { markSvg } from './mark.mjs';

const out = process.argv[2] ?? 'brand-preview.png';
const png = (svg, w) => new Resvg(svg, { fitTo: { mode: 'width', value: w } }).render().asPng();
const uri = (buf) => `data:image/png;base64,${buf.toString('base64')}`;

const tiles = [
  // [label, detail, container, renderSize, displaySize, pixelated]
  ['512', 'full', 'squircle', 360, 360, false],
  ['circle', 'full', 'circle', 160, 160, false],
  ['64', 'medium', 'squircle', 64, 64, false],
  ['48', 'medium', 'squircle', 48, 48, false],
  ['32 small', 'small', 'squircle', 32, 128, true],
  ['16 small', 'small', 'squircle', 16, 128, true],
  ['32 actual', 'small', 'squircle', 32, 32, false],
  ['16 actual', 'small', 'squircle', 16, 16, false],
];
let x = 24;
const imgs = [];
for (const [label, detail, container, rs, ds, pix] of tiles) {
  const y = 24 + (360 - ds) / 2;
  imgs.push(
    `<image href="${uri(png(markSvg({ detail, container }), rs))}" x="${x}" y="${y}" width="${ds}" height="${ds}" ${pix ? 'style="image-rendering:pixelated"' : ''}/>`,
    `<text x="${x}" y="408" fill="#9a96b0" font-size="13" font-family="Segoe UI">${label}</text>`,
  );
  x += ds + 28;
}
const W = x;
const sheet = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="430">
  <rect width="${W}" height="215" fill="#15141c"/><rect y="215" width="${W}" height="215" fill="#eeedf3"/>
  ${imgs.join('\n')}
</svg>`;
writeFileSync(out, new Resvg(sheet, { font: { loadSystemFonts: true } }).render().asPng());
console.log('wrote', out);
