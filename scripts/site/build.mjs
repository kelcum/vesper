// Builds the xan.gripe/vesper page in website/vesper/.
//
//   node scripts/site/build.mjs
//
// - copies the app's design tokens and fonts, so the site and app always match
// - converts docs/screenshots/*.png to WebP (needs Python with Pillow)
// - renders the Fast Flag table from the real catalog
// - renders download, version and changelog blocks from website/vesper/release.json
//   ("unreleased" shows "Coming soon": the page never links to a file that isn't there)
//
// The folder is then copied as-is into the kelcum.github.io repository
// (scripts/site/publish.mjs).

import { execFileSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as prettier from 'prettier';
import { sectionFor } from '../release/notes.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const site = join(root, 'website/vesper');
const read = (p) => readFileSync(join(root, p), 'utf8');
const ensure = (p) => mkdirSync(p, { recursive: true });

const esc = (s) =>
  String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);

function replaceBlock(html, name, content) {
  const re = new RegExp(`(<!-- build:${name} -->)[\\s\\S]*?(<!-- /build:${name} -->)`);
  if (!re.test(html)) throw new Error(`index.html has no build:${name} block`);
  return html.replace(re, `$1\n${content}\n$2`);
}

function sizeLabel(bytes) {
  return bytes >= 1024 * 1024 ? `${(bytes / 1024 / 1024).toFixed(1)} MB` : `${Math.round(bytes / 1024)} KB`;
}

/** Tiny Markdown subset for CHANGELOG sections: ### headings and - bullets. */
function changelogHtml(md) {
  const out = [];
  let list = false;
  for (const line of md.split(/\r?\n/)) {
    const t = line.trim();
    if (t.startsWith('### ')) {
      if (list) out.push('</ul>');
      list = false;
      out.push(`<h3>${esc(t.slice(4))}</h3>`);
    } else if (t.startsWith('- ')) {
      if (!list) out.push('<ul>');
      list = true;
      out.push(`<li>${esc(t.slice(2))}</li>`);
    } else if (t && list) {
      out[out.length - 1] = out[out.length - 1].replace('</li>', ` ${esc(t)}</li>`);
    } else if (t) {
      out.push(`<p>${esc(t)}</p>`);
    }
  }
  if (list) out.push('</ul>');
  return out.join('\n');
}

// 1. Tokens and fonts
ensure(join(site, 'assets/fonts'));
copyFileSync(join(root, 'src/styles/tokens.css'), join(site, 'assets/tokens.css'));
for (const f of ['Geist-Variable.woff2', 'GeistMono-Variable.woff2', 'OFL.txt']) {
  copyFileSync(join(root, 'src/assets/fonts', f), join(site, 'assets/fonts', f));
}

// 2. Screenshots → WebP
ensure(join(site, 'shots'));
const pngs = readdirSync(join(root, 'docs/screenshots')).filter((f) => f.endsWith('.png'));
execFileSync(
  'python',
  [
    // -P keeps the working directory off sys.path while allowing user-installed Pillow.
    '-P',
    '-c',
    'import sys\nfrom PIL import Image\nsrc,dst=sys.argv[1],sys.argv[2]\nfor n in sys.argv[3:]:\n    Image.open(f"{src}/{n}").convert("RGB").save(f"{dst}/{n[:-4]}.webp","WEBP",quality=86,method=6)',
    join(root, 'docs/screenshots'),
    join(site, 'shots'),
    ...pngs,
  ],
  { stdio: 'inherit' },
);

// 3. Flag table
const catalog = JSON.parse(read('src-tauri/resources/flags/catalog.json'));
const label = {
  rendering: 'Rendering',
  graphics: 'Graphics',
  performance: 'Performance',
  ui: 'UI',
  debug: 'Debug',
  engine: 'Engine',
};
const rows = catalog.flags
  .map((f) => {
    const confidence =
      f.status === 'removal-announced'
        ? '<span class="pill warn">Being removed</span>'
        : f.behavior === 'established'
          ? '<span class="pill ok">Established</span>'
          : '<span class="pill neutral">Uncertain</span>';
    const cats = f.categories.map((c) => label[c] ?? c).join(', ');
    return `<tr><td><strong>${esc(f.title)}</strong><span class="name">${esc(f.name)}</span></td><td><p>${esc(f.summary)}</p></td><td>${esc(cats)}</td><td>${confidence}</td></tr>`;
  })
  .join('\n');
