// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The isolation prototype ADR 0122 §3 asked for (#5949, PE-39): an artifact rendered in a
//! sandboxed opaque-origin frame of the window's own document, or in a native webview of its own
//! whose label matches no capability. Each run renders one way and asks one of §3's five
//! questions, prints `RESULT <rendering> <test> PASS|FAIL <what it saw>`, and exits 0 on a pass.
//!
//! `artifact-isolation-probe <frame|webview> <reach|loop|speak|escape|walk>`
//!
//! It drives itself rather than being driven by WebDriver: the WebDriver plugin adds script
//! handlers of its own to every webview it can reach, which is the very surface test 1 asks
//! about, so the webview measured would not be the one shipped.

use std::collections::HashMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use percent_encoding::percent_decode_str;
use tauri::http::{Request, Response, StatusCode};
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};

/// What the probe saw, from both sides of every boundary.
#[derive(Default)]
struct Seen {
    /// How many times the stand-in for the vault-values command answered.
    reveals: usize,
    /// What reached each channel, by artifact id: `(channel, message)`.
    heard: Vec<(String, String)>,
    /// Every request that reached the canary scheme: something got off its folder.
    canary: Vec<String>,
    /// Every request the artifact scheme answered: `(webview label, artifact id, path, status)`.
    served: Vec<(String, String, String, u16)>,
    /// How many times the window's own script said it still runs.
    pongs: usize,
}

type Shared = Arc<Mutex<Seen>>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Rendering {
    Frame,
    Webview,
}

/// An artifact's own origin: one per artifact, so no two share storage. WebView2 reaches a
/// custom scheme only as `http://<scheme>.<host>` (wry's workaround), so Windows spells it so.
fn origin(id: &str) -> String {
    if cfg!(windows) {
        format!("http://artifact.{id}")
    } else {
        format!("artifact://{id}")
    }
}

/// Where a page is caught getting off its folder: any request here is a failure.
fn canary() -> &'static str {
    if cfg!(windows) {
        "http://canary.probe"
    } else {
        "canary://probe"
    }
}

/// The artifact id a request is for, from its host.
fn artifact_of(host: &str) -> &str {
    host.strip_prefix("artifact.").unwrap_or(host)
}

/// The artifact id a native webview renders, from its label `artifact-<id>-<n>`.
fn artifact_of_label(label: &str) -> Option<&str> {
    let rest = label.strip_prefix("artifact-")?;
    Some(&rest[..rest.rfind('-')?])
}

#[tauri::command]
fn vault_reveal(seen: State<'_, Shared>) -> String {
    seen.lock().expect("seen").reveals += 1;
    "the secret".into()
}

#[tauri::command]
fn heard(channel: String, message: String, seen: State<'_, Shared>) {
    seen.lock().expect("seen").heard.push((channel, message));
}

#[tauri::command]
fn pong(seen: State<'_, Shared>) {
    seen.lock().expect("seen").pongs += 1;
}

