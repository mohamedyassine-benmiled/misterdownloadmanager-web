// Public documentation assets. Never connect to the private review server/account.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { startPreview } from './preview.mjs';
const { chromium } = await import(process.env.MDM_PLAYWRIGHT_MODULE || 'playwright');
const root = new URL('../../', import.meta.url);
const out = new URL('docs/images/webmister/', root);
await mkdir(out, { recursive: true });
const preview = await startPreview();
const browser = await chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1440, height: 940 }, deviceScaleFactor: 1 });
const errors = []; page.on('pageerror', (e) => errors.push(e.message));
const captures = [];
async function capture(name, subject, selector = null) {
  await page.evaluate(() => { document.activeElement?.blur(); scrollTo(0, 0); });
  await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
  assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1), name + ': no document overflow');
  const options = { path: fileURLToPath(new URL(name + '.png', out)) };
  if (selector) await page.locator(selector).screenshot(options);
  else await page.screenshot({ ...options, fullPage: true });
  captures.push({ file: name + '.png', subject,
    viewport: page.viewportSize(), origin: 'Unretouched Chromium screenshot of the shipped Web UI against tools/web/preview.mjs. All transfers are synthetic, as labeled in the image.' });
}
try {
  await page.goto(preview.url); await page.locator('#downloads-table').waitFor({ state: 'visible' });
  await page.locator('#theme').selectOption('light');
  assert.equal(await page.title(), 'Webmister Download Manager');
  await capture('workspace-light', 'Downloads, categories and persistent queue toolbar, light theme');
  await page.locator('#theme').selectOption('dark');
  await capture('workspace-dark', 'Same workspace, dark theme');
  await page.locator('#theme').selectOption('light');
  await page.locator('#download-list tr').filter({ hasText: 'sample-debian' }).locator('.file-source').click();
  await page.locator('#detail-connections tr').first().waitFor();
  await capture('download-details', 'Desktop-familiar progress dialog with connection details', '#details');
  await page.locator('#close-details').click();
  await page.goto(preview.url + '?mode=torrent'); await page.locator('#downloads-table').waitFor({ state: 'visible' });
  await page.locator('#download-list tr').filter({ hasText: 'sample-open-data' }).locator('.file-source').click();
  await page.locator('#detail-tab-torrent').click();
  await capture('torrent-files', 'Post-add torrent file selection and verification details', '#details');
  await page.locator('#close-details').click();
  await page.goto(preview.url); await page.locator('#downloads-table').waitFor({ state: 'visible' });
  await page.locator('#show-add').click();
  await page.locator('#url').fill('https://downloads.example.test/sample-project-assets.zip');
  await capture('add-url', 'Add a synthetic public download link', '#add-panel');
  await page.locator('#add-tab-torrent').click();
  await page.locator('#url').fill('magnet:?xt=urn:btih:0123456789012345678901234567890123456789');
  await capture('add-torrent', 'Add a synthetic magnet or choose a torrent file', '#add-panel');
  await page.locator('#dismiss-add').click();
  await page.setViewportSize({ width: 1440, height: 1600 });
  for (const [view, name, subject] of [
    ['settings', 'settings-downloads', 'Supported application settings with synthetic server defaults'],
    ['security', 'settings-security', 'Owner authentication and session policy with synthetic values'],
    ['devices', 'devices', 'Device pairing controls and a synthetic extension device'],
    ['queues', 'queues', 'Queue concurrency and a synthetic overnight schedule'],
    ['categories', 'categories', 'Category and bounded server folder controls with synthetic values'],
  ]) {
    await page.locator(`[data-page="${view}"]`).first().click();
    await page.locator(`[data-view="${view}"]`).waitFor({ state: 'visible' });
    await page.locator('#section-loading').waitFor({ state: 'hidden' });
    if (view === 'settings') await page.locator('#settings-fields input').first().waitFor();
    if (view === 'devices') await page.locator('#device-list').getByText('Synthetic Firefox extension').waitFor();
    if (view === 'queues') {
      await page.locator('#new-queue').click();
      await page.locator('#queue-name').fill('Synthetic overnight downloads');
      await page.locator('#queue-concurrency').fill('2');
      await page.locator('#queue-start').fill('01:00:00');
      await page.locator('#queue-stop').fill('06:00:00');
    }
    if (view === 'categories') {
      await page.locator('#new-category').click();
      await page.locator('#category-name').fill('Synthetic archives');
      await page.locator('#category-types').fill('ZIP 7Z TAR GZ');
      await page.locator('#category-folder').fill('Archives');
    }
    const selector = view === 'settings' ? '#settings-fields fieldset:first-child'
      : view === 'security' ? '#auth-settings-form' : '#management-dialog';
    await capture(name, subject, selector);
    await page.locator('#close-management').click();
  }
  await page.goto(preview.url); await page.locator('#downloads-table').waitFor({ state: 'visible' });
  await page.setViewportSize({ width: 390, height: 844 });
  await capture('workspace-mobile', 'Downloads workspace on a narrow mobile viewport');
  assert.deepEqual(errors, []);
  const sources = {};
  for (const file of ['crates/remote/web/index.html', 'crates/remote/web/app.css', 'crates/remote/web/app.js', 'tools/web/preview.mjs']) {
    sources[file] = createHash('sha256').update((await readFile(new URL(file, root), 'utf8')).replaceAll('\r\n', '\n')).digest('hex');
  }
  await writeFile(new URL('captures.json', out), JSON.stringify({ command: 'node tools/web/readme-shots.mjs', source_hash_normalization: 'UTF-8 text with LF line endings', sources, captures }, null, 2) + '\n');
} finally { await page.close(); await browser.close(); await preview.close(); }
