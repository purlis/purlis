// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What an artifact's webview is served** (ADR 0120 §3, #5952): the folder's bytes read once,
//! when its tab is pressed, and nothing else.
//!
//! - **Only the bytes read at the press are served**, from memory ([`Snapshot`]). A file or a
//!   link planted in the folder afterwards is not a key, and a link there at the press is not
//!   read at all: on unix each folder and file is opened through the one before it, never
//!   through a link (`O_NOFOLLOW`), so a link swapped in while it is read is refused too.
//! - **A request names a key or nothing** ([`key`]): one decode, and no segment that is empty,
//!   `.` or `..`, no backslash and no NUL. What walks out of the folder is not in the map.
//! - **Every response carries the CSP** ([`csp`]): the artifact's own origin and nothing else,
//!   no network, no frames, no forms, no `<base>`, and no script — unless the person approved
//!   this very template on this machine ([`super::approval`], #5959), when its own script runs
//!   and still reaches no network.
//!
//! This module renders nothing and knows no webview: the app serves what it says.

use std::collections::BTreeMap;
use std::path::Path;

use percent_encoding::percent_decode_str;
use sha2::{Digest, Sha256};

use super::{DATA_DIR, DATA_FILE, PAGE};

/// The URL scheme an artifact is served on (`artifact://<origin>/<path>`).
pub const SCHEME: &str = "artifact";

/// The path under an artifact's origin that is purlis's, never a file's: what the webview tells
/// purlis (a key that hands the keyboard back), attributed by the webview's label.
pub const OWN_PATH: &str = "/.purlis/";

/// **purlis's theme, as a stylesheet the page may link** (#5955): served from the page's own
/// origin at this path, which the CSP's `style-src` already admits, so handing the theme over
/// loosens nothing. purlis's own script in the page links it first, so the page's own rules win.
pub const THEME_SHEET: &str = "/.purlis/theme.css";

/// What a custom property of the theme sheet starts with: `surface.base` is
/// `--purlis-surface-base`, apart from any name a page uses for its own.
pub const THEME_PREFIX: &str = "--purlis-";

/// **The theme sheet for a window drawn in `appearance` with `values`** (ADR 0120, #5955):
/// `color-scheme` set to purlis's appearance, never the operating system's, and each of
/// purlis's colour tokens as a custom property. Only a token the vocabulary names, with a value
/// that is a colour, is written, so nothing the sheet is handed can end its rule or reach out.
pub fn theme_sheet(appearance: &str, values: &BTreeMap<String, String>) -> Result<String, String> {
    use crate::theme::vocabulary::{TOKENS, is_colour};
    if !matches!(appearance, "dark" | "light") {
        return Err(format!(
            "{:?} is not an appearance: dark or light",
            crate::personas::one_line(appearance)
        ));
    }
    let mut sheet = format!(":root {{\n  color-scheme: {appearance};\n");
    for token in TOKENS {
        if let Some(value) = values.get(*token).filter(|value| is_colour(value)) {
            let name = token.replace('.', "-");
            sheet.push_str(&format!("  {THEME_PREFIX}{name}: {value};\n"));
        }
    }
    sheet.push_str("}\n");
    Ok(sheet)
}

/// An artifact folder's files, read once: by their path inside the folder, `/`-separated.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    files: BTreeMap<String, Vec<u8>>,
    fingerprint: String,
    script: bool,
}

impl Snapshot {
    /// Reads every file of `folder` (a folder [`super::resolve`] answered). A link is never
    /// followed and never served, nor is a name starting with a dot. Its page must be among
    /// what was read.
    pub fn read(folder: &Path) -> Result<Self, String> {
        let mut files = BTreeMap::new();
        read_folder(folder, &mut files).map_err(|e| {
            format!(
                "the artifact's folder could not be read ({})",
                crate::rewrite::os_words(&e)
            )
        })?;
        if !files.contains_key(PAGE) {
            return Err(format!("the artifact has no {PAGE}, the page it opens on"));
        }
        let fingerprint = fingerprint(&files);
        let script = holds_script(&files);
        Ok(Self {
            files,
            fingerprint,
            script,
        })
    }

    /// The bytes read for `key`, a path inside the folder as [`key`] gives it.
    pub fn get(&self, key: &str) -> Option<&[u8]> {
        self.files.get(key).map(Vec::as_slice)
    }

    /// The template's fingerprint: every path and byte but the data files' ([`is_data`]), so
    /// a page regenerated with new data keeps it (ADR 0120 §4).
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    /// Whether any of the template's pages holds script a browser runs ([`super::script`]):
    /// what makes it a page each person approves before it runs.
    pub fn holds_script(&self) -> bool {
        self.script
    }

