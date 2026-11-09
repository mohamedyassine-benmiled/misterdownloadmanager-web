// Embedded, same-origin MDM workspace. Browser sessions use HttpOnly cookies; no owner token
// is written to localStorage. The existing bearer-token surface is reserved for paired devices.
'use strict';
const $ = (id) => document.getElementById(id);
let TX = {}, LANG = 'en', authenticated = false, owner = false, csrf = null, setup = false;
let data = null, config = null, authConfig = null, page = 'downloads', filter = 'all';
let timer = null, refreshing = null, failures = 0, chosenFile = null, detailId = null, lastDetails = 0;
let removing = [], revoking = null, lastActivity = 0;
let navigationGeneration = 0;
let loadedDetailId = null, detailLoading = false, detailRequestSequence = 0;
const rows = new Map(), queueRows = new Map(), selected = new Set(), dirty = new Set();
const edits = new WeakMap();

function T(s) { return TX[s] || s; }
function F(s, args) { return T(s).replace(/\{(\w+)\}/g, (m, key) => key in args ? String(args[key]) : m); }
function num(n, digits = 0) { return new Intl.NumberFormat(LANG, { maximumFractionDigits: digits }).format(n); }
function size(n) {
  if (n == null) return T('Unknown');
  for (const [scale, key] of [[1073741824, '{n} GB'], [1048576, '{n} MB'], [1024, '{n} KB']]) {
    if (n >= scale || scale === 1024) return (TX['unit:' + key] || key).replace('{n}', num(n / scale, n >= 1073741824 ? 2 : 1));
  }
}
function speed(n) { const key = n >= 1048576 ? '{n} MB/sec' : '{n} KB/sec'; return (TX['unit:' + key] || key).replace('{n}', num(n / (n >= 1048576 ? 1048576 : 1024), 1)); }
function eta(x) { if (!x.size || !x.speed || x.status !== 'Downloading') return '—'; const s = Math.max(0, Math.ceil((x.size - x.downloaded) / x.speed)); return `${Math.floor(s / 3600) ? Math.floor(s / 3600) + ':' : ''}${String(Math.floor(s / 60) % 60).padStart(2, '0')}:${String(s % 60).padStart(2, '0')}`; }
function element(tag, text, className) { const node = document.createElement(tag); if (text != null) node.textContent = text; if (className) node.className = className; return node; }
function translate(root = document) {
  for (const el of root.querySelectorAll('[data-t]')) el.textContent = T(el.dataset.t);
  for (const el of root.querySelectorAll('[data-ta]')) { el.setAttribute('aria-label', T(el.dataset.ta)); el.title = T(el.dataset.ta); }
  for (const el of root.querySelectorAll('[data-tp]')) el.placeholder = T(el.dataset.tp);
}
async function loadTexts() {
  try {
    const r = await fetch('api/texts?lang=' + encodeURIComponent((navigator.languages || [navigator.language]).join(',')), { cache: 'no-store' });
    const j = await r.json(); TX = j.texts || {}; LANG = j.lang || 'en';
    document.documentElement.lang = LANG; document.documentElement.dir = j.rtl ? 'rtl' : 'ltr';
  } catch { /* Existing catalogs fall back to the source English. */ }
  translate();
}
class ApiError extends Error { constructor(code, body, retry) { super(code); this.code = code; this.body = body; this.retry = retry; } }
async function api(path, options = {}) {
  const headers = { ...options.headers };
  let body = options.body;
  if (options.json !== undefined) { body = JSON.stringify(options.json); headers['Content-Type'] = 'application/json'; }
  const method = body !== undefined || options.post ? 'POST' : 'GET';
  if (method !== 'GET' && csrf) headers['X-CSRF-Token'] = csrf;
  const controller = new AbortController(); const timeout = setTimeout(() => controller.abort(), 15000);
  try {
    const r = await fetch('api/' + path, { method, headers, body, credentials: 'same-origin', cache: 'no-store', redirect: 'error', signal: controller.signal });
    let j = null; try { if (r.status !== 204) j = await r.json(); } catch { /* Never treat an HTML proxy reply as acceptance. */ }
    if (!r.ok || j?.error || (r.status !== 204 && !j)) {
      const code = j?.code || String(r.status);
      if (code === 'login-required') signedOut();
      throw new ApiError(code, j, r.headers.get('Retry-After'));
    }
    return j;
  } finally { clearTimeout(timeout); controller.abort(); }
}
function message(error) {
  const problems = {
    'destination-exists': T('A file or download already uses that destination. Choose another name.'),
    'invalid-file-name': T('Use a valid file name without folder separators.'),
    'move-not-ready': T('Only completed files or downloads with no received data can be moved. Pause the download first.'),
    'pause-unfinished-first': T('Pause an unfinished download before changing its source or checksum.'),
    'file-busy': T('A file operation is already running. Wait for it to finish.'),
    'file-unavailable': T('The file could not be read. Check its location and server permissions.'),
    'resume-state-unavailable': T('Resume data could not be updated. Check server storage before trying again.'),
    'invalid-checksum': T('Enter a valid MD5, SHA-1, SHA-256, SHA-512 or CRC32 checksum.'),
    'bad-address': T('Enter an HTTP, HTTPS, FTP or FTPS address without embedded credentials.'),
    'plain-file-required': T('This operation is available for regular file downloads only.'),
    'torrent-busy': T('A torrent operation is running. Wait for its result in Activity.'),
    'torrent-metadata-unavailable': T('Torrent metadata is unavailable. Wait for discovery or restore the saved torrent file.'),
    'invalid-torrent-files': T('Select at least one torrent file. Each file must be selected only once.'),
    'invalid-trackers': T('Enter up to 100 HTTP, HTTPS or UDP tracker addresses, one per line.'),
  };
  if (problems[error.code]) return problems[error.code];
  if (error.code === 'temporarily-banned') return F('Too many failed logins. Try again in {seconds} seconds, or use local recovery.', { seconds: error.retry || 3600 });
  if (error.code === 'invalid-credentials') return T('The username, password or setup code is incorrect.');
  if (error.code === 'confirm-unsafe-subnet') return T('A /0 subnet trusts everyone. Confirm the warning before saving it.');
  if (error.code === 'invalid-subnet') return T('Enter valid IPv4/IPv6 addresses or CIDRs, one per line.');
  if (error.code === 'owner-login-required') return T('Sign in with the owner password to manage these settings.');
  if (error.code === 'configuration-changed-reload' || error.code === 'invalid-auth-policy') return T('The configuration changed or a value is invalid. Reload this page section and check the values.');
  if (error.code === 'auth-storage-unavailable') return T('Authentication storage is unavailable. Check server logs and directory permissions.');
  if (error.code === 'auth-busy') return T('The server is busy. Wait a moment and try again.');
  if (error.code === 'unverified-client') return T('The proxy client address could not be verified. Check trusted proxy configuration.');
  if (error.code === 'folder-outside-downloads') return T('Choose a folder inside the downloads directory.');
  if (error.code === 'unsupported') return T('MDM cannot download this address.');
  if (error.code === 'not-a-torrent') return T('This is not a torrent file.');
  return error.body?.code ? F('The server refused the operation: {reason}', { reason: error.body.code }) : T('The server did not answer. Check the connection and try again.');
}
let toastTimer;
function toast(text, bad = false) { $('toast').textContent = text; $('toast').classList.toggle('bad', bad); $('toast').hidden = false; clearTimeout(toastTimer); toastTimer = setTimeout(() => { $('toast').hidden = true; }, bad ? 7000 : 3500); }
async function submit(form, operation) {
  const revision = edits.get(form) || 0;
  const button = form.querySelector('[type="submit"]'); if (button) { button.disabled = true; button.dataset.submitting = 'true'; }
  try { await operation(); if ((edits.get(form) || 0) === revision) dirty.delete(form.id); } catch (e) { toast(message(e), true); }
  finally { if (button) { delete button.dataset.submitting; button.disabled = button.dataset.blocked === 'true'; } }
}
function button(label, action, className) {
  const b = element('button', label, className); b.type = 'button';
  b.addEventListener('click', async () => { b.disabled = true; try { await action(); } catch (e) { toast(message(e), true); } finally { b.disabled = false; } }); return b;
}
function selectOptions(select, options) {
  const key = JSON.stringify(options); if (select.dataset.options === key) return;
  const old = select.value; select.replaceChildren(...options.map(([value, label]) => { const o = element('option', label); o.value = value; return o; }));
  select.dataset.options = key; if (options.some(([value]) => value === old)) select.value = old;
}
function showLogin(isSetup = false) {
  setup = isSetup; authenticated = false; stopPolling();
  $('workspace').hidden = true; $('logout').hidden = true; $('login-panel').hidden = false;
  $('setup-help').hidden = $('setup-code-field').hidden = !setup;
  $('setup-code').required = setup;
  $('login-title').textContent = setup ? T('Create your owner account') : T('Sign in to your server');
  $('login-submit').textContent = setup ? T('Create account') : T('Sign in');
  $('login-password').autocomplete = setup ? 'new-password' : 'current-password';
  $('login-password').minLength = setup ? 12 : 1;
  if (setup) { const m = /^#pair=(\d{8})$/.exec(location.hash); if (m) { $('setup-code').value = m[1]; history.replaceState(null, '', location.pathname); } }
}
function signedOut() { csrf = null; owner = false; data = null; config = null; selected.clear(); rows.clear(); $('download-list').replaceChildren(); for (const input of document.querySelectorAll('input[type="password"]')) input.value = ''; dirty.clear(); closeDetails(); showLogin(); }
async function restoreSession() {
  const s = await api('v1/auth/session');
  if (!s.authenticated) { showLogin(!s.configured); return; }
  csrf = s.csrf; owner = s.owner; authenticated = true;
  $('login-panel').hidden = true; $('workspace').hidden = false; $('logout').hidden = false; $('bypass-note').hidden = owner;
  await navigate('downloads', 'all', true); refresh();
}
$('login-form').addEventListener('submit', async (e) => {
  e.preventDefault(); $('login-error').textContent = ''; $('login-submit').disabled = true;
  try {
    const reply = await api(setup ? 'v1/auth/setup' : 'v1/auth/login', { json: { username: $('login-username').value, password: $('login-password').value, code: setup ? $('setup-code').value : '' } });
    csrf = reply.csrf; $('login-password').value = ''; $('setup-code').value = ''; await restoreSession();
  } catch (error) { $('login-error').textContent = message(error); }
  finally { $('login-submit').disabled = false; }
});
$('logout').addEventListener('click', async () => { if (dirty.size && !confirm(T('Discard unsaved changes?'))) return; try { await api('v1/auth/logout', { post: true }); signedOut(); } catch (e) { toast(message(e), true); } });
$('owner-login').addEventListener('click', () => showLogin());

function stopPolling() { clearTimeout(timer); timer = null; }
function schedule() { stopPolling(); if (authenticated && document.visibilityState === 'visible') timer = setTimeout(refresh, failures ? Math.min(15000, 1500 * (failures + 1)) : 1500); }
function refresh() {
  if (!authenticated) return Promise.resolve();
  if (refreshing) return refreshing;
  refreshing = (async () => {
    try {
      const next = await api('v1/downloads');
      if (!Array.isArray(next.downloads) || !Array.isArray(next.queues) || !Array.isArray(next.categories)) throw new Error('Invalid snapshot');
      if (!authenticated) return;
      data = next; failures = 0; $('offline').hidden = true; $('loading').hidden = true; render();
      if (detailId && owner && Date.now() - lastDetails > 5000) loadDetails(detailId, false).catch(() => {});
    } catch (e) {
      if (!authenticated) return;
      failures++; $('offline').textContent = data ? T('Connection lost. Showing the last snapshot; changes may not have reached the server.') : T('Waiting for the server. Retrying automatically...'); $('offline').hidden = false;
    } finally { refreshing = null; schedule(); }
  })(); return refreshing;
}
function byId(id) { return data?.downloads.find((x) => x.id === Number(id)); }
function visibleDownloads() {
  const search = $('search').value.trim().toLocaleLowerCase(LANG); const category = $('filter-category').value;
  const list = (data?.downloads || []).filter((x) => (filter === 'all' || x.status === filter) && (!category || x.category === category) && (!search || `${x.file} ${x.url}`.toLocaleLowerCase(LANG).includes(search)));
  const sort = $('sort').value; const collator = new Intl.Collator(LANG, { numeric: true });
  return list.sort((a, b) => sort === 'name' ? collator.compare(a.file, b.file) : sort === 'size' ? (b.size || 0) - (a.size || 0) || b.id - a.id : sort === 'progress' ? (b.size ? b.downloaded / b.size : 0) - (a.size ? a.downloaded / a.size : 0) || b.id - a.id : b.id - a.id);
}
function render() {
  if (!data) return;
  $('summary').textContent = speed(data.speed) + (data.free == null ? '' : ' · ' + F('{size} free', { size: size(data.free) }));
  for (const count of document.querySelectorAll('[data-count]')) count.textContent = num(count.dataset.count === 'all' ? data.downloads.length : data.downloads.filter((x) => x.status === count.dataset.count).length);
  const categories = data.categories.map((c) => [c.key, T(c.name)]);
  selectOptions($('add-category'), [['', T('Automatic (by file type)')], ...categories]);
  selectOptions($('filter-category'), [['', T('All categories')], ...categories]);
  selectOptions($('detail-category'), categories);
  selectOptions($('add-when'), [['', T('Start now')], ...data.queues.map((q) => [q.name, F('Wait in {queue}', { queue: T(q.name) })])]);
  const activeElement = document.activeElement; const shown = visibleDownloads(); const keep = new Set();
  for (const [index, x] of shown.entries()) {
    keep.add(x.id); let row = rows.get(x.id);
    if (!row) {
      row = $('download-row').content.firstElementChild.cloneNode(true);
      row.querySelector('.select-download').addEventListener('change', (e) => { if (e.target.checked) selected.add(x.id); else selected.delete(x.id); renderSelection(); });
      row.querySelector('.filename').addEventListener('click', () => loadDetails(x.id).catch((e) => toast(message(e), true)));
      row.querySelector('.toggle').addEventListener('click', () => runAction(byId(x.id)?.status === 'Downloading' ? 'pause' : 'start', [x.id]));
      row.querySelector('.remove').addEventListener('click', () => askRemove([x.id])); rows.set(x.id, row);
    }
    row.querySelector('.filename').textContent = x.file; row.querySelector('.filename').title = x.file;
    row.querySelector('.select-download').setAttribute('aria-label', F('Select {name}', { name: x.file }));
    row.querySelector('.state').textContent = T(x.status); row.querySelector('.state').className = 'state s-' + x.status.toLowerCase();
    row.classList.toggle('s-complete-row', x.status === 'Complete');
    const pct = x.status === 'Complete' ? 100 : x.size ? Math.max(0, Math.min(100, 100 * x.downloaded / x.size)) : 0;
    const track = row.querySelector('.progress-track'); track.setAttribute('aria-label', F('Progress for {name}', { name: x.file }));
    if (x.size || x.status === 'Complete') track.setAttribute('aria-valuenow', String(Math.round(pct))); else track.removeAttribute('aria-valuenow');
    row.querySelector('.progress-fill').style.width = pct + '%';
    row.querySelector('.progress-text').textContent = x.size || x.status === 'Complete' ? num(pct, 1) + '%' : size(x.downloaded);
    row.querySelector('.file-size').textContent = size(x.size); row.querySelector('.file-speed').textContent = x.status === 'Downloading' ? speed(x.speed) : '—'; row.querySelector('.file-eta').textContent = eta(x);
    row.querySelector('.toggle').hidden = x.status === 'Complete'; row.querySelector('.toggle').textContent = x.status === 'Downloading' ? T('Pause') : T('Start');
    row.querySelector('.remove').textContent = T('Remove'); row.querySelector('.row-error').hidden = !x.error; row.querySelector('.row-error').textContent = x.error || '';
    if ($('download-list').children[index] !== row) $('download-list').insertBefore(row, $('download-list').children[index] || null);
  }
  for (const [id, row] of rows) if (!keep.has(id)) { row.remove(); rows.delete(id); }
  for (const id of selected) if (!byId(id)) selected.delete(id);
  $('downloads-table').hidden = !shown.length; $('empty').hidden = !!shown.length;
  renderSelection(); renderQueueControls();
  if (detailId && !byId(detailId)) closeDetails();
  if (activeElement?.isConnected && document.activeElement !== activeElement) activeElement.focus({ preventScroll: true });
}
function renderSelection() {
  for (const [id, row] of rows) { row.querySelector('.select-download').checked = selected.has(id); row.classList.toggle('selected', selected.has(id)); }
  const shown = visibleDownloads(); const count = shown.filter((x) => selected.has(x.id)).length;
  $('select-all').checked = !!shown.length && count === shown.length; $('select-all').indeterminate = count > 0 && count < shown.length;
  $('bulk').hidden = selected.size === 0; $('selection-count').textContent = F('{n} selected', { n: num(selected.size) });
}
function renderQueueControls() {
  const keep = new Set();
  for (const q of data.queues) {
    keep.add(q.name); let row = queueRows.get(q.name);
    if (!row) { row = element('div', null, 'queue-control'); row.append(element('span'), button('', () => queueAction(q.name, !data.queues.find((x) => x.name === q.name)?.running))); queueRows.set(q.name, row); $('queue-controls').append(row); }
    row.firstChild.textContent = `${T(q.name)} · ${num(q.ids.length)}`; row.lastChild.textContent = q.running ? T('Stop') : T('Start');
  }
  for (const [name, row] of queueRows) if (!keep.has(name)) { row.remove(); queueRows.delete(name); }
}
async function runAction(action, ids) {
  try { const r = await api('v1/actions', { json: { action, ids } }); if (r.skipped?.length) toast(F('Completed: {done}. Skipped: {skipped}.', { done: r.done.length, skipped: r.skipped.length }), true); await refresh(); }
  catch (e) { toast(message(e), true); }
}
async function queueAction(name, start) { await api('v1/queues/action', { json: { name, start } }); await refresh(); }
function askRemove(ids) { removing = ids.filter((id) => byId(id)); if (!removing.length) return; $('remove-text').textContent = F('Remove {n} downloads from the list?', { n: num(removing.length) }); $('remove-files').checked = false; $('remove-dialog').showModal(); }
$('remove-dialog').addEventListener('close', async () => {
  const ids = removing; removing = []; if ($('remove-dialog').returnValue !== 'remove') return;
  const running = ids.filter((id) => byId(id)?.status === 'Downloading');
  try { if (running.length) await api('v1/actions', { json: { action: 'pause', ids: running } }); await runAction($('remove-files').checked ? 'delete' : 'remove', ids); }
  catch (e) { toast(message(e), true); }
});
$('select-all').addEventListener('change', (e) => { for (const x of visibleDownloads()) if (e.target.checked) selected.add(x.id); else selected.delete(x.id); renderSelection(); });
$('clear-selection').addEventListener('click', () => { selected.clear(); renderSelection(); });
$('bulk-remove').addEventListener('click', () => askRemove([...selected]));
for (const b of document.querySelectorAll('[data-bulk]')) b.addEventListener('click', () => runAction(b.dataset.bulk, [...selected]));
for (const id of ['search', 'filter-category', 'sort']) $(id).addEventListener(id === 'search' ? 'input' : 'change', render);

async function manage(payload) { return api('v1/manage', { json: payload }); }
async function loadDetails(id, full = true) {
  const changing = detailId !== id;
  if (!changing && detailLoading) return;
  if (!changing && loadedDetailId === id && detailForms.some((form) => dirty.has(form))) full = false;
  if (changing && detailId !== null && detailForms.some((form) => dirty.has(form)) && !confirm(T('Discard unsaved changes?'))) return;
  const x = byId(id); if (!x) return;
  if (changing) {
    resetDetailDrafts(); loadedDetailId = null;
    $('detail-form').hidden = $('file-tools').hidden = $('torrent-tools').hidden = true;
    $('detail-info').replaceChildren(element('p', T('Loading download details...'), 'muted'));
    $('detail-connections').replaceChildren(); $('detail-log').replaceChildren();
  }
  detailId = id; lastDetails = Date.now(); $('details').hidden = false; $('transfer-layout').classList.add('with-details'); $('detail-name').textContent = x.file;
  const sequence = ++detailRequestSequence; detailLoading = true;
  let d;
  try { d = owner ? await manage({ op: 'details', id }) : { url: x.url, path: T('Owner login required'), connections: [], log: [] }; }
  finally { if (sequence === detailRequestSequence) detailLoading = false; }
  if (detailId !== id || sequence !== detailRequestSequence) return;
  loadedDetailId = id;
  const info = [[T('Address'), d.url], [T('Destination'), d.path], [T('Status'), T(byId(id)?.status || x.status)]];
  if (d.torrent) info.push([T('Torrent info hash'), d.torrent.hash], [T('Uploaded'), size(d.torrent.uploaded)]);
  $('detail-info').replaceChildren(...info.flatMap(([key, value]) => [element('dt', key), element('dd', value)]));
  $('detail-form').hidden = !owner;
  renderFileTools(d, byId(id) || x, full);
  renderTorrent(d.torrent, full);
  if (full && owner) { $('detail-description').value = d.description || ''; $('detail-limit').value = d.limit_kb || 0; $('detail-category').value = d.category || x.category; dirty.delete('detail-form'); }
  $('detail-connections').replaceChildren(...(d.connections?.length ? d.connections.map((c) => element('p', `${c.id} · ${speed(c.speed)} · ${c.status}`)) : [element('p', T('No active connection details.'), 'muted')]));
  $('detail-log').replaceChildren(...(d.log || []).slice().reverse().map(([at, text]) => element('li', new Date(at * 1000).toLocaleTimeString(LANG) + ' · ' + text)));
}
const detailForms = ['detail-form', 'move-form', 'source-form', 'checksum-form', 'torrent-form', 'trackers-form'];
function resetDetailDrafts() { for (const form of detailForms) { const node = $(form); node.reset(); edits.set(node, (edits.get(node) || 0) + 1); dirty.delete(form); } }
function closeDetails() { detailId = loadedDetailId = null; detailLoading = false; detailRequestSequence++; $('details').hidden = true; $('transfer-layout').classList.remove('with-details'); resetDetailDrafts(); }
$('close-details').addEventListener('click', () => { if (!detailForms.some((form) => dirty.has(form)) || confirm(T('Discard unsaved changes?'))) closeDetails(); });
$('detail-form').addEventListener('submit', (e) => { e.preventDefault(); submit(e.target, async () => { await manage({ op: 'download.update', id: detailId, category: $('detail-category').value, limit_kb: Number($('detail-limit').value), description: $('detail-description').value }); toast(T('Changes saved.')); await refresh(); }); });

function renderFileTools(d, x, full) {
  $('file-tools').hidden = !owner || !d.plain_file;
  if (full) {
    $('move-name').value = d.file || x.file; $('move-folder').value = d.relative_folder || '';
    $('source-url').value = d.url || x.url; $('checksum-expected').value = d.expected_checksum || '';
    for (const form of detailForms.filter((f) => f !== 'detail-form')) dirty.delete(form);
    $('source-result').textContent = '';
  }
  const running = x.status === 'Downloading';
  blockSubmit('move-form', !!d.moving || running || (x.status !== 'Complete' && x.downloaded > 0));
  $('move-result').textContent = d.moving ? T('Moving the file...') : T('Completed moves appear above. Failures are recorded in Activity.');
  blockSubmit('source-form', running || x.status === 'Complete' || !!d.moving);
  blockSubmit('checksum-form', running || !!d.moving || d.checksum_result?.state === 'checking');
  const state = d.checksum_result?.state;
  $('checksum-result').textContent = state === 'checking' ? T('Checking...') : state === 'match' ? T('The file matches the expected checksum.') : state === 'mismatch' ? T('The file does NOT match the expected checksum.') : state === 'error' ? T('The file could not be read. Check its location and server permissions.') : d.expected_checksum ? T('Expected checksum saved; checked when the download finishes.') : '';
  $('checksum-result').classList.toggle('error', state === 'mismatch' || state === 'error');
  $('checksum-actual').hidden = !d.checksum_result?.actual; $('checksum-actual').textContent = d.checksum_result?.actual || '';
}
function blockSubmit(form, blocked) { const b = $(form).querySelector('[type="submit"]'); b.dataset.blocked = String(!!blocked); b.disabled = !!blocked || b.dataset.submitting === 'true'; }
function renderTorrent(t, full) {
  $('torrent-tools').hidden = !owner || !t;
  if (!owner || !t) return;
  $('torrent-health').textContent = !t.metadata_available ? T('Torrent metadata is unavailable. Wait for discovery or restore the saved torrent file.') : t.files_truncated ? T('This torrent has too many files to edit here. File selection is read-only.') : t.busy ? T('A torrent operation is running. Wait for its result in Activity.') : t.pieces_total == null ? T('Peer and piece information appears while the torrent session is available.') : F('{complete} of {total} pieces verified', { complete: num(t.pieces_complete), total: num(t.pieces_total) });
  if (full || (!dirty.has('torrent-form') && !$('torrent-form').contains(document.activeElement))) {
    $('torrent-order').checked = t.in_order; $('torrent-ends').checked = t.ends_first; $('torrent-upload').value = t.upload_limit_kb || 0;
    $('torrent-files').replaceChildren(...(t.files || []).map((file) => {
      const label = element('label', null, 'check torrent-file'); const input = element('input'); input.type = 'checkbox'; input.value = String(file.index); input.checked = file.selected;
      const text = element('span', file.path); text.dir = 'auto'; text.append(element('small', file.downloaded == null ? size(file.size) : F('{received} of {total}', { received: size(file.downloaded), total: size(file.size) }))); label.append(input, text); return label;
    }));
  }
  blockSubmit('torrent-form', t.busy || !t.metadata_available || t.files_truncated);
  for (const action of document.querySelectorAll('[data-torrent-action]')) { action.dataset.blocked = String(!!t.busy); action.disabled = !!t.busy; }
  blockSubmit('trackers-form', !!t.busy);
  $('torrent-trackers').replaceChildren(...(t.trackers?.length ? t.trackers.map((url) => element('li', url)) : [element('li', T('No trackers listed.'))]));
  $('torrent-peers').replaceChildren(...(t.peers?.length ? t.peers.map((peer) => { const row = element('p', peer.address); row.append(element('small', peer.client || T('Unknown')), element('small', F('Received {received} · uploaded {uploaded}', { received: size(peer.downloaded), uploaded: size(peer.uploaded) }))); return row; }) : [element('p', T('No connected peers.'), 'muted')]));
}
for (const [form, payload, feedback] of [
  ['move-form', () => ({ op: 'download.move', name: $('move-name').value, folder: $('move-folder').value }), () => T('Move requested. The destination updates when the file operation finishes.')],
  ['source-form', () => ({ op: 'download.source', url: $('source-url').value }), () => T('Address replaced. Start the download to resume.')],
  ['checksum-form', () => ({ op: 'download.checksum', expected: $('checksum-expected').value }), () => T('Checksum saved.')],
  ['torrent-form', () => ({ op: 'torrent.update', files: [...$('torrent-files').querySelectorAll('input:checked')].map((i) => Number(i.value)), in_order: $('torrent-order').checked, ends_first: $('torrent-ends').checked, upload_limit_kb: Number($('torrent-upload').value) }), () => T('Torrent changes requested. Results appear in Activity.')],
  ['trackers-form', () => ({ op: 'torrent.action', action: 'add-trackers', trackers: $('tracker-addresses').value }), () => T('Tracker update requested. Results appear in Activity.')],
]) $(form).addEventListener('submit', (e) => { e.preventDefault(); const id = detailId, revision = edits.get(e.target) || 0; submit(e.target, async () => {
  await manage({ ...payload(), id }); toast(feedback());
  if (form === 'trackers-form' && detailId === id && (edits.get(e.target) || 0) === revision) $('tracker-addresses').value = '';
  await refresh(); if (detailId === id) await loadDetails(id, false);
}); });
for (const b of document.querySelectorAll('[data-torrent-action]')) b.addEventListener('click', async () => {
  b.disabled = true; const id = detailId;
  try { await manage({ op: 'torrent.action', id, action: b.dataset.torrentAction }); toast(T('Torrent operation requested. Results appear in Activity.')); if (detailId === id) await loadDetails(id, false); }
  catch (e) { toast(message(e), true); } finally { b.disabled = b.dataset.blocked === 'true'; }
});

$('show-add').addEventListener('click', () => { $('add-panel').hidden = !$('add-panel').hidden; if (!$('add-panel').hidden) $('url').focus(); });
$('close-add').addEventListener('click', () => { $('add-panel').hidden = true; });
$('torrent-file').addEventListener('change', () => {
  const file = $('torrent-file').files[0]; chosenFile = null; $('file-name').hidden = true;
  if (!file) return; if (file.size > 10 * 1024 * 1024) { toast(T('This file is too large to be a torrent.'), true); $('torrent-file').value = ''; return; }
  chosenFile = file; $('url').value = ''; $('file-name').textContent = file.name; $('file-name').hidden = false;
});
$('url').addEventListener('input', () => { if ($('url').value) { chosenFile = null; $('torrent-file').value = ''; $('file-name').hidden = true; } });
document.querySelector('label.file').addEventListener('keydown', (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); $('torrent-file').click(); } });
$('add-form').addEventListener('submit', (e) => { e.preventDefault(); submit(e.target, async () => {
  const category = $('add-category').value, queue = $('add-when').value, start = queue === '', url = $('url').value.trim();
  if (!url && !chosenFile) { $('url').focus(); return; }
  const result = chosenFile ? await api('v1/torrent?' + new URLSearchParams({ category, queue, start: String(start) }), { body: await chosenFile.arrayBuffer(), headers: { 'Content-Type': 'application/x-bittorrent' } }) : await api('v1/downloads', { json: { url, category, queue, start } });
  if (!Number.isSafeInteger(result.id) && result.pending !== true) throw new Error('No acceptance');
  toast(result.pending ? T('Added. MDM is reading the torrent: it joins the list in a moment.') : T('Added.'));
  $('url').value = ''; chosenFile = null; $('torrent-file').value = ''; $('file-name').hidden = true; await refresh();
}); });

