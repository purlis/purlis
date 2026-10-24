// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis skill revert` to a version whose frontmatter would widen a chat (SK-19, #2782): it
//! says so in the window's own sentence (`widening_said`), and asks before reverting, unless
//! `--yes` is given. A "no" leaves the skill as it was. Behind the `skill-studio` flag. The
//! project's git directory sits beside it, so no `.git` folder is made inside the fixture.

mod support;

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const WIDE: &str = "---\nname: deploy\ndescription: Ships the build to production.\nallowed-tools: Bash\n---\n\nWide.\n";
const NARROW: &str =
    "---\nname: deploy\ndescription: Ships the build to production.\n---\n\nNarrow.\n";

/// git in `at`, reading no config of the machine's.
fn git(at: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
        ])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .current_dir(at)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

struct Project {
    _tmp: tempfile::TempDir,
    at: PathBuf,
    /// The version that widens, by its commit.
    wide: String,
}

impl Project {
    /// A project that saves by committing, whose skill `deploy` was [`WIDE`] and is [`NARROW`].
    fn new() -> Project {
        let tmp = tempfile::tempdir().unwrap();
        let at = tmp.path().canonicalize().unwrap();
        let root = at.join("plane");
        std::fs::create_dir_all(root.join("skills/deploy")).unwrap();
        std::fs::create_dir_all(at.join("home")).unwrap();
        std::fs::write(
            root.join("charter.toml"),
            "schema = 1\n\n[plane]\nmode = \"commit\"\n",
        )
        .unwrap();
        std::fs::write(root.join(".gitignore"), ".charter/\n.purlis/\n").unwrap();
        std::fs::write(root.join("skills/deploy/SKILL.md"), WIDE).unwrap();
        let separate = format!("--separate-git-dir={}", at.join("plane.git").display());
        git(&root, &["init", "-q", "-b", "main", &separate, "."]);
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-q", "-m", "wide"]);
        let wide = git(&root, &["rev-parse", "HEAD"]);
        std::fs::write(root.join("skills/deploy/SKILL.md"), NARROW).unwrap();
        git(&root, &["commit", "-q", "-am", "narrow"]);
        Project {
            _tmp: tmp,
            at,
            wide,
        }
    }

    fn root(&self) -> PathBuf {
        self.at.join("plane")
    }

    fn run(&self, args: &[&str], input: &str) -> Output {
        let root = self.root();
        let mut command = support::purlis();
        command
            .args(args)
            .current_dir(&root)
            .env("CHARTER_ROOT", &root)
            .env("HOME", self.at.join("home"))
            .env("PURLIS_CONFIG_HOME", self.at.join("home/config"))
            .env("PURLIS_SKILL_STUDIO", "1")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("NO_COLOR", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for spelling in purlis_core::envvar::spellings("PURLIS_SESSION_ID") {
            command.env_remove(spelling);
        }
        let mut child = command.spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }

    fn skill_md(&self) -> String {
        std::fs::read_to_string(self.root().join("skills/deploy/SKILL.md")).unwrap()
    }
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn a_revert_to_a_version_that_widens_says_so_and_asks_first() {
    let project = Project::new();

    let declined = project.run(&["skill", "revert", "deploy", &project.wide], "n\n");

    let said = text(&declined);
    assert_eq!(declined.status.code(), Some(1), "{said}");
    assert!(said.contains("Widens the chat"), "{said}");
    assert!(said.contains("leaves them out when it hands"), "{said}");
    assert!(said.contains("Nothing was reverted"), "{said}");
    assert_eq!(project.skill_md(), NARROW);

    let yes = project.run(&["skill", "revert", "deploy", &project.wide, "--yes"], "");

    assert_eq!(yes.status.code(), Some(0), "{}", text(&yes));
    assert!(text(&yes).contains("Widens the chat"));
    assert_eq!(project.skill_md(), WIDE);
}