const table = `<div class="table-wrap"><table>
<thead><tr><th scope="col">Flag</th><th scope="col">What it does</th><th scope="col">Area</th><th scope="col">Effect</th></tr></thead>
<tbody>
${rows}
</tbody></table></div>
<p class="small muted">Catalog reviewed ${esc(catalog.reviewed)}. Source: <a href="${esc(catalog.allowlist.url)}">Roblox's allowlist announcement</a>.</p>`;

// 4. Release blocks
const release = JSON.parse(readFileSync(join(site, 'release.json'), 'utf8'));
let html = readFileSync(join(site, 'index.html'), 'utf8');
html = replaceBlock(html, 'flags', table);

const footerLinks = release.repoPublic
  ? `<a href="${esc(release.repo)}">GitHub</a>\n<a href="${esc(release.repo)}/releases">Changelog</a>`
  : '';
html = replaceBlock(html, 'footer-links', footerLinks);

if (release.status === 'released' && release.installer) {
  const inst = release.installer;
  for (const file of [inst, release.portable].filter(Boolean)) {
    if (!existsSync(join(site, 'download', file.file))) {
      throw new Error(
        `release.json lists download/${file.file}, which doesn't exist; refusing to publish a broken link`,
      );
    }
  }
  const href = `download/${encodeURIComponent(inst.file)}`;
  html = replaceBlock(html, 'nav-cta', `<a class="btn primary small" href="${href}">Download</a>`);
  html = replaceBlock(
    html,
    'hero-cta',
    `<a class="btn primary" href="${href}">Download for Windows <span class="sub">v${esc(release.version)} · ${sizeLabel(inst.size)}</span></a>` +
      (release.repoPublic ? `\n<a class="btn ghost" href="${esc(release.repo)}">GitHub</a>` : ''),
  );
  html = replaceBlock(
    html,
    'hero-meta',
    `<p class="meta">Version ${esc(release.version)} · Windows 10 and 11 · 64-bit · free and open source (MIT)</p>`,
  );
  const files = [
    `<li><a href="${href}">${esc(inst.file)}</a><span>Installer · ${sizeLabel(inst.size)}</span><span class="sha">SHA-256 ${esc(inst.sha256)}</span></li>`,
    release.portable
      ? `<li><a href="download/${encodeURIComponent(release.portable.file)}">${esc(release.portable.file)}</a><span>Portable · ${sizeLabel(release.portable.size)}</span><span class="sha">SHA-256 ${esc(release.portable.sha256)}</span></li>`
      : '',
  ].join('\n');
  const notes = sectionFor(read('CHANGELOG.md'), release.version);
  html = replaceBlock(
    html,
    'download',
    `<h2>Vesper ${esc(release.version)}</h2>
<div class="download">
<p class="muted">Released ${esc(release.date)} · Windows 10 and 11 (64-bit) · installs per user, no administrator rights.</p>
<a class="btn primary" href="${href}">Download installer <span class="sub">${sizeLabel(inst.size)}</span></a>
<ul class="files">${files}</ul>
${notes ? `<details><summary>What's new in ${esc(release.version)}</summary>${changelogHtml(notes)}</details>` : ''}
</div>`,
  );
} else {
  html = replaceBlock(html, 'nav-cta', '<a class="btn primary small" href="#download">Coming soon</a>');
  html = replaceBlock(html, 'hero-cta', '<span class="btn primary disabled" aria-disabled="true">Coming soon</span>');
  html = replaceBlock(html, 'hero-meta', '<p class="meta">Windows 10 and 11 · 64-bit · free and open source (MIT)</p>');
  html = replaceBlock(
    html,
    'download',
    `<h2>Coming soon.</h2>
<p class="muted">Vesper ${esc(release.version)} is in private testing. The installer will appear here, with its checksum, when it's published.</p>`,
  );
}

// Format like the rest of the repo, so a rebuild with no changes leaves no diff.
const indexPath = join(site, 'index.html');
const options = await prettier.resolveConfig(indexPath);
writeFileSync(indexPath, await prettier.format(html, { ...options, filepath: indexPath }));
console.log(`Built website/vesper (${release.status}, ${catalog.flags.length} flags, ${pngs.length} screenshots).`);