async function navigate(next, nextFilter = 'all', force = false) {
  if (!force && dirty.size && !confirm(T('Discard unsaved changes?'))) return;
  if (next !== 'downloads' && !owner) { showLogin(); return; }
  if (detailForms.some((form) => dirty.has(form))) closeDetails();
  dirty.clear(); page = next; filter = nextFilter; const generation = ++navigationGeneration;
  for (const section of document.querySelectorAll('[data-view]')) section.hidden = page !== 'downloads' || section.dataset.view !== page;
  $('section-loading').hidden = page === 'downloads'; $('section-loading').textContent = T('Loading settings...');
  for (const b of document.querySelectorAll('[data-page]')) { if (b.dataset.page === page && (!b.dataset.filter || b.dataset.filter === filter)) b.setAttribute('aria-current', 'page'); else b.removeAttribute('aria-current'); }
  $('view-title').textContent = filter === 'all' ? T('All downloads') : T(filter);
  if (page === 'downloads') { if (data) render(); return; }
  try {
    if (page === 'security') { const result = await api('v1/auth/settings'); if (generation !== navigationGeneration) return; authConfig = result; renderAuth(); }
    else if (page === 'devices') await renderDevices();
    else { const result = await api('v1/manage'); if (generation !== navigationGeneration) return; config = result; if (page === 'settings') renderSettings(); if (page === 'queues') renderQueues(); if (page === 'categories') renderCategories(); }
    if (generation !== navigationGeneration) return;
    $('section-loading').hidden = true; document.querySelector(`[data-view="${page}"]`).hidden = false;
  } catch (e) { if (generation === navigationGeneration) $('section-loading').textContent = message(e); throw e; }
}
for (const b of document.querySelectorAll('[data-page]')) b.addEventListener('click', () => navigate(b.dataset.page, b.dataset.filter).catch((e) => toast(message(e), true)));

