// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **An existing `gh` login is offered, and imported only on the person's yes** (#1376, ADR
//! 0077 §5), with a stand-in `gh` and `glab` first on `PATH`.
//!
//! The stand-ins write every way they were asked into a log, and `gh` keeps a login: signed in to
//! github.com, with a token it prints when asked for it. The test asserts what the person relies
//! on:
//!
//! - the offer asks `gh` only whether it is signed in, and never for its token; nothing is
//!   written, and no account exists until the import;
//! - the import asks for the token once, checks it with the forge (a recording) and keeps it in
//!   the account's own keyring item, its method `import`, and in no other file;
//! - once imported, the login is offered no more;
//! - a chat on that host is refused any forge credential, and asks `gh` nothing, while the
//!   person's own calls still reach `gh` (V22c, ADR 0070 §4);
//! - a `gh` with no login to give is refused in purlis's words, which hold nothing `gh` printed.
//!
//! The body runs in a child whose `PATH` holds only the stand-ins' directory, and whose config
//! home is this test's own, so the accounts the backend reads are the ones signed in here.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use purlis_core::forge::accounts::{self, Method, State};
use purlis_core::forge::backend::{Caller, Owner};
use purlis_core::forge::cliimport::{self, CliLogin};
use purlis_core::forge::http::TokenSource;
use purlis_core::forge::recorded::Recorded;
use purlis_core::forge::route::HostScope;
use purlis_core::forge::transport::Transport;
use purlis_core::forge::{Forge, ForgeError, Kind};
use secrecy::ExposeSecret;

/// What the stand-in `gh` keeps as its login's token. Not shaped like any forge's token: the
/// commit hook scans for those.
const KEPT: &str = "standin-cli-login-7f3a";

/// Set on the child: the directory the stand-ins and their log are in.
const STANDINS: &str = "CLI_IMPORT_STANDINS";

/// The stand-ins' log: one line per time either was run, its arguments.
fn log(standins: &Path) -> Vec<String> {
    std::fs::read_to_string(standins.join("asked.log"))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

/// Writes the stand-in `gh` and `glab` into `dir`. `gh` is signed in to github.com and prints
/// [`KEPT`] for `auth token`, unless `no_token` is there, when it has none to give; `glab` is
/// signed in nowhere. Every other ask fails, as a forge that refused would.
fn stand_ins(dir: &Path) {
    let logged = dir.join("asked.log");
    let gh = format!(
        "#!/bin/sh\n\
         echo \"gh $*\" >> '{log}'\n\
         case \"$1 $2 $3 $4\" in\n\
           'auth status --hostname github.com') exit 0 ;;\n\
           'auth token --hostname github.com')\n\
             if [ -e '{dir}/no_token' ]; then echo 'no oauth token found' >&2; exit 1; fi\n\
             echo '{KEPT}'; exit 0 ;;\n\
         esac\n\
         echo 'not this stand-in' >&2\n\
         exit 1\n",
        log = logged.display(),
        dir = dir.display(),
    );
    let glab = format!(
        "#!/bin/sh\necho \"glab $*\" >> '{}'\necho 'not logged in' >&2\nexit 1\n",
        logged.display()
    );
    for (name, body) in [("gh", gh), ("glab", glab)] {
        let at = dir.join(name);
        std::fs::write(&at, body).unwrap();
        std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

/// Every file under `root` but the test build's keyring stub, which stands in for the keychain.
fn files_but_the_keyring(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().is_some_and(|n| n != "keyring-stub.json") {
                found.push(path);
            }
        }
    }
    found
}

/// Runs `test` in a child whose `PATH` is the stand-ins' directory alone and whose config home is
/// a directory of its own; answers `None` in the parent, and the two directories in the child.
fn in_a_child(test: &str) -> Option<(PathBuf, PathBuf)> {
    if let Some(standins) = std::env::var_os(STANDINS).map(PathBuf::from) {
        let config = purlis_core::machine::config_root().expect("the child's config home");
        return Some((standins, config));
    }
    let standins = tempfile::tempdir().unwrap();
    let config = tempfile::tempdir().unwrap();
    stand_ins(standins.path());
    purlis_core::testrun::rerun_steered(
        &[test],
        &[
            ("PATH", standins.path().as_os_str()),
            (STANDINS, standins.path().as_os_str()),
            (purlis_core::machine::HOME_VAR, config.path().as_os_str()),
        ],
    );
    None
}

/// The recorded github.com that checks the imported token, and every token it was asked with.
type Asked = Arc<Mutex<Vec<String>>>;

fn import(config: &Path) -> (Result<accounts::ForgeAccount, String>, Asked) {
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/forge_contract/accounts/github/token_check.json"),
    )
    .unwrap();
    let recorded = Arc::new(Recorded::parse(&text).unwrap());
    let asked: Asked = Arc::default();
    let seen = asked.clone();
    let reach =
        move |_: &Forge, tokens: Arc<dyn TokenSource>| -> Result<Arc<dyn Transport>, ForgeError> {
            seen.lock()
                .unwrap()
                .push(tokens.token().unwrap().expose_secret().to_string());
            Ok(recorded.clone() as Arc<dyn Transport>)
        };
    let keyring = accounts::keyring_at(config);
    let got = cliimport::import(
        &HostScope::for_a_test(),
        config,
        keyring.as_ref(),
        &reach,
        Kind::GitHub,
        "github.com",
    );
    (got, asked)
}

