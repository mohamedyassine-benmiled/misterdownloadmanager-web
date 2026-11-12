//! The remote control end to end on this computer (127.0.0.1 only): the page, pairing, the
//! checks every request goes through, and commands reaching the app marked as remote.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use mdm_remote::{Config, Remote, Status};

/// A remote control on 127.0.0.1, any port, with a stand-in app that answers every command
/// with `{"seen": …}` and records it.
fn start(
    rt: &tokio::runtime::Runtime,
) -> (
    Remote,
    String,
    Arc<Mutex<Vec<mdm_ipc::Command>>>,
    tempfile::TempDir,
) {
    start_mode(rt, true)
}

fn start_mode(
    rt: &tokio::runtime::Runtime,
    tls: bool,
) -> (
    Remote,
    String,
    Arc<Mutex<Vec<mdm_ipc::Command>>>,
    tempfile::TempDir,
) {
    start_with_qbit(rt, tls, false, false)
}

fn start_with_qbit(
    rt: &tokio::runtime::Runtime,
    tls: bool,
    qbittorrent: bool,
    bypass: bool,
) -> (
    Remote,
    String,
    Arc<Mutex<Vec<mdm_ipc::Command>>>,
    tempfile::TempDir,
) {
    let dir = tempfile::tempdir().unwrap();
    let (tx, rx) = std::sync::mpsc::channel::<mdm_ipc::Incoming>();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    std::thread::spawn(move || {
        while let Ok(inc) = rx.recv() {
            let mut answer = serde_json::json!({
                "seen": inc.cmd.query.clone().or(inc.cmd.action.clone()).or(inc.cmd.url.clone()),
                "remote": inc.cmd.remote,
            });
            if inc
                .cmd
                .web_request
                .as_ref()
                .is_some_and(|v| v["op"] == "qbittorrent.control")
            {
                answer = serde_json::json!({"done": [7], "skipped": []});
            }
            if inc
                .cmd
                .web_request
                .as_ref()
                .is_some_and(|v| v["op"] == "qbittorrent.categories")
            {
                answer = serde_json::json!([{ "name": "tv-sonarr", "folder": "/downloads/tv" }]);
            }
            if inc
                .cmd
                .web_request
                .as_ref()
                .is_some_and(|v| v["op"] == "qbittorrent.files")
            {
                answer = serde_json::json!({"paths": {"root_path": "Show"}, "details": {
                    "metadata_available": true, "files_truncated": false,
                    "files": [{"index": 0, "path": "episode.mkv", "size": 100, "downloaded": 50, "selected": true}]
                }});
            }
            if inc
                .cmd
                .web_request
                .as_ref()
                .is_some_and(|v| v["op"] == "qbittorrent.info")
            {
                answer = serde_json::json!([{ "hash": "0123456789abcdef0123456789abcdef01234567",
                    "name": "episode.mkv", "category_key": "tv", "status": "Downloading", "live": true, "seeding": false,
                    "size": 100, "downloaded": 50, "uploaded": 20, "download_speed": 10,
                    "upload_speed": 2, "seeded_seconds": 0, "ratio_limit": -2.0, "seeding_time_limit": -2, "paths": {"save_path": "/downloads/tv",
                    "root_path": "", "content_path": "/downloads/tv/episode.mkv"} }]);
            }
            if inc
                .cmd
                .web_request
                .as_ref()
                .is_some_and(|v| v["op"] == "qbittorrent.preferences")
            {
                answer = serde_json::json!({"save_path": "/downloads"});
            }
            if inc
                .cmd
                .web_request
                .as_ref()
                .is_some_and(|v| v["op"] == "qbittorrent.set_category")
            {
                answer = serde_json::json!({"ok": true});
            }
            if inc
                .cmd
                .web_request
                .as_ref()
                .is_some_and(|v| v["op"] == "qbittorrent.removal_status")
            {
                answer = serde_json::json!({"ready": true});
            }
            if inc
                .cmd
                .web_request
                .as_ref()
                .is_some_and(|v| v["op"] == "qbittorrent.share_limits")
            {
                answer = serde_json::json!({"ok": true});
            }
            if inc
                .cmd
                .web_request
                .as_ref()
                .is_some_and(|v| v["op"] == "qbittorrent.create_category")
            {
                answer = serde_json::json!({"categories": []});
            }
            if inc
                .cmd
                .web_request
                .as_ref()
                .is_some_and(|v| v["op"] == "qbittorrent.add")
            {
                answer = serde_json::json!({"pending": true, "request_id": "test-request"});
            }
            log.lock().unwrap().push(inc.cmd);
            if let Some(r) = inc.reply {
                let _ = r.send(answer.to_string());
            }
        }
    });
    let sink = mdm_ipc::Sink::new(tx, Arc::new(|| {}));
    let texts: mdm_remote::Texts = Arc::new(|lang: &str| mdm_remote::PageTexts {
        lang: if lang.starts_with("de") {
            "de".into()
        } else {
            "en".into()
        },
        rtl: false,
        texts: [("Pair".to_owned(), "Koppeln".to_owned())].into(),
    });
    let remote = Remote::new(rt.handle().clone(), dir.path().join("remote"), sink, texts);
    remote.apply(Config {
        on: true,
        tls,
        qbittorrent,
        qbittorrent_subnets: "127.0.0.1/32,::1/128".into(),
        qbittorrent_auth_bypass: bypass,
        port: 0,
        listen: "127.0.0.1".into(),
        hosts: vec!["mdm.example".into()],
    });
    let Status::Listening(addrs) = remote.status() else {
        panic!("not listening: {:?}", remote.status());
    };
    assert_eq!(addrs.len(), 1);
    assert!(addrs[0].ip().is_loopback());
    let base = format!(
        "{}://127.0.0.1:{}/",
        if tls { "https" } else { "http" },
        addrs[0].port()
    );
    (remote, base, seen, dir)
}

