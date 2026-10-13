import { chromium } from '@playwright/test';
import { spawn, execFileSync } from 'node:child_process';
import { mkdir } from 'node:fs/promises';
import { resolve } from 'node:path';
import assert from 'node:assert/strict';

// Tests the actual EXE, SQLite, native theme command and WebView2 IPC, not fixtures.
// Remote debugging is enabled ONLY on the child process for this local test.
const executable = resolve(
  process.argv[2] ?? 'src-tauri/target/release/teledrop.exe',
);
const output = resolve('test-results/desktop');
await mkdir(output, { recursive: true });
let child;
let browser;
let original;
let page;
const pause = (ms) => new Promise((done) => setTimeout(done, ms));

async function start() {
  child = spawn(executable, [], {
    env: {
      ...process.env,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9223',
    },
    stdio: 'ignore',
  });
  for (let attempt = 0; attempt < 80; attempt++) {
    if (child.exitCode !== null)
      throw new Error(`TeleDrop exited: ${child.exitCode}`);
    try {
      const response = await fetch('http://127.0.0.1:9223/json/version');
      if (response.ok) break;
    } catch {
      /* WebView2 is starting. */
    }
    if (attempt === 79)
      throw new Error('WebView2 debugging endpoint did not start');
    await pause(250);
  }
  browser = await chromium.connectOverCDP('http://127.0.0.1:9223');
  for (let attempt = 0; attempt < 40; attempt++) {
    page = browser
      .contexts()
      .flatMap((context) => context.pages())
      .find((candidate) => /tauri\.localhost|tauri:\/\//.test(candidate.url()));
    if (page) break;
    await pause(250);
  }
  if (!page) throw new Error('TeleDrop WebView was not found');
  await page.getByRole('heading', { name: 'Обзор', exact: true }).waitFor();
}

async function invoke(command, args = {}) {
  return page.evaluate(
    ({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args),
    { command, args },
  );
}

async function close() {
  await browser?.close();
  browser = undefined;
  if (child && child.exitCode === null) {
    execFileSync('pwsh', [
      '-NoProfile',
      '-Command',
      `(Get-Process -Id ${child.pid}).CloseMainWindow() | Out-Null`,
    ]);
    for (let attempt = 0; attempt < 80 && child.exitCode === null; attempt++)
      await pause(250);
    if (child.exitCode === null)
      throw new Error('TeleDrop did not gracefully exit');
  }
}

try {
  await start();
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  const snapshot = await invoke('snapshot');
  original = snapshot.settings;
  assert.equal(snapshot.version, '0.1.0');
  if (snapshot.tokenConfigured || snapshot.settings.autoConnect)
    throw new Error(
      'Desktop smoke requires an unconfigured bot and autoConnect=false. Do not run on an active bot.',
    );
  for (const theme of ['light', 'dark']) {
    await invoke('save_settings', { settings: { ...original, theme } });
    await page.waitForFunction(
      (theme) => document.documentElement.dataset.theme === theme,
      theme,
    );
    for (const [navigation, heading] of [
      ['Обзор', 'Обзор'],
      ['Загрузки', 'Загрузки'],
      ['История', 'История'],
      ['Настройки', 'Настройки'],
      ['Журнал', 'Журнал событий'],
    ]) {
      await page
        .locator('aside')
        .getByRole('button', { name: navigation, exact: true })
        .click();
      await page.getByRole('heading', { name: heading, exact: true }).waitFor();
      if (navigation === 'История')
        await page.getByText('Читаем историю…').waitFor({ state: 'hidden' });
      assert.ok((await page.locator('svg').count()) >= 5);
      await page.screenshot({
        path: resolve(output, `${theme}-${navigation}.png`),
      });
    }
  }
  assert.deepEqual(errors, []);
  await close();
  await start();
  assert.equal((await invoke('snapshot')).settings.theme, 'dark');
  await page.waitForFunction(
    () => document.documentElement.dataset.theme === 'dark',
  );
  await invoke('save_settings', { settings: original });
  original = undefined;
  await close();
  console.log(
    'PASS: release EXE starts; 5 pages × 2 themes render; real SQLite theme survives restart; shutdown completes.',
  );
} finally {
  if (original && page && browser) {
    try {
      await invoke('save_settings', { settings: original });
    } catch {
      /* Preserve failure for diagnostics. */
    }
  }
  await close();
}