#[test]
fn a_gh_login_is_offered_without_its_token_and_imported_only_on_the_persons_yes() {
    purlis_core::unsteered!();
    let Some((standins, config)) = in_a_child(
        "a_cli_login_is_imported_only_when_asked::a_gh_login_is_offered_without_its_token_and_imported_only_on_the_persons_yes",
    ) else {
        return;
    };

    // The offer: gh, signed in to github.com. glab is signed in nowhere, so it is not offered.
    let offers = cliimport::offered_here(&config);
    assert_eq!(
        offers,
        [CliLogin {
            kind: Kind::GitHub,
            host: "github.com".into(),
            cli: "gh",
        }]
    );
    let asked = log(&standins);
    assert!(
        asked
            .iter()
            .any(|line| line == "gh auth status --hostname github.com"),
        "{asked:?}"
    );
    assert!(
        !asked.iter().any(|line| line.contains("token")),
        "the offer asked for the token: {asked:?}"
    );
    assert!(
        accounts::list(&config, accounts::keyring_at(&config).as_ref())
            .unwrap()
            .is_empty(),
        "the offer imported something"
    );
    assert!(
        !accounts::path(&config).exists(),
        "the offer wrote the accounts file"
    );

    // The person's yes.
    let (got, checked_with) = import(&config);
    let account = got.unwrap();
    assert_eq!(
        (account.kind, account.host.as_str(), account.login.as_str()),
        (Kind::GitHub, "github.com", "octocat")
    );
    assert_eq!(account.method, Method::Import);
    assert_eq!(account.state, State::SignedIn);
    let asked = log(&standins);
    assert_eq!(
        asked
            .iter()
            .filter(|line| line.as_str() == "gh auth token --hostname github.com")
            .count(),
        1,
        "{asked:?}"
    );
    assert_eq!(*checked_with.lock().unwrap(), [KEPT.to_string()]);
    let keyring = accounts::keyring_at(&config);
    assert_eq!(
        keyring
            .get(&accounts::service(&account), "token")
            .unwrap()
            .map(|s| s.into_inner())
            .as_deref(),
        Some(KEPT)
    );
    let searched = files_but_the_keyring(&config);
    assert!(searched.contains(&accounts::path(&config)));
    for path in &searched {
        let text = String::from_utf8_lossy(&std::fs::read(path).unwrap()).into_owned();
        assert!(!text.contains(KEPT), "{} holds the token", path.display());
    }

    // Offered no more: the host has a purlis account now.
    assert!(cliimport::offered_here(&config).is_empty());

    // A chat on github.com has lost gh's login, and asks gh nothing; the person still asks gh.
    let before = log(&standins).len();
    let backend = Forge::default_of(Kind::GitHub).backend();
    let refused = backend
        .owned(&Caller::chat("c1"), &Owner::new("octocat"))
        .unwrap_err();
    assert!(
        refused.said().contains("imported into purlis"),
        "{}",
        refused.said()
    );
    assert!(!refused.said().contains(KEPT));
    assert_eq!(log(&standins).len(), before, "a chat's call reached gh");
    let _ = backend.owned(&Caller::command(), &Owner::new("octocat"));
    assert!(
        log(&standins)[before..]
            .iter()
            .any(|line| line.starts_with("gh api")),
        "the person's call never reached gh"
    );
}

#[test]
fn a_gh_with_no_login_to_give_is_refused_in_purlis_words() {
    purlis_core::unsteered!();
    let Some((standins, config)) = in_a_child(
        "a_cli_login_is_imported_only_when_asked::a_gh_with_no_login_to_give_is_refused_in_purlis_words",
    ) else {
        return;
    };
    std::fs::write(standins.join("no_token"), "").unwrap();
    let (got, checked_with) = import(&config);
    let why = got.unwrap_err();
    assert!(why.contains("gh has no login on github.com"), "{why}");
    assert!(
        !why.contains("oauth"),
        "the CLI's own words were repeated: {why}"
    );
    assert!(
        checked_with.lock().unwrap().is_empty(),
        "the forge was asked"
    );
    assert!(
        accounts::list(&config, accounts::keyring_at(&config).as_ref())
            .unwrap()
            .is_empty()
    );
}