function renderSettings() {
  const root = $('settings-fields'); root.replaceChildren(); const groups = new Map();
  for (const f of config.fields) {
    let group = groups.get(f.section);
    if (!group) { group = element('fieldset'); group.append(element('legend', T(f.section))); root.append(group); groups.set(f.section, group); }
    const label = element('label'); const input = element('input'); input.name = f.key; input.dataset.setting = f.key;
    input.type = f.kind === 'boolean' ? 'checkbox' : f.kind === 'number' ? 'number' : 'text';
    if (f.kind === 'boolean') { input.checked = config.values[f.key]; label.className = 'check'; label.append(input, element('span', T(f.label))); }
    else { input.value = config.values[f.key] ?? ''; if (f.kind === 'number') { input.min = f.min; input.max = f.max; input.required = true; } else input.maxLength = f.max; label.append(element('span', T(f.label)), input); }
    group.append(label);
  }
  $('settings-result').textContent = '';
}
$('settings-form').addEventListener('submit', (e) => { e.preventDefault(); submit(e.target, async () => {
  const values = {}; for (const input of e.target.querySelectorAll('[data-setting]')) values[input.name] = input.type === 'checkbox' ? input.checked : input.type === 'number' ? Number(input.value) : input.value;
  config = await manage({ op: 'settings.save', revision: config.revision, values }); $('settings-result').textContent = T('Changes saved.');
}); });
function renderAuth() {
  const p = authConfig.policy;
  $('current-username').textContent = authConfig.username; $('new-username').value = authConfig.username;
  $('bypass-localhost').checked = p.bypass_localhost; $('bypass-subnets').checked = p.bypass_subnets;
  $('auth-subnets').value = p.subnets.join('\n'); $('trusted-proxies').value = p.trusted_proxies.join('\n');
  $('max-failures').value = p.max_failures; $('ban-seconds').value = p.ban_seconds; $('session-timeout').value = p.session_timeout;
  $('confirm-unsafe').checked = false; $('auth-result').textContent = '';
  $('ban-list').replaceChildren(...Object.entries(authConfig.bans).filter(([, until]) => until * 1000 > Date.now()).map(([ip, until]) => element('p', `${ip} · ${new Date(until * 1000).toLocaleString(LANG)}`)));
  if (!$('ban-list').children.length) $('ban-list').append(element('p', T('No active login bans.'), 'muted'));
}
const lines = (id) => $(id).value.split('\n').map((s) => s.trim()).filter(Boolean);
$('auth-settings-form').addEventListener('submit', (e) => { e.preventDefault(); submit(e.target, async () => {
  authConfig = await api('v1/auth/settings', { json: { revision: authConfig.revision, current_password: $('auth-current-password').value, confirm_unsafe: $('confirm-unsafe').checked,
    policy: { bypass_localhost: $('bypass-localhost').checked, bypass_subnets: $('bypass-subnets').checked, subnets: lines('auth-subnets'), trusted_proxies: lines('trusted-proxies'), max_failures: Number($('max-failures').value), ban_seconds: Number($('ban-seconds').value), session_timeout: Number($('session-timeout').value) } } });
  $('auth-current-password').value = ''; $('auth-result').textContent = T('Changes saved.');
}); });
$('confirm-password').addEventListener('input', () => $('confirm-password').setCustomValidity(''));
$('credentials-form').addEventListener('submit', (e) => { e.preventDefault(); if ($('new-password').value !== $('confirm-password').value) { $('confirm-password').setCustomValidity(T('Passwords do not match.')); $('confirm-password').reportValidity(); return; }
  submit(e.target, async () => { await api('v1/auth/credentials', { json: { username: $('new-username').value, password: $('new-password').value, current_password: $('credentials-current').value } }); e.target.reset(); signedOut(); toast(T('Credentials changed. Sign in again.')); });
});