#[test]
fn qbittorrent_bypass_is_explicit_and_rejects_forwarded_requests() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (remote, base, _, _dir) = start_with_qbit(&rt, false, true, true);
    rt.block_on(async {
        let c = client();
        let url = format!("{base}api/v2/app/webapiVersion");
        assert_eq!(c.get(&url).send().await.unwrap().status(), 200);
        assert_eq!(
            c.get(&url)
                .header("x-forwarded-for", "127.0.0.1")
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
        assert_eq!(
            c.get(&url)
                .header("cookie", "SID=invalid")
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
        assert_eq!(
            c.get(&url)
                .header("origin", "http://evil.example")
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
    });
    remote.stop();
}

#[test]
fn qbittorrent_controls_reach_owner_bridge_with_validated_hashes() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (remote, base, seen, _dir) = start_with_qbit(&rt, false, true, true);
    rt.block_on(async {
        let client = client();
        let pause = format!("{base}api/v2/torrents/stop");
        assert_eq!(
            client
                .post(&pause)
                .header("content-type", "application/x-www-form-urlencoded")
                .body("hashes=bad")
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
        assert_eq!(
            client
                .post(&pause)
                .header("content-type", "application/x-www-form-urlencoded")
                .body("hashes=all")
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
        let resume = format!("{base}api/v2/torrents/resume");
        assert_eq!(
            client
                .post(&resume)
                .header("content-type", "application/x-www-form-urlencoded")
                .body("hashes=0123456789abcdef0123456789abcdef01234567")
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
    });
    let commands = seen.lock().unwrap();
    let controls: Vec<_> = commands
        .iter()
        .filter(|cmd| {
            cmd.web_request
                .as_ref()
                .is_some_and(|v| v["op"] == "qbittorrent.control")
        })
        .collect();
    assert_eq!(controls.len(), 2);
    assert!(controls.iter().all(|cmd| cmd.remote && cmd.web_owner));
    assert_eq!(controls[0].web_request.as_ref().unwrap()["action"], "pause");
    assert_eq!(
        controls[1].web_request.as_ref().unwrap()["action"],
        "resume"
    );
    drop(commands);
    remote.stop();
}

#[test]
fn qbittorrent_remove_maps_the_delete_files_choice() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (remote, base, seen, _dir) = start_with_qbit(&rt, false, true, true);
    rt.block_on(async {
        let client = client();
        let url = format!("{base}api/v2/torrents/delete");
        assert_eq!(
            client
                .post(&url)
                .header("content-type", "application/x-www-form-urlencoded")
                .body("hashes=all&deleteFiles=true")
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
        assert_eq!(
            client
                .post(&url)
                .header("content-type", "application/x-www-form-urlencoded")
                .body("hashes=all")
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
    });
    let commands = seen.lock().unwrap();
    let controls: Vec<_> = commands
        .iter()
        .filter_map(|cmd| cmd.web_request.as_ref())
        .filter(|request| request["op"] == "qbittorrent.control")
        .collect();
    assert_eq!(controls.len(), 4);
    assert_eq!(controls[0]["action"], "pause");
    assert_eq!(controls[1]["action"], "delete");
    assert_eq!(controls[2]["action"], "pause");
    assert_eq!(controls[3]["action"], "remove");
    drop(commands);
    remote.stop();
}

#[test]
fn qbittorrent_files_translate_the_owner_bridge_response() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (remote, base, seen, _dir) = start_with_qbit(&rt, false, true, true);
    rt.block_on(async {
        let client = client();
        let url =
            format!("{base}api/v2/torrents/files?hash=0123456789abcdef0123456789abcdef01234567");
        let result = client.get(url).send().await.unwrap();
        assert_eq!(result.status(), 200);
        let body: serde_json::Value = result.json().await.unwrap();
        assert_eq!(body[0]["name"], "Show/episode.mkv");
        assert_eq!(body[0]["progress"], 0.5);
        assert_eq!(
            client
                .get(format!("{base}api/v2/torrents/files?hash=all"))
                .send()
                .await
                .unwrap()
                .status(),
            400
        );
    });
    assert!(seen.lock().unwrap().iter().any(|cmd| {
        cmd.remote
            && cmd.web_owner
            && cmd
                .web_request
                .as_ref()
                .is_some_and(|v| v["op"] == "qbittorrent.files")
    }));
    remote.stop();
}

#[test]
fn qbittorrent_info_translates_owner_bridge_and_rejects_unsupported_filters() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (remote, base, seen, _dir) = start_with_qbit(&rt, false, true, true);
    rt.block_on(async {
        let client = client();
        let result = client
            .get(format!("{base}api/v2/torrents/info?category=tv"))
            .send()
            .await
            .unwrap();
        assert_eq!(result.status(), 200);
        let body: serde_json::Value = result.json().await.unwrap();
        assert_eq!(body[0]["progress"], 0.5);
        assert_eq!(body[0]["content_path"], "/downloads/tv/episode.mkv");
        let filtered = client
            .get(format!("{base}api/v2/torrents/info?filter=completed"))
            .send()
            .await
            .unwrap();
        assert_eq!(filtered.status(), 200);
        assert!(
            filtered
                .json::<serde_json::Value>()
                .await
                .unwrap()
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            client
                .get(format!("{base}api/v2/torrents/info?filter=unsupported"))
                .send()
                .await
                .unwrap()
                .status(),
            501
        );
    });
    assert!(seen.lock().unwrap().iter().any(|cmd| {
        cmd.remote
            && cmd.web_owner
            && cmd
                .web_request
                .as_ref()
                .is_some_and(|v| v["op"] == "qbittorrent.info" && v["category"] == "tv")
    }));
    remote.stop();
}

#[test]
fn qbittorrent_preferences_report_mdm_default_folder() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (remote, base, seen, _dir) = start_with_qbit(&rt, false, true, true);
    rt.block_on(async {
        let response = client()
            .get(format!("{base}api/v2/app/preferences"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let value: serde_json::Value = response.json().await.unwrap();
        assert_eq!(value["save_path"], "/downloads");
    });
    assert!(seen.lock().unwrap().iter().any(|cmd| {
        cmd.remote
            && cmd.web_owner
            && cmd
                .web_request
                .as_ref()
                .is_some_and(|v| v["op"] == "qbittorrent.preferences")
    }));
    remote.stop();
}

#[test]
fn qbittorrent_category_assignment_uses_owner_bridge() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (remote, base, seen, _dir) = start_with_qbit(&rt, false, true, true);
    rt.block_on(async {
        let response = client()
            .post(format!("{base}api/v2/torrents/setCategory"))
            .header("content-type", "application/x-www-form-urlencoded")
            .body("hashes=all&category=tv-sonarr")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
    });
    assert!(seen.lock().unwrap().iter().any(|cmd| cmd.remote
        && cmd.web_owner
        && cmd.web_request.as_ref().is_some_and(
            |v| v["op"] == "qbittorrent.set_category" && v["category"] == "tv-sonarr"
        )));
    remote.stop();
}

#[test]
fn qbittorrent_share_limits_use_mdm_torrent_settings() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (remote, base, seen, _dir) = start_with_qbit(&rt, false, true, true);
    rt.block_on(async {
        let url = format!("{base}api/v2/torrents/setShareLimits");
        let response = client()
            .post(&url)
            .header("content-type", "application/x-www-form-urlencoded")
            .body("hashes=all&ratioLimit=1.5&seedingTimeLimit=60")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let response = client()
            .post(&url)
            .header("content-type", "application/x-www-form-urlencoded")
            .body("hashes=all&ratioLimit=-2&seedingTimeLimit=-2&inactiveSeedingTimeLimit=5")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 501);
    });
    assert!(seen.lock().unwrap().iter().any(|cmd| {
        cmd.remote
            && cmd.web_owner
            && cmd.web_request.as_ref().is_some_and(|v| {
                v["op"] == "qbittorrent.share_limits" && v["ratio"] == 1.5 && v["minutes"] == 60
            })
    }));
    remote.stop();
}

#[test]
fn qbittorrent_add_magnet_uses_existing_owner_bridge() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (remote, base, seen, _dir) = start_with_qbit(&rt, false, true, true);
    rt.block_on(async {
        let body = "--test-boundary\r\nContent-Disposition: form-data; name=\"urls\"\r\n\r\nmagnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567\r\n--test-boundary\r\nContent-Disposition: form-data; name=\"category\"\r\n\r\ntv-sonarr\r\n--test-boundary--\r\n";
        let response = client().post(format!("{base}api/v2/torrents/add"))
            .header("content-type", "multipart/form-data; boundary=test-boundary")
            .body(body).send().await.unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.text().await.unwrap(), "Ok.");
        let response = client().post(format!("{base}api/v2/torrents/add"))
            .header("content-type", "application/x-www-form-urlencoded")
            .body("urls=magnet%3A%3Fxt%3Durn%3Abtih%3A0123456789abcdef0123456789abcdef01234567&category=tv-sonarr&paused=False")
            .send().await.unwrap();
        assert_eq!(response.status(), 200);
    });
    assert!(seen.lock().unwrap().iter().any(|cmd| {
        cmd.remote
            && cmd.web_owner
            && cmd
                .web_request
                .as_ref()
                .is_some_and(|v| v["op"] == "qbittorrent.add" && v["category"] == "tv-sonarr")
    }));
    remote.stop();
}

