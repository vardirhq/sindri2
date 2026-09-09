#!/usr/bin/env node
/**
 * The baker's command line.
 *
 *   node src/cli.ts <recipe.isobake.json> [--out DIR] [--check] [--quiet]
 *
 * `--check` bakes without writing and fails if what is on disk differs, which is
 * how CI asserts that a checked-in asset is still what its recipe produces. It
 * compares PNG *pixels* rather than PNG bytes, so a zlib upgrade that changes
 * the compressed stream and no pixel does not fail a build.
 */

import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';

import { type BakeOutput, type BakeResult, bake } from './bake.ts';
import { decodePng } from './png.ts';
import { firstDifference } from './image.ts';
import { RecipeError, parseRecipe } from './recipe.ts';

interface Options {
  recipe: string;
  /** Where assets are written. Always present: parsing fails without it. */
  out: string;
  check: boolean;
  quiet: boolean;
}

function parseArgs(argv: string[]): Options {
  const options = { recipe: '', out: null as string | null, check: false, quiet: false };

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === '--check') options.check = true;
    else if (arg === '--quiet') options.quiet = true;
    else if (arg === '--out') options.out = argv[++i] ?? '';
    else if (arg.startsWith('--out=')) options.out = arg.slice('--out='.length);
    else if (arg.startsWith('--')) throw new Error(`unknown option ${arg}`);
    else if (options.recipe) throw new Error('bake one recipe at a time');
    else options.recipe = arg;
  }

  if (!options.recipe) throw new Error('usage: cli.ts <recipe.isobake.json> [--out DIR] [--check]');
  const out = options.out;
  if (!out) throw new Error('--out DIR is required: it is where the assets are written');
  return { ...options, out };
}

async function readRecipe(path: string) {
  const json = await readFile(path, 'utf8');
  return parseRecipe(json, path);
}

/** Write a file, creating the directories above it. */
async function write(root: string, file: BakeOutput): Promise<void> {
  const target = join(root, file.path);
  await mkdir(dirname(target), { recursive: true });
  await writeFile(target, file.contents);
}

/**
 * Compare a baked file against what is on disk.
 *
 * Returns a sentence saying what differs, or null when it matches.
 */
async function difference(root: string, file: BakeOutput): Promise<string | null> {
  const target = join(root, file.path);
  let existing: Buffer;
  try {
    existing = await readFile(target);
  } catch {
    return `${file.path} has not been baked yet`;
  }

  if (file.path.endsWith('.png')) {
    const baked = decodePng(file.contents);
    const stored = decodePng(existing);
    const at = firstDifference(baked, stored);
    if (!at) return null;
    if (at.x < 0) {
      return (
        `${file.path} is ${stored.width}x${stored.height} on disk ` +
        `but bakes to ${baked.width}x${baked.height}`
      );
    }
    return `${file.path} differs from the bake at pixel ${at.x},${at.y}`;
  }

  return existing.equals(file.contents) ? null : `${file.path} differs from the bake`;
}

function describe(result: BakeResult): string {
  const { report } = result;
  const lines = [
    `${report.id}: ${result.frames.length} frames of ${report.canvas.width}x${report.canvas.height}`,
    `  anchor        ${report.anchor[0]}, ${report.anchor[1]} (the centre of every frame)`,
    `  sprite scale  ${report.spriteScale.map((v) => v.toFixed(4)).join(' x ')} world units`,
    `  palette       ${report.palette.length} colours: ${report.palette.join(' ')}`,
  ];
  for (const frame of report.frames) {
    lines.push(`  ${frame.direction.padEnd(12)}content ${frame.content}, margin ${frame.margin}px`);
  }
  for (const warning of report.warnings) lines.push(`  warning: ${warning}`);
  return lines.join('\n');
}

async function main(): Promise<number> {
  const options = parseArgs(process.argv.slice(2));
  const recipe = await readRecipe(options.recipe);
  const result = bake(recipe);
  const root = resolve(options.out);

  if (!options.quiet) console.log(describe(result));

  if (options.check) {
    const differences = (await Promise.all(result.files.map((file) => difference(root, file)))).filter(
      (entry): entry is string => entry !== null,
    );
    if (differences.length > 0) {
      for (const line of differences) console.error(`error: ${line}`);
      console.error(`re-bake with: node ${process.argv[1]} ${options.recipe} --out ${options.out}`);
      return 1;
    }
    if (!options.quiet) console.log('  up to date');
    return 0;
  }

  for (const file of result.files) await write(root, file);
  if (!options.quiet) {
    for (const file of result.files) console.log(`  wrote ${join(options.out, file.path)}`);
  }
  return 0;
}

main()
  .then((code) => {
    process.exitCode = code;
  })
  .catch((error: unknown) => {
    if (error instanceof RecipeError) console.error(`error: ${error.message}`);
    else console.error(`error: ${(error as Error).message ?? error}`);
    process.exitCode = 1;
  });