function dataRow(title, subtitle, buttons = []) { const row = element('div', null, 'data-row'); const text = element('div'); text.append(element('strong', title), element('small', subtitle)); const actions = element('div', null, 'buttons'); actions.append(...buttons); row.append(text, actions); return row; }
function renderQueues() {
  $('queue-form').hidden = true; $('scheduler-timezone').textContent = F('Scheduler timezone: {timezone}. Times use the server clock.', { timezone: config.server.timezone });
  $('queue-list').replaceChildren(...config.queues.map((q, index) => dataRow(T(q.name), F('{n} downloads · {concurrency} concurrent', { n: q.ids.length, concurrency: q.files_at_once }), [
    button(q.running ? T('Stop') : T('Start'), async () => { await queueAction(q.name, !q.running); config = await api('v1/manage'); renderQueues(); }),
    button(T('Edit'), () => editQueue(q)), ...(index > 1 ? [button(T('Delete queue'), async () => { if (!confirm(T('Delete this queue? Its downloads and files will be kept.'))) return; config = await manage({ op: 'queue.delete', name: q.name, revision: config.revision }); renderQueues(); })] : []),
  ])));
}
function editQueue(q = null) {
  $('queue-form').reset(); $('queue-form').hidden = false; dirty.delete('queue-form');
  $('queue-old-name').value = q?.name || ''; $('queue-name').value = q?.name || ''; $('queue-name').readOnly = !!q && config.queues.indexOf(q) < 2;
  $('queue-concurrency').value = q?.files_at_once || 4;
  const s = q?.schedule || {}; const clock = (t) => t ? t.map((n) => String(n).padStart(2, '0')).join(':') : '';
  $('queue-start').value = clock(s.start_at); $('queue-stop').value = clock(s.stop_at); $('queue-retries').value = s.retries ?? ''; $('queue-startup').checked = !!s.start_on_startup;
  $('queue-once').value = s.once ? `${s.once[0]}-${String(s.once[1]).padStart(2, '0')}-${String(s.once[2]).padStart(2, '0')}` : '';
  $('queue-days').replaceChildren(element('legend', T('Days')));
  for (let i = 0; i < 7; i++) { const input = element('input'); input.type = 'checkbox'; input.dataset.day = i; input.checked = s.days ? s.days[i] : true; const label = element('label'); label.append(input, document.createTextNode(new Intl.DateTimeFormat(LANG, { weekday: 'short', timeZone: 'UTC' }).format(new Date(Date.UTC(2023, 0, 1 + i))))); $('queue-days').append(label); }
  const ids = q?.ids || []; const downloads = [...ids.map(byId).filter(Boolean), ...(data?.downloads || []).filter((x) => !ids.includes(x.id))];
  $('queue-members').replaceChildren(...downloads.map((x) => { const row = element('div', null, 'member-row'); row.dataset.id = x.id; const label = element('label'); const check = element('input'); check.type = 'checkbox'; check.checked = ids.includes(x.id); label.append(check, document.createTextNode(x.file)); row.append(label,
    button(T('Up'), () => { if (row.previousElementSibling) row.parentElement.insertBefore(row, row.previousElementSibling); dirty.add('queue-form'); }),
    button(T('Down'), () => { if (row.nextElementSibling) row.parentElement.insertBefore(row.nextElementSibling, row); dirty.add('queue-form'); })); return row; }));
}
$('new-queue').addEventListener('click', () => editQueue()); $('queue-cancel').addEventListener('click', () => { $('queue-form').hidden = true; dirty.delete('queue-form'); });
$('queue-form').addEventListener('submit', (e) => { e.preventDefault(); submit(e.target, async () => {
  const clock = (id) => $(id).value ? [...$(id).value.split(':').map(Number), 0].slice(0, 3) : null;
  config = await manage({ op: 'queue.save', revision: config.revision, old_name: $('queue-old-name').value || null, name: $('queue-name').value, files_at_once: Number($('queue-concurrency').value), ids: [...$('queue-members').children].filter((row) => row.querySelector('input').checked).map((row) => Number(row.dataset.id)),
    schedule: { start_at: clock('queue-start'), stop_at: clock('queue-stop'), days: [...$('queue-days').querySelectorAll('input')].map((el) => el.checked), start_on_startup: $('queue-startup').checked, retries: $('queue-retries').value === '' ? null : Number($('queue-retries').value), once: $('queue-once').value ? $('queue-once').value.split('-').map(Number) : null } }); renderQueues(); toast(T('Changes saved.')); await refresh();
}); });
function renderCategories() {
  $('category-form').hidden = true;
  $('category-list').replaceChildren(...config.categories.map((c) => dataRow(T(c.name), c.folder ?? T('Folder managed by the desktop configuration'), c.custom_id ? [button(T('Edit'), () => editCategory(c)), button(T('Delete category'), async () => { if (!confirm(T('Delete this category? Its files will be kept.'))) return; config = await manage({ op: 'category.delete', revision: config.revision, id: c.custom_id }); renderCategories(); await refresh(); })] : [])));
}
function editCategory(c = null) { $('category-form').reset(); $('category-form').hidden = false; $('category-id').value = c?.custom_id || ''; $('category-name').value = c?.name || ''; $('category-types').value = c?.types || ''; $('category-folder').value = c?.folder || ''; dirty.delete('category-form'); }
$('new-category').addEventListener('click', () => editCategory()); $('category-cancel').addEventListener('click', () => { $('category-form').hidden = true; dirty.delete('category-form'); });
$('category-form').addEventListener('submit', (e) => { e.preventDefault(); submit(e.target, async () => { config = await manage({ op: 'category.save', revision: config.revision, id: $('category-id').value ? Number($('category-id').value) : null, name: $('category-name').value, types: $('category-types').value, folder: $('category-folder').value }); renderCategories(); toast(T('Changes saved.')); await refresh(); }); });

