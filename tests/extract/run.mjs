// Automated tests for src-tauri/src/chrome/extract.js.
// Runs the real extraction script in headless Chrome against saved example pages.
//
//   yarn test:extract                 run all fixtures
//   yarn test:extract --show <name>   print the full extraction of one fixture
//   yarn test:extract --url <url>     print the extraction of a live page (manual smoke test)
//
// Requires Google Chrome (set CHROME_PATH to override) and Node 20.10+ (--experimental-websocket).
import { spawn } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const SCRIPT = readFileSync(resolve(here, '../../src-tauri/src/chrome/extract.js'), 'utf8');
const FIXTURES = join(here, 'fixtures');

const CHROME_CANDIDATES = [
  process.env.CHROME_PATH,
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
  '/usr/bin/google-chrome',
  '/usr/bin/chromium',
  'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe',
].filter(Boolean);

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function launchChrome() {
  const chromePath = CHROME_CANDIDATES.find((p) => existsSync(p));
  if (!chromePath) throw new Error('Google Chrome not found. Set CHROME_PATH.');
  const profile = mkdtempSync(join(tmpdir(), 'extract-test-'));
  const proc = spawn(chromePath, [
    '--headless=new',
    '--remote-debugging-port=0',
    `--user-data-dir=${profile}`,
    '--no-first-run',
    '--no-default-browser-check',
    '--allow-file-access-from-files',
    '--window-size=1280,900',
    'about:blank',
  ], { stdio: 'ignore' });
  const portFile = join(profile, 'DevToolsActivePort');
  for (let i = 0; i < 100 && !existsSync(portFile); i++) await sleep(100);
  if (!existsSync(portFile)) throw new Error('Chrome did not start (no DevToolsActivePort).');
  const port = readFileSync(portFile, 'utf8').split('\n')[0].trim();
  const version = await (await fetch(`http://127.0.0.1:${port}/json/version`)).json();
  const cdp = await connect(version.webSocketDebuggerUrl);
  return {
    cdp,
    async close() {
      cdp.close();
      proc.kill();
      await sleep(200);
      rmSync(profile, { recursive: true, force: true });
    },
  };
}

function connect(url) {
  return new Promise((resolveConn, reject) => {
    const ws = new WebSocket(url);
    let nextId = 1;
    const pending = new Map();
    const listeners = new Set();
    ws.onmessage = (ev) => {
      const msg = JSON.parse(ev.data);
      if (msg.id && pending.has(msg.id)) {
        const { res, rej } = pending.get(msg.id);
        pending.delete(msg.id);
        if (msg.error) rej(new Error(msg.error.message));
        else res(msg.result);
      } else if (msg.method) {
        for (const l of listeners) l(msg);
      }
    };
    ws.onerror = reject;
    ws.onopen = () => resolveConn({
      send(method, params = {}, sessionId) {
        const id = nextId++;
        ws.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
        return new Promise((res, rej) => pending.set(id, { res, rej }));
      },
      waitFor(method, sessionId, timeoutMs = 15000) {
        return new Promise((res, rej) => {
          const timer = setTimeout(() => { listeners.delete(fn); rej(new Error(`Timed out waiting for ${method}`)); }, timeoutMs);
          const fn = (msg) => {
            if (msg.method === method && msg.sessionId === sessionId) {
              clearTimeout(timer);
              listeners.delete(fn);
              res(msg.params);
            }
          };
          listeners.add(fn);
        });
      },
      close() { ws.close(); },
    });
  });
}

async function extract(cdp, url, settleMs = 300) {
  const { targetId } = await cdp.send('Target.createTarget', { url: 'about:blank' });
  const { sessionId } = await cdp.send('Target.attachToTarget', { targetId, flatten: true });
  try {
    await cdp.send('Page.enable', {}, sessionId);
    const loaded = cdp.waitFor('Page.loadEventFired', sessionId, 30000);
    await cdp.send('Page.navigate', { url }, sessionId);
    await loaded;
    await sleep(settleMs);
    const res = await cdp.send('Runtime.evaluate', { expression: SCRIPT, returnByValue: true, awaitPromise: true }, sessionId);
    if (res.exceptionDetails) throw new Error(`Script error: ${JSON.stringify(res.exceptionDetails)}`);
    return JSON.parse(res.result.value);
  } finally {
    await cdp.send('Target.closeTarget', { targetId });
  }
}

