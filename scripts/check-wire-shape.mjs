/**
 * Checks that the frontend still describes the wire format the backend sends.
 *
 * `types.ts` and `mock.ts` are hand-written mirrors of the Rust structs rather
 * than anything generated from them, which leaves a gap the typechecker cannot
 * see: rename a field in `model.rs` and both TypeScript files still compile,
 * the app still builds, and the widget quietly renders an em dash forever
 * because `snapshot.cpu.uzage` is `undefined`.
 *
 * So this compares shapes rather than types. The backend's side is read from
 * `cargo run --example snapshot`, the frontend's from `syntheticSnapshot`, and
 * every leaf path is matched.
 *
 * Two directions matter, and for different reasons:
 *
 * - A path the backend sends but the frontend lacks is a widget rendering
 *   `undefined` — the bug above.
 * - A path the frontend reads but the backend never sends is a field the UI
 *   has been treating as data all along, and getting nothing.
 *
 * Values are ignored; only presence and kind are compared. `null` is treated as
 * compatible with anything, because a `null` on either side is this project's
 * way of saying "the platform cannot report this" and not a shape claim.
 *
 * Run with `npm run check:wire`.
 */

import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

import { syntheticSnapshot } from '../src/lib/mock.ts';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');

/**
 * Reads JSON out of `cargo run --example <name>`.
 *
 * A compile error here means the example itself is stale, which is a problem
 * with this check rather than a shape mismatch. Saying so is more useful than
 * letting `JSON.parse` fail on the compiler's output.
 */
function fromExample(name) {
  try {
    return JSON.parse(
      execFileSync('cargo', ['run', '--quiet', '--example', name], {
        cwd: resolve(root, 'src-tauri'),
        encoding: 'utf8',
        maxBuffer: 1 << 24,
      }),
    );
  } catch (error) {
    console.error(`could not run \`cargo run --example ${name}\`:`);
    console.error(String(error.stderr || error.message).trimEnd());
    process.exit(1);
  }
}

const rust = fromExample('snapshot');

// `t` is large enough that the synthetic values avoid their zero/clamped
// edges, so a `0` here means the shape has a hole rather than the data being
// coincidentally low.
const ts = syntheticSnapshot(1, 137.5);

/**
 * Flattens an object into `path -> kind`, the way a change to a field name
 * would break a reader.
 *
 * Arrays are collapsed to a single `[]` entry: their length is data, not shape.
 * A list of tuples (`gpu.devices[].engines` is `Vec<(String, f32)>`) collapses
 * too, because the tuple's shape is fixed by its Rust type.
 */
function leaves(value, prefix = '', into = new Map()) {
  if (Array.isArray(value)) {
    if (value.length > 0) leaves(value[0], `${prefix}[]`, into);
    else into.set(`${prefix}[]`, 'empty');
    return into;
  }
  if (value !== null && typeof value === 'object') {
    for (const [key, child] of Object.entries(value)) {
      leaves(child, prefix ? `${prefix}.${key}` : key, into);
    }
    return into;
  }
  into.set(prefix, value === null ? 'null' : typeof value);
  return into;
}

const rustLeaves = leaves(rust);
const tsLeaves = leaves(ts);

/** Whether a value the backend sends can stand in for what the frontend expects. */
function compatible(expected, actual) {
  if (expected === 'null' || actual === 'null') return true;
  if (expected === 'empty' || actual === 'empty') return true;
  // Integers and floats both arrive as JSON numbers.
  if ((expected === 'number' && actual === 'number') || (expected === 'boolean' && actual === 'boolean')) {
    return true;
  }
  return expected === actual;
}

const problems = [];

for (const [path, kind] of rustLeaves) {
  if (!tsLeaves.has(path)) {
    problems.push(`the frontend does not read ${path} (backend sends ${kind})`);
  } else if (!compatible(tsLeaves.get(path), kind)) {
    problems.push(`${path}: frontend expects ${tsLeaves.get(path)}, backend sends ${kind}`);
  }
}

for (const [path, kind] of tsLeaves) {
  if (!rustLeaves.has(path)) {
    problems.push(`the frontend reads ${path} (${kind}), which the backend never sends`);
  }
}

if (problems.length > 0) {
  console.error(`wire shape mismatch (${problems.length}):`);
  for (const problem of problems) console.error(`  - ${problem}`);
  process.exit(1);
}

console.log(`wire shape agrees: ${rustLeaves.size} fields match the backend`);