    /// The paths read, in order.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(String::as_str)
    }
}

/// Whether `key` is one of the folder's data files: `data.json`, or under `data/`.
pub fn is_data(key: &str) -> bool {
    key == DATA_FILE || key.starts_with(&format!("{DATA_DIR}/"))
}

/// Whether a page of the template, never a data file, holds script.
fn holds_script(files: &BTreeMap<String, Vec<u8>>) -> bool {
    files.iter().any(|(path, bytes)| {
        !is_data(path)
            && super::script::is_page(path)
            && super::script::holds_script(&String::from_utf8_lossy(bytes))
    })
}

fn fingerprint(files: &BTreeMap<String, Vec<u8>>) -> String {
    let mut hash = Sha256::new();
    for (path, bytes) in files.iter().filter(|(path, _)| !is_data(path)) {
        hash.update(path.as_bytes());
        hash.update([0]);
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    hash.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// The key a request's path names in a [`Snapshot`], or nothing: one percent-decode, `/` is
/// the page, and no segment that is empty, `.` or `..`, nor a backslash or NUL anywhere.
pub fn key(path: &str) -> Option<String> {
    let decoded = percent_decode_str(path.strip_prefix('/')?)
        .decode_utf8()
        .ok()?;
    let key = if decoded.is_empty() { PAGE } else { &decoded };
    let refused = key.contains(['\\', '\0'])
        || key
            .split('/')
            .any(|segment| matches!(segment, "" | "." | ".."));
    (!refused).then(|| key.to_owned())
}

/// **An artifact's origin host**: one per artifact, so no two share storage, and the same for
/// the same folder of the same workspace on every press, so its own storage stays its own.
/// Lowercase hex, which every platform's URL parser keeps as it is.
pub fn origin_host(workspace: &Path, folder: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(workspace.to_string_lossy().as_bytes());
    hash.update([0]);
    hash.update(folder.as_bytes());
    hash.finalize()
        .iter()
        .take(16)
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// **The CSP on every response** (ADR 0120 §3), for a page served at `own` (its origin, as the
/// webview spells it): its own origin only. No connection, no frame, no form, no `<base>`, no
/// worker, no plugin, ever. Script only when `runs` — the person approved this template here
/// (ADR 0120 §4) — and then only the template's own: from its origin and inline in the bytes
/// approved, never `eval`.
pub fn csp(own: &str, runs: bool) -> String {
    let script = if runs {
        format!("{own} 'unsafe-inline'")
    } else {
        "'none'".to_owned()
    };
    format!(
        "default-src 'none'; script-src {script}; style-src {own} 'unsafe-inline'; \
         img-src {own}; font-src {own}; media-src {own}; connect-src 'none'; \
         form-action 'none'; base-uri 'none'; frame-src 'none'; child-src 'none'; \
         worker-src 'none'; object-src 'none'; manifest-src 'none'; frame-ancestors 'none'"
    )
}

/// The media type a key is served as; anything not named is bytes, never sniffed.
pub fn content_type(key: &str) -> &'static str {
    let extension = key.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    match extension.as_deref() {
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json",
        Some("txt") => "text/plain; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        _ => "application/octet-stream",
    }
}

/// How deep a folder [`Snapshot::read`] goes: deeper is not read.
const DEEPEST: usize = 16;

#[cfg(unix)]
fn read_folder(folder: &Path, files: &mut BTreeMap<String, Vec<u8>>) -> std::io::Result<()> {
    use crate::skills::at;
    use std::io::Read as _;

    fn walk(
        dir: &std::os::fd::OwnedFd,
        prefix: &str,
        depth: usize,
        files: &mut BTreeMap<String, Vec<u8>>,
    ) -> std::io::Result<()> {
        for entry in at::entries(dir)? {
            if entry.name.starts_with('.') {
                continue;
            }
            let path = format!("{prefix}{}", entry.name);
            match entry.kind {
                rustix::fs::FileType::Directory if depth < DEEPEST => {
                    let inner = at::folder_in(dir, &entry.name)?;
                    walk(&inner, &format!("{path}/"), depth + 1, files)?;
                }
                rustix::fs::FileType::RegularFile => {
                    let flags = rustix::fs::OFlags::RDONLY
                        | rustix::fs::OFlags::NOFOLLOW
                        | rustix::fs::OFlags::CLOEXEC;
                    let fd = rustix::fs::openat(
                        dir,
                        entry.name.as_str(),
                        flags,
                        rustix::fs::Mode::empty(),
                    )?;
                    let mut bytes = Vec::new();
                    std::fs::File::from(fd).read_to_end(&mut bytes)?;
                    files.insert(path, bytes);
                }
                // A link, a socket, a device: never read.
                _ => {}
            }
        }
        Ok(())
    }

    walk(&at::folder(folder)?, "", 0, files)
}

#[cfg(not(unix))]
fn read_folder(folder: &Path, files: &mut BTreeMap<String, Vec<u8>>) -> std::io::Result<()> {
    fn walk(
        dir: &Path,
        prefix: &str,
        depth: usize,
        files: &mut BTreeMap<String, Vec<u8>>,
    ) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if name.starts_with('.') {
                continue;
            }
            let path = format!("{prefix}{name}");
            // `file_type` of a directory entry is the entry's own: a link is a link.
            let kind = entry.file_type()?;
            if kind.is_dir() && depth < DEEPEST {
                walk(&entry.path(), &format!("{path}/"), depth + 1, files)?;
            } else if kind.is_file() {
                files.insert(path, std::fs::read(entry.path())?);
            }
        }
        Ok(())
    }

    walk(folder, "", 0, files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_names_a_key_only_when_it_stays_in_its_folder() {
        assert_eq!(key("/").as_deref(), Some("index.html"));
        assert_eq!(key("/inside.png").as_deref(), Some("inside.png"));
        assert_eq!(key("/sub/page.html").as_deref(), Some("sub/page.html"));
        assert_eq!(key("/a%20b.css").as_deref(), Some("a b.css"));
        for walks in [
            "",
            "inside.png",
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
            assert_eq!(key(walks), None, "{walks}");
        }
    }

    #[test]
    fn the_csp_admits_the_own_origin_only_and_no_script_connection_frame_form_or_base() {
        let csp = csp("artifact://abc", false);
        for directive in [
            "default-src 'none'",
            "script-src 'none'",
            "connect-src 'none'",
            "form-action 'none'",
            "base-uri 'none'",
            "frame-src 'none'",
        ] {
            assert!(csp.contains(directive), "{directive} in {csp}");
        }
        assert!(csp.contains("img-src artifact://abc;"));
        assert!(!csp.contains('*'), "{csp}");
    }

    #[test]
    fn an_approved_page_runs_its_own_script_and_still_reaches_no_network_frame_or_eval() {
        let csp = csp("artifact://abc", true);
        assert!(
            csp.contains("script-src artifact://abc 'unsafe-inline';"),
            "{csp}"
        );
        for directive in [
            "default-src 'none'",
            "connect-src 'none'",
            "form-action 'none'",
            "base-uri 'none'",
            "frame-src 'none'",
            "worker-src 'none'",
        ] {
            assert!(csp.contains(directive), "{directive} in {csp}");
        }
        assert!(!csp.contains("unsafe-eval"), "{csp}");
        assert!(!csp.contains('*'), "{csp}");
    }

    #[test]
    fn a_snapshot_holds_script_when_a_page_of_its_template_does_and_never_for_its_data() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path();
        std::fs::create_dir(folder.join("data")).unwrap();
        std::fs::write(
            folder.join("index.html"),
            r#"<h1>Board</h1><script type="application/json">{"a":1}</script>"#,
        )
        .unwrap();
        // Data is inert JSON; even markup there is never a page of the template.
        std::fs::write(folder.join("data/page.html"), "<script>x()</script>").unwrap();
        std::fs::write(folder.join("app.js"), "x()").unwrap();
        assert!(!Snapshot::read(folder).unwrap().holds_script());

        std::fs::write(folder.join("chart.svg"), "<svg onload='x()'/>").unwrap();
        assert!(Snapshot::read(folder).unwrap().holds_script());
    }

    #[test]
    fn each_folder_of_each_workspace_has_its_own_origin() {
        let one = origin_host(Path::new("/p/workspaces/a"), "state");
        assert_eq!(one, origin_host(Path::new("/p/workspaces/a"), "state"));
        assert_ne!(one, origin_host(Path::new("/p/workspaces/b"), "state"));
        assert_ne!(one, origin_host(Path::new("/p/workspaces/a"), "other"));
        assert_eq!(one.len(), 32);
        assert!(
            one.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
    }

    #[test]
    fn a_snapshot_holds_the_files_read_and_no_link_and_nothing_planted_after() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("page");
        std::fs::create_dir_all(folder.join("img")).unwrap();
        std::fs::write(folder.join("index.html"), "<h1>hi</h1>").unwrap();
        std::fs::write(folder.join("img/inside.png"), "png").unwrap();
        std::fs::write(folder.join(".hidden"), "no").unwrap();
        std::fs::write(dir.path().join("secret.txt"), "secret").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(dir.path().join("secret.txt"), folder.join("link.txt")).unwrap();

        let read = Snapshot::read(&folder).unwrap();
        assert_eq!(
            read.paths().collect::<Vec<_>>(),
            ["img/inside.png", "index.html"]
        );
        assert_eq!(read.get("index.html"), Some(&b"<h1>hi</h1>"[..]));

        std::fs::write(folder.join("late.html"), "late").unwrap();
        assert_eq!(read.get("late.html"), None);
    }

    #[test]
    fn a_folder_with_no_page_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("other.html"), "x").unwrap();
        let why = Snapshot::read(dir.path()).unwrap_err();
        assert!(why.contains("index.html"), "{why}");
    }

    #[test]
    fn the_fingerprint_follows_the_template_and_not_its_data() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path();
        std::fs::create_dir(folder.join("data")).unwrap();
        std::fs::write(folder.join("index.html"), "page").unwrap();
        std::fs::write(folder.join("data.json"), "1").unwrap();
        let first = Snapshot::read(folder).unwrap().fingerprint().to_owned();

        std::fs::write(folder.join("data.json"), "2").unwrap();
        std::fs::write(folder.join("data/more.json"), "3").unwrap();
        assert_eq!(Snapshot::read(folder).unwrap().fingerprint(), first);

        std::fs::write(folder.join("index.html"), "page!").unwrap();
        assert_ne!(Snapshot::read(folder).unwrap().fingerprint(), first);
    }

    fn drawn(appearance: &str, values: &[(&str, &str)]) -> Result<String, String> {
        let values: BTreeMap<String, String> = values
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        theme_sheet(appearance, &values)
    }

    #[test]
    fn the_theme_reaches_the_page_as_purlis_s_tokens_and_its_colour_scheme() {
        let sheet = drawn(
            "dark",
            &[
                ("surface.base", "#101112"),
                ("accent.wash-hover", "#2233aa80"),
            ],
        )
        .unwrap();
        assert!(sheet.starts_with(":root {"), "{sheet}");
        assert!(sheet.contains("color-scheme: dark;"), "{sheet}");
        assert!(sheet.contains("--purlis-surface-base: #101112;"), "{sheet}");
        assert!(
            sheet.contains("--purlis-accent-wash-hover: #2233aa80;"),
            "{sheet}"
        );
        assert!(
            drawn("light", &[])
                .unwrap()
                .contains("color-scheme: light;"),
            "light follows purlis too"
        );
    }

    #[test]
    fn a_theme_sheet_writes_only_known_tokens_with_colour_values() {
        let sheet = drawn(
            "light",
            &[
                (
                    "surface.base",
                    "red; } body { background: url(https://x.example/)",
                ),
                ("not.a.token", "#ffffff"),
                ("text.primary", "#000"),
            ],
        )
        .unwrap();
        assert!(!sheet.contains("url("), "{sheet}");
        assert!(!sheet.contains("not-a-token"), "{sheet}");
        assert!(!sheet.contains("--purlis-surface-base"), "{sheet}");
        assert!(sheet.contains("--purlis-text-primary: #000;"), "{sheet}");
        assert_eq!(sheet.matches('}').count(), 1, "{sheet}");
    }

    #[test]
    fn an_appearance_is_dark_or_light_and_nothing_else() {
        for not in ["", "auto", "dark; color: red", "system"] {
            assert!(drawn(not, &[]).is_err(), "{not}");
        }
    }

    #[test]
    fn the_theme_sheet_is_served_under_purlis_s_own_path_which_names_no_file() {
        assert!(THEME_SHEET.starts_with(OWN_PATH));
        assert_eq!(key(THEME_SHEET).as_deref(), Some(".purlis/theme.css"));
        assert_eq!(content_type(THEME_SHEET), "text/css; charset=utf-8");
    }

    #[test]
    fn a_type_is_named_and_never_left_to_sniffing() {
        assert_eq!(content_type("index.html"), "text/html; charset=utf-8");
        assert_eq!(content_type("A.PNG"), "image/png");
        assert_eq!(content_type("run.sh"), "application/octet-stream");
        assert_eq!(content_type("noext"), "application/octet-stream");
    }
}