/// The fixture artifacts copied to a fresh folder, standing in for a workspace's `artifacts/`.
/// `%CANARY%` and `%ORIGIN:<id>%` in a page become this platform's spelling as they are copied.
fn lay_out_the_plane() -> PathBuf {
    let from = Path::new(env!("CARGO_MANIFEST_DIR")).join("artifacts");
    let plane = std::env::temp_dir().join(format!(
        "artifact-isolation-probe-{}-{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));
    copy_folder(&from, &plane);
    plane
}

fn copy_folder(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("a folder for the plane");
    for entry in std::fs::read_dir(from).expect("the fixture artifacts") {
        let entry = entry.expect("an entry");
        let path = entry.path();
        let target = to.join(entry.file_name());
        if path.is_dir() {
            copy_folder(&path, &target);
        } else if path.extension().is_some_and(|e| e == "html") {
            let mut text = std::fs::read_to_string(&path).expect("a page");
            text = text.replace("%CANARY%", canary());
            for id in [
                "reach", "loop", "speaker", "intruder", "escape", "walk", "outside",
            ] {
                text = text.replace(&format!("%ORIGIN:{id}%"), &origin(id));
            }
            std::fs::write(&target, text).expect("a page copied");
        } else {
            std::fs::copy(&path, &target).expect("a file copied");
        }
    }
}

/// A symlink planted after the pin, pointing out of `walk/` at a sibling's file.
fn plant_a_late_symlink(plane: &Path) {
    let link = plane.join("walk").join("late.png");
    let target = Path::new("..").join("outside").join("secret.png");
    #[cfg(unix)]
    let planted = std::os::unix::fs::symlink(&target, &link);
    #[cfg(windows)]
    let planted = std::os::windows::fs::symlink_file(&target, &link);
    if let Err(error) = planted {
        println!("NOTE the late symlink could not be planted: {error}");
    }
}

/// What was pinned: each artifact's files, read once at the pin, by their exact relative path.
/// purlis serves only these bytes, from memory: a file or symlink planted after the pin, a path
/// spelled with `..`, a backslash or another case, is simply not a key (ADR 0122 §3).
type Pinned = HashMap<String, HashMap<String, Vec<u8>>>;

fn pin(plane: &Path) -> Pinned {
    let mut pinned = Pinned::new();
    for entry in std::fs::read_dir(plane).expect("the plane") {
        let entry = entry.expect("an entry");
        if entry.file_type().expect("a type").is_dir() {
            let mut files = HashMap::new();
            read_pinned(&entry.path(), "", &mut files);
            pinned.insert(entry.file_name().to_string_lossy().into_owned(), files);
        }
    }
    pinned
}

/// Files only: a symlink in the folder at the pin is not followed and not served.
fn read_pinned(folder: &Path, prefix: &str, files: &mut HashMap<String, Vec<u8>>) {
    for entry in std::fs::read_dir(folder).expect("a pinned folder") {
        let entry = entry.expect("an entry");
        let kind = entry.file_type().expect("a type");
        let name = format!("{prefix}{}", entry.file_name().to_string_lossy());
        if kind.is_dir() {
            read_pinned(&entry.path(), &format!("{name}/"), files);
        } else if kind.is_file() {
            files.insert(name, std::fs::read(entry.path()).expect("a pinned file"));
        }
    }
}

/// The CSP on every response (ADR 0122 §3): its own origin only, no network, no frames, no
/// forms, no `<base>`.
/// [DEBUG-5948] Set by a `shape-*-app` run: serve the app's CSP (`purlis_core::artifact::serve::csp`, script approved).
static APP_CSP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn csp(id: &str) -> String {
    let own = origin(id);
    if APP_CSP.load(std::sync::atomic::Ordering::Relaxed) {
        return format!(
            "default-src 'none'; script-src {own} 'unsafe-inline'; style-src {own} 'unsafe-inline'; \
             img-src {own}; font-src {own}; media-src {own}; connect-src 'none'; \
             form-action 'none'; base-uri 'none'; frame-src 'none'; child-src 'none'; \
             worker-src 'none'; object-src 'none'; manifest-src 'none'; frame-ancestors 'none'"
        );
    }
    format!(
        "default-src {own}; script-src {own} 'unsafe-inline'; style-src {own} 'unsafe-inline'; \
         img-src {own}; font-src {own}; media-src {own}; connect-src 'none'; form-action 'none'; \
         base-uri 'none'; frame-src 'none'; child-src 'none'; worker-src 'none'; \
         object-src 'none'; manifest-src 'none'"
    )
}

/// The key a request names in the pinned store, or nothing: one decode, and no segment that is
/// empty, `.` or `..`, and no backslash or NUL anywhere.
fn pinned_key(raw: &str) -> Option<String> {
    let decoded = percent_decode_str(raw.strip_prefix('/')?)
        .decode_utf8()
        .ok()?;
    let key = if decoded.is_empty() {
        "index.html"
    } else {
        &decoded
    };
    let refused = key.contains(['\\', '\0'])
        || key
            .split('/')
            .any(|segment| matches!(segment, "" | "." | ".."));
    (!refused).then(|| key.to_owned())
}

/// The artifact scheme. Guarded, it serves the pinned bytes under the CSP; unguarded (the
/// control that shows each test can go red), whatever the path names on disk, with no CSP.
fn serve_artifact(
    label: &str,
    request: &Request<Vec<u8>>,
    guard: &Guard,
    seen: &Shared,
) -> Response<Vec<u8>> {
    let uri = request.uri();
    let id = artifact_of(uri.host().unwrap_or("")).to_owned();
    let raw = uri.path().to_owned();
    if raw == "/.purlis/say" {
        return say(label, &id, uri.query().unwrap_or(""), seen);
    }
    let found = match guard {
        Guard::On(pinned) => pinned_key(&raw).and_then(|key| pinned.get(&id)?.get(&key).cloned()),
        Guard::Off(plane) => {
            let decoded = percent_decode_str(raw.trim_start_matches('/')).decode_utf8_lossy();
            std::fs::read(plane.join(&id).join(decoded.as_ref())).ok()
        }
    };
    let status = if found.is_some() {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    };
    seen.lock().expect("seen").served.push((
        label.to_owned(),
        id.clone(),
        raw.clone(),
        status.as_u16(),
    ));
    let mut response = Response::builder()
        .status(status)
        .header("Content-Type", content_type(&raw))
        .header("X-Content-Type-Options", "nosniff")
        .header("Cache-Control", "no-store");
    if matches!(guard, Guard::On(_)) {
        response = response.header("Content-Security-Policy", csp(&id));
    }
    response
        .body(found.unwrap_or_default())
        .expect("a response")
}

/// The guards, on, or off for the control run.
enum Guard {
    On(Pinned),
    Off(PathBuf),
}

/// A native webview's channel: a request to its own origin's `/.purlis/say`, attributed by the
/// webview's label, which the page cannot choose. A request from any other webview, or for
/// another artifact's origin, speaks on no channel.
fn say(label: &str, id: &str, query: &str, seen: &Shared) -> Response<Vec<u8>> {
    let mut seen = seen.lock().expect("seen");
    seen.served
        .push((label.to_owned(), id.to_owned(), "/.purlis/say".into(), 204));
    if artifact_of_label(label) == Some(id)
        && let Some(message) = query.strip_prefix("m=")
    {
        let message = percent_decode_str(message).decode_utf8_lossy().into_owned();
        seen.heard.push((id.to_owned(), message));
    }
    Response::builder()
        .status(StatusCode::NO_CONTENT)
        .body(Vec::new())
        .expect("a response")
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("png") => "image/png",
        Some("css") => "text/css",
        _ => "application/octet-stream",
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rendering = match args.first().map(String::as_str) {
        Some("frame") => Rendering::Frame,
        Some("webview") => Rendering::Webview,
        _ => usage(),
    };
    let test = args.get(1).cloned().unwrap_or_else(|| usage());
    if !TESTS.contains(&test.as_str()) && !test.starts_with("shape-") {
        usage();
    }
    // The control: every guard off, so a test that passes guarded is seen to fail for its reason.
    let guarded = match args.get(2).map(String::as_str) {
        None => true,
        Some("unguarded") if rendering == Rendering::Webview => false,
        _ if test.starts_with("shape-") => true,
        Some(_) => usage(),
    };

    // A probe whose window never comes up fails, rather than holding its run open.
    let named = format!("{rendering:?}").to_lowercase();
    let asked = test.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(90));
        println!("RESULT {named} {asked} FAIL no answer within 90 s: the window never ran");
        let _ = std::io::stdout().flush();
        std::process::exit(1);
    });

    let plane = lay_out_the_plane();
    let guard = if guarded {
        Guard::On(pin(&plane))
    } else {
        Guard::Off(plane.clone())
    };
    plant_a_late_symlink(&plane);
    let seen: Shared = Arc::default();

    let serving = (guard, seen.clone());
    let catching = seen.clone();
    let driving = (seen.clone(), plane);
    tauri::Builder::default()
        .manage(seen)
        .register_uri_scheme_protocol("artifact", move |ctx, request| {
            serve_artifact(ctx.webview_label(), &request, &serving.0, &serving.1)
        })
        .register_uri_scheme_protocol("canary", move |_ctx, request| {
            catching
                .lock()
                .expect("seen")
                .canary
                .push(request.uri().to_string());
            Response::builder()
                .status(StatusCode::OK)
                .body(Vec::new())
                .expect("a response")
        })
        .invoke_handler(tauri::generate_handler![vault_reveal, heard, pong])
        .setup(move |app| {
            if !guarded {
                // Unguarded, an artifact's webview is granted what the window is.
                app.add_capability(
                    tauri::ipc::CapabilityBuilder::new("unguarded")
                        .window("artifact-*")
                        .permission("allow-vault-reveal"),
                )?;
            }
            let app = app.handle().clone();
            std::thread::spawn(move || {
                let (seen, plane) = driving;
                let verdict = drive(&app, rendering, guarded, &test, &seen);
                let (passed, detail) = match &verdict {
                    Ok(detail) => ("PASS", detail.clone()),
                    Err(detail) => ("FAIL", detail.clone()),
                };
                let rendering = format!("{rendering:?}").to_lowercase();
                let control = if guarded { "" } else { " unguarded" };
                println!("RESULT {rendering}{control} {test} {passed} {detail}");
                let _ = std::io::stdout().flush();
                let _ = std::fs::remove_dir_all(&plane);
                std::process::exit(i32::from(verdict.is_err()));
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("the probe runs");
}

const TESTS: [&str; 5] = ["reach", "loop", "speak", "escape", "walk"];

fn usage() -> ! {
    eprintln!(
        "usage: artifact-isolation-probe <frame|webview> <{}> [unguarded, webview only]",
        TESTS.join("|")
    );
    std::process::exit(2);
}

/// Opens artifact pages one way or the other.
struct Opener<'a> {
    app: &'a AppHandle,
    rendering: Rendering,
    guarded: bool,
    opened: usize,
}

impl Opener<'_> {
    fn open(&mut self, id: &str, page: &str) -> Result<(), String> {
        let url = format!("{}/{page}", origin(id));
        self.opened += 1;
        match self.rendering {
            Rendering::Frame => {
                let script = format!(
                    "probe.frame({}, {})",
                    serde_json::to_string(id).expect("json"),
                    serde_json::to_string(&url).expect("json")
                );
                self.app
                    .get_webview_window("main")
                    .ok_or("no window")?
                    .eval(script)
                    .map_err(|e| e.to_string())
            }
            Rendering::Webview => {
                let label = format!("artifact-{id}-{}", self.opened);
                let parsed = url.parse().map_err(|e| format!("{url}: {e}"))?;
                // A page navigates only within its own folder; anything else is refused here,
                // before any request is made.
                let own = format!("{}/", origin(id));
                let guarded = self.guarded;
                WebviewWindowBuilder::new(self.app, label, WebviewUrl::CustomProtocol(parsed))
                    .title(format!("artifact {id}"))
                    .inner_size(400.0, 200.0)
                    .on_navigation(move |to| !guarded || to.as_str().starts_with(&own))
                    .build()
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
        }
    }
}

/// Waits until `done` says so, at most `within`.
fn until(seen: &Shared, within: Duration, done: impl Fn(&Seen) -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < within {
        if done(&seen.lock().expect("seen")) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

fn heard_on(seen: &Seen, channel: &str) -> Vec<String> {
    seen.heard
        .iter()
        .filter(|(c, _)| c == channel)
        .map(|(_, m)| m.clone())
        .collect()
}

/// A page's report on its channel, as JSON, once it has made one.
fn report(seen: &Shared, channel: &str, within: Duration) -> Result<serde_json::Value, String> {
    if !until(seen, within, |s| !heard_on(s, channel).is_empty()) {
        return Err(format!("the {channel} page never reported on its channel"));
    }
    let said = heard_on(&seen.lock().expect("seen"), channel).remove(0);
    serde_json::from_str(&said).map_err(|e| format!("{said}: {e}"))
}

fn drive(
    app: &AppHandle,
    rendering: Rendering,
    guarded: bool,
    test: &str,
    seen: &Shared,
) -> Result<String, String> {
    // The window's own script says it runs once it has loaded.
    if !until(seen, Duration::from_secs(20), |s| s.pongs >= 1) {
        return Err("the window never loaded".into());
    }
    let mut opener = Opener {
        app,
        rendering,
        guarded,
        opened: 0,
    };
    match test {
        // 1. The page reaches no Tauri command, the vault-values stand-in among them.
        "reach" => {
            opener.open("reach", "index.html")?;
            let tried = report(seen, "reach", Duration::from_secs(15))?;
            let reveals = seen.lock().expect("seen").reveals;
            if reveals == 0 {
                Ok(format!("no command answered; the page tried {tried}"))
            } else {
                Err(format!(
                    "vault_reveal answered {reveals} time(s); the page tried {tried}"
                ))
            }
        }
        // 2. An endless loop in the page leaves the window's own script running.
        "loop" => {
            opener.open("loop", "index.html")?;
            if !until(seen, Duration::from_secs(10), |s| {
                s.served.iter().any(|(_, id, _, _)| id == "loop")
            }) {
                return Err("the loop page was never served".into());
            }
            // Its loop starts 200 ms after it loads.
            std::thread::sleep(Duration::from_millis(1500));
            let before = seen.lock().expect("seen").pongs;
            app.get_webview_window("main")
                .ok_or("no window")?
                .eval("probe.ping()")
                .map_err(|e| e.to_string())?;
            if until(seen, Duration::from_secs(5), |s| s.pongs > before) {
                Ok("the window's script answered while the page looped".into())
            } else {
                Err("the window's script did not answer within 5 s of the page's loop".into())
            }
        }
        // 3. Neither a second artifact nor a nested frame speaks on the page's channel.
        "speak" => {
            opener.open("speaker", "index.html")?;
            opener.open("intruder", "index.html")?;
            let ran = until(seen, Duration::from_secs(15), |s| {
                !heard_on(s, "speaker").is_empty() && !heard_on(s, "intruder").is_empty()
            });
            std::thread::sleep(Duration::from_secs(2));
            let speaker = heard_on(&seen.lock().expect("seen"), "speaker");
            if !ran {
                Err(format!(
                    "both pages did not speak on their own channels: speaker heard {speaker:?}"
                ))
            } else if speaker == ["\"honest\""] {
                Ok(format!(
                    "the speaker's channel heard only the speaker: {speaker:?}"
                ))
            } else {
                Err(format!("the speaker's channel heard {speaker:?}"))
            }
        }
        // 4. A script-free page loads and navigates to nothing off its folder.
        "escape" => {
            for page in ESCAPES {
                opener.open("escape", page)?;
            }
            let file = std::env::current_dir()
                .map_err(|e| e.to_string())?
                .join(env!("CARGO_MANIFEST_DIR"))
                .join("artifacts")
                .join("outside")
                .join("secret.png");
            let file_url = tauri::Url::from_file_path(&file).map_err(|()| "a file URL")?;
            opener.open("escape", &format!("file.html#{file_url}"))?;
            let file_said = report(seen, "escape", Duration::from_secs(15))?;
            std::thread::sleep(Duration::from_secs(4));
            let seen = seen.lock().expect("seen");
            let off_folder: Vec<_> = seen
                .served
                .iter()
                .filter(|(_, id, _, _)| id != "escape")
                .map(|(_, id, path, status)| format!("{id}{path} {status}"))
                .collect();
            let served: Vec<_> = seen
                .served
                .iter()
                .map(|(_, _, p, s)| format!("{p} {s}"))
                .collect();
            if seen.canary.is_empty() && off_folder.is_empty() && file_said == "blocked" {
                Ok(format!(
                    "nothing off the folder was asked for; served {served:?}"
                ))
            } else {
                Err(format!(
                    "off the folder: canary {:?}, other artifacts {off_folder:?}, file: {file_said}",
                    seen.canary
                ))
            }
        }
        // 5. No request walks out of the folder.
        "walk" => {
            opener.open("walk", "index.html")?;
            let said = report(seen, "walk", Duration::from_secs(15))?;
            let loaded = said.get("loaded").cloned().unwrap_or_default();
            if loaded == serde_json::json!(["inside.png"]) {
                Ok(format!("only its own file loaded; {said}"))
            } else {
                Err(format!("loaded {loaded}; {said}"))
            }
        }
        shape => shape_of(app, shape, seen),
    }
}

/// **[DEBUG-5948] Which shape of webview runs a page's script and its init script** on this
/// platform: `shape-<toplevel|child>-<plain|incognito>[-app]`, `-app` adding the app's other
/// builder flags. Diagnostic only: it reports what reached the scheme, never fails.
fn shape_of(app: &AppHandle, shape: &str, seen: &Shared) -> Result<String, String> {
    let child = shape.contains("-child");
    let incognito = shape.contains("-incognito");
    let flags = shape.ends_with("-app");
    APP_CSP.store(flags, std::sync::atomic::Ordering::Relaxed);
    let url: tauri::Url = format!("{}/index.html", origin("walk")).parse().map_err(|e| format!("{e}"))?;
    let label = "artifact-walk-1";
    let init = r#"(() => { new Image().src = "/.purlis/say?m=" + encodeURIComponent(JSON.stringify("init:" + document.readyState)); })();"#;
    if child {
        let mut builder = tauri::webview::WebviewBuilder::new(label, WebviewUrl::CustomProtocol(url))
            .initialization_script(init)
            .incognito(incognito);
        if flags {
            builder = builder
                .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
                .focused(false)
                .devtools(false)
                .disable_drag_drop_handler();
        }
        let window = app.get_window("main").ok_or("no window")?;
        window
            .add_child(
                builder,
                tauri::LogicalPosition::new(0.0, 0.0),
                tauri::LogicalSize::new(400.0, 300.0),
            )
            .map_err(|e| e.to_string())?;
    } else {
        WebviewWindowBuilder::new(app, label, WebviewUrl::CustomProtocol(url))
            .inner_size(400.0, 200.0)
            .initialization_script(init)
            .incognito(incognito)
            .build()
            .map_err(|e| e.to_string())?;
    }
    until(seen, Duration::from_secs(15), |s| heard_on(s, "walk").len() >= 2);
    std::thread::sleep(Duration::from_secs(1));
    let seen = seen.lock().expect("seen");
    let served: Vec<_> = seen.served.iter().map(|(l, id, p, s)| format!("{l} {id}{p} {s}")).collect();
    Ok(format!("heard {:?}; served {served:?}", heard_on(&seen, "walk")))
}

/// The script-free pages of test 4, each getting off its folder one way.
const ESCAPES: [&str; 9] = [
    "meta.html",
    "meta-sibling.html",
    "base.html",
    "form.html",
    "css-import.html",
    "css-url.html",
    "svg.html",
    "prefetch.html",
    "sibling.html",
];

#[cfg(test)]
mod tests {
    use super::pinned_key;

    #[test]
    fn a_path_names_a_pinned_key_only_when_it_stays_in_its_folder() {
        assert_eq!(pinned_key("/").as_deref(), Some("index.html"));
        assert_eq!(pinned_key("/inside.png").as_deref(), Some("inside.png"));
        assert_eq!(
            pinned_key("/sub/page.html").as_deref(),
            Some("sub/page.html")
        );
        for walks in [
            "/../outside/secret.png",
            "/..%2foutside%2fsecret.png",
            "/%2e%2e/outside/secret.png",
            "/%2E%2E%2Foutside",
            "/..%5coutside%5csecret.png",
            "/..\\outside\\secret.png",
            "/sub/..%2f..%2foutside",
            "/./inside.png",
            "//inside.png",
            "/inside.png%00",
            "/%ff",
        ] {
            assert_eq!(pinned_key(walks), None, "{walks}");
        }
    }
}
