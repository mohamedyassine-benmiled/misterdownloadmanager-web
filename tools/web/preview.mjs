// Development-only HTTP simulator. All transfers and credentials are synthetic.
// Real authentication is tested separately by crates/remote/tests/server.rs.
import http from 'node:http';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = new URL('../../', import.meta.url);
const read = (path) => readFileSync(new URL(path, root), 'utf8');
const password = 'correct horse battery staple';
const decode = (s) => s.replace(/\\([\\nt])/g, (_, c) => ({ n: '\n', t: '\t', '\\': '\\' })[c]);
function translations(lang) {
  const texts = {};
  let source; try { source = read(`i18n/${lang}.tsv`); } catch { source = read('i18n/en.tsv'); }
  for (const row of source.split(/\r?\n/)) {
    if (!row || row.startsWith('#')) continue;
    const [context, key, value] = row.split('\t');
    if (key && value && (context === '' || context === 'unit')) texts[(context ? 'unit:' : '') + decode(key)] = decode(value);
  }
  return { lang, rtl: lang === 'ar', texts };
}
function fields() {
  const source = read('crates/app/src/app/web.rs');
  return [...source.matchAll(/field!\(\s*"([^"]+)",\s*"([^"]+)",\s*"([^"]+)",\s*"([^"]+)",\s*(\d+),\s*(\d+)(?:u64)?\s*\)/g)]
    .map(([, key, label, section, kind, min, max]) => ({ key, label, section, kind, min: Number(min), max: Number(max) }));
}
function fresh() {
  const definitions = fields();
  const values = Object.fromEntries(definitions.map((f) => [f.key, f.kind === 'boolean' ? false : f.kind === 'number' ? f.min : '']));
  values.max_connections = 8; values.auto_types = 'ZIP ISO PDF MP4'; values.dns_provider = 'system';
  return { configured: true, authenticated: true, owner: true, username: 'owner', csrf: 'synthetic-csrf', revision: 1, lang: 'en', mode: 'workspace', password,
    downloads: [
      { id: 1, file: 'sample-debian-13.1-amd64.iso', url: 'https://downloads.example.test/debian.iso', status: 'Downloading', size: 4000000000, downloaded: 2380000000, speed: 12800000, category: 'General' },
      { id: 2, file: 'sample-project-assets.zip', url: 'https://downloads.example.test/assets.zip', status: 'Queued', size: 183000000, downloaded: 0, speed: 0, category: 'Compressed' },
      { id: 3, file: 'sample-documentation.pdf', url: 'https://downloads.example.test/guide.pdf', status: 'Complete', size: 4200000, downloaded: 4200000, speed: 0, category: 'General' },
      { id: 4, file: 'sample-backup-with-a-long-filename-that-must-not-overflow-the-workspace.tar.gz', url: 'https://downloads.example.test/backup.tar.gz', status: 'Error', size: null, downloaded: 1200, speed: 0, category: 'General', error: 'Synthetic fixture: the source link expired.' },
      { id: 5, file: 'sample-recording.mp4', url: 'https://downloads.example.test/recording.mp4', status: 'Stopped', size: 89000000, downloaded: 24000000, speed: 0, category: 'General' },
    ],
    queues: [{ name: 'Main download queue', files_at_once: 4, running: false, sync: false, ids: [2], schedule: { days: [true,true,true,true,true,true,true], start_at: null, stop_at: null, retries: null, once: null, start_on_startup: false } }, { name: 'Synchronization queue', files_at_once: 4, running: false, sync: true, ids: [], schedule: { days: [true,true,true,true,true,true,true] } }],
    categories: [{ key: 'General', name: 'General', folder: '', types: '', custom_id: null }, { key: 'Compressed', name: 'Compressed', folder: 'Compressed', types: '', custom_id: null }],
    definitions, values, devices: [{ id: 'fixture-device', name: 'Synthetic Firefox extension', seen: 1794182400, added: 1794180000 }],
    policy: { bypass_localhost: false, bypass_subnets: false, subnets: [], trusted_proxies: [], max_failures: 5, ban_seconds: 3600, session_timeout: 3600 },
  };
}

