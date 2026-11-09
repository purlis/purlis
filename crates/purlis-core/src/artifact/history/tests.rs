// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// When the tests' times start: git refuses a commit dated much earlier.
const B: i64 = 1_700_000_000;

/// A plane in git with workspace `alpha` and artifact `board`, and a config home of its own.
struct Here {
    _dir: tempfile::TempDir,
    root: PathBuf,
    workspace: PathBuf,
    config: PathBuf,
}

impl Here {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap().join("plane");
        let workspace = root.join("workspaces/alpha");
        std::fs::create_dir_all(workspace.join("artifacts/board")).unwrap();
        std::fs::write(root.join(".gitignore"), "data.json\ndata/\n").unwrap();
        let here = Self {
            config: dir.path().canonicalize().unwrap().join("config"),
            _dir: dir,
            root,
            workspace,
        };
        here.git(&["init", "-q"], B + 1_000);
        here
    }

    fn folder(&self) -> PathBuf {
        self.workspace.join("artifacts/board")
    }

    fn git(&self, args: &[&str], at: i64) {
        let date = format!("{at} +0000");
        let done = crate::forklock::output(
            std::process::Command::new("git")
                .args(["-c", "user.name=Ada", "-c", "user.email=ada@example.com"])
                .args(args)
                .current_dir(&self.root)
                .env("GIT_COMMITTER_DATE", &date)
                .env("GIT_AUTHOR_DATE", &date),
        )
        .unwrap();
        assert!(done.status.success(), "{args:?}: {done:?}");
    }

    /// Writes the template's page and commits it at `at`.
    fn commit_page(&self, page: &str, at: i64, subject: &str) {
        std::fs::write(self.folder().join("index.html"), page).unwrap();
        self.git(&["add", "-A"], at);
        self.git(&["commit", "-q", "-m", subject], at);
    }

    /// Puts `data` in place as a Regenerate would, and keeps it at `at`.
    fn regenerate(&self, data: &str, at: i64) -> String {
        std::fs::write(self.folder().join("data.json"), data).unwrap();
        keep(
            &self.config,
            &self.workspace,
            "board",
            &self.folder(),
            &declared(&["data.json"]),
            stamp(at),
        )
        .unwrap()
    }

    fn versions(&self) -> Vec<Version> {
        versions(Some(&self.config), &self.workspace, "board", &self.folder())
    }

    fn as_of(&self, version: &str) -> Result<Then, String> {
        as_of(
            Some(&self.config),
            &self.workspace,
            "board",
            &self.folder(),
            version,
        )
    }

    fn compare(&self, from: &str, to: &str) -> Result<Compared, String> {
        compare(
            Some(&self.config),
            &self.workspace,
            "board",
            &self.folder(),
            from,
            to,
        )
    }

    /// Every path under the plane, relative to it, but git's own.
    fn plane_files(&self) -> Vec<String> {
        fn walk(root: &Path, at: &Path, out: &mut Vec<String>) {
            for entry in std::fs::read_dir(at).unwrap() {
                let path = entry.unwrap().path();
                if path.file_name().is_some_and(|n| n == ".git") {
                    continue;
                }
                if path.is_dir() {
                    walk(root, &path, out);
                } else {
                    out.push(path.strip_prefix(root).unwrap().display().to_string());
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.root, &self.root, &mut out);
        out.sort();
        out
    }
}

fn declared(names: &[&str]) -> Declared {
    Declared {
        api: 1,
        data: names.iter().map(|n| (*n).to_owned()).collect(),
        queries: Vec::new(),
    }
}

fn stamp(at: i64) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp(at, 0).unwrap()
}

fn page(then: &Then) -> String {
    String::from_utf8(then.snapshot.get("index.html").unwrap().to_vec()).unwrap()
}

fn data_then(then: &Then) -> Option<String> {
    then.snapshot
        .get("data.json")
        .map(|bytes| String::from_utf8(bytes.to_vec()).unwrap())
}

fn data_ids(versions: &[Version]) -> Vec<String> {
    versions
        .iter()
        .filter(|v| v.kind == Kind::Regenerated)
        .map(|v| v.id.clone())
        .collect()
}

