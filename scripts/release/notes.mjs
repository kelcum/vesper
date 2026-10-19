import { readFileSync } from 'node:fs';

export function sectionFor(changelog, version) {
  const lines = changelog.split(/\r?\n/);
  const start = lines.findIndex((l) => l.startsWith(`## [${version}]`));
  if (start === -1) return null;
  const end = lines.findIndex((l, i) => i > start && (l.startsWith('## [') || /^\[[^\]]+\]: /.test(l)));
  return lines
    .slice(start + 1, end === -1 ? undefined : end)
    .join('\n')
    .trim();
}

if (import.meta.url === `file:///${process.argv[1].replace(/\\/g, '/')}` || process.argv[1]?.endsWith('notes.mjs')) {
  const version = process.argv[2];
  if (!version) {
    console.error('usage: notes.mjs <version>');
    process.exit(2);
  }
  const text = sectionFor(readFileSync(new URL('../../CHANGELOG.md', import.meta.url), 'utf8'), version);
  if (!text) {
    console.error(`CHANGELOG.md has no section for ${version}`);
    process.exit(1);
  }
  process.stdout.write(text + '\n');
}
