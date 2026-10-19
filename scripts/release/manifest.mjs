import { readFileSync, writeFileSync } from 'node:fs';
import { basename } from 'node:path';
import { sectionFor } from './notes.mjs';

const args = process.argv.slice(2);
const macAt = args.indexOf('--mac');
const mac = macAt === -1 ? null : args.splice(macAt, 2)[1];
const [version, installer, out, base = 'https://xan.gripe/vesper/download'] = args;
if (!version || !installer || !out || (macAt !== -1 && !mac)) {
  console.error(
    'usage: manifest.mjs <version> <installer.exe> <out.json> [download-base-url] [--mac <Vesper.app.tar.gz>]',
  );
  process.exit(2);
}

const entry = (file) => ({
  signature: readFileSync(`${file}.sig`, 'utf8').trim(),
  url: `${base.replace(/\/$/, '')}/${encodeURIComponent(basename(file))}`,
});
const changelog = readFileSync(new URL('../../CHANGELOG.md', import.meta.url), 'utf8');
const notes = sectionFor(changelog, version) ?? `Vesper ${version}`;

const platforms = { 'windows-x86_64': entry(installer) };
if (mac) {
  platforms['darwin-aarch64'] = entry(mac);
  platforms['darwin-x86_64'] = entry(mac);
}
const manifest = { version, notes, pub_date: new Date().toISOString(), platforms };
writeFileSync(out, JSON.stringify(manifest, null, 2) + '\n');
console.log(`wrote ${out} for ${version} (${Object.keys(platforms).join(', ')})`);
