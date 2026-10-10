// Writes the updater manifest Vesper reads from xan.gripe/vesper/updates/.
//
//   node scripts/release/manifest.mjs <version> <installer.exe> <out.json> [download-base-url]
//
// The installer's .sig file (made by `tauri build` with the update key) must
// sit next to it. Vesper refuses any update whose signature doesn't match the
// public key compiled into the app.

import { readFileSync, writeFileSync } from 'node:fs';
import { basename } from 'node:path';
import { sectionFor } from './notes.mjs';

const [version, installer, out, base = 'https://xan.gripe/vesper/download'] = process.argv.slice(2);
if (!version || !installer || !out) {
  console.error('usage: manifest.mjs <version> <installer.exe> <out.json> [download-base-url]');
  process.exit(2);
}

const signature = readFileSync(`${installer}.sig`, 'utf8').trim();
const changelog = readFileSync(new URL('../../CHANGELOG.md', import.meta.url), 'utf8');
const notes = sectionFor(changelog, version) ?? `Vesper ${version}`;

const manifest = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: {
    'windows-x86_64': {
      signature,
      url: `${base.replace(/\/$/, '')}/${encodeURIComponent(basename(installer))}`,
    },
  },
};
writeFileSync(out, JSON.stringify(manifest, null, 2) + '\n');
console.log(`wrote ${out} for ${version}`);
