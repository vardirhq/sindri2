// Loads a wasm-pack build in a real browser and reports what happened.
//
// Compiling wasm proves almost nothing about delivery. This check insists on a
// configured canvas, settled audio promises, and — when SINDRI_EXPECT_ASSETS is
// set — real HTTP requests for every kind of project asset Gather needs. It can
// also deliberately remove a browser capability to prove the page fails in a
// way a player can actually read.
import { chromium } from 'playwright';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize, resolve } from 'node:path';

const ROOT = resolve(process.argv[2] ?? 'examples/cube');
const SHOT = process.argv[3];
const EXPECT_ASSETS = process.env.SINDRI_EXPECT_ASSETS === '1';
const EXPECT_FAILURE = process.env.SINDRI_EXPECT_FAILURE || '';
let BASE = process.env.SINDRI_BASE_PATH || '/';
if (!BASE.startsWith('/')) BASE = `/${BASE}`;
if (!BASE.endsWith('/')) BASE += '/';

const TYPES = {
  '.html': 'text/html',
  '.js': 'text/javascript',
  '.wasm': 'application/wasm',
  '.json': 'application/json',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.ttf': 'font/ttf',
  '.wav': 'audio/wav',
  '.ogg': 'audio/ogg',
  '.mp3': 'audio/mpeg',
};

const server = createServer(async (request, response) => {
  const path = normalize(decodeURIComponent(new URL(request.url, 'http://x').pathname));
  if (path === '/favicon.ico' || path === `${BASE}favicon.ico`) {
    response.writeHead(204).end();
    return;
  }
  if (!path.startsWith(BASE)) {
    response.writeHead(404).end('outside base path');
    return;
  }
  const relative = path.slice(BASE.length);
  const file = join(ROOT, relative === '' ? 'index.html' : relative);
  try {
    let body = await readFile(file);
    if (EXPECT_FAILURE === 'canvas' && relative === '') {
      body = Buffer.from(body.toString().replace('id="sindri-canvas"', 'id="wrong-canvas"'));
    }
    if (EXPECT_FAILURE === 'webgpu' && relative === '') {
      // Navigator.gpu is no longer reliably forgeable in Chromium. The Rust
      // template test proves the guard exists; this forces that exact branch
      // so a real browser still proves the player-facing result.
      body = Buffer.from(body.toString().replace('else if (!navigator.gpu) {', 'else if (true) {'));
    }
    response.writeHead(200, { 'content-type': TYPES[extname(file)] ?? 'application/octet-stream' });
    response.end(body);
  } catch {
    response.writeHead(404).end('not found');
  }
});
await new Promise((done) => server.listen(0, done));
const port = server.address().port;

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

// The interface existing is not the same as WebGPU working. Chrome on Android
// exposes `navigator.gpu` more widely than its drivers can serve, and a page
// that only checks for the interface starts anyway and fails where nobody can
// see it — which is what a blank canvas on a phone turned out to be.
if (EXPECT_FAILURE === 'adapter') {
  await page.addInitScript(() => {
    GPU.prototype.requestAdapter = () => Promise.resolve(null);
  });
}

// The one failure the page cannot catch for itself. An adapter exists, so the
// checks pass and `init()` resolves — and then the device request fails inside
// the event loop winit has already handed to the page, with nobody to return
// it to. This is the case that proves the engine's failure event reaches the
// page, rather than the failure living in a console no player opens.
if (EXPECT_FAILURE === 'device') {
  await page.addInitScript(() => {
    GPUAdapter.prototype.requestDevice = () =>
      Promise.reject(new Error('the device request was refused for this test'));
  });
}

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
const fetchedAssets = new Set();
page.on('console', (message) => {
  if (message.type() === 'error') problems.push(message.text());
});
page.on('pageerror', (error) => problems.push(String(error.message)));
page.on('response', (response) => {
  if (!response.ok()) {
    problems.push(`HTTP ${response.status()} ${new URL(response.url()).pathname}`);
  }
});
page.on('request', (request) => {
  const path = new URL(request.url()).pathname;
  const marker = `${BASE}assets/`;
  if (path.startsWith(marker)) fetchedAssets.add(path.slice(marker.length));
});

