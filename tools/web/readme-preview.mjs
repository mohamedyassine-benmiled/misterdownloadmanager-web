// Local GitHub-style reading preview only; not a replacement website or a shipped server route.
// Requires Python-Markdown (also used by tools/website/build.py).
import http from 'node:http';
import { execFileSync } from 'node:child_process';
import { readFile, mkdir } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
const { chromium } = await import(process.env.MDM_PLAYWRIGHT_MODULE || 'playwright');
const root = new URL('../../', import.meta.url);
const source = await readFile(new URL('README.md', root), 'utf8');
const body = execFileSync('python', ['-c', "import markdown,sys; print(markdown.markdown(sys.stdin.read(), extensions=['tables','fenced_code','toc']))"], { input: source, encoding: 'utf8' });
const server = http.createServer(async (request, response) => {
  const path = new URL(request.url, 'http://127.0.0.1').pathname;
  if (path === '/') {
    response.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' }).end(`<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>Webmister README preview</title><style>
    *{box-sizing:border-box}body{margin:0;background:#fff;color:#1f2328;font:16px/1.65 -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}article{max-width:1012px;margin:28px auto;padding:32px 40px;border:1px solid #d1d9e0;border-radius:6px}h1{font-size:2em}h1,h2{padding-bottom:.3em;border-bottom:1px solid #d1d9e0;line-height:1.25}h1,h2,h3{margin:24px 0 16px}a{color:#0969da;text-decoration:none}a:hover{text-decoration:underline}img{max-width:100%;height:auto}picture{display:block}pre{overflow:auto;padding:16px;background:#f6f8fa;border-radius:6px;font:13px/1.5 ui-monospace,Consolas,monospace}code{font:85% ui-monospace,Consolas,monospace;overflow-wrap:anywhere}p,ul,ol,table,blockquote,details{margin:0 0 16px}table{border-collapse:collapse;display:block;overflow:auto}td,th{border:1px solid #d1d9e0;padding:6px 13px}tr:nth-child(even){background:#f6f8fa}blockquote{padding:0 1em;color:#59636e;border-inline-start:3px solid #d1d9e0}summary{cursor:pointer}summary:focus-visible,a:focus-visible{outline:2px solid #0969da;outline-offset:3px}@media(max-width:700px){article{margin:0;border:0;padding:20px 16px}h1{font-size:26px}}
    </style></head><body><article>${body}</article></body></html>`); return;
  }
  if (!/^\/(brand\/webmister\.svg|docs\/images\/webmister\/[a-z-]+\.png)$/.test(path)) { response.writeHead(404).end(); return; }
  try { response.writeHead(200, { 'Content-Type': path.endsWith('.svg') ? 'image/svg+xml' : 'image/png' }).end(await readFile(new URL(path.slice(1), root))); }
  catch { response.writeHead(404).end(); }
});
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const browser = await chromium.launch({ headless: true });
const out = new URL('target/webmister-readme-review/', root); await mkdir(out, { recursive: true });
try {
  for (const [name, width] of [['desktop', 1440], ['mobile', 390]]) {
    const page = await browser.newPage({ viewport: { width, height: 1000 } });
    await page.goto(`http://127.0.0.1:${server.address().port}/`);
    await page.locator('picture img').waitFor({ state: 'visible' });
    await page.screenshot({ path: fileURLToPath(new URL(name + '-opening.png', out)) });
    await page.screenshot({ path: fileURLToPath(new URL(name + '.png', out)), fullPage: true });
    await page.close();
  }
} finally { await browser.close(); await new Promise((r) => server.close(r)); }
