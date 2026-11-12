// Development-only popup fixture. No extension install, native host or real server required.
// node tools/extension/preview.mjs
// http://127.0.0.1:8766/?state=connected|offline|revoked&lang=en|de|ar
// All connection/pairing data is synthetic; this is not an integration test.
import http from "node:http";
import { readFileSync } from "node:fs";

const root = new URL("../../extension/", import.meta.url);
const assets = new Map([
  ["/popup.js", ["popup.js", "text/javascript"]],
  ["/popup.css", ["popup.css", "text/css"]],
  ["/icons/icon-32.png", ["icons/icon-32.png", "image/png"]],
  ["/icons/webmister-32.png", ["icons/webmister-32.png", "image/png"]],
]);
const server = http.createServer((req, res) => {
  const url = new URL(req.url, "http://127.0.0.1:8766");
  res.setHeader("Cache-Control", "no-store");
  if (url.pathname === "/") {
    const lang = ["en", "de", "ar"].includes(url.searchParams.get("lang")) ? url.searchParams.get("lang") : "en";
    const messages = JSON.parse(readFileSync(new URL(`_locales/${lang}/messages.json`, root), "utf8"));
    const state = url.searchParams.get("state") || "connected";
    const fixture = { lang, messages, state };
    const html = readFileSync(new URL("popup.html", root), "utf8")
      .replace("<head>", '<head><title>MDM popup — synthetic development fixture</title>')
      .replace('<script src="popup.js"></script>', `<script>(${mock.toString()})(${JSON.stringify(fixture).replace(/</g, "\\u003c")});</script><script src="popup.js"></script>`);
    res.writeHead(200, { "Content-Type": "text/html; charset=utf-8" }).end(html);
  } else if (assets.has(url.pathname)) {
    const [file, type] = assets.get(url.pathname);
    res.writeHead(200, { "Content-Type": type }).end(readFileSync(new URL(file, root)));
  } else {
    res.writeHead(404).end();
  }
});
server.listen(8766, "127.0.0.1", () => console.log("Synthetic popup fixture: http://127.0.0.1:8766/"));

function mock({ lang, messages, state }) {
  let remote = "https://nas.example.test/downloads/mister-download-manager/";
  let connected = state === "connected";
  let why = state === "revoked" ? "not-paired" : "unreachable";
  let paused = false;
  globalThis.fixtureActions = [];
  globalThis.chrome = {
    i18n: {
      getUILanguage: () => lang,
      getMessage: (key, subs = []) => {
        if (key === "@@bidi_dir") return lang === "ar" ? "rtl" : "ltr";
        const entry = messages[key];
        if (!entry) return "";
        return entry.message.replace(/\$(\w+)\$/g, (_, name) => {
          const index = Number(entry.placeholders?.[name.toLowerCase()]?.content?.slice(1)) - 1;
          return subs[index] || "";
        });
      },
    },
    runtime: {
      getManifest: () => ({ permissions: [] }),
      sendMessage: async (m) => {
        globalThis.fixtureActions.push(m);
        if (m.type === "state") return { remote, connected, why: connected ? null : why, paused, version: "fixture" };
        if (m.type === "remote-off") { remote = null; connected = false; }
        if (m.type === "pause") paused = m.paused;
        if (m.type === "remote-pair") {
          if (m.code.replace(/\s/g, "") !== "12345678") return { ok: false, why: "wrong-code" };
          try {
            const u = new URL(m.url);
            if (u.protocol !== "https:" || u.username || u.password || u.search || u.hash) throw new Error();
            remote = u.href;
          } catch { return { ok: false, why: "address" }; }
          connected = true;
        }
        return { ok: true };
      },
    },
    tabs: { query: async () => [] },
  };
}