await page.goto(`http://127.0.0.1:${port}${BASE}`, { waitUntil: 'load' });

if (EXPECT_FAILURE) {
  await page.waitForSelector('#sindri-error[data-visible="true"]');
  const message = await page.locator('#sindri-error').innerText();
  const expected = {
    canvas: 'missing the #sindri-canvas',
    webgpu: 'WebGPU is unavailable',
    adapter: 'WebGPU is unavailable',
    // Not the message's wording, which belongs to wgpu: what matters is that a
    // failure raised after startup finished arrived on the page at all.
    device: 'Gather stopped',
  }[EXPECT_FAILURE];
  if (SHOT) await page.screenshot({ path: SHOT });
  await browser.close();
  server.close();
  console.log(`expected startup failure: ${message.replaceAll('\n', ' ')}`);
  if (!message.includes(expected)) {
    console.log(`problem: expected the failure UI to mention '${expected}'`);
    process.exit(1);
  }
  console.log('the page surfaced the expected startup failure');
  process.exit(0);
}

const webgpu = await page.evaluate(() => Boolean(navigator.gpu));
await page.waitForTimeout(6000);

await page.mouse.click(480, 270);
await page.keyboard.press('KeyD');
await page.waitForTimeout(2000);
const audio = await page.evaluate(() =>
  window.__plays.map((record) => ({
    settled: record.settled,
    playedTo: record.element ? Number(record.element.currentTime.toFixed(2)) : 0,
  })),
);

const canvas = await page.evaluate(() => {
  const element = document.querySelector('canvas');
  return element ? { width: element.width, height: element.height } : null;
});
if (SHOT) await page.screenshot({ path: SHOT });
await browser.close();
server.close();

const started = Boolean(canvas) && canvas.width > 300;
const refused = audio.filter(
  (record) => record.settled !== 'played' || record.playedTo === 0,
);

const requiredAssetKinds = [
  ['manifest', 'sindri.manifest.json'],
  ['scene', 'gather.scene.json'],
  ['script', 'scripts/player.decay'],
  ['texture', 'textures/player.png'],
  ['sheet', 'textures/player.sheet.json'],
  ['font', 'fonts/Inter.ttf'],
  ['audio', 'audio/background.wav'],
];
// Exported assets sit below a content-hash directory; the manifest itself
// deliberately does not. Match the logical ID at the end of either shape.
const fetched = (asset) =>
  [...fetchedAssets].some((path) => path === asset || path.endsWith(`/${asset}`));
const missingAssets = EXPECT_ASSETS
  ? requiredAssetKinds.filter(([, asset]) => !fetched(asset))
  : [];

console.log(`webgpu: ${webgpu ? 'yes' : 'no'}`);
console.log(`canvas: ${canvas ? `${canvas.width}x${canvas.height}` : 'none'}`);
console.log(`base path: ${BASE}`);
console.log(
  audio.length === 0
    ? 'audio: none requested'
    : `audio: ${audio.length - refused.length}/${audio.length} playing`,
);
if (EXPECT_ASSETS) console.log(`assets fetched: ${fetchedAssets.size}`);
for (const [kind, asset] of missingAssets) {
  console.log(`problem: no HTTP ${kind} request for assets/${asset}`);
}
for (const record of refused) console.log(`problem: audio ${record.settled}`);
for (const problem of problems) console.log(`problem: ${problem}`);
if (!webgpu || !started || problems.length > 0 || refused.length > 0 || missingAssets.length > 0) {
  console.log('the page did not start the engine');
  process.exit(1);
}
console.log('the engine ran in a browser');
