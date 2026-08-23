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

if (EXPECT_FAILURE === 'webgpu') {
  await page.addInitScript(() => {
    Object.defineProperty(Navigator.prototype, 'gpu', {
      configurable: true,
      get: () => undefined,
    });
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
page.on('request', (request) => {
  const path = new URL(request.url()).pathname;
  const marker = `${BASE}assets/`;
  if (path.startsWith(marker)) fetchedAssets.add(path.slice(marker.length));
});

await page.goto(`http://127.0.0.1:${port}${BASE}`, { waitUntil: 'load' });

if (EXPECT_FAILURE) {
  await page.waitForSelector('#sindri-error[data-visible="true"]');
  const message = await page.locator('#sindri-error').innerText();
  const expected = EXPECT_FAILURE === 'webgpu' ? 'WebGPU is unavailable' : 'missing the #sindri-canvas';
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
const missingAssets = EXPECT_ASSETS
  ? requiredAssetKinds.filter(([, asset]) => !fetchedAssets.has(asset))
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
