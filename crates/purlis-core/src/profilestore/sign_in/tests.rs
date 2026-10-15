// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::cell::RefCell;
use std::path::PathBuf;

use crate::profilestore::{Adopted, Folders, MachineProfile};

/// What a [`Look`] was asked, in order: each program run with its words and folder variables,
/// and each path whose name was looked for. A `Look` has no way to read a file, so this is
/// every file purlis touches to tell whether a profile is signed in.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Asked {
    Ran {
        args: Vec<String>,
        env: Vec<(String, String)>,
    },
    LookedFor(PathBuf),
}

/// A machine whose harness answers its status words with `status`, and where `there` are the
/// paths a name is found at.
struct Recorded {
    status: Option<i32>,
    there: Vec<PathBuf>,
    asked: RefCell<Vec<Asked>>,
}

impl Recorded {
    fn answering(status: Option<i32>) -> Self {
        Self {
            status,
            there: Vec::new(),
            asked: RefCell::new(Vec::new()),
        }
    }

    fn with_there(path: PathBuf) -> Self {
        Self {
            status: None,
            there: vec![path],
            asked: RefCell::new(Vec::new()),
        }
    }

    fn asked(&self) -> Vec<Asked> {
        self.asked.borrow().clone()
    }
}

impl Look for Recorded {
    fn status(&self, _program: &str, args: &[&str], env: &[(String, String)]) -> Option<i32> {
        self.asked.borrow_mut().push(Asked::Ran {
            args: args.iter().map(|a| (*a).to_owned()).collect(),
            env: env.to_vec(),
        });
        self.status
    }

    fn is_there(&self, path: &Path) -> bool {
        self.asked
            .borrow_mut()
            .push(Asked::LookedFor(path.to_path_buf()));
        self.there.iter().any(|p| p == path)
    }
}

/// A profile purlis made of `kind`, its folders under `/profiles/work`.
fn made(kind: &str) -> MachineProfile {
    let data = (kind == "opencode").then(|| PathBuf::from("/profiles/work/data"));
    MachineProfile {
        name: "work".to_owned(),
        harness: kind.to_owned(),
        title: "work".to_owned(),
        folders: Folders {
            config: PathBuf::from("/profiles/work/config"),
            data,
        },
        adopted: None,
        imported: None,
        rest: Default::default(),
    }
}

fn profile(kind: &str) -> Profile {
    crate::profiles::of_record(&made(kind)).expect("a made profile runs")
}

fn ran(args: &[&str], env: &[(&str, &str)]) -> Asked {
    Asked::Ran {
        args: args.iter().map(|a| (*a).to_owned()).collect(),
        env: env
            .iter()
            .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
            .collect(),
    }
}

// Each harness's own status words, as its answers were recorded: Codex 0.147.0's
// `codex login status` printed "Not logged in" and exited 1 with an empty `CODEX_HOME`, and
// exits 0 once signed in; Claude Code's `claude auth status` exits 0 signed in and 1 signed out.

#[test]
fn claude_code_is_asked_its_own_auth_status_in_the_profiles_folder() {
    let look = Recorded::answering(Some(0));

    assert_eq!(standing(&profile("claude"), &look), Standing::SignedIn);
    assert_eq!(
        look.asked(),
        [ran(
            &["auth", "status"],
            &[("CLAUDE_CONFIG_DIR", "/profiles/work/config")]
        )]
    );
}

#[test]
fn claude_code_answering_one_is_signed_out() {
    let look = Recorded::answering(Some(1));

    assert_eq!(standing(&profile("claude"), &look), Standing::SignedOut);
}

#[test]
fn codex_is_asked_its_own_login_status_in_the_profiles_folder() {
    let signed_out = Recorded::answering(Some(1));
    let signed_in = Recorded::answering(Some(0));

    assert_eq!(
        standing(&profile("codex"), &signed_out),
        Standing::SignedOut
    );
    assert_eq!(standing(&profile("codex"), &signed_in), Standing::SignedIn);
    assert_eq!(
        signed_out.asked(),
        [ran(
            &["login", "status"],
            &[("CODEX_HOME", "/profiles/work/config")]
        )]
    );
}

#[test]
fn a_status_that_does_not_answer_or_answers_otherwise_cannot_tell() {
    for status in [None, Some(2), Some(127), Some(-1)] {
        let look = Recorded::answering(status);
        assert_eq!(
            standing(&profile("claude"), &look),
            Standing::CannotTell,
            "{status:?}"
        );
    }
}

#[test]
fn opencode_is_signed_in_where_its_credential_file_is_there_by_name() {
    let file = PathBuf::from("/profiles/work/data/opencode/auth.json");
    let look = Recorded::with_there(file.clone());

    assert_eq!(standing(&profile("opencode"), &look), Standing::SignedIn);
    assert_eq!(look.asked(), [Asked::LookedFor(file)], "no program is run");
}

#[test]
fn opencode_with_no_credential_file_cannot_tell_as_it_may_run_on_a_free_or_local_model() {
    let look = Recorded::answering(None);

    assert_eq!(standing(&profile("opencode"), &look), Standing::CannotTell);
}

