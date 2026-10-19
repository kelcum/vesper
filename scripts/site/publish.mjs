import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFileSync, cpSync, existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
const flags = new Set(args.filter((a) => a.startsWith('--')));
const [from, pages] = args.filter((a) => !a.startsWith('--'));
if (!from || !pages) {
  console.error('usage: publish.mjs <release-files> <pages-repo> [--beta] [--repo-public]');
  process.exit(2);
}
const fail = (msg) => {
  console.error(`publish: ${msg}`);
  process.exit(1);
};

const version = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8')).version;
const installer = `Vesper_${version}_x64-setup.exe`;
const portable = `Vesper_${version}_x64-portable.zip`;
const macDmg = `Vesper_${version}_universal.dmg`;
const macUpdate = `Vesper_${version}_universal.app.tar.gz`;
const sha256 = (p) => createHash('sha256').update(readFileSync(p)).digest('hex');

if (!existsSync(join(pages, '.git'))) fail(`${pages} isn't a git clone of the Pages repository`);
for (const f of [installer, `${installer}.sig`, 'latest.json', 'SHA256SUMS.txt']) {
  if (!existsSync(join(from, f))) fail(`${f} is missing from ${from}`);
}
const sums = new Map(
  readFileSync(join(from, 'SHA256SUMS.txt'), 'utf8')
    .split(/\r?\n/)
    .filter(Boolean)
    .map((l) => l.split(/\s+\*?/).reverse()),
);
for (const [file, expected] of sums) {
  if (!existsSync(join(from, file))) fail(`SHA256SUMS.txt lists ${file}, which is missing`);
  if (sha256(join(from, file)) !== expected) fail(`${file} doesn't match SHA256SUMS.txt`);
}
if (!sums.has(installer)) fail(`SHA256SUMS.txt doesn't cover ${installer}`);

const manifest = JSON.parse(readFileSync(join(from, 'latest.json'), 'utf8'));
const target = manifest.platforms?.['windows-x86_64'];
if (manifest.version !== version) fail(`latest.json is for ${manifest.version}, package.json says ${version}`);
if (!target?.url?.endsWith(`/download/${encodeURIComponent(installer)}`))
  fail(`latest.json doesn't point at ${installer}`);
if (target.signature !== readFileSync(join(from, `${installer}.sig`), 'utf8').trim()) {
  fail(`latest.json's signature doesn't match ${installer}.sig`);
}
const macTargets = ['darwin-aarch64', 'darwin-x86_64'].map((p) => manifest.platforms?.[p]).filter(Boolean);
const hasMac = macTargets.length > 0;
if (hasMac) {
  for (const f of [macUpdate, `${macUpdate}.sig`, macDmg]) {
    if (!existsSync(join(from, f))) fail(`latest.json has Mac updates, but ${f} is missing from ${from}`);
    if (!f.endsWith('.sig') && !sums.has(f)) fail(`SHA256SUMS.txt doesn't cover ${f}`);
  }
  const macSig = readFileSync(join(from, `${macUpdate}.sig`), 'utf8').trim();
  for (const t of macTargets) {
    if (!t.url?.endsWith(`/download/${encodeURIComponent(macUpdate)}`))
      fail(`latest.json doesn't point at ${macUpdate}`);
    if (t.signature !== macSig) fail(`latest.json's signature doesn't match ${macUpdate}.sig`);
  }
}

const out = join(pages, 'vesper');
mkdirSync(join(out, 'updates'), { recursive: true });
const manifestText = JSON.stringify(manifest, null, 2) + '\n';

if (flags.has('--beta')) {
  mkdirSync(join(out, 'download'), { recursive: true });
  copyFileSync(join(from, installer), join(out, 'download', installer));
  if (hasMac) copyFileSync(join(from, macUpdate), join(out, 'download', macUpdate));
  writeFileSync(join(out, 'updates', 'beta.json'), manifestText);
  console.log(`Staged ${version} on the beta channel in ${out}.`);
} else {
  const site = join(root, 'website/vesper');
  mkdirSync(join(site, 'download'), { recursive: true });
  const entry = (file) => {
    copyFileSync(join(from, file), join(site, 'download', file));
    return { file, size: statSync(join(from, file)).size, sha256: sha256(join(from, file)) };
  };
  const releasePath = join(site, 'release.json');
  const release = JSON.parse(readFileSync(releasePath, 'utf8'));
  Object.assign(release, {
    status: 'released',
    version,
    date: new Date().toLocaleDateString('en-CA'),
    installer: entry(installer),
    portable: existsSync(join(from, portable)) ? entry(portable) : null,
    mac: hasMac ? entry(macDmg) : null,
    macUpdate: hasMac ? entry(macUpdate) : null,
    repoPublic: flags.has('--repo-public'),
  });
  writeFileSync(releasePath, JSON.stringify(release, null, 2) + '\n');
  execFileSync(process.execPath, [join(root, 'scripts/site/build.mjs')], { stdio: 'inherit' });

  cpSync(site, out, { recursive: true, filter: (p) => !p.endsWith('release.json') });
  writeFileSync(join(out, 'updates', 'latest.json'), manifestText);
  writeFileSync(join(out, 'updates', 'beta.json'), manifestText);
  console.log(`Staged Vesper ${version} in ${out}.`);
  console.log('Commit website/vesper/release.json in this repository too.');
}
console.log(
  `Review it, then publish: git -C "${pages}" add vesper && git -C "${pages}" commit -m "Vesper ${version}" && git -C "${pages}" push`,
);
