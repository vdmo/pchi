#!/usr/bin/env node
/**
 * The cables.gl leg of the multi-tool live show demo.
 *
 * Drives the real, live, publicly-hosted cables.gl editor via headless
 * Chromium — not a mock — pointing its real WebSocket_v2 op at this
 * demo's running cables-pchi-relay, then runs a caller-supplied trigger
 * command (sending the actual control update through a *different*
 * tool's bridge) and confirms the verdict arrives in the op's own
 * parameter panel. This is the "one truth" proof: the update doesn't
 * come from cables.gl's own HTTP endpoint, so receiving it live proves
 * propagation is tool-agnostic, not just a same-tool echo.
 *
 * Getting a public-origin page to reach a Conductor on localhost means
 * working through a real Chrome security boundary (Local Network
 * Access) — see the --disable-features flags below, and
 * tools/cables-pchi-relay/README.md's "Verification status" section
 * for how this was discovered.
 */
const puppeteer = require('puppeteer-core');
const { execSync } = require('child_process');
const http = require('http');

function sleep(ms) { return new Promise(r => setTimeout(r, ms)); }

function arg(name, fallback) {
  const i = process.argv.indexOf(`--${name}`);
  return i !== -1 ? process.argv[i + 1] : fallback;
}

async function getCablesFrame(page) {
  return page.frames().find(f => f.url().includes('sandbox.cables.gl'));
}

// Asks the relay itself whether a cables client is actually connected
// right now — ground truth, not an assumption from "we set the URL
// field once a while ago." The op's connection can drop and not
// reconnect on its own (seen in practice right after a URL change).
function relayClientsConnected(httpPort) {
  return new Promise((resolve) => {
    const req = http.get({ hostname: '127.0.0.1', port: httpPort, path: '/status', timeout: 3000 }, res => {
      let data = '';
      res.on('data', c => data += c);
      res.on('end', () => {
        try { resolve(JSON.parse(data).cables_clients_connected); }
        catch { resolve(0); }
      });
    });
    req.on('error', () => resolve(0));
    req.on('timeout', () => { req.destroy(); resolve(0); });
  });
}

