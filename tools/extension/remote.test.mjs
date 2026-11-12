// Fork-owned regressions. test.mjs remains the entry point and owns the native harness.
import assert from "node:assert/strict";

export async function run(h) {
  const { fakeApi, load, ask, settle, requests, natives, steps, ok, DEFAULT_TYPES } = h;
  for (const firefox of [true, false]) {
    for (const hostVersion of ['0.2.5', '0.1.0']) {
      const fake = fakeApi({ firefox, rules: { version: hostVersion } });
      const original = fake.api.runtime.getManifest;
      fake.api.runtime.getManifest = () => ({ ...original(), version: '9.0.0', version_name: '9.0.0 (MDM 0.2.5)' });
      load(fake.api, firefox);
      const state = await ask(fake.listeners)({ type: 'state' }, { id: 'ours', url: 'chrome-extension://ours/popup.html' });
      assert.equal(state.connected, true);
      assert.equal(state.why, hostVersion === '0.2.5' ? null : 'update-app', 'compare native engine compatibility, not the independent fork version');
    }
  }
  ok('fork extension versions preserve native engine compatibility and still warn for genuinely older hosts');
  const remoteToken = "a".repeat(64);
  const remoteBase = "https://nas.test/mdm/";
  const remoteSender = { id: "ours", url: "chrome-extension://ours/popup.html" };
  const remoteList = { api_version: 1, connected: true, rules: { types: DEFAULT_TYPES, capture: true } };
  const jsonReply = (body, status = 200) => ({ status, body: JSON.stringify(body) });
  function remoteApi(firefox = true, host = false) {
    const fake = fakeApi({ firefox, host });
    fake.store.remote = { url: remoteBase, token: remoteToken };
    return fake;
  }
  for (const firefox of [true, false]) {
    const { api, calls, listeners, store } = fakeApi({ firefox, host: false });
    h.served = { [`${remoteBase}api/pair`]: jsonReply({ token: remoteToken }), [`${remoteBase}api/status`]: jsonReply(remoteList) };
    load(api, firefox);
    await settle();
    const send = ask(listeners), start = requests.length;
    assert.equal((await send({ type: "remote-pair", url: " https://nas.test/mdm ", code: "1234 5678" }, remoteSender)).ok, true);
    assert.equal(store.remote.url, remoteBase, "preserves a proxy prefix and adds its trailing slash");
    const state = await send({ type: "state" }, remoteSender);
    assert.equal(state.connected, true); assert.equal(state.remote, remoteBase);
    const sent = requests.slice(start);
    assert.deepEqual(sent.map((r) => [r.url, r.opts.method]), [[`${remoteBase}api/pair`, "POST"], [`${remoteBase}api/status`, "GET"], [`${remoteBase}api/status`, "GET"]]);
    assert.equal(sent[0].opts.headers.Authorization, undefined, "pairing gets no old token");
    assert.equal(sent[1].opts.headers.Authorization, `Bearer ${remoteToken}`);
    for (const r of sent) {
      assert.equal(r.opts.credentials, "omit"); assert.equal(r.opts.redirect, "error"); assert.equal(r.opts.signal.aborted, true);
    }
    assert.deepEqual(natives(calls), []); assert.equal(store.rules.opened, false);
    await send({ type: "remote-open", url: store.remote.url }, remoteSender); await send({ type: "open-app" }, remoteSender);
    assert.deepEqual(calls.filter((c) => c[0] === "openTab"), [["openTab", remoteBase], ["openTab", remoteBase]]);
    ok(`remote ${firefox ? "Firefox" : "Chromium"}: URL prefix, pairing, actual connection check and web UI link`);
  }
  {
    const { api, listeners } = remoteApi(); load(api, true); await settle();
    const send = ask(listeners), start = requests.length;
    for (const url of ["bad", "http://nas.test/", "https://user:secret@nas.test/", "https://nas.test/?token=x", "https://nas.test/#pair=1"]) {
      assert.equal((await send({ type: "remote-pair", url, code: "12345678" }, remoteSender)).why, "address", url);
      assert.equal((await send({ type: "remote-open", url }, remoteSender)).why, "address", url);
    }
    assert.equal(requests.length, start);
    ok("remote: rejects plaintext, embedded credentials, queries and fragments before any request");
  }
  for (const firefox of [true, false]) {
    const { api, calls, listeners } = remoteApi(firefox);
    h.served = { [`${remoteBase}api/add`]: jsonReply({ id: 51, pending: false }) };
    load(api, firefox); await settle();
    const start = requests.length; let suggested = false;
    const download = { id: 100, url: "https://files.test/archive.zip", filename: "archive.zip" };
    if (firefox) listeners.created(download); else listeners.determining(download, () => { suggested = true; });
    await settle();
    const sent = requests.slice(start).filter((r) => r.url.endsWith("api/add"));
    assert.equal(sent.length, 1);
    assert.deepEqual(JSON.parse(sent[0].opts.body), { url: download.url, start: true }, "no browser secrets forwarded");
    assert.ok(steps(calls).includes("cancel")); assert.equal(suggested, false); assert.deepEqual(natives(calls), []);
    ok(`remote ${firefox ? "Firefox" : "Chromium"}: public download captured with no native host or forwarded secrets`);
  }
  for (const reply of [null, jsonReply({}), { status: 200, body: "<html>Sign in to proxy</html>" }, jsonReply({ error: "unsupported" }), jsonReply({ id: 0 }), jsonReply({ pending: false }), jsonReply({ error: "unavailable" }, 503)]) {
    const { api, calls, listeners } = remoteApi(false);
    h.served = reply ? { [`${remoteBase}api/add`]: reply } : {};
    load(api, false); await settle();
    const start = requests.length; let suggested = false;
    listeners.determining({ id: 101, url: "https://files.test/a.zip", filename: "a.zip" }, () => { suggested = true; });
    await settle();
    assert.equal(suggested, true); assert.deepEqual(steps(calls), []);
    assert.equal(requests.slice(start).filter((r) => r.url.endsWith("api/add")).length, 1, "no ambiguous retry");
  }
  ok("remote: offline, rejected, HTML and malformed acceptance replies leave the browser download intact");
  {
    const { api, calls, listeners, store } = remoteApi(true, true);
    h.served = { [`${remoteBase}api/status`]: jsonReply({ error: "not paired" }, 401) }; load(api, true);
    const state = await ask(listeners)({ type: "state" }, remoteSender);
    assert.equal(state.connected, false); assert.equal(state.why, "not-paired");
    assert.equal(store.remote.url, remoteBase); assert.equal(store.remote.token, null);
    listeners.created({ id: 102, url: "https://files.test/a.zip", filename: "a.zip" }); await settle();
    assert.deepEqual(natives(calls), []); assert.ok(!steps(calls).includes("cancel"));
    ok("remote: a revoked token keeps the destination and never falls back to local MDM");
  }
  {
    const { api, calls, listeners, store } = remoteApi(true, true);
    h.served = { [`${remoteBase}api/add`]: jsonReply({}, 401) }; load(api, true); await settle();
    listeners.created({ id: 103, url: "https://files.test/a.zip", filename: "a.zip" }); await settle();
    assert.equal(store.remote.token, null); assert.equal(store.remote.url, remoteBase);
    assert.deepEqual(steps(calls), ["pause", "resume"]); assert.deepEqual(natives(calls), []);
    ok("remote: revocation during capture resumes the browser's transfer");
  }
  {
    const { api, calls, listeners } = remoteApi(); h.served = { [`${remoteBase}api/add`]: jsonReply({ id: 99 }) };
    load(api, true); await settle(); const start = requests.length;
    listeners.beforeRequest({ method: "POST", url: "https://files.test/report.zip", requestId: "remote-post", requestBody: { formData: { secret: ["value"] } } });
    listeners.created({ id: 104, url: "https://files.test/report.zip", filename: "report.zip" }); await settle();
    assert.equal(requests.length, start); assert.deepEqual(steps(calls), ["pause", "resume"]);
    ok("remote: browser-session POST downloads stay with the browser");
  }
  {
    const { api, listeners, store } = remoteApi(); h.served = {}; load(api, true);
    const state = await ask(listeners)({ type: "state" }, remoteSender);
    assert.equal(state.connected, false); assert.equal(state.why, "unreachable"); assert.equal(store.remote.token, remoteToken);
    h.served[`${remoteBase}api/status`] = jsonReply(remoteList);
    assert.equal((await ask(listeners)({ type: "state" }, remoteSender)).connected, true);
    ok("remote: connection checks distinguish offline from revoked and recover without re-pairing");
  }
  {
    const { api, listeners, store } = remoteApi(); let respond;
    h.served = { [`${remoteBase}api/status`]: { respond: () => new Promise((resolve) => { respond = resolve; }) } }; load(api, true);
    const waiting = ask(listeners)({ type: "state" }, remoteSender); await settle();
    store.remote = { url: "https://other.test/", token: "b".repeat(64) }; respond({ status: 401, json: async () => ({}) }); await waiting;
    assert.equal(store.remote.token, "b".repeat(64));
    ok("remote: late responses cannot overwrite a different destination");
  }
  {
    const { api, listeners } = remoteApi(); let signal;
    h.served = { [`${remoteBase}api/status`]: { respond: async (opts) => { signal = opts.signal; return { status: 200, json: () => new Promise(() => {}) }; } } };
    load(api, true, { setTimeout: (f, ms) => setTimeout(f, ms === 15000 ? 5 : ms) });
    assert.equal((await ask(listeners)({ type: "state" }, remoteSender)).connected, false); assert.equal(signal.aborted, true);
    ok("remote: the timeout includes the response body and aborts the underlying fetch");
  }
  {
    const { api, listeners, store } = remoteApi(); h.served = { [`${remoteBase}api/forget`]: jsonReply({}) };
    load(api, true); await ask(listeners)({ type: "remote-off" }, remoteSender);
    assert.equal(store.remote, null); assert.equal(store.remoteStatus, null); assert.ok(!store.rules?.source);
    ok("remote: switching back to this computer clears server state and rules");
  }
  {
    const { api, listeners, store } = remoteApi();
    h.served = { [`${remoteBase}api/status`]: jsonReply({ code: "not-found", error: "not found" }, 404), [`${remoteBase}api/list`]: jsonReply({ downloads: [], queues: [], categories: [] }) };
    load(api, true); assert.equal((await ask(listeners)({ type: "state" }, remoteSender)).connected, true); assert.equal(store.remote.token, remoteToken);
    ok("remote: older servers fall back to the legacy list endpoint only on a missing status route");
  }
  {
    const { api, listeners, store } = remoteApi();
    h.served = { [`${remoteBase}api/status`]: jsonReply({ api_version: 1, connected: true, rules: { types: "ZIP", capture: true, excludedSites: "private.test", excludedAddresses: [], minKb: 512 } }) };
    const before = requests.length; load(api, true); await ask(listeners)({ type: "state" }, remoteSender);
    assert.equal(store.rules.types, "ZIP"); assert.equal(store.rules.minKb, 512); assert.equal(store.rules.excludedSites, "private.test");
    assert.ok(requests.slice(before).every((r) => !r.url.endsWith("api/list")));
    ok("remote: server capture rules are loaded without requesting download-list permissions");
  }
  {
    const { api, listeners, store } = remoteApi();
    h.served = { [`${remoteBase}api/status`]: jsonReply({ api_version: 1, connected: true, rules: { types: "", capture: false, opened: true } }) };
    load(api, true); await ask(listeners)({ type: "state" }, remoteSender);
    assert.equal(store.rules.types, ""); assert.equal(store.rules.capture, false); assert.equal(store.rules.opened, true);
    ok("remote: explicit empty/disabled capture rules and owner inline-capture policy are respected");
  }
}
