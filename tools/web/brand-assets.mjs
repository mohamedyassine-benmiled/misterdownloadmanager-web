// Reproduce the fork's raster icons from the authored SVG; no edits to upstream artwork.
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
const { chromium } = await import(process.env.MDM_PLAYWRIGHT_MODULE || 'playwright');
const root = new URL('../../', import.meta.url);
const svg = (await readFile(new URL('brand/webmister.svg', root), 'utf8')).replaceAll('\r\n', '\n');
const browser = await chromium.launch({ headless: true });
const icons = new URL('extension/icons/', root);
await mkdir(icons, { recursive: true });
try {
  for (const size of [16, 32, 48, 128, 256]) {
    const page = await browser.newPage({ viewport: { width: size, height: size }, deviceScaleFactor: 1 });
    await page.setContent(`<style>html,body{margin:0;width:100%;height:100%;background:transparent}svg{display:block;width:100%;height:100%}</style>${svg}`);
    await page.screenshot({ path: fileURLToPath(new URL(`webmister-${size}.png`, icons)), omitBackground: true });
    await page.close();
  }
  await writeFile(new URL('brand/assets.json', root), JSON.stringify({
    source: 'brand/webmister.svg', source_sha256: createHash('sha256').update(svg).digest('hex'),
    license: 'GPL-3.0-or-later', method: 'Authored SVG rasterized without retouching using Chromium/Playwright.',
    generated: [16, 32, 48, 128, 256].map((n) => `extension/icons/webmister-${n}.png`),
    command: 'node tools/web/brand-assets.mjs',
  }, null, 2) + '\n');
} finally { await browser.close(); }
