// Writes SHA256SUMS.txt for every file in a folder (sha256sum format).
//
//   node scripts/release/checksums.mjs <folder>

import { createHash } from 'node:crypto';
import { readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const dir = process.argv[2];
if (!dir) {
  console.error('usage: checksums.mjs <folder>');
  process.exit(2);
}
const lines = readdirSync(dir)
  .filter((f) => f !== 'SHA256SUMS.txt' && statSync(join(dir, f)).isFile())
  .sort()
  .map(
    (f) =>
      `${createHash('sha256')
        .update(readFileSync(join(dir, f)))
        .digest('hex')}  ${f}`,
  );
writeFileSync(join(dir, 'SHA256SUMS.txt'), lines.join('\n') + '\n');
console.log(lines.join('\n'));
