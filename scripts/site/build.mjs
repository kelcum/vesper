// Builds the xan.gripe/vesper page in website/vesper/.
//
//   node scripts/site/build.mjs
//
// - copies the app's fonts, so the site and app use the same type
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

// 1. Fonts
ensure(join(site, 'assets/fonts'));
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
        ? '<span class="chip warn">Being removed</span>'
        : f.behavior === 'established'
          ? '<span class="chip ok">Established</span>'
          : '<span class="chip neutral">Uncertain</span>';
    const cats = f.categories.map((c) => label[c] ?? c).join(', ');
    return `<tr><td><strong>${esc(f.title)}</strong><span class="name">${esc(f.name)}</span></td><td><p>${esc(f.summary)}</p></td><td>${esc(cats)}</td><td>${confidence}</td></tr>`;
  })
  .join('\n');
const table = `<div class="table-wrap"><table>
<thead><tr><th scope="col">Flag</th><th scope="col">What it does</th><th scope="col">Area</th><th scope="col">Effect</th></tr></thead>
<tbody>
${rows}
</tbody></table></div>
<p class="table-note">Catalog reviewed ${esc(catalog.reviewed)}. Source: <a href="${esc(catalog.allowlist.url)}">Roblox's allowlist announcement</a>.</p>`;

// 4. Release blocks
const release = JSON.parse(readFileSync(join(site, 'release.json'), 'utf8'));
let html = readFileSync(join(site, 'index.html'), 'utf8');
html = replaceBlock(html, 'flags', table);

// GitHub links only once the repository is public.
const footerLinks = release.repoPublic
  ? `<a href="${esc(release.repo)}">GitHub</a>\n<a href="${esc(release.repo)}/releases">Changelog</a>`
  : '';
const downloadIcon =
  '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 3v12"/><path d="m7 10 5 5 5-5"/><path d="M5 21h14"/></svg>';
html = replaceBlock(html, 'footer-links', footerLinks);

if (release.status === 'released' && release.installer) {
  const inst = release.installer;
  for (const file of [inst, release.portable, release.mac, release.macUpdate].filter(Boolean)) {
    if (!existsSync(join(site, 'download', file.file))) {
      throw new Error(
        `release.json lists download/${file.file}, which doesn't exist; refusing to publish a broken link`,
      );
    }
  }
  const href = `download/${encodeURIComponent(inst.file)}`;
  // With a Mac build, every download button comes in a Windows and a Mac version.
  const macHref = release.mac ? `download/${encodeURIComponent(release.mac.file)}` : null;
  const macButton = macHref
    ? `\n<a class="btn btn-primary" href="${macHref}" download>${downloadIcon} Download for Mac</a>`
    : '';
  html = replaceBlock(
    html,
    'nav-cta',
    macHref
      ? '<a class="btn btn-primary" href="#download">Download</a>'
      : `<a class="btn btn-primary" href="${href}" download>Download</a>`,
  );
  html = replaceBlock(
    html,
    'hero-cta',
    `<a class="btn btn-primary" href="${href}" download>${downloadIcon} Download for Windows</a>${macButton}`,
  );
  html = replaceBlock(
    html,
    'hero-meta',
    macHref
      ? `<p class="cta-meta reveal">Version ${esc(release.version)} · Windows 10 &amp; 11 (64-bit) and macOS 11 or later · updates itself</p>`
      : `<p class="cta-meta reveal">Version ${esc(release.version)} · ${sizeLabel(inst.size)} · Windows 10 &amp; 11 (64-bit) · updates itself</p>`,
  );
  const portable = release.portable
    ? `<li><b>Portable:</b> <a href="download/${encodeURIComponent(release.portable.file)}" download>${esc(release.portable.file)}</a> (${sizeLabel(release.portable.size)}), keeps everything in a folder next to Vesper.exe</li>`
    : '';
  // Only once a release really has a Mac build.
  const mac = release.mac
    ? `<li><b>macOS:</b> <a href="download/${encodeURIComponent(release.mac.file)}" download>${esc(release.mac.file)}</a> (${sizeLabel(release.mac.size)}), macOS 11 or later, Apple silicon and Intel. SHA-256 <code>${esc(release.mac.sha256)}</code></li>`
    : '';
  const notes = sectionFor(read('CHANGELOG.md'), release.version);
  html = replaceBlock(
    html,
    'download',
    `<div class="cta"><a class="btn btn-primary" href="${href}" download>${downloadIcon} ${macHref ? 'Download for Windows' : `Download Vesper ${esc(release.version)}`}</a>${macButton}</div>
<ul class="facts">
<li><b>${sizeLabel(inst.size)}</b> installer, released ${esc(release.date)}, no administrator rights needed</li>
<li><b>Updates itself:</b> new versions show up inside Vesper, signed so only real ones install. Your settings stay.</li>
${portable}
<li><b>SHA-256</b> <code>${esc(inst.sha256)}</code></li>
${mac}
</ul>
${notes ? `<details><summary>What's new in ${esc(release.version)}</summary>${changelogHtml(notes)}</details>` : ''}
<p class="note">Vesper is new and its installer isn't code-signed yet, so Windows may show <b>“Windows protected your PC”</b>. Choose <b>More info</b>, then <b>Run anyway</b>.</p>${
      release.mac
        ? `\n<p class="note">On a Mac, open the disk image, drag Vesper to Applications and open it. The first time, macOS says it can't check Vesper: open <b>System Settings → Privacy &amp; Security</b> (<b>System Preferences → Security &amp; Privacy</b> on macOS 11 and 12) and choose <b>Open Anyway</b>.</p>`
        : ''
    }`,
  );
} else {
  html = replaceBlock(html, 'nav-cta', '<a class="btn btn-primary" href="#download">Coming soon</a>');
  html = replaceBlock(html, 'hero-cta', '<span class="btn soon" aria-disabled="true">Coming soon</span>');
  html = replaceBlock(html, 'hero-meta', '<p class="cta-meta reveal">Windows 10 &amp; 11 · 64-bit · free</p>');
  html = replaceBlock(
    html,
    'download',
    `<div class="cta"><span class="btn soon" aria-disabled="true">Coming soon</span></div>
<p class="note">Vesper ${esc(release.version)} is in private testing. The installer will appear here, with its checksum, when it's published.</p>`,
  );
}

// Format like the rest of the repo, so a rebuild with no changes leaves no diff.
const indexPath = join(site, 'index.html');
const options = await prettier.resolveConfig(indexPath);
writeFileSync(indexPath, await prettier.format(html, { ...options, filepath: indexPath }));
console.log(`Built website/vesper (${release.status}, ${catalog.flags.length} flags, ${pngs.length} screenshots).`);
