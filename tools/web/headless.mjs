// Integration test: real headless binary, real HTTPS/cookies and the shipped web assets.
// Uses a fresh data folder and loopback source; never touches an existing MDM instance.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { once } from 'node:events';
import { access, cp, mkdir, mkdtemp, readFile, readdir, writeFile } from 'node:fs/promises';
import http from 'node:http';
import net from 'node:net';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
const { chromium } = await import(process.env.MDM_PLAYWRIGHT_MODULE || 'playwright');
const root = fileURLToPath(new URL('../../', import.meta.url));
const suffix = process.platform === 'win32' ? '.exe' : '';
const app = path.resolve(process.env.MDM_TEST_APP || path.join(root, 'target/debug/misterdownloadmanager' + suffix));
await access(app); // Fail before starting servers if the required build is missing.
const scratch = path.join(root, 'target/headless-tests');
await mkdir(scratch, { recursive: true });
const dir = await mkdtemp(path.join(scratch, 'run-'));
const downloads = path.join(dir, 'downloads');
const env = { ...process.env, MDM_DATA_DIR: path.join(dir, 'config'), MDM_REMOTE_URL: '', MDM_REMOTE: 'on', MDM_OFFSCREEN: '1' };
let child, extensionContext, output = '', checks = 0;
let starts = 0;
const ok = (name) => { checks++; console.log('ok - ' + name); };
async function command(args) {
  const process = spawn(app, args, { env, windowsHide: true });
  let stdout = '', stderr = '';
  process.stdout.on('data', (b) => { stdout += b; });
  process.stderr.on('data', (b) => { stderr += b; });
  const timer = setTimeout(() => process.kill(), 15000);
  try { const [code] = await once(process, 'exit'); assert.equal(code, 0, stderr || stdout); return stdout; }
  finally { clearTimeout(timer); }
}
async function waitFor(work, message) {
  let last; const deadline = Date.now() + 30000;
  while (Date.now() < deadline) {
    if (child?.exitCode != null) throw new Error(`Headless app exited: ${output}`);
    try { const value = await work(); if (value) return value; } catch (e) { last = e; }
    await new Promise((r) => setTimeout(r, 250));
  }
  throw new Error(message, { cause: last });
}
async function bounded(promise, message) {
  let timer;
  try { return await Promise.race([promise, new Promise((_, reject) => { timer = setTimeout(() => reject(new Error(message)), 20000); })]); }
  finally { clearTimeout(timer); }
}
const portProbe = net.createServer();
await new Promise((r) => portProbe.listen(0, '127.0.0.1', r));
const port = portProbe.address().port;
await new Promise((r) => portProbe.close(r));
const base = `https://127.0.0.1:${port}/`;
const content = Buffer.from('Real headless integration test download.\n'.repeat(4096));
const source = http.createServer((req, res) => {
  if (req.url === '/') {
    res.writeHead(200, { 'Content-Type': 'text/html' });
    res.end('<!doctype html><title>Loopback download source</title><a href="/browser-capture.zip" download>Download test file</a>');
    return;
  }
  const fromBrowser = req.url === '/browser-capture.zip';
  const name = fromBrowser ? 'browser-capture.zip' : 'headless-test.zip';
  res.writeHead(200, { 'Content-Length': content.length, 'Content-Type': 'application/zip', 'Content-Disposition': `attachment; filename="${name}"` });
  if (req.method === 'HEAD') { res.end(); return; }
  if (!fromBrowser) { res.end(content); return; }
  // Leave time for the real browser's onCreated handler to pause and hand off the transfer.
  let sent = 0;
  const interval = setInterval(() => {
    res.write(content.subarray(sent, sent += 16384));
    if (sent >= content.length) { clearInterval(interval); res.end(); }
  }, 100);
  res.on('close', () => clearInterval(interval));
});
await new Promise((r) => source.listen(0, '127.0.0.1', r));
const sourceURL = `http://127.0.0.1:${source.address().port}/headless-test.zip`;
const browser = await chromium.launch({ headless: true });
const context = await browser.newContext({ ignoreHTTPSErrors: true });
const anonymous = await browser.newContext({ ignoreHTTPSErrors: true });
const page = await context.newPage();
const errors = [];
page.on('pageerror', (e) => errors.push(e.message));
const password = 'integration test owner password';
let csrf;
const request = async (endpoint, body, client = context.request) => {
  const response = await client.fetch(base + 'api/' + endpoint, {
    method: body === undefined ? 'GET' : 'POST', data: body,
    headers: body === undefined ? {} : { 'x-csrf-token': csrf || '' }, timeout: 8000,
  });
  const result = await response.json();
  assert.equal(response.status(), 200, JSON.stringify(result));
  assert.equal(result.error, undefined, JSON.stringify(result));
  return result;
};
async function start() {
  // Exercise new flags on initial startup and legacy aliases when verifying persistence.
  const mode = starts++ === 0 ? ['--web', '--webhost=127.0.0.1'] : ['--headless', '--listen', '127.0.0.1'];
  child = spawn(app, [...mode, '--data', env.MDM_DATA_DIR, '--downloads', downloads, '--port', String(port)], { env, windowsHide: true });
  child.stdout.on('data', (b) => { output += b; });
  child.stderr.on('data', (b) => { output += b; });
  await waitFor(async () => (await anonymous.request.get(base + 'api/v1/auth/session', { timeout: 1000 })).ok(), 'HTTPS server did not start');
}
async function stop() {
  if (!child || child.exitCode != null) return;
  const exited = once(child, 'exit');
  await command(['/exit']);
  const timer = setTimeout(() => child.kill(), 15000);
  try { const [code] = await exited; assert.equal(code, 0, 'clean headless shutdown'); }
  finally { clearTimeout(timer); }
}
try {
  await start();
  assert.equal(JSON.parse(await command(['--cli', 'health'])).ready, true);
  const code = (await command(['--cli', 'pair'])).match(/enter the code (\d{4}) (\d{4})/);
  assert.ok(code, 'local CLI supplies one-time enrollment code');
  await page.goto(base);
  await page.locator('#setup-code').fill(code[1] + code[2]);
  await page.locator('#login-username').fill('owner');
  await page.locator('#login-password').fill(password);
  await page.locator('#login-submit').click();
  await page.locator('#empty').waitFor({ state: 'visible' });
  csrf = (await request('v1/auth/session')).csrf;
  const cookie = (await context.cookies()).find((c) => c.name === '__Secure-mdm-session');
  assert.ok(cookie?.httpOnly && cookie.secure && cookie.sameSite === 'Strict');
  ok('headless startup, CLI health and browser owner enrollment with protected cookies');

  const denied = await context.request.post(base + 'api/v1/manage', { data: { op: 'config' } });
  assert.equal(denied.status(), 403);
  assert.equal((await anonymous.request.get(base + 'api/v1/manage')).status(), 401);
  ok('real management API rejects missing authentication and CSRF');
  let config = await request('v1/manage');
  const oldRevision = config.revision;
  config = await request('v1/manage', { op: 'settings.save', revision: config.revision, values: { max_connections: 12 } });
  assert.equal(config.values.max_connections, 12);
  const conflict = await context.request.post(base + 'api/v1/manage', { headers: { 'x-csrf-token': csrf }, data: { op: 'settings.save', revision: oldRevision, values: { max_connections: 8 } } });
  assert.equal((await conflict.json()).code, 'configuration-changed-reload');
  await page.locator('[data-page="settings"]').first().click();
  await page.locator('[data-setting="capture_opened"]').check();
  await page.locator('#settings-form [type="submit"]').click();
  await waitFor(async () => (await page.locator('#settings-result').textContent()).includes('Changes saved'), 'real settings form did not save its schema values');
  config = await request('v1/manage');
  assert.equal(config.values.capture_opened, true);
  const categoryName = 'Integration files';
  config = await request('v1/manage', { op: 'category.save', revision: config.revision, id: null, name: categoryName, types: 'ZIP', folder: 'Integration' });
  assert.ok(config.categories.some((c) => c.name === categoryName));
  const escaping = await context.request.post(base + 'api/v1/manage', { headers: { 'x-csrf-token': csrf }, data: { op: 'category.save', revision: config.revision, name: 'Escape', types: '', folder: '../outside' } });
  assert.equal((await escaping.json()).code, 'folder-outside-downloads');
  config = await request('v1/manage', { op: 'queue.save', revision: config.revision, old_name: null, name: 'Night queue', files_at_once: 2, ids: [], schedule: { start_at: [23, 30, 0], stop_at: [6, 30, 0], days: [true, true, true, true, true, false, false] } });
  assert.deepEqual(config.queues.find((q) => q.name === 'Night queue').schedule.start_at, [23, 30, 0]);
  ok('application-loop settings, custom category, bounded paths and queue schedules');

  const pairing = await request('v1/devices/pair', { current_password: password });
  // Owner enrollment uses the same peer address and pairing throttle. Wait for
  // that throttle rather than assuming the preceding UI work took a second.
  let device;
  for (let attempt = 0; attempt < 5; attempt++) {
    const response = await anonymous.request.post(base + 'api/pair', { data: { code: pairing.code, name: 'Integration extension' }, timeout: 8000 });
    device = await response.json();
    if (response.status() !== 429 || device.code !== 'too-soon') {
      assert.equal(response.status(), 200, JSON.stringify(device));
      assert.equal(device.error, undefined, JSON.stringify(device));
      break;
    }
    if (attempt === 4) assert.fail('pairing remained rate limited');
    await new Promise((resolve) => setTimeout(resolve, 350));
  }
  assert.equal(device.scope, 'capture');
  const bearer = { authorization: 'Bearer ' + device.token };
  const status = await anonymous.request.get(base + 'api/status', { headers: bearer });
  const captureRules = await status.json();
  assert.equal(captureRules.api_version, 1);
  assert.equal(captureRules.rules.opened, true, 'owner capture settings reach scoped extensions');
  assert.equal((await anonymous.request.get(base + 'api/list', { headers: bearer })).status(), 403);
  const added = await anonymous.request.post(base + 'api/add', { headers: bearer, data: { url: sourceURL, category: categoryName, start: true } });
  const acceptance = await added.json();
  assert.ok(Number.isSafeInteger(acceptance.id), JSON.stringify(acceptance));
  const transfer = await waitFor(async () => (await request('v1/downloads')).downloads.find((d) => d.id === acceptance.id && d.status === 'Complete'), 'real download did not finish');
  assert.equal(transfer.downloaded, content.length);
  assert.deepEqual(await readFile(path.join(downloads, 'Integration', 'headless-test.zip')), content);
  await request('v1/manage', { op: 'download.update', id: transfer.id, limit_kb: 2048, description: 'Persisted integration test', category: categoryName });
  ok('capture-only extension adds a real download without history or owner privileges');

  const expectedHash = 'sha256:' + createHash('sha256').update(content).digest('hex');
  await request('v1/manage', { op: 'download.checksum', id: transfer.id, expected: expectedHash });
  await waitFor(async () => (await request('v1/manage', { op: 'details', id: transfer.id })).checksum_result?.state === 'match', 'real checksum verification did not finish');
  await request('v1/manage', { op: 'download.checksum', id: transfer.id, expected: 'sha256:' + '0'.repeat(64) });
  await waitFor(async () => (await request('v1/manage', { op: 'details', id: transfer.id })).checksum_result?.state === 'mismatch', 'checksum mismatch was not reported');
  await request('v1/manage', { op: 'download.checksum', id: transfer.id, expected: expectedHash });
  await waitFor(async () => (await request('v1/manage', { op: 'details', id: transfer.id })).checksum_result?.state === 'match', 'checksum could not be restored');
  await writeFile(path.join(downloads, 'occupied.bin'), 'do not overwrite');
  const collision = await context.request.post(base + 'api/v1/manage', { headers: { 'x-csrf-token': csrf }, data: { op: 'download.move', id: transfer.id, folder: '', name: 'occupied.bin' } });
  assert.equal((await collision.json()).code, 'destination-exists');
  assert.equal(await readFile(path.join(downloads, 'occupied.bin'), 'utf8'), 'do not overwrite');
  await request('v1/downloads', { url: 'https://source.example.test/reserved.zip', start: false, category: 'General' });
  const reservedName = ['win32', 'darwin'].includes(process.platform) ? 'RESERVED.ZIP' : 'reserved.zip';
  const reserved = await context.request.post(base + 'api/v1/manage', { headers: { 'x-csrf-token': csrf }, data: { op: 'download.move', id: transfer.id, folder: '', name: reservedName } });
  assert.equal((await reserved.json()).code, 'destination-exists', 'canonical and case aliases cannot replace queued download destinations');
  await page.locator('#close-management').click();
  await page.locator('[data-page="downloads"]').first().click();
  await page.locator('.filename').filter({ hasText: 'headless-test.zip' }).click();
  await page.locator('#detail-tab-files').click();
  await page.locator('#file-tools summary').filter({ hasText: 'Move or rename' }).click();
  assert.equal(await page.locator('#move-folder').inputValue(), 'Integration');
  await page.locator('#move-name').fill('renamed-test.bin'); await page.locator('#move-folder').fill('Moved');
  await page.locator('#move-form [type="submit"]').click();
  await waitFor(async () => (await request('v1/manage', { op: 'details', id: transfer.id })).path.endsWith('renamed-test.bin'), 'real move did not finish');
  assert.deepEqual(await readFile(path.join(downloads, 'Moved', 'renamed-test.bin')), content);
  await assert.rejects(access(path.join(downloads, 'Integration', 'headless-test.zip')));
  const queued = await request('v1/downloads', { url: 'https://source.example.test/expired.zip', start: false, category: categoryName });
  await request('v1/manage', { op: 'download.source', id: queued.id, url: sourceURL + '?renewed=1' });
  await request('v1/actions', { action: 'start', ids: [queued.id] });
  await waitFor(async () => (await request('v1/downloads')).downloads.some((d) => d.id === queued.id && d.status === 'Complete'), 'replacement source did not download');
  ok('real checksum match/mismatch, non-overwriting move/rename and replacement download source');
  await page.locator('#close-details').click();

  config = await request('v1/manage');
  config = await request('v1/manage', { op: 'settings.save', revision: config.revision, values: { torrent_dht: false, torrent_lsd: false, torrent_upnp: false, torrent_incoming: false } });
  const encode = (value) => {
    if (Buffer.isBuffer(value) || typeof value === 'string') { const bytes = Buffer.from(value); return Buffer.concat([Buffer.from(bytes.length + ':'), bytes]); }
    if (typeof value === 'number') return Buffer.from(`i${value}e`);
    if (Array.isArray(value)) return Buffer.concat([Buffer.from('l'), ...value.map(encode), Buffer.from('e')]);
    return Buffer.concat([Buffer.from('d'), ...Object.keys(value).sort().flatMap((key) => [encode(key), encode(value[key])]), Buffer.from('e')]);
  };
  const torrentInfo = { files: [{ length: 4, path: ['readme.txt'] }, { length: 4, path: ['payload.bin'] }], name: 'web-test-torrent', 'piece length': 16384, pieces: createHash('sha1').update('abcdefgh').digest() };
  const torrentHash = createHash('sha1').update(encode(torrentInfo)).digest('hex');
  const torrentAdded = await context.request.post(base + 'api/v1/torrent?start=false', { headers: { 'x-csrf-token': csrf, 'content-type': 'application/x-bittorrent' }, data: encode({ info: torrentInfo }) });
  assert.equal(torrentAdded.status(), 200);
  const torrentAdmission = await torrentAdded.json();
  assert.ok(torrentAdmission.request_id);
  const torrentItem = await waitFor(async () => (await request('v1/downloads')).downloads.find((d) => d.url.includes(torrentHash)), 'torrent metadata did not join the list');
  const admission = await request('v1/manage', { op: 'torrent.add.status', request_id: torrentAdmission.request_id });
  assert.equal(admission.state, 'added'); assert.equal(admission.id, torrentItem.id);
  const blockedDir = path.join(downloads, 'Integration', 'blocked-admission');
  await mkdir(blockedDir, { recursive: true }); await writeFile(path.join(blockedDir, 'readme.txt'), 'keep existing contents');
  const blockedInfo = { ...torrentInfo, name: 'blocked-admission' };
  const blockedAdd = await context.request.post(base + 'api/v1/torrent?' + new URLSearchParams({ start: 'false', category: categoryName }), { headers: { 'x-csrf-token': csrf, 'content-type': 'application/x-bittorrent' }, data: encode({ info: blockedInfo }) });
  assert.equal(blockedAdd.status(), 200); const blockedJob = await blockedAdd.json();
  const blockedResult = await waitFor(async () => { const result = await request('v1/manage', { op: 'torrent.add.status', request_id: blockedJob.request_id }); return result.state === 'pending' ? null : result; }, 'torrent admission failure was not reported');
  assert.equal(blockedResult.state, 'confirmation'); assert.equal(blockedResult.code, 'torrent-files-exist');
  assert.equal(await readFile(path.join(blockedDir, 'readme.txt'), 'utf8'), 'keep existing contents');
  const noConsent = await context.request.post(base + 'api/v1/manage', { headers: { 'x-csrf-token': csrf }, data: { op: 'torrent.add.confirm', request_id: blockedJob.request_id } });
  assert.equal((await noConsent.json()).code, 'confirmation-required');
  const tokenConsent = await anonymous.request.post(base + 'api/v1/manage', { headers: { authorization: 'Bearer ' + device.token }, data: { op: 'torrent.add.confirm', request_id: blockedJob.request_id, approve_existing: true } });
  assert.equal(tokenConsent.status(), 401, 'an extension token is not an owner browser session');
  const cancelled = await request('v1/manage', { op: 'torrent.add.cancel', request_id: blockedJob.request_id }); assert.equal(cancelled.state, 'cancelled');
  assert.equal(await readFile(path.join(blockedDir, 'readme.txt'), 'utf8'), 'keep existing contents');
  const firstPiece = Buffer.alloc(16384, 97), secondPiece = Buffer.alloc(16384, 98);
  const reuseInfo = { files: [{ length: firstPiece.length, path: ['first.bin'] }, { length: secondPiece.length, path: ['second.bin'] }], name: 'reuse-verified', 'piece length': 16384, pieces: Buffer.concat([createHash('sha1').update(firstPiece).digest(), createHash('sha1').update(secondPiece).digest()]) };
  const reuseDir = path.join(downloads, 'Integration', 'reuse-verified'); await mkdir(reuseDir, { recursive: true });
  await writeFile(path.join(reuseDir, 'first.bin'), firstPiece); await writeFile(path.join(reuseDir, 'second.bin'), Buffer.alloc(16384));
  const reuseResponse = await context.request.post(base + 'api/v1/torrent?' + new URLSearchParams({ start: 'true', category: categoryName }), { headers: { 'x-csrf-token': csrf, 'content-type': 'application/x-bittorrent' }, data: encode({ info: reuseInfo }) });
  const reuseJob = await reuseResponse.json();
  await waitFor(async () => (await request('v1/manage', { op: 'torrent.add.status', request_id: reuseJob.request_id })).state === 'confirmation', 'existing-data confirmation did not appear');
  assert.deepEqual(await readFile(path.join(reuseDir, 'first.bin')), firstPiece);
  const reuseResult = await request('v1/manage', { op: 'torrent.add.confirm', request_id: reuseJob.request_id, approve_existing: true });
  assert.equal(reuseResult.state, 'added'); assert.equal(reuseResult.reused, true);
  assert.equal((await request('v1/manage', { op: 'torrent.add.confirm', request_id: reuseJob.request_id, approve_existing: true })).id, reuseResult.id);
  const verified = await waitFor(async () => { const details = await request('v1/manage', { op: 'details', id: reuseResult.id }); return details.torrent.pieces_complete === 1 ? details : null; }, 'existing matching piece was not verified');
  assert.equal(verified.torrent.pieces_total, 2);
  assert.deepEqual(await readFile(path.join(reuseDir, 'first.bin')), firstPiece);
  await request('v1/actions', { action: 'pause', ids: [reuseResult.id] });
  const duplicateTorrent = await context.request.post(base + 'api/v1/torrent?start=false', { headers: { 'x-csrf-token': csrf, 'content-type': 'application/x-bittorrent' }, data: encode({ info: reuseInfo }) });
  const duplicateTorrentResult = await duplicateTorrent.json();
  assert.equal(duplicateTorrentResult.id, reuseResult.id); assert.equal(duplicateTorrentResult.duplicate, true);
  const duplicateReview = await request('v1/manage', { op: 'download.add', url: sourceURL, start: false, category: categoryName });
  assert.equal(duplicateReview.confirmation, 'duplicate');
  const staleDuplicate = await request('v1/manage', { op: 'download.add', url: sourceURL, start: false, category: categoryName, existing_id: 999999, duplicate_choice: 'new' });
  assert.equal(staleDuplicate.confirmation, 'duplicate');
  const useExisting = await request('v1/manage', { op: 'download.add', url: sourceURL, start: false, existing_id: duplicateReview.id, duplicate_choice: 'existing' });
  assert.equal(useExisting.id, duplicateReview.id); assert.equal(useExisting.duplicate, true);
  const separateCopy = await request('v1/manage', { op: 'download.add', url: sourceURL, start: false, category: categoryName, existing_id: duplicateReview.id, duplicate_choice: 'new' });
  assert.notEqual(separateCopy.id, duplicateReview.id);
  assert.deepEqual(await readFile(path.join(downloads, 'Moved', 'renamed-test.bin')), content);
  const queuedUrl = sourceURL + '?separate-queued-copies=1';
  const queuedFirst = await request('v1/manage', { op: 'download.add', url: queuedUrl, start: false, category: categoryName });
  const queuedQuestion = await request('v1/manage', { op: 'download.add', url: queuedUrl, start: false, category: categoryName });
  assert.equal(queuedQuestion.confirmation, 'duplicate');
  const queuedSecond = await request('v1/manage', { op: 'download.add', url: queuedUrl, start: false, category: categoryName, existing_id: queuedFirst.id, duplicate_choice: 'new' });
  const firstPath = (await request('v1/manage', { op: 'details', id: queuedFirst.id })).path;
  const secondPath = (await request('v1/manage', { op: 'details', id: queuedSecond.id })).path;
  assert.notEqual(firstPath, secondPath, 'queued copies must reserve distinct destinations before any files exist');
  for (const id of [queuedFirst.id, queuedSecond.id]) {
    await request('v1/actions', { action: 'start', ids: [id] });
    await waitFor(async () => (await request('v1/downloads')).downloads.some((d) => d.id === id && d.status === 'Complete'), 'separate queued copy did not finish');
  }
  const completedFirstPath = (await request('v1/manage', { op: 'details', id: queuedFirst.id })).path;
  const completedSecondPath = (await request('v1/manage', { op: 'details', id: queuedSecond.id })).path;
  assert.notEqual(completedFirstPath, completedSecondPath, 'completed numbered copies must not converge on one file');
  assert.deepEqual(await readFile(completedFirstPath), content); assert.deepEqual(await readFile(completedSecondPath), content);
  ok('owner-confirmed torrent reuse verifies good pieces, rejects bad pieces, cancels safely and handles duplicate Web links');
  const invalidAdd = await context.request.post(base + 'api/v1/torrent?start=false', { headers: { 'x-csrf-token': csrf, 'content-type': 'application/x-bittorrent' }, data: Buffer.from('dnot-valid-bencode') });
  assert.equal(invalidAdd.status(), 200); const invalidJob = await invalidAdd.json();
  const invalidResult = await waitFor(async () => { const result = await request('v1/manage', { op: 'torrent.add.status', request_id: invalidJob.request_id }); return result.state === 'pending' ? null : result; }, 'invalid torrent metadata failure was not reported');
  assert.equal(invalidResult.state, 'error'); assert.equal(invalidResult.code, 'torrent-read-failed');
  let torrentDetails = await request('v1/manage', { op: 'details', id: torrentItem.id });
  assert.equal(torrentDetails.torrent.files.length, 2);
  await request('v1/manage', { op: 'torrent.update', id: torrentItem.id, files: [1], in_order: true, ends_first: false, upload_limit_kb: 128 });
  await waitFor(async () => !(await request('v1/manage', { op: 'details', id: torrentItem.id })).torrent.busy, 'torrent file selection did not settle');
  torrentDetails = await request('v1/manage', { op: 'details', id: torrentItem.id });
  assert.deepEqual(torrentDetails.torrent.files.map((f) => f.selected), [false, true]);
  assert.equal(torrentDetails.torrent.upload_limit_kb, 128); assert.equal(torrentDetails.torrent.in_order, true);
  const invalidFiles = await context.request.post(base + 'api/v1/manage', { headers: { 'x-csrf-token': csrf }, data: { op: 'torrent.update', id: torrentItem.id, files: [999], in_order: true, ends_first: true, upload_limit_kb: 0 } });
  assert.equal((await invalidFiles.json()).code, 'invalid-torrent-files');
  await request('v1/manage', { op: 'torrent.action', id: torrentItem.id, action: 'recheck' });
  await waitFor(async () => !(await request('v1/manage', { op: 'details', id: torrentItem.id })).torrent.busy, 'torrent recheck request did not settle');
  await request('v1/manage', { op: 'torrent.action', id: torrentItem.id, action: 'reannounce' });
  await waitFor(async () => !(await request('v1/manage', { op: 'details', id: torrentItem.id })).torrent.busy, 'torrent reannounce did not settle');
  await request('v1/manage', { op: 'torrent.action', id: torrentItem.id, action: 'add-trackers', trackers: 'https://tracker.example.test/announce' });
  await waitFor(async () => (await request('v1/manage', { op: 'details', id: torrentItem.id })).torrent.trackers.includes('https://tracker.example.test/announce'), 'torrent tracker edit did not persist');
  ok('real saved torrent metadata, selected files, upload/order settings, recheck/reannounce and tracker editing');

  const profile = path.join(dir, 'chromium-profile');
  const browserDownloads = path.join(dir, 'browser-downloads');
  const extensionPath = path.join(dir, 'extension');
  await cp(process.env.MDM_TEST_EXTENSION ? path.resolve(process.env.MDM_TEST_EXTENSION) : path.join(root, 'extension'), extensionPath, { recursive: true });
  if (process.env.MDM_TEST_EXTENSION_VERSION) {
    const packaged = JSON.parse(await readFile(path.join(extensionPath, 'manifest.json'), 'utf8'));
    assert.equal(packaged.name, 'Webmister Download Manager');
    assert.equal(packaged.version, process.env.MDM_TEST_EXTENSION_VERSION.split('-')[0], 'test the versioned release extension, not a stale source package');
  }
  // The only test substitution: a deliberately unregistered native-host name. Startup must
  // never launch or talk to a user's installed MDM, even if its normal host is registered.
  const background = await readFile(path.join(extensionPath, 'background.js'), 'utf8');
  assert.ok(background.includes('const HOST = "com.misterdownloadmanager.host";'));
  const testHost = 'com.misterdownloadmanager.test_' + randomUUID().replaceAll('-', '');
  await writeFile(path.join(extensionPath, 'background.js'), background.replace('const HOST = "com.misterdownloadmanager.host";', `const HOST = ${JSON.stringify(testHost)};`));
  await mkdir(path.join(profile, 'Default'), { recursive: true });
  await mkdir(browserDownloads);
  await writeFile(path.join(profile, 'Default', 'Preferences'), JSON.stringify({ download: { default_directory: browserDownloads, prompt_for_download: false } }));
  extensionContext = await chromium.launchPersistentContext(profile, {
    channel: 'chromium', headless: true, ignoreHTTPSErrors: true,
    args: ['--ignore-certificate-errors', '--enable-unsafe-extension-debugging'],
    ignoreDefaultArgs: ['--disable-extensions'],
  });
  const debug = await extensionContext.browser().newBrowserCDPSession();
  // Playwright's download override skips extension interception. Restore Chromium's own
  // download flow; the fresh profile's preferences still keep all files in this test folder.
  await debug.send('Browser.setDownloadBehavior', { behavior: 'default' });
  const { id: extensionID } = await bounded(debug.send('Extensions.loadUnpacked', { path: extensionPath }), 'Chromium extension loading timed out');
  const worker = extensionContext.serviceWorkers()[0] || await extensionContext.waitForEvent('serviceworker');
  const popup = await extensionContext.newPage();
  await popup.goto(`chrome-extension://${extensionID}/popup.html`);
  await popup.locator('#to-remote').check();
  const extensionPairing = await request('v1/devices/pair', { current_password: password });
  await popup.locator('#remote-url').fill(base);
  await popup.locator('#remote-code').fill(extensionPairing.code);
  await popup.locator('#remote-pair').click();
  await waitFor(() => worker.evaluate(async () => !!(await chrome.storage.local.get('remote')).remote?.token), 'real extension did not pair');
  const capturePage = await extensionContext.newPage();
  await capturePage.goto(`http://127.0.0.1:${source.address().port}/`);
  await capturePage.getByRole('link', { name: 'Download test file' }).click();
  await waitFor(async () => (await request('v1/downloads')).downloads.find((d) => d.url.endsWith('/browser-capture.zip') && d.status === 'Complete'), 'real extension capture did not finish');
  assert.deepEqual(await readFile(path.join(downloads, 'Integration', 'browser-capture.zip')), content);
  await waitFor(() => worker.evaluate(async () => (await chrome.downloads.search({})).every((d) => d.state === 'interrupted')), 'browser kept a duplicate transfer');
  assert.deepEqual(await readdir(browserDownloads), [], 'browser download folder stays empty after handoff');
  ok('real Chromium extension pairs through its popup and captures a download without a native host');
  const extensionDevice = (await request('v1/devices')).devices.find((d) => d.id !== device.id);
  assert.equal(extensionDevice.scope, 'capture');
  await request('v1/devices/revoke', { id: extensionDevice.id, current_password: password });
  await capturePage.getByRole('link', { name: 'Download test file' }).click();
  await waitFor(() => worker.evaluate(async () => (await chrome.downloads.search({})).some((d) => d.state === 'complete')), 'revoked extension did not leave the transfer with the browser');
  assert.deepEqual(await readFile(path.join(browserDownloads, 'browser-capture.zip')), content);
  const destination = await worker.evaluate(async () => (await chrome.storage.local.get('remote')).remote);
  assert.equal(destination.url, base); assert.equal(destination.token, null);
  await extensionContext.close(); extensionContext = null;
  ok('revoked Chromium extension preserves the browser download and never falls back to a native host');

  await page.locator('[data-page="security"]').first().click();
  await page.locator('#max-failures').waitFor({ state: 'visible' });
  assert.equal(await page.locator('#max-failures').inputValue(), '5');
  await page.locator('#max-failures').fill('3');
  await page.locator('#auth-current-password').fill(password);
  await page.locator('#auth-settings-form [type="submit"]').click();
  await waitFor(async () => (await request('v1/auth/settings')).policy.max_failures === 3, 'authentication policy did not save');
  await page.evaluate(() => { document.activeElement?.blur(); scrollTo(0, 0); });
  await page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
  await page.screenshot({ path: path.join(dir, 'real-authentication.png'), fullPage: true });
  await stop();
  await start();
  assert.equal((await anonymous.request.get(base + 'api/v1/manage')).status(), 401);
  await request('v1/auth/login', { username: 'owner', password });
  csrf = (await request('v1/auth/session')).csrf;
  config = await request('v1/manage');
  assert.equal(config.values.max_connections, 12);
  assert.ok(config.categories.some((c) => c.name === categoryName));
  assert.ok(config.queues.some((q) => q.name === 'Night queue'));
  assert.equal((await request('v1/manage', { op: 'details', id: transfer.id })).description, 'Persisted integration test');
  assert.ok((await request('v1/manage', { op: 'details', id: transfer.id })).path.endsWith('renamed-test.bin'));
  assert.equal((await request('v1/manage', { op: 'details', id: transfer.id })).expected_checksum, expectedHash);
  const restoredTorrent = (await request('v1/manage', { op: 'details', id: torrentItem.id })).torrent;
  assert.equal(restoredTorrent.in_order, true); assert.equal(restoredTorrent.upload_limit_kb, 128);
  assert.deepEqual(restoredTorrent.files.map((f) => f.selected), [false, true]);
  assert.equal((await request('v1/auth/settings')).policy.max_failures, 3);
  assert.equal((await anonymous.request.get(base + 'api/status', { headers: bearer })).status(), 200);
  ok('browser authentication settings, clean shutdown and persisted owner/download/config/device state');

  await request('v1/devices/revoke', { id: device.id, current_password: password });
  assert.equal((await anonymous.request.get(base + 'api/status', { headers: bearer })).status(), 401);
  for (let i = 0; i < 3; i++) await anonymous.request.post(base + 'api/v1/auth/login', { data: { username: 'owner', password: 'wrong' } });
  assert.equal((await anonymous.request.post(base + 'api/v1/auth/login', { data: { username: 'owner', password } })).status(), 429);
  await command(['--cli', 'web-auth', 'unban']);
  await request('v1/auth/login', { username: 'owner', password });
  await command(['--cli', 'web-auth', 'reset-proxies']);
  await command(['--cli', 'web-auth', 'reset-owner']);
  assert.equal((await request('v1/auth/session')).configured, false);
  assert.deepEqual(errors, []);
  ok('device revocation, login bans and protected local recovery');
  console.log(`headless: ${checks} checks passed; isolated artifacts in ${path.relative(root, dir)}`);
} catch (error) {
  await page.screenshot({ path: path.join(dir, 'failure.png'), fullPage: true }).catch(() => {});
  console.error('Headless artifacts: ' + path.relative(root, dir));
  console.error('Browser errors:', errors);
  console.error('Login feedback:', await page.locator('#login-error').textContent().catch(() => 'unavailable'));
  if (extensionContext) {
    const worker = extensionContext.serviceWorkers()[0];
    if (worker) console.error('Extension diagnostics:', await worker.evaluate(async () => {
      const stored = await chrome.storage.local.get(['remote', 'remoteStatus', 'rules']);
      return { paired: !!stored.remote?.token, status: stored.remoteStatus?.why,
        types: stored.rules?.types, downloads: (await chrome.downloads.search({})).map((d) => ({ state: d.state, error: d.error })) };
    }).catch(() => 'worker unavailable'));
  }
  throw error;
} finally {
  try { await stop(); } finally {
    if (child && child.exitCode == null) child.kill();
    if (extensionContext) await extensionContext.close();
    await context.close(); await anonymous.close(); await browser.close();
    await new Promise((r) => source.close(r));
  }
}