#[test]
fn each_regenerate_keeps_its_data_and_only_the_newest_most_are_kept() {
    let here = Here::new();
    here.commit_page("<p>v1</p>", B + 1_000, "First page");
    let kept: Vec<String> = (0..MOST as i64 + 3)
        .map(|n| here.regenerate(&format!(r#"{{"open": {n}}}"#), B + 2_000 + n * 60))
        .collect();
    let versions = here.versions();
    let mut newest_first: Vec<String> = kept[3..].to_vec();
    newest_first.reverse();
    assert_eq!(
        data_ids(&versions),
        newest_first,
        "the oldest three are dropped"
    );
    let oldest = here.as_of(&kept[3]).unwrap();
    assert_eq!(data_then(&oldest).as_deref(), Some(r#"{"open": 3}"#));
    assert!(
        here.as_of(&kept[0]).is_err(),
        "a dropped snapshot is not a version any more"
    );
}

#[test]
fn snapshots_are_kept_in_the_machine_config_and_never_written_to_the_plane() {
    let here = Here::new();
    here.commit_page("<p>v1</p>", B + 1_000, "First page");
    std::fs::write(here.folder().join("data.json"), "{}").unwrap();
    let before = here.plane_files();
    here.regenerate(r#"{"open": 1}"#, B + 2_000);
    here.regenerate(r#"{"open": 2}"#, B + 3_000);
    assert_eq!(here.plane_files(), before, "nothing new in the plane");
    let kept = crate::machine::dir(&here.config).join(DIR);
    assert!(kept.is_dir(), "kept under the config home: {kept:?}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&kept).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "each person's own");
    }
}

#[test]
fn two_regenerates_in_one_second_are_two_versions() {
    let here = Here::new();
    here.commit_page("<p>v1</p>", B + 1_000, "First page");
    let one = here.regenerate(r#"{"open": 1}"#, B + 2_000);
    let two = here.regenerate(r#"{"open": 2}"#, B + 2_000);
    assert_ne!(one, two);
    assert_eq!(data_ids(&here.versions()), [two, one]);
}

#[test]
fn as_of_a_regenerate_is_the_template_committed_by_then_with_the_data_kept_then() {
    let here = Here::new();
    here.commit_page("<p>v1</p>", B + 1_000, "First page");
    let first = here.regenerate(r#"{"open": 1}"#, B + 2_000);
    here.commit_page("<p>v2</p>", B + 3_000, "Second page");
    here.regenerate(r#"{"open": 2}"#, B + 4_000);

    let then = here.as_of(&first).unwrap();
    assert_eq!(page(&then), "<p>v1</p>");
    assert_eq!(data_then(&then).as_deref(), Some(r#"{"open": 1}"#));
    assert_eq!(then.at, B + 2_000);
    assert_eq!(then.template.as_ref().map(|t| t.at), Some(B + 1_000));
    assert_eq!(then.data_at, Some(B + 2_000));
    let kept = then
        .data
        .as_deref()
        .expect("its data is read from the snapshot");
    assert!(kept.starts_with(crate::machine::dir(&here.config)));
}

#[test]
fn as_of_a_template_change_pairs_it_with_the_data_kept_before_it() {
    let here = Here::new();
    here.commit_page("<p>v1</p>", B + 1_000, "First page");
    here.regenerate(r#"{"open": 1}"#, B + 2_000);
    here.regenerate(r#"{"open": 15}"#, B + 2_500);
    here.commit_page("<p>v2</p>", B + 3_000, "Second page");
    here.regenerate(r#"{"open": 2}"#, B + 4_000);

    let versions = here.versions();
    let at: Vec<(i64, Kind)> = versions.iter().map(|v| (v.at, v.kind)).collect();
    assert_eq!(
        at,
        [
            (B + 4_000, Kind::Regenerated),
            (B + 3_000, Kind::TemplateChanged),
            (B + 2_500, Kind::Regenerated),
            (B + 2_000, Kind::Regenerated),
            (B + 1_000, Kind::TemplateChanged),
        ],
        "newest first"
    );
    let second = &versions[1];
    assert_eq!(second.by.as_deref(), Some("Ada"));
    assert_eq!(second.said.as_deref(), Some("Second page"));

    let then = here.as_of(&second.id).unwrap();
    assert_eq!(page(&then), "<p>v2</p>");
    assert_eq!(data_then(&then).as_deref(), Some(r#"{"open": 15}"#));

    let first = here.as_of(&versions[4].id).unwrap();
    assert_eq!(page(&first), "<p>v1</p>");
    assert_eq!(first.data, None, "no data was kept by then");
    assert_eq!(data_then(&first), None);
}

#[test]
fn a_template_never_committed_is_shown_as_it_is_today_and_says_so() {
    let here = Here::new();
    std::fs::write(here.folder().join("index.html"), "<p>local</p>").unwrap();
    let id = here.regenerate(r#"{"open": 1}"#, B + 2_000);
    let then = here.as_of(&id).unwrap();
    assert_eq!(page(&then), "<p>local</p>");
    assert_eq!(then.template, None);
}

#[test]
fn a_version_that_is_not_one_of_the_artifacts_is_refused() {
    let here = Here::new();
    here.commit_page("<p>v1</p>", B + 1_000, "First page");
    for asked in [
        "data:../../etc",
        "data:000000002000-00",
        "template:HEAD",
        "template:0000000000000000000000000000000000000000",
        "now",
        "",
    ] {
        assert!(here.as_of(asked).is_err(), "{asked:?} is no version");
    }
}

#[test]
fn as_of_serves_no_link_and_no_data_file_from_the_tree() {
    let here = Here::new();
    std::fs::write(here.folder().join("data.json"), r#"{"open": 99}"#).unwrap();
    std::os::unix::fs::symlink("/etc/hosts", here.folder().join("hosts")).unwrap();
    here.commit_page("<p>v1</p>", B + 1_000, "First page");
    here.git(
        &["add", "-f", "workspaces/alpha/artifacts/board/data.json"],
        B + 1_100,
    );
    here.git(&["commit", "-q", "-m", "data too"], B + 1_100);
    let template = &here.versions()[0];
    let then = here.as_of(&template.id).unwrap();
    let paths: Vec<&str> = then.snapshot.paths().collect();
    assert_eq!(paths, ["index.html"]);
}

#[test]
fn compare_names_the_values_and_template_files_that_changed() {
    let here = Here::new();
    here.commit_page("<p>v1</p>", B + 1_000, "First page");
    let first = here.regenerate(
        r#"{"open": 4, "milestones": ["M1", "M2"], "gone": true}"#,
        B + 2_000,
    );
    std::fs::write(here.folder().join("style.css"), "p {}").unwrap();
    here.commit_page("<p>v2</p>", B + 3_000, "Second page");
    let second = here.regenerate(
        r#"{"open": 3, "milestones": ["M1", "M3", "M4"]}"#,
        B + 4_000,
    );

    let compared = here.compare(&first, &second).unwrap();
    assert_eq!(
        compared.template,
        [
            FileChange {
                path: "index.html".into(),
                how: How::Changed
            },
            FileChange {
                path: "style.css".into(),
                how: How::Added
            },
        ]
    );
    let said: Vec<(String, Option<String>, Option<String>)> = compared
        .data
        .iter()
        .map(|c| (c.path.clone(), c.before.clone(), c.after.clone()))
        .collect();
    let s = |v: &str| Some(v.to_owned());
    assert_eq!(
        said,
        [
            ("data.json › gone".into(), s("true"), None),
            (
                "data.json › milestones › 1".into(),
                s("\"M2\""),
                s("\"M3\"")
            ),
            ("data.json › milestones › 2".into(), None, s("\"M4\"")),
            ("data.json › open".into(), s("4"), s("3")),
        ]
    );
    assert!(!compared.cut);
}

#[test]
fn compare_with_now_reads_the_folder_as_it_is() {
    let here = Here::new();
    here.commit_page("<p>v1</p>", B + 1_000, "First page");
    let first = here.regenerate(r#"{"open": 4}"#, B + 2_000);
    std::fs::write(here.folder().join("data.json"), r#"{"open": 5}"#).unwrap();
    let compared = here.compare(&first, NOW).unwrap();
    assert!(compared.template.is_empty());
    assert_eq!(compared.data.len(), 1);
    assert_eq!(compared.data[0].after.as_deref(), Some("5"));
}

#[test]
fn a_long_compare_is_cut_and_says_so() {
    let here = Here::new();
    here.commit_page("<p>v1</p>", B + 1_000, "First page");
    let many = |base: usize| {
        let rows: Vec<String> = (0..MOST_CHANGES + 10)
            .map(|n| (n + base).to_string())
            .collect();
        format!("[{}]", rows.join(","))
    };
    let one = here.regenerate(&many(0), B + 2_000);
    let two = here.regenerate(&many(1), B + 3_000);
    let compared = here.compare(&one, &two).unwrap();
    assert_eq!(compared.data.len(), MOST_CHANGES);
    assert!(compared.cut);
}

#[test]
fn a_long_value_is_shown_cut() {
    let long = "x".repeat(VALUE_MOST * 2);
    let shown = shown(&serde_json::Value::String(long));
    assert_eq!(shown.chars().count(), VALUE_MOST + 1);
    assert!(shown.ends_with('…'));
}