async function renderDevices() {
  $('server-url').value = new URL('.', location.href).href;
  const result = await api('v1/devices');
  $('device-list').replaceChildren(...result.devices.map((d) => dataRow(d.name, F('Last seen: {time}', { time: new Date(d.seen * 1000).toLocaleString(LANG) }), [button(T('Revoke device'), () => { revoking = d.id; $('revoke-password').value = ''; $('password-dialog').showModal(); })])));
  if (!result.devices.length) $('device-list').append(element('p', T('No paired extension devices.'), 'muted'));
}
$('device-pair-form').addEventListener('submit', (e) => { e.preventDefault(); submit(e.target, async () => { const result = await api('v1/devices/pair', { json: { current_password: $('device-password').value } }); $('device-password').value = ''; $('pair-code').textContent = result.code.slice(0, 4) + ' ' + result.code.slice(4); toast(T('Pairing code created. It is single-use and expires in 10 minutes.')); }); });
$('password-dialog').addEventListener('close', async () => { const id = revoking; revoking = null; const password = $('revoke-password').value; $('revoke-password').value = ''; if ($('password-dialog').returnValue !== 'revoke') return; try { await api('v1/devices/revoke', { json: { id, current_password: password } }); await renderDevices(); } catch (e) { toast(message(e), true); } });