async function main() {
  const relayWsPort = arg('relay-ws-port', '9101');
  const relayHttpPort = arg('relay-http-port', '9100');
  const triggerCmd = arg('trigger-cmd', null);
  const chromiumPath = arg('chromium-path', '/snap/bin/chromium');
  const screenshotPath = arg('screenshot', 'cables-leg-result.png');

  const browser = await puppeteer.launch({
    executablePath: chromiumPath,
    headless: true,
    args: [
      '--no-sandbox', '--disable-setuid-sandbox', '--enable-unsafe-swiftshader',
      '--disable-features=LocalNetworkAccessChecks,LocalNetworkAccessChecksWebSockets',
    ],
    defaultViewport: { width: 1700, height: 1000 },
    protocolTimeout: 180000,
  });
  const page = await browser.newPage();

  let sawFirstReceive = false;
  page.on('console', msg => {
    const t = msg.text();
    if (/GL Driver|WebGL-0x/.test(t)) return;
    if (t.includes('WebSocket_v2')) sawFirstReceive = true;
  });

  console.log('[cables leg] loading the live cables.gl WebSocket example patch...');
  await page.goto('https://cables.gl/edit/gu7DBo', { waitUntil: 'networkidle2', timeout: 30000 }).catch(() => {});

  // 120s, not 60s: this environment has seen real, unrelated CPU
  // contention (other processes at 200%+) stretch a normally-fast page
  // load well past a minute. Generous on purpose, not a sign anything's
  // actually slow about the patch itself.
  const deadline = Date.now() + 120000;
  while (!sawFirstReceive && Date.now() < deadline) await sleep(500);
  console.log('[cables leg] patch is live:', sawFirstReceive);
  if (!sawFirstReceive) {
    console.log('[cables leg] WARNING: readiness signal never seen — proceeding anyway, but node selection may fail.');
  }

  // Pan-independent node selection: fixed canvas coordinates aren't
  // reliable across page loads, search->click->Enter is.
  await page.mouse.click(726, 559).catch(() => {}); // close tutorial overlay if present
  await sleep(1500);
  await page.mouse.move(500, 500);
  await page.keyboard.down('Control'); await page.keyboard.press('KeyF'); await page.keyboard.up('Control');
  await sleep(1500);
  await page.keyboard.type('WebSocket', { delay: 80 });
  await sleep(1500);
  await page.mouse.click(175, 300);
  await sleep(800);
  await page.keyboard.press('Enter');
  await sleep(2000);

  const frame = await getCablesFrame(page);
  const relayUrl = `ws://127.0.0.1:${relayWsPort}`;

  async function pointOpAtRelay() {
    return frame.evaluate((newUrl) => {
      const el = Array.from(document.querySelectorAll('input[class*="watchPortValue"]'))[0];
      if (!el) return null;
      el.focus();
      const setter = Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype, 'value').set;
      setter.call(el, newUrl);
      el.dispatchEvent(new Event('input', { bubbles: true }));
      el.dispatchEvent(new Event('change', { bubbles: true }));
      el.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
      el.dispatchEvent(new KeyboardEvent('keyup', { key: 'Enter', bubbles: true }));
      el.blur();
      return el.value;
    }, relayUrl);
  }

  const setUrl = await pointOpAtRelay();
  if (setUrl !== relayUrl) {
    console.log('[cables leg] FAILED: could not point the real op at the relay.');
    await page.screenshot({ path: screenshotPath });
    await browser.close();
    process.exit(1);
  }
  console.log('[cables leg] real WebSocket_v2 op now connected to', relayUrl);

  // Don't trust that setting the URL means a connection exists by the
  // time we trigger — ask the relay. The op's connection has been
  // observed to drop shortly after a URL change and not reconnect on
  // its own; if that's happened, re-apply the URL to force a fresh
  // connection attempt rather than sending a trigger into a dead socket.
  let connected = 0;
  for (let i = 0; i < 10 && connected === 0; i++) {
    await sleep(1000);
    connected = await relayClientsConnected(relayHttpPort);
  }
  console.log('[cables leg] relay reports', connected, 'cables client(s) connected');

  let result = { allFields: [], rawData: null };
  const maxAttempts = 3;
  for (let attempt = 1; attempt <= maxAttempts; attempt++) {
    if (connected === 0) {
      console.log('[cables leg] relay sees no connected client — re-pointing the op to force reconnect');
      await pointOpAtRelay();
      for (let i = 0; i < 10 && connected === 0; i++) {
        await sleep(1000);
        connected = await relayClientsConnected(relayHttpPort);
      }
      console.log('[cables leg] relay now reports', connected, 'cables client(s) connected');
    }

    if (triggerCmd) {
      console.log(`[cables leg] attempt ${attempt}/${maxAttempts}: triggering via a DIFFERENT tool's bridge:`, triggerCmd);
      console.log(execSync(triggerCmd).toString().trim());
    }
    await sleep(5000);
    result = await frame.evaluate(() => {
      const fields = Array.from(document.querySelectorAll('input[type="text"], textarea'))
        .map(el => ({ cls: el.className, value: el.value }))
        .filter(x => x.value && x.value.length > 0);
      const raw = fields.find(f => f.value.includes('gate_state'));
      return { allFields: fields, rawData: raw ? raw.value : null };
    });
    if (result.rawData) break;
    console.log(`[cables leg] attempt ${attempt} saw nothing yet, retrying...`);
    connected = await relayClientsConnected(relayHttpPort);
  }

  await page.screenshot({ path: screenshotPath });
  await browser.close();

  console.log('[cables leg] RESULT_JSON_START');
  console.log(JSON.stringify(result, null, 1));
  console.log('[cables leg] RESULT_JSON_END');

  if (!result.rawData) {
    console.log('[cables leg] FAILED: no gate_state payload observed in the op\'s Raw Data output.');
    process.exit(1);
  }
  console.log('[cables leg] SUCCESS: the real cables.gl op received', result.rawData);
}

main().catch(e => { console.error('[cables leg] FAILED:', e.message); process.exit(1); });
