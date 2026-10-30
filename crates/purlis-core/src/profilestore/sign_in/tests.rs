// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::cell::RefCell;
use std::path::PathBuf;

use crate::profilestore::{Adopted, Folders, MachineProfile};

/// [`super::login_for`], by the harness's own default.
fn login_for(profile: &Profile, program: &str, provider: Option<&str>) -> Result<Login, String> {
    super::login_for(profile, program, provider, Way::Asked)
}

/// **A flow asked for by name is the harness's own word for it**: opencode's `--method` labels on
/// its ChatGPT plan alone, Codex's `--device-auth`, and none where a harness has no such flow
/// (#4044).
#[test]
fn a_flow_is_asked_for_in_the_harnesss_own_words() {
    let words = |kind: &str, plan, way| {
        super::login_for(&profile(kind), "/bin/it", plan, way).map(|login| login.args)
    };

    assert_eq!(
        words("opencode", Some("openai"), Way::Browser).unwrap()[2..],
        [
            "--provider",
            "openai",
            "--method",
            "ChatGPT Pro/Plus (browser)"
        ][..]
    );
    assert_eq!(
        words("opencode", Some("openai"), Way::Device).unwrap()[2..],
        [
            "--provider",
            "openai",
            "--method",
            "ChatGPT Pro/Plus (headless)"
        ][..]
    );
    assert_eq!(words("codex", None, Way::Browser).unwrap(), ["login"]);
    assert_eq!(
        words("codex", None, Way::Device).unwrap(),
        ["login", "--device-auth"]
    );
    assert!(words("claude", None, Way::Device).is_err());
    assert!(words("opencode", Some("github-copilot"), Way::Device).is_err());
    assert_eq!(Way::named("device"), Some(Way::Device));
    assert_eq!(Way::named("anything"), None);
}

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
        sign_in: None,
        pinned: None,
        sandbox_only: false,
        kept_for: None,
        rest: Default::default(),
    }
}

fn profile(kind: &str) -> Profile {
    crate::profiles::of_record(&made(kind)).expect("a made profile runs")
}

/// A profile purlis made of `kind`, in a folder of the test's own: what a harness says of it
/// is remembered by its folder, so no other test's answer is.
fn profile_in(kind: &str, folder: &str) -> Profile {
    let mut record = made(kind);
    record.folders.config = PathBuf::from(format!("/profiles/{folder}/config"));
    if record.folders.data.is_some() {
        record.folders.data = Some(PathBuf::from(format!("/profiles/{folder}/data")));
    }
    crate::profiles::of_record(&record).expect("a made profile runs")
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

    let why = refusal(&profile_in("claude", "refused"), &[], &look).expect("refused");

    assert!(why.contains("'work' is not signed in"), "{why}");
    assert!(why.contains("Sign in"), "{why}");
    assert!(why.contains("purlis profile sign-in work"), "{why}");
    assert!(why.contains("Nothing was started"), "{why}");
}