for (const form of document.querySelectorAll('form')) if (!['login-form', 'add-form', 'device-pair-form'].includes(form.id)) form.addEventListener('input', () => { if (form.id) { dirty.add(form.id); edits.set(form, (edits.get(form) || 0) + 1); } });
window.addEventListener('beforeunload', (e) => { if (dirty.size) { e.preventDefault(); e.returnValue = ''; } });
for (const name of ['pointerdown', 'keydown']) document.addEventListener(name, (e) => {
  if (!e.isTrusted || !authenticated || Date.now() - lastActivity < 30000) return;
  lastActivity = Date.now(); api('v1/auth/activity', { post: true }).catch(() => {});
}, { passive: true });
document.addEventListener('visibilitychange', () => { if (document.visibilityState === 'visible') refresh(); else stopPolling(); });
window.addEventListener('pageshow', (e) => { if (e.persisted) restoreSession().catch(() => signedOut()); });
function theme() { document.documentElement.dataset.theme = $('theme').value === 'system' ? (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light') : $('theme').value; }
try { const value = localStorage.getItem('mdm-theme'); if (['light', 'dark', 'system'].includes(value)) $('theme').value = value; localStorage.removeItem('mdm-token'); } catch { /* Storage may be unavailable. */ }
$('theme').addEventListener('change', () => { theme(); try { localStorage.setItem('mdm-theme', $('theme').value); } catch { /* Visit only. */ } });
matchMedia('(prefers-color-scheme: dark)').addEventListener('change', theme); theme();
loadTexts().then(restoreSession).catch((e) => { showLogin(); $('login-error').textContent = message(e); });
