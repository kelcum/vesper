import { readFileSync } from 'node:fs';

const path = new URL('../src-tauri/resources/flags/catalog.json', import.meta.url);
const catalog = JSON.parse(readFileSync(path, 'utf8'));
const errors = [];
const warn = [];

const prefix = /^(?:DF|SF|F)(Flag|Int|Log|String)[A-Za-z0-9_]+$/;
const kinds = { Flag: ['toggle'], Int: ['choice', 'number'], Log: ['choice', 'number'], String: ['text'] };
const names = new Set();

for (const f of catalog.flags) {
  const m = prefix.exec(f.name);
  if (!m) errors.push(`${f.name}: invalid name`);
  else if (!kinds[m[1]].includes(f.input.kind)) errors.push(`${f.name}: input ${f.input.kind} doesn't fit its type`);
  if (names.has(f.name)) errors.push(`${f.name}: duplicate`);
  names.add(f.name);
  if (!f.references?.includes('allowlist')) errors.push(`${f.name}: must cite the allowlist`);
  for (const r of f.references ?? []) if (!catalog.sources[r]) errors.push(`${f.name}: unknown source "${r}"`);
  if (f.robloxDefault && !catalog.sources[f.robloxDefault.source]) errors.push(`${f.name}: unknown default source`);
  if (!['established', 'uncertain'].includes(f.behavior))
    errors.push(`${f.name}: behavior must be established or uncertain`);
  const text = `${f.title} ${f.summary} ${f.details}`.toLowerCase();
  for (const claim of ['fps boost', 'more fps', '+fps', 'boosts', 'faster']) {
    if (text.includes(claim)) errors.push(`${f.name}: performance claim ("${claim}")`);
  }
}

for (const k of catalog.knownIgnored) if (names.has(k.name)) errors.push(`${k.name}: both allowlisted and ignored`);
for (const [key, s] of Object.entries(catalog.sources)) {
  if (!/^https:\/\//.test(s.url)) errors.push(`source ${key}: URL must be https`);
}

const ageDays = (Date.now() - new Date(catalog.reviewed).getTime()) / 86_400_000;
if (Number.isNaN(ageDays)) errors.push('reviewed date is invalid');
else if (ageDays > 90)
  warn.push(`catalog last reviewed ${Math.floor(ageDays)} days ago; check Roblox's allowlist for changes`);

for (const w of warn) console.warn(`warning: ${w}`);
if (errors.length) {
  for (const e of errors) console.error(`error: ${e}`);
  process.exit(1);
}
console.log(
  `Flag catalog OK: ${catalog.flags.length} flags, ${catalog.knownIgnored.length} known-ignored, reviewed ${catalog.reviewed}.`,
);
