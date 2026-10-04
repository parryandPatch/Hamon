/**
 * Checks that the frontend catalog and the Rust catalog are the same contract.
 *
 * `catalog.ts` and `layout.rs` each declare the list of widget kinds and the
 * default config for each. They cannot share one source — one side carries the
 * human-facing label and summary, the other is what `add_widget` returns — so
 * they have to be kept in step by hand. Drift here is silent: a kind present in
 * one file but not the other still builds, still typechecks, and only shows up
 * as a widget rendering its fields with no values.
 *
 * The Rust side is read from `cargo run --example catalog`, which exists solely
 * to print the contract as JSON, so this comparison uses the real definitions
 * rather than a second parser's opinion of them.
 *
 * Run with `npm run check:catalog`.
 */

import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

import { CATALOG } from '../src/lib/catalog.ts';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');

/**
 * Reads JSON out of `cargo run --example catalog`.
 *
 * A compile error means the example is stale, which is a problem with this
 * check rather than a catalog mismatch; saying so beats letting `JSON.parse`
 * fail on the compiler's output.
 */
let rust;
try {
  rust = JSON.parse(
    execFileSync('cargo', ['run', '--quiet', '--example', 'catalog'], {
      cwd: resolve(root, 'src-tauri'),
      encoding: 'utf8',
      maxBuffer: 1 << 24,
    }),
  );
} catch (error) {
  console.error('could not run `cargo run --example catalog`:');
  console.error(String(error.stderr || error.message).trimEnd());
  process.exit(1);
}

const rustKinds = rust.map((entry) => entry.kind);
const tsKinds = CATALOG.map((entry) => entry.kind);

const problems = [];

const duplicates = tsKinds.filter((kind, i) => tsKinds.indexOf(kind) !== i);
for (const kind of new Set(duplicates)) {
  problems.push(`catalog.ts declares "${kind}" more than once`);
}

for (const kind of rustKinds) {
  if (!tsKinds.includes(kind)) problems.push(`"${kind}" is in layout.rs but not catalog.ts`);
}
for (const kind of tsKinds) {
  if (!rustKinds.includes(kind)) problems.push(`"${kind}" is in catalog.ts but not layout.rs`);
}

/**
 * Compares default config payloads by value, ignoring key order and the
 * difference between `{ a: "x" }` and `{ a: '"x"' }`.
 */
function normalise(value) {
  if (Array.isArray(value)) return value.map(normalise);
  if (value && typeof value === 'object') {
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map((key) => [key, normalise(value[key])]),
    );
  }
  return value;
}

const rustByKind = new Map(rust.map((entry) => [entry.kind, entry]));
for (const entry of CATALOG) {
  const other = rustByKind.get(entry.kind);
  if (!other) continue;
  const mine = JSON.stringify(normalise(entry.defaults));
  const theirs = JSON.stringify(normalise(other.defaults));
  if (mine !== theirs) {
    problems.push(
      `${entry.kind} defaults differ\n      catalog.ts: ${mine}\n      layout.rs:   ${theirs}`,
    );
  }
}

if (problems.length > 0) {
  console.error(`catalog mismatch (${problems.length}):`);
  for (const problem of problems) console.error(`  - ${problem}`);
  process.exit(1);
}

console.log(`catalogs agree: ${CATALOG.length} kinds, defaults match layout.rs`);