#[test]
fn qbittorrent_login_is_opt_in_and_uses_owner_credentials() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (disabled, disabled_base, _, _dir) = start_mode(&rt, false);
    let (remote, base, _, _dir2) = start_with_qbit(&rt, false, true, false);
    rt.block_on(async {
        let c = client();
        let login = |url: String| c.post(url).header("content-type", "application/x-www-form-urlencoded")
            .body("username=owner&password=correct+horse+battery+staple");
        assert_eq!(login(format!("{disabled_base}api/v2/auth/login")).send().await.unwrap().status(), 404);
        let setup = c.post(format!("{base}api/v1/auth/setup"))
            .json(&serde_json::json!({"code":remote.start_pairing(), "username":"owner", "password":"correct horse battery staple"}))
            .send().await.unwrap();
        assert_eq!(setup.status(), 200);
        let response = login(format!("{base}api/v2/auth/login")).send().await.unwrap();
        assert_eq!(response.status(), 200);
        let cookie = response.headers()["set-cookie"].to_str().unwrap();
        assert!(cookie.starts_with("SID=") && cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict"));
        let cookie = cookie.split(';').next().unwrap().to_owned();
        assert_eq!(response.text().await.unwrap(), "Ok.");
        assert_eq!(login(format!("{base}api/v2/auth/login")).header("origin", "http://evil.example").send().await.unwrap().status(), 403);
        assert_eq!(login(format!("{base}api/v2/auth/login")).header("origin", "chrome-extension://untrusted").send().await.unwrap().status(), 403);
        assert_eq!(login(format!("{base}api/v2/auth/login")).header("x-forwarded-for", "127.0.0.1").send().await.unwrap().status(), 403);
        let version = format!("{base}api/v2/app/webapiVersion");
        assert_eq!(c.get(&version).send().await.unwrap().status(), 403);
        assert_eq!(c.get(&version).header("cookie", &cookie).send().await.unwrap().status(), 200);
        let categories = format!("{base}api/v2/torrents/categories");
        let create = format!("{base}api/v2/torrents/createCategory");
        assert_eq!(c.get(&categories).send().await.unwrap().status(), 403);
        assert_eq!(c.post(&create).header("cookie", &cookie).header("content-type", "application/x-www-form-urlencoded").body("category=tv-sonarr").send().await.unwrap().status(), 200);
        let result = c.get(&categories).header("cookie", &cookie).send().await.unwrap();
        assert_eq!(result.status(), 200);
        let value: serde_json::Value = result.json().await.unwrap();
        assert_eq!(value["tv-sonarr"]["savePath"], "/downloads/tv");
        assert_eq!(c.get(&version).header("cookie", format!("{cookie}; {cookie}")).send().await.unwrap().status(), 403);
        assert_eq!(c.post(format!("{base}api/v2/auth/logout")).header("cookie", &cookie).send().await.unwrap().status(), 200);
        assert_eq!(c.get(&version).header("cookie", &cookie).send().await.unwrap().status(), 403);
    });
    disabled.stop();
    remote.stop();
}