#[test]
fn a_chat_is_never_refused_where_the_harness_cannot_tell() {
    for (at, status) in [Some(0), None, Some(3)].into_iter().enumerate() {
        let look = Recorded::answering(status);
        let profile = profile_in("codex", &format!("cannot-tell-{at}"));
        assert_eq!(refusal(&profile, &[], &look), None, "{status:?}");
    }
    assert_eq!(
        refusal(
            &profile_in("opencode", "cannot-tell-oc"),
            &[],
            &Recorded::answering(None)
        ),
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

    let why = login_of(&record, None, None).expect_err("nothing to sign in");

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
    let claude = login_for(&profile("claude"), "/bin/claude", None).unwrap();
    let codex = login_for(&profile("codex"), "/bin/codex", None).unwrap();
    let opencode = login_for(&profile("opencode"), "/bin/opencode", Some("openai")).unwrap();

    assert_eq!(claude.args, ["auth", "login"]);
    assert_eq!(
        claude.env,
        [(
            "CLAUDE_CONFIG_DIR".to_owned(),
            "/profiles/work/config".to_owned()
        )]
    );
    assert_eq!(codex.args, ["login"]);
    assert_eq!(opencode.args, ["auth", "login", "--provider", "openai"]);
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
    let login = login_for(&profile("claude"), "/Users/me/.local/bin/claude", None).unwrap();

    assert_eq!(
        login.line().unwrap(),
        "/usr/bin/env CLAUDE_CONFIG_DIR=/profiles/work/config /Users/me/.local/bin/claude auth login\n"
    );
}

#[test]
fn a_folder_with_a_space_is_one_word_of_the_line() {
    let mut record = made("codex");
    record.folders.config = PathBuf::from("/Users/my profiles/work/config");
    let login = login_for(
        &crate::profiles::of_record(&record).unwrap(),
        "/bin/codex",
        None,
    )
    .unwrap();

    assert_eq!(
        login.line().unwrap(),
        "/usr/bin/env 'CODEX_HOME=/Users/my profiles/work/config' /bin/codex login\n"
    );
}

#[test]
fn a_folder_with_a_quote_or_a_backslash_is_never_typed_into_a_shell() {
    for folder in ["/Users/o'neil/work/config", "/Users/a\\b/work/config"] {
        let mut record = made("codex");
        record.folders.config = PathBuf::from(folder);
        let login = login_for(
            &crate::profiles::of_record(&record).unwrap(),
            "/bin/codex",
            None,
        )
        .unwrap();

        assert!(login.line().is_err(), "{folder}");
    }
}

#[test]
fn no_sign_in_is_typed_into_a_shell_on_windows() {
    let login = login_for(&profile("codex"), "codex", None).unwrap();

    let why = login
        .line_on(crate::sandbox::Os::Windows)
        .expect_err("no line on Windows");

    assert!(why.contains("on Windows"), "{why}");
    assert!(why.contains("codex login"), "{why}");
}

#[test]
fn opencode_is_signed_in_only_to_a_provider_purlis_names_and_never_to_anthropic() {
    let opencode = profile("opencode");

    let copilot = login_for(&opencode, "/bin/opencode", Some("github-copilot")).unwrap();
    assert_eq!(
        copilot.args,
        ["auth", "login", "--provider", "github-copilot"]
    );
    for refused in [None, Some("anthropic"), Some("")] {
        let why = login_for(&opencode, "/bin/opencode", refused).expect_err("refused");
        assert!(why.contains("never"), "{why}");
    }
    assert!(
        providers("opencode")
            .iter()
            .all(|p| p.provider != "anthropic")
    );
    assert!(providers("claude").is_empty());
    assert!(login_for(&profile("claude"), "/bin/claude", Some("openai")).is_err());
}

#[test]
fn what_a_harness_said_is_remembered_for_a_minute_and_forgotten_on_sign_in() {
    struct Counted(std::cell::Cell<u32>);
    impl Look for Counted {
        fn status(&self, _: &str, _: &[&str], _: &[(String, String)]) -> Option<i32> {
            self.0.set(self.0.get() + 1);
            Some(1)
        }
        fn is_there(&self, _: &Path) -> bool {
            false
        }
    }
    let look = Counted(std::cell::Cell::new(0));
    let profile = profile_in("claude", "remembered");
    let now = Instant::now();

    for _ in 0..5 {
        assert_eq!(standing_at(&profile, &look, now), Standing::SignedOut);
    }
    assert_eq!(look.0.get(), 1, "five starts asked the harness once");
    standing_at(&profile, &look, now + REMEMBERED + Duration::from_secs(1));
    assert_eq!(look.0.get(), 2, "asked again after a minute");
    forget(&profile);
    standing_at(&profile, &look, now + REMEMBERED + Duration::from_secs(2));
    assert_eq!(look.0.get(), 3, "asked again after Sign in");
}

#[test]
fn a_refused_start_leaves_one_wait_until_the_harness_says_it_is_signed_in() {
    let root = Path::new("/projects/waiting");
    let profile = profile_in("claude", "waits");
    let now = Instant::now();
    standing_at(&profile, &Recorded::answering(Some(1)), now);
    waits_on(root, &profile);
    waits_on(root, &profile);

    let waits = waiting(root);
    assert_eq!(waits.len(), 1, "one wait however many starts");
    assert_eq!(waits[0].profile.name, "work");
    assert_eq!(waits[0].harness, "Claude Code");
    assert!(waiting(Path::new("/projects/other")).is_empty());

    forget(&profile);
    assert_eq!(waiting(root).len(), 1, "not asked lately: it still waits");
    standing_at(&profile, &Recorded::answering(Some(0)), Instant::now());
    assert!(waiting(root).is_empty(), "signed in, and let go");
    standing_at(
        &profile,
        &Recorded::answering(Some(1)),
        Instant::now() + REMEMBERED * 2,
    );
    assert!(waiting(root).is_empty(), "gone for good");
}

#[test]
fn a_watch_after_sign_in_lets_the_wait_go_once_the_harness_says_signed_in() {
    struct Then(std::sync::atomic::AtomicU32);
    impl Look for Then {
        fn status(&self, _: &str, _: &[&str], _: &[(String, String)]) -> Option<i32> {
            let asked = self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Some(if asked < 2 { 1 } else { 0 })
        }
        fn is_there(&self, _: &Path) -> bool {
            false
        }
    }
    let root = Path::new("/projects/watched");
    let profile = profile_in("codex", "watched");
    waits_on(root, &profile);
    let (tell, told) = std::sync::mpsc::channel();

    watch(
        profile,
        Then(std::sync::atomic::AtomicU32::new(0)),
        Duration::from_millis(1),
        Duration::from_secs(5),
        move || tell.send(()).unwrap(),
    )
    .join()
    .unwrap();

    told.try_recv().expect("the window was told");
    assert!(waiting(root).is_empty());
}

#[test]
fn a_dismissed_wait_is_gone() {
    let root = Path::new("/projects/dismissed");
    let profile = profile_in("codex", "dismissed");
    waits_on(root, &profile);

    dismiss(root, "work");

    assert!(waiting(root).is_empty());
}

#[test]
fn a_folder_with_a_line_break_is_never_typed_into_a_shell() {
    let mut record = made("codex");
    record.folders.config = PathBuf::from("/profiles/a\nb/config");
    let login = login_for(
        &crate::profiles::of_record(&record).unwrap(),
        "/bin/codex",
        None,
    )
    .unwrap();

    assert!(login.line().is_err());
}

#[test]
fn the_plans_sign_in_offers_are_the_ones_the_harnesss_declaration_lists() {
    let declared: Vec<&str> = crate::harness_declaration::builtin("opencode")
        .unwrap()
        .sign_in
        .plans
        .iter()
        .map(|plan| plan.provider.as_str())
        .collect();
    assert_eq!(declared, ["openai", "github-copilot"]);
    let offered: Vec<&str> = providers("opencode")
        .iter()
        .map(|plan| plan.provider.as_str())
        .collect();
    assert_eq!(offered, declared);
    for id in declared {
        let login = login_for(&profile("opencode"), "/bin/opencode", Some(id)).unwrap();
        assert_eq!(login.args, ["auth", "login", "--provider", id]);
    }
    // A harness whose declaration lists none signs in to its own vendor's plan, naming none.
    assert!(providers("codex").is_empty());
    let codex = login_for(&profile("codex"), "/bin/codex", None).unwrap();
    assert_eq!(codex.args, ["login"]);
}

/// A runner's record of the desktop's profile `work` of `kind`, kept for it and pinned.
fn kept_pinned(kind: &str) -> MachineProfile {
    MachineProfile {
        kept_for: Some("01JA0DESKT0P00000000000000".to_owned()),
        pinned: Some(crate::profilestore::pin::Pinned {
            version: "2.1.300".to_owned(),
            at: 1,
        }),
        ..made(kind)
    }
}

/// **On a runner, Sign in runs the pinned version it installed, not one found on `PATH`**
/// (#4007): a runner image carries no harness, so the version a chat's start installs is the
/// one that signs the profile in.
#[test]
fn a_runners_sign_in_runs_the_pinned_version_it_installed() {
    let installed = PathBuf::from("/runner/data/harnesses/claude/2.1.300/claude");

    let login = login_of(&kept_pinned("claude"), None, Some(&installed)).expect("a sign-in");

    assert_eq!(login.program, installed.display().to_string());
    assert_eq!(login.args, ["auth", "login"]);
    assert!(
        login
            .env
            .iter()
            .any(|(n, v)| n == "CLAUDE_CONFIG_DIR" && v == "/profiles/work/config"),
        "{:?}",
        login.env
    );
}

/// A machine that answers every status with 0, and installs `pinned` as a runner's pinned
/// version: the program each status was asked of, in order.
struct WithPinned {
    pinned: Option<PathBuf>,
    programs: RefCell<Vec<String>>,
}

impl Look for WithPinned {
    fn status(&self, program: &str, _: &[&str], _: &[(String, String)]) -> Option<i32> {
        self.programs.borrow_mut().push(program.to_owned());
        Some(0)
    }
    fn is_there(&self, _: &Path) -> bool {
        false
    }
    fn pinned_program(&self, _: &MachineProfile) -> Option<PathBuf> {
        self.pinned.clone()
    }
}

/// **On a runner, whether a profile is signed in is asked of its pinned version** (#4007), and
/// of the harness on `PATH` where none is installed for it.
#[test]
fn a_runners_standing_is_asked_of_its_pinned_version_where_it_is_installed() {
    let installed = PathBuf::from("/runner/data/harnesses/claude/2.1.300/claude");
    let mut record = kept_pinned("claude");
    record.folders.config = PathBuf::from("/profiles/pinned-standing/config");
    let pinned = WithPinned {
        pinned: Some(installed.clone()),
        programs: RefCell::new(Vec::new()),
    };
    let unpinned = WithPinned {
        pinned: None,
        programs: RefCell::new(Vec::new()),
    };

    assert_eq!(standing_of(&record, &pinned), Some(Standing::SignedIn));
    forget_of(&record);
    assert_eq!(standing_of(&record, &unpinned), Some(Standing::SignedIn));

    assert_eq!(*pinned.programs.borrow(), [installed.display().to_string()]);
    assert_eq!(*unpinned.programs.borrow(), ["claude"]);
}