const countOf = (haystack, needle) => haystack.split(needle).length - 1;

function check(result, expect) {
  const failures = [];
  const text = result.text;
  for (const s of expect.contains || []) if (!text.includes(s)) failures.push(`missing: ${JSON.stringify(s)}`);
  for (const s of expect.notContains || []) if (text.includes(s)) failures.push(`should not contain: ${JSON.stringify(s)}`);
  for (const [s, max] of Object.entries(expect.maxCount || {})) {
    const n = countOf(text, s);
    if (n > max) failures.push(`${JSON.stringify(s)} appears ${n} times (max ${max})`);
  }
  if (expect.images !== undefined && result.images.length !== expect.images) failures.push(`images: got ${result.images.length}, expected ${expect.images}`);
  if (expect.drawn !== undefined && result.drawn.length !== expect.drawn) failures.push(`drawn: got ${result.drawn.length}, expected ${expect.drawn}`);
  if (expect.code !== undefined && result.code.length !== expect.code) failures.push(`code editors: got ${result.code.length}, expected ${expect.code}`);
  if (expect.codeLanguages) {
    const langs = result.code.map((c) => c.language);
    if (JSON.stringify(langs) !== JSON.stringify(expect.codeLanguages)) failures.push(`code languages: got ${JSON.stringify(langs)}, expected ${JSON.stringify(expect.codeLanguages)}`);
  }
  if (expect.root && result.root !== expect.root) failures.push(`root: got ${result.root}, expected ${expect.root}`);
  if (expect.minFramesSkipped !== undefined && result.framesSkipped < expect.minFramesSkipped) failures.push(`framesSkipped: got ${result.framesSkipped}, expected >= ${expect.minFramesSkipped}`);
  if (expect.imageNaturalWidth && result.images[0]?.naturalWidth !== expect.imageNaturalWidth) failures.push(`image naturalWidth: got ${result.images[0]?.naturalWidth}, expected ${expect.imageNaturalWidth}`);
  if (/[\u2066-\u2069\u200b\ufeff]/.test(text)) failures.push('text contains invisible bidi/zero-width characters');
  if (result.error) failures.push(`script fell back after an error: ${result.error}`);
  return failures;
}

async function main() {
  const args = process.argv.slice(2);
  const browser = await launchChrome();
  let failed = 0;
  try {
    if (args[0] === '--url') {
      const r = await extract(browser.cdp, args[1], 2500);
      console.log(JSON.stringify({ ...r, text: undefined }, null, 2));
      console.log('\n===== TEXT =====\n' + r.text);
      return;
    }
    const names = readdirSync(FIXTURES).filter((f) => f.endsWith('.html')).map((f) => f.slice(0, -5)).sort();
    const only = args[0] === '--show' ? [args[1]] : names;
    for (const name of only) {
      const url = pathToFileURL(join(FIXTURES, `${name}.html`)).href;
      const result = await extract(browser.cdp, url);
      if (args[0] === '--show') {
        console.log(JSON.stringify({ ...result, text: undefined }, null, 2));
        console.log('\n===== TEXT =====\n' + result.text);
        continue;
      }
      const expect = JSON.parse(readFileSync(join(FIXTURES, `${name}.expect.json`), 'utf8'));
      const failures = check(result, expect);
      if (failures.length) {
        failed++;
        console.log(`FAIL  ${name}`);
        for (const f of failures) console.log(`      - ${f}`);
      } else {
        console.log(`PASS  ${name}`);
      }
    }
    if (args[0] !== '--show') console.log(`\n${only.length - failed}/${only.length} passed`);
  } finally {
    await browser.close();
  }
  if (failed) process.exitCode = 1;
}

main().catch((e) => {
  console.error(e);
  process.exitCode = 1;
});