#[test]
fn http_owner_session_and_origin_protection() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (remote, base, _, _dir) = start_mode(&rt, false);
    assert!(remote.fingerprint().is_none());
    assert!(remote.urls()[0].starts_with("http://"));
    rt.block_on(async {
        let c = client();
        let origin = base.trim_end_matches('/');
        let setup = c.post(format!("{base}api/v1/auth/setup"))
            .header("origin", origin)
            .json(&serde_json::json!({"code":remote.start_pairing(),"username":"owner","password":"correct horse battery staple"}))
            .send().await.unwrap();
        assert_eq!(setup.status(), 200);
        let set = setup.headers()["set-cookie"].to_str().unwrap();
        assert!(set.starts_with("mdm-session=") && set.contains("HttpOnly") && !set.contains("Secure"));
        let cookie = set.split(';').next().unwrap().to_owned();
        assert_eq!(c.get(format!("{base}api/v1/auth/settings")).header("cookie", &cookie).header("origin", origin).send().await.unwrap().status(), 200);
        assert_eq!(c.get(format!("{base}api/v1/auth/settings")).header("cookie", &cookie).header("origin", "http://evil.example").send().await.unwrap().status(), 403);
    });
    remote.stop();
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap()
}

#[test]
fn pair_then_control() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (remote, base, seen, _dir) = start(&rt);
    rt.block_on(async {
        let c = client();
        // The page, hardened.
        let page = c.get(&base).send().await.unwrap();
        assert_eq!(page.status(), 200);
        let csp = page.headers()["content-security-policy"].to_str().unwrap().to_owned();
        assert!(csp.contains("frame-ancestors 'none'") && csp.contains("script-src 'self'"));
        assert_eq!(page.headers()["x-frame-options"], "DENY");
        assert!(page.text().await.unwrap().contains("app.js"));
        // Desktop artwork is embedded, same-origin and explicitly allowlisted.
        for icon in ["cat-compressed", "cat-finished", "tb-start-queue", "tb-delete-completed"] {
            let image = c.get(format!("{base}desktop-icons/{icon}.png")).send().await.unwrap();
            assert_eq!(image.status(), 200);
            assert_eq!(image.headers()["content-type"], "image/png");
            assert!(image.bytes().await.unwrap().starts_with(b"\x89PNG"));
        }
        assert_eq!(c.get(format!("{base}desktop-icons/unknown.png")).send().await.unwrap().status(), 404);
        // Texts in the browser's language.
        let t: serde_json::Value = c
            .get(format!("{base}api/texts?lang=de-DE,de"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(t["lang"], "de");
        assert_eq!(t["texts"]["Pair"], "Koppeln");

        // Nothing without pairing.
        let r = c.get(format!("{base}api/list")).send().await.unwrap();
        assert_eq!(r.status(), 401);
        let r = c
            .get(format!("{base}api/list"))
            .bearer_auth("0123456789abcdef")
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 401, "a made-up token");

        // No code shown: refused.
        let pair = |code: &str| {
            c.post(format!("{base}api/pair"))
                .json(&serde_json::json!({ "code": code, "name": "Test phone", "scope": "control" }))
                .send()
        };
        assert_eq!(pair("12345678").await.unwrap().status(), 410);
        let code = remote.start_pairing();
        tokio::time::sleep(mdm_remote::pairing::PACE).await;
        let wrong = if code == "00000000" { "11111111" } else { "00000000" };
        let r = pair(wrong).await.unwrap();
        assert_eq!(r.status(), 403);
        let body: serde_json::Value = r.json().await.unwrap();
        assert_eq!(body["left"], 4);
        // Too soon after a wrong one, even with the right code.
        assert_eq!(pair(&code).await.unwrap().status(), 429);
        tokio::time::sleep(mdm_remote::pairing::PACE).await;
        let r = pair(&code).await.unwrap();
        assert_eq!(r.status(), 200);
        let body: serde_json::Value = r.json().await.unwrap();
        let token = body["token"].as_str().unwrap().to_owned();
        assert_eq!(remote.pairing().1.as_deref(), Some("Test phone"));
        assert_eq!(remote.devices().len(), 1);

        // Paired: commands reach the app, marked remote.
        let r = c
            .get(format!("{base}api/list"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        let body: serde_json::Value = r.json().await.unwrap();
        assert_eq!(body["seen"], "list");
        assert_eq!(body["remote"], true);
        let r = c
            .post(format!("{base}api/add"))
            .bearer_auth(&token)
            .json(&serde_json::json!({ "url": "https://example.com/a.zip", "queue": "Main download queue" }))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        {
            let log = seen.lock().unwrap();
            let add = log.last().unwrap();
            assert!(add.remote && add.silent && add.later);
            assert_eq!(add.queue.as_deref(), Some("Main download queue"));
        }
        // A torrent file: its bytes, never a path.
        let r = c
            .post(format!("{base}api/torrent?start=true"))
            .bearer_auth(&token)
            .body(b"d4:infod4:name1:xee".to_vec())
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        {
            let log = seen.lock().unwrap();
            let t = log.last().unwrap();
            assert!(t.torrent_file.is_some() && t.url.is_none() && !t.later);
        }
        let r = c
            .post(format!("{base}api/torrent"))
            .bearer_auth(&token)
            .body(b"<html>".to_vec())
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 400);
        // Only the list's actions.
        let r = c
            .post(format!("{base}api/act"))
            .bearer_auth(&token)
            .json(&serde_json::json!({ "action": "exit", "ids": [] }))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 400);

        // Another site's page, a rebinding name: refused before anything is looked at.
        let r = c
            .post(format!("{base}api/add"))
            .bearer_auth(&token)
            .header("origin", "https://evil.example")
            .json(&serde_json::json!({ "url": "https://example.com/b.zip" }))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 403);
        let r = c
            .get(format!("{base}api/list"))
            .bearer_auth(&token)
            .header("host", "evil.example")
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 421);
        let r = c
            .get(format!("{base}api/list"))
            .bearer_auth(&token)
            .header("host", "mdm.example")
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200, "a name the user added");
        let own = base.trim_end_matches('/').to_owned();
        let r = c
            .post(format!("{base}api/act"))
            .bearer_auth(&token)
            .header("origin", own)
            .json(&serde_json::json!({ "action": "pause", "ids": [1] }))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200, "its own page");

        // The page opened from a link on another site: shown (it holds nothing).
        let r = c
            .get(&base)
            .header("sec-fetch-site", "cross-site")
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        let r = c
            .get(format!("{base}api/list"))
            .bearer_auth(&token)
            .header("sec-fetch-site", "cross-site")
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 403, "but its API isn't for other sites");

        // Plain http: sent on to https.
        let plain = base.replacen("https", "http", 1);
        let r = c.get(&plain).send().await.unwrap();
        assert_eq!(r.status(), 301);
        assert_eq!(r.headers()["location"], base.as_str());

        // Removed in Options: refused at once.
        let id = remote.devices()[0].id.clone();
        remote.remove(&id);
        let r = c
            .get(format!("{base}api/list"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 401);
    });
    // Turned off: the port is let go.
    let port = base
        .rsplit(':')
        .next()
        .unwrap()
        .trim_end_matches('/')
        .parse::<u16>()
        .unwrap();
    remote.stop();
    assert_eq!(remote.status(), Status::Off);
    std::thread::sleep(Duration::from_millis(300));
    assert!(std::net::TcpListener::bind(("127.0.0.1", port)).is_ok());
}

