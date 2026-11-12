// Fork-owned server transport. Loaded before background.js in Firefox and by importScripts
// in Chromium. Only this factory is global; caches and credentials stay private per worker.
// background.js supplies explicit callbacks for the shared capture/status lifecycle.
globalThis.MDMRemote = (() => {
function create({ ext, withTimeout, showStatus, refreshRules, resetRules, waitForRules }) {
  // Fork release numbers are not native-host protocol/engine versions. The packager uses
  // the standard human-readable version_name to retain that compatibility baseline.
  const manifest = ext.runtime.getManifest();
  const engineVersion = /\(MDM (\d+\.\d+\.\d+)\)$/.exec(manifest.version_name || "")?.[1] || manifest.version;
  // Keep the destination after revocation: never silently use this computer's native host.
  async function target() {
    const { remote } = await ext.storage.local.get({ remote: null });
    return remote && typeof remote.url === "string" && remote.url ? remote : null;
  }

  function address(value) {
    try {
      const u = new URL(String(value).trim());
      if (u.protocol !== "https:" || u.username || u.password || u.search || u.hash) return null;
      if (!u.pathname.endsWith("/")) u.pathname += "/";
      return u.href;
    } catch {
      return null;
    }
  }

  async function call(base, path, body, token) {
    base = address(base);
    if (!base) return { status: 0, body: null, why: "address" };
    const controller = new AbortController();
    try {
      return await withTimeout((async () => {
        const r = await fetch(new URL(path, base), {
          method: body === undefined ? "GET" : "POST",
          headers: { ...(body === undefined ? {} : { "Content-Type": "application/json" }), ...(token ? { Authorization: `Bearer ${token}` } : {}) },
          ...(body === undefined ? {} : { body: JSON.stringify(body) }),
          credentials: "omit",
          cache: "no-store",
          redirect: "error",
          signal: controller.signal,
        });
        let json = null;
        try { json = await r.json(); } catch { /* HTML proxy errors aren't API replies. */ }
        return { status: r.status, body: json };
      })(), 15000);
    } catch {
      return { status: 0, body: null };
    } finally {
      // Timeout covers the response body and releases the underlying request as well.
      controller.abort();
    }
  }

  async function note(destination, why) {
    const current = await target();
    if (!current || current.url !== destination.url || current.token !== destination.token) return;
    await ext.storage.local.set({ remoteStatus: { url: destination.url, why, at: Date.now() } });
    if (why === "not-paired") await ext.storage.local.set({ remote: { url: destination.url, token: null } });
    await showStatus();
  }

  const problem = (r) => r.why || (r.status === 401 ? "not-paired" : r.status === 0 ? "unreachable" : "failed");
  let info = null;
  let infoRequest = null;
  async function fetchInfo(destination) {
    if (infoRequest?.url === destination.url && infoRequest?.token === destination.token) return infoRequest.promise;
    const promise = (async () => {
      let reply = await call(destination.url, "api/status", undefined, destination.token);
      if (reply.status === 404) reply = { ...(await call(destination.url, "api/list", undefined, destination.token)), legacy: true };
      info = { url: destination.url, token: destination.token, reply, at: Date.now() };
      return reply;
    })();
    const request = { url: destination.url, token: destination.token, promise };
    infoRequest = request;
    try { return await promise; } finally { if (infoRequest === request) infoRequest = null; }
  }

  async function check(destination) {
    if (!destination.token) return { ok: false, why: "not-paired" };
    const r = await fetchInfo(destination);
    const ok = r.status === 200 && !r.body?.error && (r.legacy
      ? Array.isArray(r.body?.downloads) && Array.isArray(r.body?.queues) && Array.isArray(r.body?.categories)
      : r.body?.api_version === 1 && r.body?.connected === true && typeof r.body?.rules?.types === "string");
    const why = ok ? null : problem(r);
    await note(destination, why);
    return { ok, why };
  }

  async function send(destination, message) {
    if (!destination.token || message.post !== undefined) return { ok: false, why: "refused" };
    try {
      const u = new URL(message.url);
      if (u.username || u.password) return { ok: false, why: "refused" };
    } catch { return { ok: false, why: "refused" }; }
    // Only the address: never forward browser cookies/auth/proxy or convert POST to GET.
    const r = await call(destination.url, "api/add", { url: message.url, start: true }, destination.token);
    if (r.status === 200 && r.body && !r.body.error && ((Number.isSafeInteger(r.body.id) && r.body.id > 0) || r.body.pending === true)) {
      await note(destination, null);
      return { ok: true };
    }
    // No ambiguous automatic retry without a durable server idempotency contract.
    await note(destination, problem(r));
    return { ok: false, why: "refused" };
  }

  // mdm_ipc::capture::DEFAULT_AUTO_TYPES; the extension regression keeps these equal.
  const DEFAULT_TYPES =
    "00* 3DM 3DS 3GP 3MF 7Z AAB AAC ABC ACE AFDESIGN AFPHOTO AI AIF AIFF APE APK APKM APKS APPIMAGE APPX APPXBUNDLE ARJ ARROW ARW ASF AU AVI AVRO AZW AZW3 BAK BAM BIN BLEND BZ BZ2 C4D CAB CAF CBR CBZ CCD CHD CKPT CPIO CR2 CR3 CRAM CSO DAE DAT DCM DDS DEB DFF DIVX DJVU DMG DNG DOC DOCX DSF DWG DXF EPS EPUB EXE EXR F3D F4V FASTA FASTQ FB2 FBX FEATHER FIG FITS FLAC FLATPAK FQ GCODE GCZ GEM GGML GGUF GLB GLTF GPKG GRIB GRIB2 GZ GZIP H5 HDF5 HDR IGES IGS IMG INDD IPA ISO ISZ JAR JOBLIB JSONL KERAS KRA LAS LAZ LLAMAFILE LWO LZ LZ4 LZH LZMA M2T M2TS M2V M4A M4B M4V MA MAT MAX MB MBTILES MDF MDS MID MIDI MKA MKV MLMODEL MOBI MOV MP3 MP4 MPA MPE MPEG MPG MSI MSIX MSIXBUNDLE MSP MSU MTS MXF NC NDJSON NEF NEMO NII NPY NPZ NRG NSP NSZ NUPKG OBJ ODP ODS ODT OGG OGM OGV ONNX OPUS ORC ORF OTF OVA OVF PAK PARQUET PB PBF PDF PICKLE PK3 PK4 PKG PKL PLJ PLY POTX PPS PPSX PPT PPTX PSB PSD PT PTH QCOW QCOW2 QT R0* R1* R2* R3* R4* R5* R6* R7* R8* R9* RA RAF RAR RDS RM RMVB RPM RTF RUN RVZ RW2 SAFETENSORS SEA SF2 SFZ SHP SIT SITX SKETCH SKP SLDASM SLDPRT SNAP SQLITE SQLITE3 STEP STL STP TAR TBZ TBZ2 TFLITE TGA TGZ TIF TIFF TLZ TORRENT TTA TTC TTF TXZ TZST UNITYPACKAGE USD USDA USDC USDZ VDB VDI VHD VHDX VMDK VOB VPK VSIX WAV WBFS WEBM WHL WIM WMA WMV WRL WTV WUA WV X3D XAPK XCF XCI XLS XLSB XLSM XLSX XZ Z Z0* ZIP ZIPX ZPR ZST ZTL";

  async function readRules(destination) {
    const cached = info?.url === destination.url && info?.token === destination.token && Date.now() - info.at < 60000;
    const reply = destination.token ? (cached ? info.reply : await fetchInfo(destination)) : null;
    const current = await target();
    if (!current || current.url !== destination.url || current.token !== destination.token) return null;
    const configured = reply?.status === 200 && typeof reply.body?.rules?.types === "string" ? reply.body.rules : null;
    return { source: destination.url, types: configured ? configured.types : DEFAULT_TYPES, capture: configured?.capture !== false,
      excludedSites: configured?.excludedSites || "", excludedAddresses: Array.isArray(configured?.excludedAddresses) ? configured.excludedAddresses : [],
      opened: configured?.opened === true, minKb: Number(configured?.minKb) || 0, at: Date.now() };
  }

  async function pair(value, code) {
    const base = address(value);
    if (!base) return { ok: false, why: "address" };
    const name = ext.i18n.getMessage("remote_device_name") || "Browser";
    const r = await call(base, "api/pair", { code: String(code), name, scope: "capture" }, null);
    if (r.status === 200 && /^[a-f0-9]{64}$/i.test(r.body?.token || "")) {
      await ext.storage.local.set({ remote: { url: base, token: r.body.token }, remoteStatus: { url: base, why: null, at: Date.now() } });
      await refreshRules();
      await showStatus();
      return { ok: true };
    }
    const why = { 0: "unreachable", 403: "wrong-code", 410: "no-code", 429: "too-soon" }[r.status] || "failed";
    return { ok: false, why, left: r.body && r.body.left };
  }

  async function off() {
    const destination = await target();
    if (destination?.token) await call(destination.url, "api/forget", {}, destination.token);
    resetRules();
    await ext.storage.local.set({ remote: null, remoteStatus: null, rules: null });
    info = null;
    await waitForRules();
    await refreshRules();
    await showStatus();
    return { ok: true };
  }

  return Object.freeze({ target, address, check, send, readRules, pair, off, engineVersion });
}
return Object.freeze({ create });
})();