export async function startPreview(port = 0) {
  let state = fresh(), delayed = null; const requests = [];
  const snapshot = () => ({ downloads: state.downloads, categories: state.categories, queues: state.queues, speed: state.downloads.reduce((n, d) => n + d.speed, 0), free: 328000000000 });
  const config = () => ({ revision: String(state.revision), fields: state.definitions, values: state.values, categories: state.categories, queues: state.queues, server: { version: 'synthetic-fixture', downloads_root: '/downloads', timezone: 'Etc/UTC' } });
  const security = () => ({ revision: state.revision, username: state.username, policy: state.policy, bans: {} });
  const server = http.createServer(async (req, res) => {
    const url = new URL(req.url, 'http://127.0.0.1');
    const path = url.pathname.replace(/^\/mdm\//, '/');
    const send = (value, status = 200) => { res.writeHead(status, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' }); res.end(JSON.stringify(value)); };
    const fail = (code, status = 400) => send({ error: code, code }, status);
    if (path === '/' || path === '/index.html') {
      state = fresh(); state.mode = url.searchParams.get('mode') || 'workspace'; state.lang = ['ar', 'de'].includes(url.searchParams.get('lang')) ? url.searchParams.get('lang') : 'en';
      if (state.mode === 'login' || state.mode === 'setup') state.authenticated = false;
      if (state.mode === 'setup') state.configured = false;
      if (state.mode === 'bypass') state.owner = false;
      if (state.mode === 'empty') state.downloads = [];
      if (state.mode === 'torrent') Object.assign(state.downloads[0], { file: 'sample-open-data', url: 'magnet:?xt=urn:btih:0123456789012345678901234567890123456789', torrent: {
        hash: '0123456789012345678901234567890123456789', metadata_available: true, files_truncated: false, busy: false, uploaded: 1024, seeded_seconds: 60,
        upload_limit_kb: 0, in_order: false, ends_first: true, files: [{ index: 0, path: 'open-data/readme.txt', size: 1024, selected: true, downloaded: 1024 }, { index: 1, path: 'open-data/a-long-filename-for-an-international-dataset-ملف-البيانات.csv', size: 8388608, selected: true, downloaded: 1048576 }],
        peers: [{ address: '192.0.2.5:6881', client: 'Synthetic peer', downloaded: 2048, uploaded: 1024 }], trackers: ['https://tracker.example.test/announce'], pieces_complete: 4, pieces_total: 32,
      } });
      res.writeHead(200, { 'Content-Type': 'text/html', 'Cache-Control': 'no-store' });
      res.end(read('crates/remote/web/index.html').replace('<title>Mister Download Manager</title>', '<title>MDM — synthetic development fixture</title>').replace('<body>', '<body><p class="notice" style="margin:0;border-radius:0;text-align:center">Development fixture — synthetic transfers; no real downloads or authentication.</p>')); return;
    }
    const assets = { '/app.js': ['crates/remote/web/app.js', 'text/javascript'], '/app.css': ['crates/remote/web/app.css', 'text/css'], '/icon.png': ['crates/app/assets/icons/app/mdm-256.png', 'image/png'], '/manifest.webmanifest': ['crates/remote/web/manifest.webmanifest', 'application/manifest+json'] };
    if (assets[path]) { const [file, type] = assets[path]; res.writeHead(200, { 'Content-Type': type }); res.end(readFileSync(new URL(file, root))); return; }
    if (path === '/api/texts') { send(translations(state.lang)); return; }
    if (!path.startsWith('/api/v1/')) { fail('not-found', 404); return; }
    let body = null;
    try {
      const chunks = []; let length = 0;
      for await (const chunk of req) { length += chunk.length; if (length > 10 * 1024 * 1024) throw new Error(); chunks.push(chunk); }
      if (chunks.length && !req.headers['content-type']?.startsWith('application/x-bittorrent')) body = JSON.parse(Buffer.concat(chunks).toString());
    } catch { fail('invalid-body'); return; }
    requests.push({ path, method: req.method, body, csrf: req.headers['x-csrf-token'] });
    if (path === '/api/v1/manage' && delayed && delayed.op === body?.op) { const delay = delayed.ms; delayed = null; await new Promise((resolve) => setTimeout(resolve, delay)); }
    if (path === '/api/v1/auth/session') { send({ configured: state.configured, authenticated: state.authenticated, owner: state.owner, csrf: state.authenticated ? state.csrf : null, username: state.authenticated ? state.username : '' }); return; }
    if (path === '/api/v1/auth/login' || path === '/api/v1/auth/setup') {
      if (body?.password !== state.password || (path.endsWith('setup') && body.code.replace(/\s/g, '') !== '12345678')) { fail('invalid-credentials', 401); return; }
      state.authenticated = state.configured = state.owner = true; state.username = body.username; send({ csrf: state.csrf, owner: true }); return;
    }
    if (!state.authenticated) { fail('login-required', 401); return; }
    if (req.method === 'POST' && req.headers['x-csrf-token'] !== state.csrf) { fail('owner-login-required', 403); return; }
    if (path === '/api/v1/auth/logout') { state.authenticated = false; send({ ok: true }); return; }
    if (path === '/api/v1/auth/activity') { send({ ok: true }); return; }
    if (path === '/api/v1/downloads' && req.method === 'GET') { if (state.mode === 'offline') fail('unavailable', 503); else send(snapshot()); return; }
    if (path === '/api/v1/downloads' || path === '/api/v1/torrent') {
      const id = Math.max(0, ...state.downloads.map((d) => d.id)) + 1;
      state.downloads.push({ id, file: body?.url?.split('/').pop() || 'sample-uploaded.torrent', url: body?.url || 'magnet:?xt=urn:btih:sample', category: body?.category || 'General', status: body?.start === false ? 'Queued' : 'Downloading', size: null, downloaded: 0, speed: 0 }); send({ id, pending: false }); return;
    }
    if (path === '/api/v1/actions') {
      const done = [], skipped = [];
      for (const id of body.ids) { const item = state.downloads.find((d) => d.id === id); if (!item) { skipped.push({ id, why: 'no such download' }); continue; } if (body.action === 'pause') { item.status = 'Stopped'; item.speed = 0; } else if (body.action === 'start') item.status = 'Downloading'; else state.downloads = state.downloads.filter((d) => d.id !== id); done.push(id); } send({ done, skipped }); return;
    }
    if (path === '/api/v1/queues/action') { const q = state.queues.find((q) => q.name === body.name); if (q) q.running = body.start; send({ done: [], skipped: [] }); return; }
    if (!state.owner) { fail('owner-login-required', 403); return; }
    if (path === '/api/v1/auth/settings') {
      if (req.method === 'POST') {
        if (body.current_password !== state.password) { fail('invalid-credentials', 401); return; }
        if ([...body.policy.subnets, ...body.policy.trusted_proxies].some((n) => /\/0$/.test(n)) && !body.confirm_unsafe) { fail('confirm-unsafe-subnet'); return; }
        state.policy = body.policy; state.revision++;
      } send(security()); return;
    }
    if (path === '/api/v1/auth/credentials') { if (body.current_password !== state.password) fail('invalid-credentials', 401); else { state.password = body.password; state.username = body.username; state.authenticated = false; send({ ok: true }); } return; }
    if (path === '/api/v1/devices') { send({ devices: state.devices }); return; }
    if (path === '/api/v1/devices/pair') { if (body.current_password !== state.password) fail('invalid-credentials', 401); else send({ code: '12345678', seconds: 600 }); return; }
    if (path === '/api/v1/devices/revoke') { if (body.current_password !== state.password) fail('invalid-credentials', 401); else { state.devices = state.devices.filter((d) => d.id !== body.id); send({ ok: true }); } return; }
    if (path === '/api/v1/manage') {
      if (req.method === 'GET' || body.op === 'config') { send(config()); return; }
      if (body.op === 'details') { const d = state.downloads.find((d) => d.id === body.id); send({ ...d, path: '/downloads/' + (d.relative_folder ? d.relative_folder + '/' : '') + d.file, plain_file: !d.torrent, relative_folder: d.relative_folder || '', moving: false, expected_checksum: d.expected_checksum || '', checksum_result: d.checksum_result || null, limit_kb: d.limit_kb || 0, description: d.description || '', connections: [{ id: 1, speed: d.speed, status: 'Receiving' }], log: [[1794182400, 'Synthetic fixture: transfer started.']] }); return; }
      if (body.op === 'download.update') { Object.assign(state.downloads.find((d) => d.id === body.id), body); send({ ok: true }); return; }
      if (body.op === 'download.move') {
        if (body.folder.includes('..')) { fail('folder-outside-downloads'); return; }
        Object.assign(state.downloads.find((d) => d.id === body.id), { file: body.name, relative_folder: body.folder }); send({ ok: true, pending: false }); return;
      }
      if (body.op === 'download.source') { state.downloads.find((d) => d.id === body.id).url = body.url; send({ ok: true }); return; }
      if (body.op === 'download.checksum') {
        Object.assign(state.downloads.find((d) => d.id === body.id), { expected_checksum: body.expected, checksum_result: body.expected ? { state: body.expected.includes('00000000') ? 'mismatch' : 'match', actual: 'sha256:' + 'a'.repeat(64) } : null });
        send({ ok: true, pending: false }); return;
      }
      if (body.op === 'torrent.update') {
        if (!body.files.length) { fail('invalid-torrent-files'); return; }
        const torrent = state.downloads.find((d) => d.id === body.id).torrent;
        Object.assign(torrent, { in_order: body.in_order, ends_first: body.ends_first, upload_limit_kb: body.upload_limit_kb });
        for (const file of torrent.files) file.selected = body.files.includes(file.index);
        send({ ok: true, pending: false }); return;
      }
      if (body.op === 'torrent.action') {
        if (body.action === 'add-trackers') state.downloads.find((d) => d.id === body.id).torrent.trackers.push(...body.trackers.split('\n'));
        send({ ok: true, pending: true }); return;
      }
      if (body.revision !== String(state.revision)) { fail('configuration-changed-reload'); return; }
      if (body.op === 'settings.save') Object.assign(state.values, body.values);
      if (body.op === 'queue.save') { const q = state.queues.find((q) => q.name === body.old_name); if (q) Object.assign(q, body); else state.queues.push({ ...body, running: false }); }
      if (body.op === 'queue.delete') state.queues = state.queues.filter((q) => q.name !== body.name);
      if (body.op === 'category.save') {
        if (body.folder.includes('..') || body.folder.startsWith('/')) { fail('folder-outside-downloads'); return; }
        const c = state.categories.find((c) => c.custom_id === body.id && body.id != null);
        if (c) Object.assign(c, body, { key: body.name }); else state.categories.push({ ...body, key: body.name, custom_id: 10 });
      }
      if (body.op === 'category.delete') state.categories = state.categories.filter((c) => c.custom_id !== body.id);
      state.revision++; send(config()); return;
    }
    fail('not-found', 404);
  });
  await new Promise((resolve) => server.listen(port, '127.0.0.1', resolve));
  return { url: `http://127.0.0.1:${server.address().port}/mdm/`, requests, state: () => state, delayNextOperation: (op, ms) => { delayed = { op, ms }; }, close: () => new Promise((resolve) => server.close(resolve)) };
}
if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const preview = await startPreview(8767); console.log('Synthetic workspace: ' + preview.url);
}