#[test]
fn never_every_address() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (tx, _rx) = std::sync::mpsc::channel();
    let dir = tempfile::tempdir().unwrap();
    let texts: mdm_remote::Texts = Arc::new(|_: &str| mdm_remote::PageTexts::default());
    let remote = Remote::new(
        rt.handle().clone(),
        dir.path().to_owned(),
        mdm_ipc::Sink::new(tx, Arc::new(|| {})),
        texts,
    );
    for listen in ["0.0.0.0", "::", "203.0.113.9"] {
        remote.apply(Config {
            on: true,
            tls: false,
            qbittorrent: false,
            qbittorrent_subnets: "127.0.0.1/32,::1/128".into(),
            qbittorrent_auth_bypass: false,
            port: 0,
            listen: listen.into(),
            hosts: Vec::new(),
        });
        assert_eq!(remote.status(), Status::NoAddress, "{listen}");
    }
}

#[test]
fn owner_authentication_and_csrf_end_to_end() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (remote, base, _seen, dir) = start(&rt);
    rt.block_on(async {
        let c = client();
        let endpoint = |path: &str| format!("{base}api/v1/{path}");
        let r = c.get(endpoint("auth/session")).send().await.unwrap();
        let view: serde_json::Value = r.json().await.unwrap();
        assert_eq!(view["configured"], false);
        assert_eq!(c.get(endpoint("auth/settings")).send().await.unwrap().status(), 401);
        let code = remote.start_pairing();
        let r = c.post(endpoint("auth/setup")).json(&serde_json::json!({
            "code": code, "username": "owner", "password": "correct horse battery staple"
        })).send().await.unwrap();
        assert_eq!(r.status(), 200);
        let set_cookie = r.headers()["set-cookie"].to_str().unwrap().to_owned();
        assert!(set_cookie.contains("Secure") && set_cookie.contains("HttpOnly") && set_cookie.contains("SameSite=Strict"));
        let cookie = set_cookie.split(';').next().unwrap().to_owned();
        let body: serde_json::Value = r.json().await.unwrap();
        let csrf = body["csrf"].as_str().unwrap().to_owned();
        assert_eq!(c.post(endpoint("auth/setup")).json(&serde_json::json!({"username":"other","password":"another long password","code":code})).send().await.unwrap().status(), 409);
        let r = c.get(endpoint("auth/settings")).header("cookie", &cookie).send().await.unwrap();
        assert_eq!(r.status(), 200);
        let settings: serde_json::Value = r.json().await.unwrap();
        assert_eq!(settings["username"], "owner");
        assert!(settings.get("password_hash").is_none());
        assert_eq!(settings["policy"]["bypass_localhost"], false);
        assert_eq!(settings["policy"]["max_failures"], 5);
        let mut policy = settings["policy"].clone();
        policy["bypass_localhost"] = true.into();
        let update = serde_json::json!({ "current_password": "correct horse battery staple", "revision": settings["revision"], "policy": policy });
        assert_eq!(c.post(endpoint("auth/settings")).header("cookie", &cookie).json(&update).send().await.unwrap().status(), 403, "CSRF required even with an authenticated cookie");
        let r = c.post(endpoint("auth/settings")).header("cookie", &cookie).header("x-csrf-token", &csrf).json(&update).send().await.unwrap();
        assert_eq!(r.status(), 200);
        let settings: serde_json::Value = r.json().await.unwrap();
        // Bypass grants a workspace session, not authentication-policy administration.
        let r = c.get(endpoint("auth/session")).send().await.unwrap();
        let bypass_cookie = r.headers()["set-cookie"].to_str().unwrap().split(';').next().unwrap().to_owned();
        let bypass: serde_json::Value = r.json().await.unwrap();
        assert_eq!(bypass["authenticated"], true); assert_eq!(bypass["owner"], false);
        assert_eq!(c.get(endpoint("auth/settings")).header("cookie", &bypass_cookie).send().await.unwrap().status(), 403);
        let spoof: serde_json::Value = c.get(endpoint("auth/session")).header("x-forwarded-for", "127.0.0.1").send().await.unwrap().json().await.unwrap();
        assert_eq!(spoof["authenticated"], false, "forwarded traffic is not direct localhost");
        // A universal allowlist needs explicit confirmation, not just a valid password.
        let mut unsafe_policy = settings["policy"].clone(); unsafe_policy["subnets"] = serde_json::json!(["::/0"]);
        let r = c.post(endpoint("auth/settings")).header("cookie", &cookie).header("x-csrf-token", &csrf)
            .json(&serde_json::json!({ "revision":settings["revision"], "policy":unsafe_policy, "current_password":"correct horse battery staple" })).send().await.unwrap();
        assert_eq!(r.status(), 400);
        let r = c.get(endpoint("auth/settings")).header("cookie", &cookie).header("origin", "https://evil.example").send().await.unwrap();
        assert_eq!(r.status(), 403);
        let r = c.get(endpoint("auth/settings")).header("cookie", &cookie).header("origin", "chrome-extension://untrusted").send().await.unwrap();
        assert_eq!(r.status(), 403, "extension origins cannot use browser administration");
        // Credential changes revoke all browser sessions and never store the password itself.
        let r = c.post(endpoint("auth/credentials")).header("cookie", &cookie).header("x-csrf-token", &csrf)
            .json(&serde_json::json!({ "username":"new-owner", "password":"a new sufficiently long password", "current_password":"correct horse battery staple" })).send().await.unwrap();
        assert_eq!(r.status(), 200);
        assert_eq!(c.get(endpoint("auth/settings")).header("cookie", &cookie).send().await.unwrap().status(), 401);
        assert_eq!(c.post(endpoint("auth/login")).json(&serde_json::json!({ "username":"new-owner", "password":"a new sufficiently long password" })).send().await.unwrap().status(), 200);
        let saved = std::fs::read_to_string(dir.path().join("remote/web-auth.json")).unwrap();
        assert!(saved.contains("$argon2id$")); assert!(!saved.contains("sufficiently long password"));
    });
    remote.stop();
}

#[test]
fn new_device_tokens_default_to_capture_only() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let (remote, base, _, _dir) = start(&rt);
    rt.block_on(async {
        let c = client();
        let code = remote.start_pairing();
        let r = c
            .post(format!("{base}api/pair"))
            .json(&serde_json::json!({ "code": code, "name": "Extension" }))
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 200);
        let body: serde_json::Value = r.json().await.unwrap();
        assert_eq!(body["scope"], "capture");
        let token = body["token"].as_str().unwrap();
        assert_eq!(
            c.get(format!("{base}api/list"))
                .bearer_auth(token)
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
        assert_eq!(
            c.get(format!("{base}api/status"))
                .bearer_auth(token)
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
        assert_eq!(
            c.get(format!("{base}api/v1/auth/settings"))
                .bearer_auth(token)
                .send()
                .await
                .unwrap()
                .status(),
            401
        );
        assert_eq!(
            c.post(format!("{base}api/add"))
                .bearer_auth(token)
                .json(&serde_json::json!({ "url":"https://example.com/file.zip" }))
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
        assert_eq!(
            c.post(format!("{base}api/act"))
                .bearer_auth(token)
                .json(&serde_json::json!({ "action":"delete", "ids":[1] }))
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
    });
    remote.stop();
}
