// Loads a wasm-pack build in a real browser and reports what happened.
//
// The engine compiled for `wasm32` for several releases and CI checked it every
// time, and nobody had ever loaded the page. Two things were broken the whole
// while: a failure in a browser was recorded and never reported, because
// `spawn_app` hands the loop to the page and `run` has already returned; and
// the surface refused every canvas, because a canvas offers no sRGB format and
// the engine took that to mean it could not encode. Neither is the kind of
// thing a compile check finds.
//
//   node scripts/browser/smoke.mjs examples/cube out.png
//
// Exits non-zero when the page fails to draw, so it can be a check rather than
// a thing somebody remembers to look at.
import { chromium } from 'playwright';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize, resolve } from 'node:path';

const ROOT = resolve(process.argv[2] ?? 'examples/cube');
const SHOT = process.argv[3];
const TYPES = {
  '.html': 'text/html',
  '.js': 'text/javascript',
  '.wasm': 'application/wasm',
  '.json': 'application/json',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
};

const server = createServer(async (request, response) => {
  const path = normalize(decodeURIComponent(new URL(request.url, 'http://x').pathname));
  // Browsers ask for this whether or not a page mentions one, and a 404 for it
  // would be reported as a problem with the engine.
  if (path === '/favicon.ico') {
    response.writeHead(204).end();
    return;
  }
  const file = join(ROOT, path === '/' ? 'index.html' : path);
  try {
    const body = await readFile(file);
    response.writeHead(200, { 'content-type': TYPES[extname(file)] ?? 'application/octet-stream' });
    response.end(body);
  } catch {
    response.writeHead(404).end('not found');
  }
});
await new Promise((done) => server.listen(0, done));
const port = server.address().port;

// WebGPU in a headless browser needs asking for, and over software Vulkan needs
// asking for twice. `CHROME_PATH` is for environments that ship their own.
const browser = await chromium.launch({
  executablePath: process.env.CHROME_PATH || undefined,
  args: [
    '--enable-unsafe-webgpu',
    '--enable-features=Vulkan',
    '--use-angle=vulkan',
    '--use-vulkan=swiftshader',
    '--enable-gpu',
    '--ignore-gpu-blocklist',
    '--no-sandbox',
  ],
});
const page = await browser.newPage({ viewport: { width: 960, height: 540 } });

// Audio the engine plays is an element created with `new Audio()` and never
// appended, so nothing in the DOM shows whether it worked. `play()` hands back
// a promise, and a browser refusing a clip rejects it rather than throwing —
// which is how three clips at a sample rate no browser decodes shipped while
// every test passed. Recording how each promise settles is the only way this
// check can tell "it played" from "it was asked to".
await page.addInitScript(() => {
  window.__plays = [];
  const play = HTMLMediaElement.prototype.play;
  HTMLMediaElement.prototype.play = function () {
    const record = { settled: 'pending' };
    window.__plays.push(record);
    const element = this;
    return play.call(this).then(
      () => {
        record.settled = 'played';
        record.element = element;
      },
      (error) => {
        record.settled = `refused: ${error.message}`;
      },
    );
  };
});

const problems = [];
page.on('console', (message) => {
  if (message.type() === 'error') problems.push(message.text());
});
page.on('pageerror', (error) => problems.push(String(error.message)));

await page.goto(`http://127.0.0.1:${port}/`, { waitUntil: 'load' });
const webgpu = await page.evaluate(() => Boolean(navigator.gpu));
// Long enough for the device request, which is the only asynchronous part of
// startup, and for a few frames after it.
await page.waitForTimeout(6000);

// Browsers refuse audio until a real user gesture, and the engine unlocks on
// the first key or pointer press. Without one, a page that plays sound looks
// exactly like a page that has none.
await page.mouse.click(480, 270);
await page.keyboard.press('KeyD');
await page.waitForTimeout(2000);
const audio = await page.evaluate(() =>
  window.__plays.map((record) => ({
    settled: record.settled,
    playedTo: record.element ? Number(record.element.currentTime.toFixed(2)) : 0,
  })),
);

// A canvas the engine never configured keeps the HTML default of 300x150, which
// is the difference between "the page loaded" and "the engine started".
const canvas = await page.evaluate(() => {
  const element = document.querySelector('canvas');
  return element ? { width: element.width, height: element.height } : null;
});
if (SHOT) await page.screenshot({ path: SHOT });
await browser.close();
server.close();

const started = Boolean(canvas) && canvas.width > 300;
// A page with no sound is not a failure; a page that asked for sound and did
// not get it is. Playing to zero counts as refused: the promise resolves for a
// clip that never advances, so the time is what separates the two.
const refused = audio.filter(
  (record) => record.settled !== 'played' || record.playedTo === 0,
);
console.log(`webgpu: ${webgpu ? 'yes' : 'no'}`);
console.log(`canvas: ${canvas ? `${canvas.width}x${canvas.height}` : 'none'}`);
console.log(
  audio.length === 0
    ? 'audio: none requested'
    : `audio: ${audio.length - refused.length}/${audio.length} playing`,
);
for (const record of refused) console.log(`problem: audio ${record.settled}`);
for (const problem of problems) console.log(`problem: ${problem}`);
if (!webgpu || !started || problems.length > 0 || refused.length > 0) {
  console.log('the page did not start the engine');
  process.exit(1);
}
console.log('the engine ran in a browser');