/// The one file a status reads by name is never opened: an `auth.json` nobody may read still
/// says the profile is signed in, on this machine's own [`Look`].
#[cfg(unix)]
#[test]
fn opencodes_credential_file_is_seen_by_its_name_and_never_opened() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let mut record = made("opencode");
    record.folders.config = dir.path().join("config");
    record.folders.data = Some(dir.path().join("data"));
    let held = dir.path().join("data/opencode");
    std::fs::create_dir_all(&held).unwrap();
    let file = held.join("auth.json");
    std::fs::write(&file, "{}\n").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000)).unwrap();
    let profile = crate::profiles::of_record(&record).unwrap();

    assert_eq!(standing(&profile, &ThisMachine), Standing::SignedIn);
}

#[test]
fn a_chat_on_a_made_profile_its_harness_says_is_signed_out_is_refused_with_sign_in() {
    let look = Recorded::answering(Some(1));

    let why = refusal(&profile("claude"), &[], &look).expect("refused");

    assert!(why.contains("'work' is not signed in"), "{why}");
    assert!(why.contains("Sign in"), "{why}");
    assert!(why.contains("purlis profile sign-in work"), "{why}");
    assert!(why.contains("Nothing was started"), "{why}");
}

#[test]
fn a_chat_is_never_refused_where_the_harness_cannot_tell() {
    for status in [Some(0), None, Some(3)] {
        let look = Recorded::answering(status);
        assert_eq!(refusal(&profile("codex"), &[], &look), None, "{status:?}");
    }
    assert_eq!(
        refusal(&profile("opencode"), &[], &Recorded::answering(None)),
        None
    );
}

#[test]
fn a_built_in_or_a_clones_profile_is_never_asked() {
    let look = Recorded::answering(Some(1));
    let mut builtin = profile("claude");
    builtin.source = crate::profiles::Source::BuiltIn;
    let mut local = profile("claude");
    local.source = crate::profiles::Source::Local;

    assert_eq!(refusal(&builtin, &[], &look), None);
    assert_eq!(refusal(&local, &[], &look), None);
    assert_eq!(look.asked(), [], "nothing was run or looked for");
}

#[test]
fn an_adopted_profile_is_signed_in_by_the_persons_own_sign_in_and_never_asked() {
    let mut record = made("claude");
    record.adopted = Some(Adopted {
        at: 1,
        variables: Vec::new(),
    });

    let why = login_of(&record).expect_err("nothing to sign in");

    assert!(why.contains("own sign-in"), "{why}");
}

#[test]
fn a_profile_signed_in_by_a_key_it_is_handed_is_not_asked() {
    let look = Recorded::answering(Some(1));

    let passed = ["ANTHROPIC_API_KEY".to_owned()];
    assert_eq!(refusal(&profile("claude"), &passed, &look), None);
    assert_eq!(look.asked(), []);
}

#[test]
fn signing_in_runs_each_harnesss_own_sign_in_in_the_profiles_folder() {
    let claude = login_for(&profile("claude"), "/bin/claude");
    let codex = login_for(&profile("codex"), "/bin/codex");
    let opencode = login_for(&profile("opencode"), "/bin/opencode");

    assert_eq!(claude.args, ["auth", "login"]);
    assert_eq!(
        claude.env,
        [(
            "CLAUDE_CONFIG_DIR".to_owned(),
            "/profiles/work/config".to_owned()
        )]
    );
    assert_eq!(codex.args, ["login"]);
    assert_eq!(opencode.args, ["auth", "login"]);
    assert_eq!(
        opencode.env,
        [
            (
                "XDG_CONFIG_HOME".to_owned(),
                "/profiles/work/config".to_owned()
            ),
            ("XDG_DATA_HOME".to_owned(), "/profiles/work/data".to_owned()),
        ]
    );
}

#[test]
fn the_line_a_shell_runs_sets_the_folders_and_runs_the_program_it_names() {
    let login = login_for(&profile("claude"), "/Users/me/.local/bin/claude");

    assert_eq!(
        login.line().unwrap(),
        "env CLAUDE_CONFIG_DIR=/profiles/work/config /Users/me/.local/bin/claude auth login\n"
    );
}

#[test]
fn a_folder_with_a_quote_or_a_space_is_one_word_of_the_line() {
    let mut record = made("codex");
    record.folders.config = PathBuf::from("/Users/o'neil/my profiles/work/config");
    let login = login_for(&crate::profiles::of_record(&record).unwrap(), "/bin/codex");

    assert_eq!(
        login.line().unwrap(),
        "env 'CODEX_HOME=/Users/o'\"'\"'neil/my profiles/work/config' /bin/codex login\n"
    );
}

#[test]
fn a_folder_with_a_line_break_is_never_typed_into_a_shell() {
    let mut record = made("codex");
    record.folders.config = PathBuf::from("/profiles/a\nb/config");
    let login = login_for(&crate::profiles::of_record(&record).unwrap(), "/bin/codex");

    assert!(login.line().is_err());
}
