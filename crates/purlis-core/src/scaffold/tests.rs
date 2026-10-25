// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

mod merge_rules;
/// The default ask rule for `charter report --yes` (#363, ADR 0059 amended 2026-09-26).
mod report_ask;

/// The guard's list of consent-gated commands is the list of rules written here.
#[test]
fn the_consent_patterns_are_the_ask_rules_init_writes() {
    assert_eq!(
        [REPORT_ASK, PROMOTE_ASK].map(|ask| ask.pattern),
        settings::CONSENT_PATTERNS
    );
}

/// Verified against CPython 3.14: `commands._fold_entries(...)`.
#[test]
fn entries_for_one_file_fold_onto_one_line_in_the_order_they_came() {
    let created: Vec<String> = [
        "charter.toml",
        ".claude/settings.json (env)",
        "opencode.json (ask: charter handoff)",
        ".claude/settings.json (ask: charter handoff)",
        ".claude/settings.json (plane-root guard)",
        ".claude/settings.json (env)",
    ]
    .map(str::to_owned)
    .to_vec();
    assert_eq!(
        fold(&created),
        vec![
            "charter.toml",
            ".claude/settings.json (env, ask: charter handoff, plane-root guard)",
            "opencode.json (ask: charter handoff)",
        ]
    );
}

/// `InitArgs` as `charter init --forge github --owner acme` builds it.
pub(super) fn plain() -> InitArgs {
    InitArgs {
        forge: Some("github".to_owned()),
        owner: "acme".to_owned(),
        host: None,
        clone_this_repo: false,
        plane_is_this_repo: false,
        adopt: None,
        now: None,
        front_door: Some("steward".to_owned()),
    }
}

/// The app reaches `init` with no argument parser in between (`opener::create_project`),
/// so clap's `conflicts_with` is not the only thing standing between two flags that name
/// two different sources for one first clone. This is the other one, and it refuses
/// before the first directory is made.
#[test]
fn adopt_and_clone_this_repo_together_are_refused_and_nothing_is_written() {
    let dir = tempfile::tempdir().expect("a directory");
    let root = dir.path().join("plane");
    std::fs::create_dir_all(&root).expect("the plane's directory");
    let place = Place {
        root: root.clone(),
        is_plane: false,
    };

    let outcome = init(
        &place,
        &InitArgs {
            clone_this_repo: true,
            adopt: Some(dir.path().join("widget")),
            ..plain()
        },
    );

    assert_eq!(outcome.code, 1);
    assert!(
        matches!(outcome.said.first(), Some(Say::Err(why)) if why.contains("Ask for one.")),
        "{:?}",
        outcome.said
    );
    assert!(!root.join(crate::plane::MANIFEST).exists());
    assert!(!root.join("personas").exists());
}

/// `--adopt` pointed at a directory that is not a repository: the plane is still made,
/// exactly as Python makes it before a first clone that fails.
#[test]
fn adopt_refuses_a_directory_that_is_not_a_repository_and_still_makes_the_plane() {
    let dir = tempfile::tempdir().expect("a directory");
    let root = dir.path().join("plane");
    let not_a_repo = dir.path().join("papers");
    for at in [&root, &not_a_repo] {
        std::fs::create_dir_all(at).expect("a directory");
    }
    let place = Place {
        root: root.clone(),
        is_plane: false,
    };

    let outcome = init(
        &place,
        &InitArgs {
            adopt: Some(not_a_repo),
            ..plain()
        },
    );

    assert_eq!(outcome.code, 1);
    assert!(
        outcome.said.iter().any(|line| matches!(
            line,
            Say::Err(why) if why.contains("is not the top level of a git working tree")
        )),
        "{:?}",
        outcome.said
    );
    assert!(root.join(crate::plane::MANIFEST).is_file());
}

#[test]
fn a_persona_name_is_the_alphabet_charter_mints() {
    assert!(persona_name_ok("steward"));
    assert!(persona_name_ok("front-door.2"));
    assert!(!persona_name_ok("_shared"));
    assert!(!persona_name_ok("Steward"));
    assert!(!persona_name_ok(""));
    assert!(!persona_name_ok("a/b"));
    assert!(!persona_name_ok(".x"));
}

// ---------------------------------------------------------------------------------------
// What `init` and `reinit` SAY, line by line, and what they leave on disk.
//
// `Say::Info/Ok/Warn/Err` are `util.info/ok/warn/err` in `charter/util.py` (`• `, `✓ `,
// `! `, `✗ `); every expected line below is the f-string `commands.cmd_init` or
// `cmd_reinit` prints for the same scenario, minus the harness-install lines this port
// documents it does not produce (the module docs: "What `init` does not do here").
// ---------------------------------------------------------------------------------------

/// A fresh directory to point `init` at, and the tempdir that keeps it alive.
fn empty_plane() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("a directory");
    let root = dir.path().join("plane");
    std::fs::create_dir_all(&root).expect("the plane's directory");
    (dir, root)
}

pub(super) fn at(root: &Path, is_plane: bool) -> Place {
    Place {
        root: root.to_path_buf(),
        is_plane,
    }
}

/// Whether this machine's Claude Code already runs the guard through charter's plugin —
/// in which case `_ensure_guard_hook` answers "present" and writes nothing
/// (`commands._plugin_dispatches_guard`). The operator's own `$HOME` decides it, so the
/// expected lines are built from the same answer rather than assuming CI's.
fn plugin_guards(root: &Path) -> bool {
    settings::plugin_dispatches_guard(root, crate::profiles::home().as_deref()).is_some()
}

/// The guard hook's label, in the list `init`/`reinit` put it in.
fn guard_label(root: &Path) -> (&'static str, bool) {
    if plugin_guards(root) {
        (
            ".claude/settings.json (plane-root guard already wired)",
            false,
        )
    } else {
        (".claude/settings.json (plane-root guard)", true)
    }
}

fn errs(outcome: &Outcome) -> Vec<&str> {
    outcome
        .said
        .iter()
        .filter_map(|s| match s {
            Say::Err(e) => Some(e.as_str()),
            _ => None,
        })
        .collect()
}

fn blocker(name: &str, path: &Path, command: &str) -> Say {
    Say::Err(
        format!(
            "{name}/ can't be created — {} already exists and is not a directory. purlis \
         never deletes or renames existing content; move or remove it yourself, then \
         re-run `purlis {command}`.",
            path.display()
        )
        .into(),
    )
}

fn escape(rel: &str, lands: &Path, command: &str) -> Say {
    Say::Err(
        format!(
            "{rel} resolves to {}, which is outside this plane or inside its .git — purlis \
         reads and writes nothing through it. Point it inside the plane or remove it \
         yourself, then re-run `purlis {command}`.",
            lands.display()
        )
        .into(),
    )
}

const NEXT: &str =
    "Next: `purlis doctor` to preflight, then `purlis discover` to build the inventory.";

/// A fresh `init` says what `cmd_init` says after it created everything: the `util.ok`
/// headline with the folded COUNT, one `util.info("  + item")` per folded entry, and the
/// `Next:` line — and no "already present" line, because nothing was.
#[test]
fn init_into_an_empty_directory_says_what_it_wrote_line_for_line() {
    let (_dir, root) = empty_plane();
    let (guard, guard_created) = guard_label(&root);

    let outcome = init(&at(&root, false), &plain());

    let settings_item = if guard_created {
        ".claude/settings.json (env, ask: charter report --yes, \
         ask: charter ws todo promote, ask: purlis report --yes, \
         ask: purlis ws todo promote, plane-root guard)"
    } else {
        ".claude/settings.json (env, ask: charter report --yes, ask: charter \
         ws todo promote, ask: purlis report --yes, ask: purlis ws todo promote)"
    };
    let mut said = vec![
        Say::Ok(
            "Initialized control plane (schema 1) — 9 items written."
                .to_owned()
                .into(),
        ),
        Say::Info("  + charter.toml".to_owned().into()),
        Say::Info("  + personas/".to_owned().into()),
        Say::Info("  + inventory/".to_owned().into()),
        Say::Info("  + workspaces/".to_owned().into()),
        Say::Info("  + .gitignore".to_owned().into()),
        Say::Info("  + .gitattributes (merge rules)".to_owned().into()),
        Say::Info(format!("  + {settings_item}").into()),
        Say::Info(
            ("  + opencode.json (ask: charter report --yes, ask: charter ws \
             todo promote, ask: purlis report --yes, ask: purlis ws todo promote)"
                .to_owned())
            .into(),
        ),
        Say::Info(
            "  + personas/steward/ (front door, declared in charter.toml)"
                .to_owned()
                .into(),
        ),
    ];
    if !guard_created {
        said.push(Say::Info(format!("  already present: {guard}").into()));
    }
    said.push(Say::Info(NEXT.to_owned().into()));
    assert_eq!(outcome.said, said);
    assert_eq!(outcome.code, 0);
    for dir in BASELINE_DIRS {
        assert!(root.join(dir).is_dir(), "{dir}");
    }
    assert_eq!(
        std::fs::read_to_string(root.join(".gitignore")).expect("a .gitignore"),
        GITIGNORE_BASELINE
    );
    let written = std::fs::read_to_string(root.join(settings::SETTINGS)).expect("settings");
    assert!(written.contains("CHARTER_HARNESS"), "{written}");
}

/// `init` again changes nothing and says so in `cmd_init`'s words: "already fully set
/// up", then everything it found, in the order it looked.
#[test]
fn init_twice_says_the_plane_is_already_set_up_and_what_it_found() {
    let (_dir, root) = empty_plane();
    init(&at(&root, false), &plain());
    let gitignore = std::fs::read(root.join(".gitignore")).expect("a .gitignore");

    let outcome = init(&at(&root, true), &plain());

    assert_eq!(
        outcome.said,
        vec![
            Say::Ok(
                "Control plane already fully set up (schema 1) — nothing to do."
                    .to_owned()
                    .into()
            ),
            Say::Info(
                ("  already present: charter.toml, personas/, inventory/, workspaces/, \
                 .gitignore, .gitattributes (merge rules), .claude/settings.json (env), \
                 .claude/settings.json (ask: charter report --yes), opencode.json (ask: \
                 charter report --yes), .claude/settings.json (ask: charter ws todo \
                 promote), opencode.json (ask: charter ws todo promote), \
                 .claude/settings.json (ask: purlis report --yes), opencode.json (ask: purlis \
                 report --yes), .claude/settings.json (ask: purlis ws todo promote), \
                 opencode.json (ask: purlis ws todo promote), \
                 .claude/settings.json (plane-root guard already wired)"
                    .to_owned())
                .into()
            ),
            Say::Info(NEXT.to_owned().into()),
        ]
    );
    assert_eq!(outcome.code, 0);
    assert_eq!(
        std::fs::read(root.join(".gitignore")).expect("a .gitignore"),
        gitignore
    );
}

/// `cmd_init`'s `util.warn` when `--owner` is empty, said right after `charter.toml` is
/// written and before the headline.
#[test]
fn init_without_an_owner_warns_that_the_forge_block_has_none() {
    let (_dir, root) = empty_plane();

    let outcome = init(
        &at(&root, false),
        &InitArgs {
            owner: String::new(),
            ..plain()
        },
    );

    assert_eq!(
        outcome.said.first(),
        Some(&Say::Warn(
            ("No --owner given — charter.toml's [[forge]] block has no owner/group set. Add \
             one before `purlis discover`."
                .to_owned())
            .into()
        ))
    );
    assert_eq!(outcome.code, 0);
}

/// A repo at `<dir>/<name>` with one commit and `origin` set to `origin`, for `--adopt`.
fn a_source_repo(dir: &Path, name: &str, origin: Option<&str>) -> PathBuf {
    let repo = dir.join(name);
    std::fs::create_dir_all(&repo).expect("the repo's directory");
    let git = |args: &[&str]| {
        let r = crate::testgit::run(&repo, args);
        assert!(r.ok(), "git {args:?} failed: {}", r.err);
    };
    git(&["init", "-q", "-b", "main", "."]);
    git(&["config", "user.email", "t@e.invalid"]);
    git(&["config", "user.name", "t"]);
    git(&["commit", "-q", "--allow-empty", "-m", "first"]);
    if let Some(origin) = origin {
        git(&["remote", "add", "origin", origin]);
    }
    repo
}

/// The `[[forge]]` block's `kind` and `owner`, as `init` wrote them.
fn forge_written(root: &Path) -> (String, String) {
    let toml: toml::Table = std::fs::read_to_string(root.join("charter.toml"))
        .expect("charter.toml")
        .parse()
        .expect("charter.toml is TOML");
    let block = &toml["forge"][0];
    let text = |key: &str| {
        block
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned()
    };
    (text("kind"), text("owner"))
}

/// #839: with no `--forge`, the forge and owner come from the remote of the repo the
/// project is made for.
#[test]
fn init_without_a_forge_reads_it_and_the_owner_from_the_adopted_repos_origin() {
    let (dir, root) = empty_plane();
    let repo = a_source_repo(
        dir.path(),
        "widget",
        Some("git@gitlab.com:group/sub/widget.git"),
    );

    let outcome = init(
        &at(&root, false),
        &InitArgs {
            forge: None,
            owner: String::new(),
            adopt: Some(repo),
            ..plain()
        },
    );

    assert_eq!(outcome.code, 0, "{}", Say::in_full(&outcome.said));
    assert_eq!(
        forge_written(&root),
        ("gitlab".to_owned(), "group/sub".to_owned())
    );
    assert!(
        !outcome.said.iter().any(|line| matches!(line, Say::Warn(_))),
        "an owner read from the origin is not a missing one: {:?}",
        outcome.said
    );
}

/// What the operator names wins over what the origin says.
#[test]
fn a_named_forge_and_owner_win_over_the_origin() {
    let (dir, root) = empty_plane();
    let repo = a_source_repo(
        dir.path(),
        "widget",
        Some("git@gitlab.com:group/widget.git"),
    );

    let outcome = init(
        &at(&root, false),
        &InitArgs {
            adopt: Some(repo),
            ..plain()
        },
    );

    assert_eq!(outcome.code, 0, "{}", Say::in_full(&outcome.said));
    assert_eq!(
        forge_written(&root),
        ("github".to_owned(), "acme".to_owned())
    );
}

/// #839's "else ask": no repo to read, or a remote that does not say, and `init` writes
/// nothing and asks for the forge by name.
#[test]
fn init_without_a_forge_it_cannot_read_asks_for_one_and_writes_nothing() {
    let (dir, root) = empty_plane();
    let no_origin = a_source_repo(dir.path(), "local", None);
    let elsewhere = a_source_repo(dir.path(), "hosted", Some("git@git.example.com:a/b.git"));
    for (adopt, why) in [
        (None, "no repo was named to read it from"),
        (Some(no_origin.clone()), "has no `origin` remote"),
        (Some(elsewhere), "neither github.com nor gitlab.com"),
    ] {
        let outcome = init(
            &at(&root, false),
            &InitArgs {
                forge: None,
                owner: String::new(),
                adopt,
                ..plain()
            },
        );

        assert_eq!(outcome.code, ASKS_FOR_FORGE, "{why}");
        let said = Say::in_full(&outcome.said);
        assert!(said.contains(why), "{said}");
        assert!(said.contains("--forge github"), "{said}");
        assert!(
            said.contains("--adopt"),
            "the refusal names the other way to answer: {said}"
        );
        assert!(said.contains("Nothing was written."), "{said}");
        assert!(
            outcome
                .asks_forge
                .as_deref()
                .is_some_and(|asked| asked.ends_with(why)),
            "the reason is handed over as data: {:?}",
            outcome.asks_forge
        );
        assert_eq!(
            std::fs::read_dir(&root)
                .expect("the plane's directory")
                .count(),
            0,
            "{why}: init wrote into the plane"
        );
    }
}

/// `--owner`'s help says the owner is read from the same origin: naming only the forge
/// still takes the owner from a remote on that forge, and from no other.
#[test]
fn a_named_forge_still_reads_the_owner_from_an_origin_on_that_forge() {
    for (forge, origin, owner) in [
        ("github", "git@github.com:acme/widget.git", "acme"),
        ("github", "git@gitlab.com:group/widget.git", ""),
    ] {
        let (dir, root) = empty_plane();
        let repo = a_source_repo(dir.path(), "widget", Some(origin));

        let outcome = init(
            &at(&root, false),
            &InitArgs {
                forge: Some(forge.to_owned()),
                owner: String::new(),
                adopt: Some(repo),
                ..plain()
            },
        );

        assert_eq!(outcome.code, 0, "{}", Say::in_full(&outcome.said));
        assert_eq!(
            forge_written(&root),
            (forge.to_owned(), owner.to_owned()),
            "{origin}"
        );
    }
}

/// `init` run again on a plane that has its `charter.toml` writes no forge, so it asks
/// for none.
#[test]
fn init_on_a_plane_that_has_its_charter_toml_asks_for_no_forge() {
    let (_dir, root) = empty_plane();
    assert_eq!(init(&at(&root, false), &plain()).code, 0);

    let again = init(
        &at(&root, true),
        &InitArgs {
            forge: None,
            ..plain()
        },
    );

    assert_eq!(again.code, 0, "{}", Say::in_full(&again.said));
    assert_eq!(
        forge_written(&root),
        ("github".to_owned(), "acme".to_owned())
    );
}

/// `cmd_init`'s first refusal: `unknown --forge 'x' — known kinds: github, gitlab`
/// (`sorted(_registry.KINDS)`), before anything is written.
#[test]
fn init_with_an_unknown_forge_names_the_kinds_it_knows_and_writes_nothing() {
    let (_dir, root) = empty_plane();

    let outcome = init(
        &at(&root, false),
        &InitArgs {
            forge: Some("bitbucket".to_owned()),
            ..plain()
        },
    );

    assert_eq!(
        outcome.said,
        vec![Say::Err(
            "unknown --forge 'bitbucket' — known kinds: github, gitlab"
                .to_owned()
                .into()
        )]
    );
    assert_eq!(outcome.code, 1);
    assert!(!root.join(crate::plane::MANIFEST).exists());
}

/// `_create_baseline_dirs`' FINDING C1 and `cmd_init`'s blocked branch: a FILE where
/// `inventory/` goes is named with `util.err`, left exactly as it was, everything else is
/// still made, and the run ends with `  created:` and `  already present:` and exit 1.
#[test]
fn init_names_a_file_where_a_baseline_directory_goes_and_still_makes_the_rest() {
    let (_dir, root) = empty_plane();
    std::fs::write(root.join("inventory"), "mine\n").expect("a file in the way");
    std::fs::create_dir(root.join("workspaces")).expect("a workspaces/ of its own");
    let (guard, guard_created) = guard_label(&root);

    let outcome = init(&at(&root, false), &plain());

    let mut created = "charter.toml, personas/, workspaces/.gitkeep, .gitignore, \
                       .gitattributes (merge rules), .claude/settings.json (env), \
                       .claude/settings.json (ask: charter report --yes), \
                       opencode.json (ask: charter report --yes), .claude/settings.json \
                       (ask: charter ws todo promote), opencode.json (ask: charter ws todo \
                       promote), .claude/settings.json (ask: purlis report --yes), \
                       opencode.json (ask: purlis report --yes), .claude/settings.json (ask: \
                       purlis ws todo promote), opencode.json (ask: purlis ws todo promote), \
                       personas/steward/ (front door, declared in charter.toml)"
        .to_owned();
    let mut present = "workspaces/".to_owned();
    if guard_created {
        created.push_str(&format!(", {guard}"));
    } else {
        present.push_str(&format!(", {guard}"));
    }
    assert_eq!(
        outcome.said,
        vec![
            blocker("inventory", &root.join("inventory"), "init"),
            Say::Info(format!("  created: {created}").into()),
            Say::Info(format!("  already present: {present}").into()),
        ]
    );
    assert_eq!(outcome.code, 1);
    assert_eq!(
        std::fs::read_to_string(root.join("inventory")).expect("the file"),
        "mine\n"
    );
    assert!(root.join("personas/steward/persona.md").is_file());
}

/// Stricter than Python, and the module docs say so: a `.gitignore` that is a link OUT of
/// the plane is a blocker, named once with where it lands, and nothing is read or
/// written through it. The rest of the plane is still made.
#[test]
fn a_gitignore_that_links_out_of_the_plane_is_a_blocker_and_is_never_written_through() {
    let (dir, root) = empty_plane();
    let outside = dir.path().join("outside");
    std::fs::create_dir(&outside).expect("somewhere else");
    std::fs::write(outside.join("bashrc"), "export X=1\n").expect("a file out there");
    std::os::unix::fs::symlink(outside.join("bashrc"), root.join(".gitignore"))
        .expect("a link out");
    let lands = outside.join("bashrc").canonicalize().expect("resolved");

    let outcome = init(&at(&root, false), &plain());

    assert_eq!(
        errs(&outcome),
        vec![match escape(".gitignore", &lands, "init") {
            Say::Err(e) => e,
            _ => unreachable!(),
        }]
    );
    assert_eq!(outcome.code, 1);
    assert_eq!(
        std::fs::read_to_string(outside.join("bashrc")).expect("the file"),
        "export X=1\n"
    );
    assert!(root.join(crate::plane::MANIFEST).is_file());
    assert!(
        outcome
            .said
            .iter()
            .any(|s| matches!(s, Say::Info(l) if l.starts_with("  created: charter.toml, "))),
        "{:?}",
        outcome.said
    );
}

/// A `personas` link out of the plane is asked about twice — by the baseline and by the
/// front door — and named ONCE. Nothing is scaffolded through it.
#[test]
fn a_personas_link_out_of_the_plane_is_named_once_though_two_steps_ask_about_it() {
    let (dir, root) = empty_plane();
    let outside = dir.path().join("elsewhere");
    std::fs::create_dir(&outside).expect("somewhere else");
    std::os::unix::fs::symlink(&outside, root.join("personas")).expect("a link out");
    let lands = outside.canonicalize().expect("resolved");

    let outcome = init(&at(&root, false), &plain());

    let named: Vec<&Say> = outcome
        .said
        .iter()
        .filter(|s| matches!(s, Say::Err(e) if e.starts_with("personas resolves to")))
        .collect();
    assert_eq!(named, vec![&escape("personas", &lands, "init")]);
    assert_eq!(outcome.code, 1);
    assert_eq!(
        std::fs::read_dir(&outside).expect("the directory").count(),
        0,
        "nothing is written through the link"
    );
}

/// A link that lands inside the plane's own `.git` is refused like one out of the plane:
/// nothing charter writes belongs in a repository's internals (module docs).
#[test]
fn a_gitignore_that_links_into_the_planes_own_git_is_refused_too() {
    let (_dir, root) = empty_plane();
    std::fs::create_dir_all(root.join(".git/info")).expect("a .git");
    std::fs::write(root.join(".git/info/exclude"), "# git's own\n").expect("exclude");
    std::os::unix::fs::symlink(".git/info/exclude", root.join(".gitignore"))
        .expect("a link into .git");
    let lands = root
        .join(".git/info/exclude")
        .canonicalize()
        .expect("resolved");

    let outcome = init(
        &at(&root, false),
        &InitArgs {
            plane_is_this_repo: true,
            ..plain()
        },
    );

    assert!(
        outcome.said.contains(&escape(".gitignore", &lands, "init")),
        "{:?}",
        outcome.said
    );
    assert_eq!(outcome.code, 1);
    assert_eq!(
        std::fs::read_to_string(root.join(".git/info/exclude")).expect("exclude"),
        "# git's own\n"
    );
}

/// A `.claude` FILE is one blocker, named once — the settings writers behind the gate are
/// never asked, so they cannot name it a second time. Python crashes writing under it.
#[test]
fn a_claude_file_where_the_settings_directory_goes_is_one_blocker_named_once() {
    let (_dir, root) = empty_plane();
    std::fs::write(root.join(".claude"), "not a directory\n").expect("a file in the way");

    let outcome = init(&at(&root, false), &plain());

    assert_eq!(
        errs(&outcome),
        vec![match blocker(".claude", &root.join(".claude"), "init") {
            Say::Err(e) => e,
            _ => unreachable!(),
        }]
    );
    assert_eq!(outcome.code, 1);
    assert_eq!(
        std::fs::read_to_string(root.join(".claude")).expect("the file"),
        "not a directory\n"
    );
}

/// A `.claude` DIRECTORY the operator already has is where the settings go, not a
/// blocker.
#[test]
fn a_claude_directory_of_its_own_takes_the_settings_file() {
    let (_dir, root) = empty_plane();
    std::fs::create_dir(root.join(".claude")).expect("a .claude of its own");

    let outcome = init(&at(&root, false), &plain());

    assert_eq!(outcome.code, 0, "{:?}", outcome.said);
    assert!(root.join(settings::SETTINGS).is_file());
}

/// A `.claude` link out of the plane: named, and no settings file appears at the far end.
#[test]
fn a_claude_link_out_of_the_plane_gets_no_settings_written_through_it() {
    let (dir, root) = empty_plane();
    let outside = dir.path().join("dotclaude");
    std::fs::create_dir(&outside).expect("somewhere else");
    std::os::unix::fs::symlink(&outside, root.join(".claude")).expect("a link out");
    let lands = outside.canonicalize().expect("resolved");

    let outcome = init(&at(&root, false), &plain());

    assert!(
        outcome.said.contains(&escape(".claude", &lands, "init")),
        "{:?}",
        outcome.said
    );
    assert_eq!(outcome.code, 1);
    assert_eq!(
        std::fs::read_dir(&outside).expect("the directory").count(),
        0
    );
}

/// `cmd_init`'s malformed branch: `util.warn(f"{_settings_left_untouched(p)}\n
/// {_hooks_snippet()}")`, the file left byte for byte, and exit 1 — the ONLY thing wrong,
/// so nothing else can be what fails the run.
#[test]
fn a_settings_file_that_is_not_json_is_left_untouched_and_init_exits_1() {
    let (_dir, root) = empty_plane();
    std::fs::create_dir(root.join(".claude")).expect(".claude");
    std::fs::write(root.join(settings::SETTINGS), "{not json\n").expect("settings");
    if plugin_guards(&root) {
        // `_ensure_guard_hook` answers "present" before it reads the file, as Python's
        // does; the branch under test is not reachable on this machine.
        return;
    }

    let outcome = init(&at(&root, false), &plain());

    let path = root.join(settings::SETTINGS);
    let warned = Say::Warn(
        format!(
            "{} is not a settings file purlis can read and write back as JSON — left it \
         completely untouched. Wire the plane-root guard yourself:\n{}",
            path.display(),
            settings::hooks_snippet()
        )
        .into(),
    );
    assert!(outcome.said.contains(&warned), "{:?}", outcome.said);
    assert_eq!(errs(&outcome), Vec::<&str>::new());
    assert_eq!(outcome.code, 1);
    assert_eq!(
        std::fs::read_to_string(&path).expect("settings"),
        "{not json\n"
    );
}

#[test]
fn an_opencode_file_that_cannot_take_an_ask_rule_keeps_it_out_of_every_harness() {
    // `commands._guard_apply`: every harness is asked first, and "only `malformed`
    // blocks" — Claude Code is first in the registry, so without the dry run its file
    // would already hold the rule when opencode refused. Nothing is written anywhere.
    let (_dir, root) = empty_plane();
    let opencode = root.join(settings::OPENCODE);
    std::fs::write(&opencode, "{\"permission\": \"ask\"}\n").expect("opencode.json");

    let outcome = init(&at(&root, false), &plain());

    let warned = Say::Warn(format!(
        "the ask rule for `charter report --yes` was not written anywhere — {} (`permission` is \
         not an object) is not valid, and purlis writes every harness or none. Fix it, \
         then run purlis init again",
        opencode.display()
    ).into());
    assert!(outcome.said.contains(&warned), "{:?}", outcome.said);
    let claude = std::fs::read_to_string(root.join(settings::SETTINGS)).unwrap_or_default();
    assert!(
        !claude.contains(settings::REPORT_RULE),
        "Claude Code took the rule opencode could not: {claude}"
    );
    assert_eq!(
        std::fs::read_to_string(&opencode).expect("opencode.json"),
        "{\"permission\": \"ask\"}\n"
    );
}

/// Makes `path` unwritable for the test and back again after, so the tempdir can go.
/// `None` when the process can write it anyway (root), where the scenario cannot exist.
struct ReadOnly(PathBuf);
impl ReadOnly {
    fn make(path: &Path) -> Option<Self> {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o555)).expect("chmod");
        let guard = ReadOnly(path.to_path_buf());
        let probe = path.join(".probe");
        if std::fs::write(&probe, "").is_ok() {
            let _ = std::fs::remove_file(probe);
            return None;
        }
        Some(guard)
    }
}
impl Drop for ReadOnly {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
    }
}

/// A write the OS refuses is said with the OS's own words — Python's `strerror`, not
/// Rust's `(os error 13)` — and fails the run. Python raises a traceback here; the Rust
/// contract is `_say_write_failed`'s shape ("could not write <path> (<strerror>) — left
/// untouched.") for every such write.
#[test]
fn a_plane_charter_cannot_write_into_says_what_the_os_said_and_exits_1() {
    let (_dir, root) = empty_plane();
    let Some(_ro) = ReadOnly::make(&root) else {
        return;
    };

    let outcome = init(&at(&root, false), &plain());

    let said = errs(&outcome);
    let toml = format!(
        "could not write {} (Permission denied)",
        root.join("charter.toml").display()
    );
    let personas = format!(
        "could not write {} (Permission denied) — left untouched.",
        root.join("personas").display()
    );
    let gitignore = format!(
        "could not read or write {} (Permission denied)",
        root.join(".gitignore").display()
    );
    for line in [&toml, &personas, &gitignore] {
        assert!(said.contains(&line.as_str()), "{line}\n{said:#?}");
    }
    assert!(said.iter().all(|l| !l.contains("os error")), "{said:#?}");
    assert_eq!(outcome.code, 1);
}

/// `strerror` is the text before Rust's ` (os error N)`, and a message with no such tail
/// is kept whole.
#[test]
fn strerror_is_the_oss_words_without_rusts_error_number() {
    assert_eq!(
        strerror(&std::io::Error::from_raw_os_error(2)),
        "No such file or directory"
    );
    assert_eq!(
        strerror(&std::io::Error::other("it is not UTF-8 text")),
        "it is not UTF-8 text"
    );
}

#[cfg(target_os = "macos")]
#[test]
fn a_refusal_through_a_link_names_the_link_and_the_file_it_leads_to() {
    // #1421: the caller names the link; the file refused is the one it leads to.
    let dir = tempfile::tempdir().expect("a directory");
    let kept = dir.path().join("kept");
    std::fs::create_dir(&kept).expect("a folder");
    let target = kept.join("ignore");
    std::fs::write(&target, "old\n").expect("the file");
    let link = dir.path().join(".gitignore");
    std::os::unix::fs::symlink(&target, &link).expect("a link");
    let _frozen = crate::rewrite::frozen::Frozen::at(&kept);

    let said = replace_text(&link, "new\n").expect_err("refused");

    let real = crate::contain::resolved(&link).expect("it resolves");
    assert!(said.starts_with("Operation not permitted"), "{said}");
    assert!(said.contains(&real.display().to_string()), "{said}");
    assert!(said.ends_with("where the link leads"), "{said}");
}

#[test]
fn a_refusal_already_naming_its_path_is_said_without_it() {
    // #1359: `init`'s `.gitignore` line named the path, then the rewording named it again.
    let path = Path::new("/project/.gitignore");
    let refused = crate::rewrite::refused_write(std::io::Error::from_raw_os_error(1), path, false);
    assert_eq!(strerror(&refused), "Operation not permitted");
    // #1421: in a sandboxed chat it still says whose refusal it was.
    let sandboxed = crate::rewrite::refused_write(std::io::Error::from_raw_os_error(1), path, true);
    assert_eq!(
        strerror(&sandboxed),
        "Operation not permitted: this chat's sandbox refused it"
    );
}

/// `cli._plane_refusal`: a `charter.toml` from a newer charter stops `init` AND `reinit`
/// before anything is written, with the one line `cli.main` prints.
#[test]
fn a_plane_from_a_newer_charter_is_refused_by_init_and_reinit_alike() {
    let (_dir, root) = empty_plane();
    std::fs::write(root.join("charter.toml"), "schema = 3\n").expect("charter.toml");
    let refusal = vec![Say::Err(
        format!(
            // The app's own wording (ADR 0045): Python's pointed at `uv tool install
            // charter-cp`, which this charter never ships on. A declared divergence.
            "{} declares schema 3, but this purlis understands 2. Upgrade purlis: update the \
         app. Nothing was run. `purlis doctor` reports it; `purlis update` is the way out.",
            root.join("charter.toml").display()
        )
        .into(),
    )];

    let first = init(&at(&root, true), &plain());
    let second = reinit(&at(&root, true));

    assert_eq!((first.said, first.code), (refusal.clone(), 1));
    assert_eq!((second.said, second.code), (refusal, 1));
    assert!(!root.join("personas").exists());
    assert!(!root.join(".gitignore").exists());
}

/// `cmd_reinit` outside a plane: its one `util.err`, and nothing scaffolded into whatever
/// directory happened to be the cwd.
#[test]
fn reinit_outside_a_plane_refuses_and_writes_nothing() {
    let (_dir, root) = empty_plane();

    let outcome = reinit(&at(&root, false));

    assert_eq!(
        outcome.said,
        vec![Say::Err(
            ("no control plane found (no charter.toml here or in any parent) — `purlis \
             reinit` only works inside one."
                .to_owned())
            .into()
        )]
    );
    assert_eq!(outcome.code, 1);
    assert!(!root.join("personas").exists());
}

/// `cmd_reinit` on a plane `init` just made: `Up to date (schema 1) — nothing to do.`,
/// and nothing else.
#[test]
fn reinit_on_a_current_plane_is_up_to_date() {
    let (_dir, root) = empty_plane();
    init(&at(&root, false), &plain());

    let outcome = reinit(&at(&root, true));

    assert_eq!(
        outcome.said,
        vec![Say::Ok(
            "Up to date (schema 1) — nothing to do.".to_owned().into()
        )]
    );
    assert_eq!(outcome.code, 0);
}

/// `cmd_reinit` healing a plane that predates a directory and the profiles ignore line:
/// `Reinitialized control plane → added …`, then what it found, and the line appended
/// under `_ensure_local_profiles_ignored`'s header with the rest of the file kept.
/// #355: the baseline `.gitignore` un-ignores `workspaces/.gitkeep`, and the LIVE block is
/// spliced in after that line, so the file it names exists — part of making `workspaces/`,
/// and said on its own when a plane made before this is healed.
#[test]
fn init_creates_workspaces_gitkeep_so_the_anchor_it_unignores_exists() {
    let (_dir, root) = empty_plane();
    init(&at(&root, false), &plain());
    assert!(root.join("workspaces/.gitkeep").is_file());

    std::fs::remove_file(root.join("workspaces/.gitkeep")).expect("an older plane");
    let outcome = reinit(&at(&root, true));

    assert!(root.join("workspaces/.gitkeep").is_file());
    assert_eq!(
        outcome.said[0],
        Say::Ok(
            "Reinitialized control plane → added workspaces/.gitkeep."
                .to_owned()
                .into()
        ),
        "{:?}",
        outcome.said
    );
    assert_eq!(outcome.code, 0);
}

/// `.gitkeep` is created, never truncated: a plane whose `.gitkeep` holds something keeps it.
#[test]
fn a_workspaces_gitkeep_that_is_there_is_left_as_it_is() {
    let (_dir, root) = empty_plane();
    init(&at(&root, false), &plain());
    std::fs::write(root.join("workspaces/.gitkeep"), "mine\n").expect("a note");
    reinit(&at(&root, true));
    assert_eq!(
        std::fs::read_to_string(root.join("workspaces/.gitkeep")).expect("read"),
        "mine\n"
    );
}

/// #440: a `workspaces/.gitkeep` that is a link is reported as a write failure, as a
/// persona's is (#434) — not taken as present — and what it points at is untouched. A
/// link out of the plane was already refused by the gate; these are the two it let by: a
/// link that stays inside the plane, and a dangling one.
#[cfg(unix)]
#[test]
fn a_workspaces_gitkeep_that_is_a_link_is_reported_and_its_target_is_untouched() {
    let (_dir, root) = empty_plane();
    init(&at(&root, false), &plain());
    let keep = root.join("workspaces/.gitkeep");
    let notes = root.join("notes.txt");
    std::fs::write(&notes, "mine\n").expect("a file inside");
    for target in [notes.clone(), root.join("gone")] {
        std::fs::remove_file(&keep).expect("unlink");
        std::os::unix::fs::symlink(&target, &keep).expect("a link");

        let outcome = reinit(&at(&root, true));

        assert_ne!(outcome.code, 0, "{}: {:?}", target.display(), outcome.said);
        assert!(
            outcome.said.iter().any(|said| matches!(
                said,
                Say::Err(text) if text.contains("workspaces/.gitkeep")
            )),
            "{:?}",
            outcome.said
        );
        assert!(keep.is_symlink());
    }
    assert_eq!(std::fs::read_to_string(&notes).expect("notes"), "mine\n");
    assert!(!root.join("gone").exists(), "nothing made where it points");
}

#[test]
fn reinit_heals_what_a_plane_predates_and_names_each_thing_it_added() {
    let (_dir, root) = empty_plane();
    init(&at(&root, false), &plain());
    std::fs::remove_dir(root.join("inventory")).expect("an old plane");
    let old = GITIGNORE_BASELINE.replace("/charter.local.toml\n", "");
    std::fs::write(root.join(".gitignore"), &old).expect("an old .gitignore");

    let outcome = reinit(&at(&root, true));

    assert_eq!(
        outcome.said,
        vec![
            Say::Ok(
                ("Reinitialized control plane → added inventory/, .gitignore \
                 (/charter.local.toml)."
                    .to_owned())
                .into()
            ),
            Say::Info(
                ("  already present: personas/, workspaces/, .gitattributes (merge rules), \
                 .claude/settings.json (plane-root guard already wired)"
                    .to_owned())
                .into()
            ),
        ]
    );
    assert_eq!(outcome.code, 0);
    assert_eq!(
        std::fs::read_to_string(root.join(".gitignore")).expect("a .gitignore"),
        format!(
            "{}\n\n# added by `charter reinit` — harness profiles stay on this machine\n\
             /charter.local.toml\n",
            old.trim_end_matches('\n')
        )
    );
}

/// `cmd_reinit` on a bare `charter.toml`: everything is added, and when nothing was found
/// there is no "already present" line at all.
#[test]
fn reinit_on_a_bare_manifest_adds_everything_and_lists_nothing_as_present() {
    let (_dir, root) = empty_plane();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").expect("charter.toml");
    let (guard, guard_created) = guard_label(&root);

    let outcome = reinit(&at(&root, true));

    let mut added = "personas/, inventory/, workspaces/, .gitignore (/.charter/, /.purlis/), \
                     .gitignore (/charter.local.toml, /purlis.local.toml), \
                     .gitignore (/sessions/), .gitignore (/skills.local/, /workspaces/*/skills.local/, /personas/*/skills.local/, /skills.local.lock), .gitattributes (merge rules), .claude/settings.json (env), \
                     .claude/settings.json (ask: charter report --yes), opencode.json (ask: \
                     charter report --yes), .claude/settings.json (ask: charter ws todo \
                     promote), opencode.json (ask: charter ws todo promote), \
                     .claude/settings.json (ask: purlis report --yes), opencode.json (ask: \
                     purlis report --yes), .claude/settings.json (ask: purlis ws todo promote), \
                     opencode.json (ask: purlis ws todo promote)"
        .to_owned();
    let mut said = Vec::new();
    if guard_created {
        added.push_str(&format!(", {guard}"));
    }
    said.push(Say::Ok(
        format!("Reinitialized control plane → added {added}.").into(),
    ));
    if !guard_created {
        said.push(Say::Info(format!("  already present: {guard}").into()));
    }
    assert_eq!(outcome.said, said);
    assert_eq!(outcome.code, 0);
}

/// The harness variable in the committed settings is spelled for the plane (RN-2z): a
/// migrated plane gets the purlis name, an unmigrated one keeps the name older builds read.
#[test]
fn reinit_names_the_harness_variable_as_the_plane_is_spelled() {
    for (manifest, wanted, not) in [
        ("purlis.toml", "PURLIS_HARNESS", "CHARTER_HARNESS"),
        ("charter.toml", "CHARTER_HARNESS", "PURLIS_HARNESS"),
    ] {
        let (_dir, root) = empty_plane();
        std::fs::write(root.join(manifest), "schema = 1\n").expect("a manifest");

        reinit(&at(&root, true));

        let written = std::fs::read_to_string(root.join(settings::SETTINGS)).expect("settings");
        assert!(written.contains(wanted), "{manifest}: {written}");
        assert!(!written.contains(not), "{manifest}: {written}");
    }
}

/// `cmd_reinit`'s blocked branch: the blocker in `reinit`'s words, then `  created:` and
/// `  already present:`, exit 1.
#[test]
fn reinit_names_a_file_where_a_baseline_directory_goes_and_still_heals_the_rest() {
    let (_dir, root) = empty_plane();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").expect("charter.toml");
    std::fs::create_dir(root.join("personas")).expect("personas/");
    std::fs::write(root.join("inventory"), "mine\n").expect("a file in the way");
    let (guard, guard_created) = guard_label(&root);

    let outcome = reinit(&at(&root, true));

    let mut created =
        "workspaces/, .gitignore (/.charter/, /.purlis/), .gitignore (/charter.local.toml, \
         /purlis.local.toml), .gitignore (/sessions/), .gitignore \
         (/skills.local/, /workspaces/*/skills.local/, /personas/*/skills.local/, /skills.local.lock), .gitattributes (merge rules), \
         .claude/settings.json (env), .claude/settings.json (ask: charter report --yes), \
         opencode.json (ask: charter report --yes), .claude/settings.json (ask: charter ws \
         todo promote), opencode.json (ask: charter ws todo promote), \
         .claude/settings.json (ask: purlis report --yes), opencode.json (ask: purlis report \
         --yes), .claude/settings.json (ask: purlis ws todo promote), opencode.json (ask: \
         purlis ws todo promote)"
            .to_owned();
    let mut present = "personas/".to_owned();
    if guard_created {
        created.push_str(&format!(", {guard}"));
    } else {
        present.push_str(&format!(", {guard}"));
    }
    assert_eq!(
        outcome.said,
        vec![
            blocker("inventory", &root.join("inventory"), "reinit"),
            Say::Info(format!("  created: {created}").into()),
            Say::Info(format!("  already present: {present}").into()),
        ]
    );
    assert_eq!(outcome.code, 1);
}

/// `reinit` refuses a `.gitignore` that links out of the plane exactly as `init` does,
/// in its own command's words.
#[test]
fn reinit_refuses_a_gitignore_that_links_out_of_the_plane() {
    let (dir, root) = empty_plane();
    init(&at(&root, false), &plain());
    std::fs::remove_file(root.join(".gitignore")).expect("the plane's own");
    let outside = dir.path().join("elsewhere.txt");
    std::fs::write(&outside, "keep\n").expect("a file out there");
    std::os::unix::fs::symlink(&outside, root.join(".gitignore")).expect("a link out");
    let lands = outside.canonicalize().expect("resolved");

    let outcome = reinit(&at(&root, true));

    assert_eq!(
        outcome.said.first(),
        Some(&escape(".gitignore", &lands, "reinit"))
    );
    assert_eq!(outcome.code, 1);
    assert_eq!(
        std::fs::read_to_string(&outside).expect("the file"),
        "keep\n"
    );
}

/// A settings directory the OS will not let charter write into: each settings write that
/// was attempted is said, and `reinit` exits 1 rather than reporting the plane healed.
#[test]
fn reinit_says_each_settings_write_the_os_refused_and_exits_1() {
    let (_dir, root) = empty_plane();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").expect("charter.toml");
    std::fs::create_dir(root.join(".claude")).expect(".claude");
    let plugin = plugin_guards(&root);
    let Some(_ro) = ReadOnly::make(&root.join(".claude")) else {
        return;
    };

    let outcome = reinit(&at(&root, true));

    let line = format!(
        "could not write {} (Permission denied) — left untouched.",
        root.join(settings::SETTINGS).display()
    );
    // `ensure_env`, and `ensure_guard_hook` unless the plugin already runs the guard.
    let expected = if plugin { 1 } else { 2 };
    assert_eq!(
        errs(&outcome).iter().filter(|l| **l == line).count(),
        expected,
        "{:?}",
        outcome.said
    );
    assert_eq!(outcome.code, 1);
}

/// #358's neighbours: `init` and `reinit` add their lines to the plane's `.gitignore` and
/// `.gitattributes` by replacing the file, so a crash between the write and the rename
/// leaves the file as it was rather than cut short.
#[test]
fn init_s_own_edits_to_gitignore_and_gitattributes_are_whole_or_not_at_all() {
    let dir = tempfile::tempdir().expect("a directory");
    let ignore = dir.path().join(".gitignore");
    let attributes = dir.path().join(".gitattributes");
    std::fs::write(&ignore, "build/\n").expect("a .gitignore");
    std::fs::write(&attributes, "*.png binary\n").expect("a .gitattributes");
    let calls = std::rc::Rc::new(std::cell::Cell::new(0));
    let counted = calls.clone();
    let _hook = crate::rewrite::hook::set(move |_, _| {
        counted.set(counted.get() + 1);
        Err(std::io::Error::other("killed before the rename"))
    });

    assert!(ensure_gitignore(&ignore).is_err());
    assert!(ensure_gitattributes(&attributes).is_err());

    assert_eq!(calls.get(), 2, "both went through the temp-and-rename");
    assert_eq!(std::fs::read_to_string(&ignore).expect("read"), "build/\n");
    assert_eq!(
        std::fs::read_to_string(&attributes).expect("read"),
        "*.png binary\n"
    );
    assert_eq!(std::fs::read_dir(dir.path()).expect("list").count(), 2);
}

/// A plane has its state folder and local settings ignored under both their names (RN-2a).
#[test]
fn the_state_folder_and_local_file_are_ignored_under_both_names() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".gitignore");
    std::fs::create_dir(dir.path().join(".purlis")).unwrap();
    std::fs::write(dir.path().join("purlis.local.toml"), "").unwrap();
    std::fs::write(&path, "# mine\n").unwrap();

    assert_eq!(ensure_gitignore(&path), Ok(true));

    let body = std::fs::read_to_string(&path).unwrap();
    let lines = text::stripped_lines(&body);
    // Both names, always: a line for a name not in use is inert, and a leftover under it
    // would otherwise be committed.
    for line in [
        "/.purlis/",
        "/.charter/",
        "/purlis.local.toml",
        "/charter.local.toml",
    ] {
        assert!(lines.contains(&line), "{line}: {body}");
    }
}

/// `commands._ensure_gitignore` against a file that has every rule — `.charter/` found
/// by Python's substring test — writes nothing and answers `False`.
#[test]
fn a_gitignore_with_every_rule_is_left_byte_for_byte() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join(".gitignore");
    let body = "build/\n!/workspaces/.gitkeep\nkeep/.charter/out\nkeep/.purlis/out\n\
                /.claude/settings.local.json\n/charter.local.toml\n/purlis.local.toml\n\
                /sessions/\n/skills.local/\n/workspaces/*/skills.local/\n/personas/*/skills.local/\n/skills.local.lock\n";
    std::fs::write(&path, body).expect("a .gitignore");

    assert_eq!(ensure_gitignore(&path), Ok(false));
    assert_eq!(std::fs::read_to_string(&path).expect("read"), body);
}

/// `commands._ensure_gitignore` appends exactly the lines it is missing, in its order,
/// under one `# added by \`charter init\`` header (`util.append_gitignore`), trailing
/// blank lines collapsed to the one before the block.
#[test]
fn a_gitignore_missing_the_local_files_gets_only_those_appended() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join(".gitignore");
    std::fs::write(
        &path,
        "node_modules/\n!/workspaces/.gitkeep\n/.charter/\n\n\n",
    )
    .expect("a .gitignore");

    assert_eq!(ensure_gitignore(&path), Ok(true));
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "node_modules/\n!/workspaces/.gitkeep\n/.charter/\n\n# added by `charter \
         init`\n/.purlis/\n/.claude/settings.local.json\n/charter.local.toml\n\
         /purlis.local.toml\n/sessions/\n/skills.local/\n/workspaces/*/skills.local/\n/personas/*/skills.local/\n/skills.local.lock\n"
    );
}

/// The other half: the workspace anchor missing brings BOTH workspace lines, and a
/// `.charter/` anywhere in the body is enough to skip `/.charter/`.
#[test]
fn a_gitignore_missing_the_workspace_anchor_gets_both_workspace_lines() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join(".gitignore");
    std::fs::write(
        &path,
        "# no .charter/ or .purlis/ here, please\n/.claude/settings.local.json\n\
         /charter.local.toml\n/purlis.local.toml\n/sessions/\n/skills.local/\n\
         /workspaces/*/skills.local/\n/personas/*/skills.local/\n/skills.local.lock\n",
    )
    .expect("a .gitignore");

    assert_eq!(ensure_gitignore(&path), Ok(true));
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "# no .charter/ or .purlis/ here, please\n/.claude/settings.local.json\n\
         /charter.local.toml\n/purlis.local.toml\n/sessions/\n/skills.local/\n\
         /workspaces/*/skills.local/\n/personas/*/skills.local/\n/skills.local.lock\n\n# added by `charter init`\n/workspaces/*/*\n\
         !/workspaces/.gitkeep\n"
    );
}

/// A plane-root chat's session records stay on this machine, the way a LOCAL workspace's
/// do (ADR 0064): a fresh plane ignores them.
#[test]
fn a_fresh_plane_keeps_the_plane_roots_session_records_on_this_machine() {
    assert!(
        GITIGNORE_BASELINE
            .lines()
            .any(|line| line == PLANE_SESSIONS_IGNORE),
        "{GITIGNORE_BASELINE}"
    );
    assert_eq!(PLANE_SESSIONS_IGNORE, "/sessions/");
}

/// And a plane made before them gains the line from `init` run again.
#[test]
fn a_gitignore_missing_the_plane_roots_session_records_gets_that_line_appended() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join(".gitignore");
    let body = "!/workspaces/.gitkeep\n/.charter/\n/.purlis/\n/.claude/settings.local.json\n\
                /charter.local.toml\n/purlis.local.toml\n";
    std::fs::write(&path, body).expect("a .gitignore");

    assert_eq!(ensure_gitignore(&path), Ok(true));
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        format!(
            "{body}\n# added by `charter init`\n/sessions/\n/skills.local/\n\
             /workspaces/*/skills.local/\n/personas/*/skills.local/\n/skills.local.lock\n"
        )
    );
}

/// ...and from `reinit`, under a header of its own that says why.
#[test]
fn reinit_keeps_the_plane_roots_session_records_on_this_machine() {
    let (_dir, root) = empty_plane();
    init(&at(&root, false), &plain());
    let old = GITIGNORE_BASELINE.replace(&format!("{PLANE_SESSIONS_IGNORE}\n"), "");
    std::fs::write(root.join(".gitignore"), &old).expect("an old .gitignore");

    let outcome = reinit(&at(&root, true));

    assert_eq!(outcome.code, 0);
    assert!(
        matches!(&outcome.said[0], Say::Ok(text) if text.contains(".gitignore (/sessions/)")),
        "{:?}",
        outcome.said
    );
    let text = std::fs::read_to_string(root.join(".gitignore")).expect("a .gitignore");
    assert!(
        text.ends_with(
            "\n\n# added by `charter reinit` — the plane root's session records stay on \
             this machine\n/sessions/\n"
        ),
        "{text}"
    );
}

/// No `.gitignore` at all: the whole baseline, and `True`.
#[test]
fn an_absent_gitignore_is_written_as_the_whole_baseline() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join(".gitignore");

    assert_eq!(ensure_gitignore(&path), Ok(true));
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        GITIGNORE_BASELINE
    );
}

/// A plane with one declared profile of `kind` whose command is nowhere on this machine,
/// approved or not — so nothing can run it, and anything that tried would say so.
fn plane_with_a_profile(
    kind: &str,
    approved: bool,
) -> (tempfile::TempDir, PathBuf, crate::profiles::Profile) {
    let (dir, root) = empty_plane();
    std::fs::write(root.join("charter.toml"), "schema = 1\n").expect("charter.toml");
    let missing = dir.path().join("nowhere").join(kind);
    std::fs::write(
        root.join("charter.local.toml"),
        format!(
            "[harness.work]\nkind = {kind:?}\ncommand = [{:?}]\n",
            missing.display().to_string()
        ),
    )
    .expect("charter.local.toml");
    let profile = crate::profiles::current(&root)
        .get("work")
        .cloned()
        .expect("the profile is declared");
    if approved {
        crate::profiletrust::record_launched(
            &root,
            "work",
            &crate::profiletrust::fingerprint(&profile),
        )
        .expect("approved");
    }
    (dir, root, profile)
}

fn line_about_work(outcome: &Outcome) -> Vec<&Say> {
    outcome
        .said
        .iter()
        .filter(|s| matches!(s, Say::Info(l) | Say::Warn(l) if l.starts_with("  profile 'work'")))
        .collect()
}

/// `init` installs nothing into a profile's config folder any more — the app arms each
/// chat with its own plugin (`crate::plugin`) — so an approved profile gets no line at
/// all, whatever its kind, and nothing is run to find out.
#[test]
fn init_and_reinit_say_nothing_about_an_approved_profile_of_any_kind() {
    for kind in ["claude", "codex", "opencode"] {
        let (_dir, root, _profile) = plane_with_a_profile(kind, true);
        assert!(
            line_about_work(&init(&at(&root, true), &plain())).is_empty(),
            "init spoke about an approved {kind} profile"
        );
        assert!(
            line_about_work(&reinit(&at(&root, true))).is_empty(),
            "reinit spoke about an approved {kind} profile"
        );
    }
}

/// What still stops a chat is a command nobody approved, and that IS said — as a warning,
/// with where it is approved.
#[test]
fn a_profile_nobody_approved_is_named_with_the_way_to_approve_it() {
    let (_dir, root, _profile) = plane_with_a_profile("claude", false);

    let outcome = reinit(&at(&root, true));

    assert_eq!(
        line_about_work(&outcome),
        vec![&Say::Warn(
            ("  profile 'work' is new and not approved yet — start a chat on it from the \
             app's new-chat picker to approve its command"
                .to_owned())
            .into()
        )]
    );
}

// ---------------------------------------------------------------------------------------
// The front door (`commands._ensure_front_door`).
// ---------------------------------------------------------------------------------------

/// The persona file is `commands._FRONT_DOOR.format(name=..., role=...)` byte for byte.
/// The fixture is that call's output from the Python charter, for `front-door.2`, whose
/// role `str.title()` makes `Front Door.2` — less its `routing: advise` line, which is
/// retired (charter#369), and with its routing section pointing at the plane's personas
/// instead of saying there are none, which a project template made untrue (#882).
#[test]
fn the_front_door_is_the_pythons_template_less_routing_and_is_declared() {
    let (_dir, root) = empty_plane();

    let outcome = init(
        &at(&root, false),
        &InitArgs {
            front_door: Some("front-door.2".to_owned()),
            ..plain()
        },
    );

    assert_eq!(outcome.code, 0, "{:?}", outcome.said);
    let dir = root.join("personas/front-door.2");
    assert_eq!(
        std::fs::read_to_string(dir.join("persona.md")).expect("persona.md"),
        include_str!("testdata/front-door.md")
    );
    assert!(dir.join("memory/.gitkeep").is_file());
    assert!(dir.join("refs/.gitkeep").is_file());
    let toml = std::fs::read_to_string(root.join("charter.toml")).expect("charter.toml");
    assert!(toml.contains("default = \"front-door.2\""), "{toml}");
}

fn scaffolded_a_front_door(outcome: &Outcome) -> bool {
    outcome.said.iter().any(|s| {
        matches!(s, Say::Info(l) if l == "  + personas/steward/ (front door, declared in charter.toml)")
    })
}

/// `personas.glob("*.md")` counts only markdown: a README of another kind is not a
/// roster, so the front door is still made.
#[test]
fn a_personas_directory_holding_only_a_non_markdown_file_still_gets_a_front_door() {
    let (_dir, root) = empty_plane();
    std::fs::create_dir(root.join("personas")).expect("personas/");
    std::fs::write(root.join("personas/README.txt"), "notes\n").expect("a note");

    let outcome = init(&at(&root, false), &plain());

    assert!(scaffolded_a_front_door(&outcome), "{:?}", outcome.said);
    assert!(root.join("personas/steward/persona.md").is_file());
}

/// `personas.glob("*/persona.md")`: a persona of its own is a roster, and none is added.
#[test]
fn a_plane_with_a_persona_of_its_own_gets_no_front_door() {
    let (_dir, root) = empty_plane();
    std::fs::create_dir_all(root.join("personas/alice")).expect("alice/");
    std::fs::write(
        root.join("personas/alice/persona.md"),
        "---\nname: alice\n---\n",
    )
    .expect("alice");

    let outcome = init(&at(&root, false), &plain());

    assert_eq!(outcome.code, 0, "{:?}", outcome.said);
    assert!(!scaffolded_a_front_door(&outcome), "{:?}", outcome.said);
    assert!(!root.join("personas/steward").exists());
}

/// `_instance.default_persona_of(...)`: a declared default is a question somebody already
/// answered.
#[test]
fn a_plane_that_declares_a_default_persona_gets_no_front_door() {
    let (_dir, root) = empty_plane();
    std::fs::write(
        root.join("charter.toml"),
        "schema = 1\n\n[persona]\ndefault = \"alice\"\n",
    )
    .expect("charter.toml");

    let outcome = init(&at(&root, true), &plain());

    assert_eq!(outcome.code, 0, "{:?}", outcome.said);
    assert!(!root.join("personas/steward").exists());
}

/// Stricter than Python, as `front_door` documents: a `charter.toml` that is not TOML
/// makes Python's `instance.load` raise and `init` end in a traceback. Here it is said,
/// no front door is scaffolded over it, and the run fails.
#[test]
fn a_manifest_that_is_not_toml_gets_no_front_door_and_fails_the_run() {
    let (_dir, root) = empty_plane();
    std::fs::write(root.join("charter.toml"), "this is = not [toml\n").expect("toml");

    let outcome = init(&at(&root, true), &plain());

    assert!(
        errs(&outcome)
            .iter()
            .any(|e| e.ends_with(" — no front door was scaffolded.")),
        "{:?}",
        outcome.said
    );
    assert_eq!(outcome.code, 1);
    assert!(!root.join("personas/steward").exists());
}

/// Every path the front door would write is gated, not only its directory: a
/// `memory` link out of the plane stops the whole scaffold, and nothing lands out there.
#[test]
fn a_front_door_whose_memory_links_out_of_the_plane_is_not_scaffolded_through_it() {
    let (dir, root) = empty_plane();
    let outside = dir.path().join("elsewhere");
    std::fs::create_dir(&outside).expect("somewhere else");
    std::fs::create_dir_all(root.join("personas/steward")).expect("steward/");
    std::os::unix::fs::symlink(&outside, root.join("personas/steward/memory")).expect("a link out");

    let outcome = init(&at(&root, false), &plain());

    assert_eq!(outcome.code, 1, "{:?}", outcome.said);
    assert!(
        errs(&outcome)
            .iter()
            .any(|e| e.starts_with("personas/steward/memory/.gitkeep resolves to")),
        "{:?}",
        outcome.said
    );
    assert!(!root.join("personas/steward/persona.md").exists());
    assert_eq!(
        std::fs::read_dir(&outside).expect("the directory").count(),
        0
    );
}

/// #434: the `.gitkeep` touch is `create_new`. A link at it — planted after the gate
/// answered — is refused, and what it points at is neither created nor appended to.
#[test]
fn a_gitkeep_that_is_a_link_is_refused_and_its_target_is_untouched() {
    let (dir, root) = empty_plane();
    let theirs = dir.path().join("theirs");
    std::fs::write(&theirs, "THEIRS\n").expect("a file outside");
    let keep = root.join(".gitkeep");
    std::os::unix::fs::symlink(&theirs, &keep).expect("a link");

    assert!(touch(&keep).is_err(), "a linked .gitkeep is refused");
    assert_eq!(
        std::fs::read_to_string(&theirs).expect("theirs"),
        "THEIRS\n"
    );

    let gone = dir.path().join("gone");
    std::fs::remove_file(&keep).expect("unlink");
    std::os::unix::fs::symlink(&gone, &keep).expect("a dangling link");
    assert!(touch(&keep).is_err(), "a dangling link is refused too");
    assert!(!gone.exists(), "nothing was created where the link points");

    // A plain file already there is left exactly as it is.
    std::fs::remove_file(&keep).expect("unlink");
    std::fs::write(&keep, "mine\n").expect("a note");
    touch(&keep).expect("an existing .gitkeep is fine");
    assert_eq!(std::fs::read_to_string(&keep).expect("kept"), "mine\n");
}

/// #434: the front door's `persona.md` goes through `rewrite::replace`, so a link planted
/// at it after the gate answered is replaced by the file, never written through.
#[test]
fn a_link_planted_at_the_front_doors_persona_md_is_never_written_through() {
    let (dir, root) = empty_plane();
    let theirs = dir.path().join("theirs");
    std::fs::write(&theirs, "THEIRS\n").expect("a file outside");
    let fired = std::rc::Rc::new(std::cell::Cell::new(false));
    let seen = std::rc::Rc::clone(&fired);
    let planted = theirs.clone();
    let _unset = crate::rewrite::hook::set(move |target, _| {
        if target.ends_with("persona.md") {
            seen.set(true);
            std::os::unix::fs::symlink(&planted, target)?;
        }
        Ok(())
    });

    let outcome = init(&at(&root, false), &plain());

    assert!(fired.get(), "persona.md went through rewrite::replace");
    assert_eq!(outcome.code, 0, "{:?}", outcome.said);
    assert_eq!(
        std::fs::read_to_string(&theirs).expect("theirs"),
        "THEIRS\n"
    );
    assert!(!root.join("personas/steward/persona.md").is_symlink());
}

// ---------------------------------------------------------------------------------------
// Inside a git repository: ADR 0035's refusal, and `_first_clone_step`'s offer.
// ---------------------------------------------------------------------------------------

fn git(dir: &Path, args: &[&str]) {
    let r = crate::testgit::run(dir, args);
    assert!(r.ok(), "git {args:?} failed: {}", r.err);
}

/// A directory named `plane` that is the top of a git repo whose `origin` is `widget` —
/// `_in_a_git_repository` in the differential run, which names the same repo.
fn a_repo(origin: &str) -> (tempfile::TempDir, PathBuf) {
    let (dir, root) = empty_plane();
    git(&root, &["init", "-q", "-b", "main", "."]);
    git(&root, &["remote", "add", "origin", origin]);
    (dir, root)
}

/// ADR 0035 / spec decision 27, the declared divergence: `init` at the top of a repo
/// writes nothing and says how to ask. The recorded `init-*` scenarios hold the CLI to the
/// lines byte for byte (ADR 0046).
#[test]
fn init_at_the_top_of_a_repository_writes_nothing_and_says_how_to_ask() {
    let (_dir, root) = a_repo("git@github.com:acme/widget.git");

    let outcome = init(&at(&root, false), &plain());

    assert_eq!(
        outcome.said,
        vec![
            Say::Err(
                ("this is the git repo 'widget', and `purlis init` does not make a \
                 repository into a control plane unless you ask it to. Nothing was \
                 written."
                    .to_owned())
                .into()
            ),
            Say::Info(
                ("A plane is a directory of its own, and this repo is the first clone in \
                 it:\n      mkdir ../widget-plane && cd ../widget-plane\n      purlis \
                 init --forge github --owner acme --adopt ../plane\n  That clones this \
                 repo into the plane's first workspace. Nothing here is written by any of \
                 it — this repo is read, and only read."
                    .to_owned())
                .into()
            ),
            Say::Info(
                ("To make THIS repo the plane instead — purlis's own plane is one, which \
                 is why the option is here — ask for it by name:\n      purlis init \
                 --plane-is-this-repo --forge github --owner acme\n  That writes \
                 charter.toml, .claude/settings.json, opencode.json, personas/, \
                 inventory/, workspaces/ into this repo, and purlis's own rules into its \
                 tracked .gitignore."
                    .to_owned())
                .into()
            ),
            Say::Info(
                ("Why the default changed: docs/adr/0035-a-plane-is-untrusted-until-the-\
                 operator-opens-it.md, and charter-app spec decision 27. `purlis init` \
                 anywhere that is not the top of a git repo is unchanged."
                    .to_owned())
                .into()
            ),
        ]
    );
    assert_eq!(outcome.code, 1);
    let left: Vec<_> = std::fs::read_dir(&root)
        .expect("the repo")
        .map(|e| e.expect("an entry").file_name())
        .collect();
    assert_eq!(left, vec![std::ffi::OsString::from(".git")]);
}

/// The flags the refusal prints back are the ones typed, as a shell reads them: the
/// front-door flag only when it is not the default.
#[test]
fn the_refusal_prints_back_only_the_flags_that_were_typed() {
    assert_eq!(typed_flags(&plain()), " --forge github --owner acme");
    assert_eq!(
        typed_flags(&InitArgs {
            owner: String::new(),
            host: Some("git.example.com".to_owned()),
            front_door: None,
            ..plain()
        }),
        " --forge github --host git.example.com --no-front-door"
    );
    assert_eq!(
        typed_flags(&InitArgs {
            front_door: Some("door".to_owned()),
            ..plain()
        }),
        " --forge github --owner acme --front-door door"
    );
}

/// `--plane-is-this-repo` is the old default asked for by name: the plane is made in the
/// repo, and `_first_clone_step` offers the first clone in `_offer_first_clone`'s words.
#[test]
fn plane_is_this_repo_scaffolds_the_repo_and_offers_its_first_clone() {
    let (_dir, root) = a_repo("git@github.com:acme/widget.git");

    let outcome = init(
        &at(&root, false),
        &InitArgs {
            plane_is_this_repo: true,
            ..plain()
        },
    );

    assert_eq!(outcome.code, 0, "{:?}", outcome.said);
    assert_eq!(
        outcome.said.last(),
        Some(&Say::Info(
            ("You are standing in the git repo 'widget'. Work happens in a workspace, not \
             in the plane root — clone it into the first one:\n      purlis init \
             --clone-this-repo\n  Nothing is cloned unless you run that. It lands in \
             workspaces/default/widget/, and declining leaves this plane complete."
                .to_owned())
            .into()
        ))
    );
    assert!(root.join("charter.toml").is_file());
}

/// A repository that already holds a `charter.toml` is a plane: `init` heals it rather
/// than refusing, and the offer names the workspace the plane declares
/// (`config.DEFAULT_WORKSPACE`, re-derived by `config.use(root)`).
#[test]
fn a_repository_that_is_already_a_plane_is_healed_and_offered_its_declared_workspace() {
    let (_dir, root) = a_repo("git@github.com:acme/widget.git");
    std::fs::write(
        root.join("charter.toml"),
        "schema = 1\n\n[workspace]\ndefault = \"lab\"\n",
    )
    .expect("charter.toml");

    let outcome = init(&at(&root, true), &plain());

    assert_eq!(outcome.code, 0, "{:?}", outcome.said);
    assert!(
        matches!(outcome.said.last(), Some(Say::Info(l))
            if l.contains("It lands in workspaces/lab/widget/,")),
        "{:?}",
        outcome.said
    );
}

/// `_first_clone_step(accepted=True)` where there is no repo: `cmd_init`'s own words, and
/// exit 1 — the plane was still made.
#[test]
fn clone_this_repo_where_there_is_no_repo_says_so_and_exits_1() {
    let (_dir, root) = empty_plane();

    let outcome = init(
        &at(&root, false),
        &InitArgs {
            clone_this_repo: true,
            ..plain()
        },
    );

    assert_eq!(
        outcome.said.last(),
        Some(&Say::Err(
            format!(
                "--clone-this-repo: there is no repo here to clone. {} is not the top level \
             of a git working tree, and the flag clones the repo you are standing in. The \
             control plane itself was still created.",
                root.display()
            )
            .into()
        ))
    );
    assert_eq!(outcome.code, 1);
    assert!(root.join("charter.toml").is_file());
}

/// `--adopt` (charter-app#175, no Python counterpart) naming the plane's own directory is
/// `--clone-this-repo` spelled another way, and is refused with the pointer to it.
#[test]
fn adopt_naming_the_planes_own_directory_points_at_clone_this_repo() {
    let (_dir, root) = a_repo("git@github.com:acme/widget.git");

    let outcome = init(
        &at(&root, false),
        &InitArgs {
            plane_is_this_repo: true,
            adopt: Some(root.clone()),
            ..plain()
        },
    );

    assert_eq!(
        outcome.said.last(),
        Some(&Say::Err(
            ("--adopt: that is this plane's own directory. To clone the repo the plane is \
             being made IN, ask for `--clone-this-repo`; `--adopt` is for a repository \
             somewhere else, with the plane beside it (ADR 0035)."
                .to_owned())
            .into()
        ))
    );
    assert_eq!(outcome.code, 1);
}

/// `commands._first_clone_name`: the tail of `origin` when it is a workspace-shaped name,
/// else the directory's own name.
#[test]
fn the_first_clone_is_named_after_origin_unless_origin_ends_in_no_name() {
    let (_dir, root) = a_repo("git@github.com:acme/widget.git");
    assert_eq!(first_clone_name(&root), "widget");

    let (_dir, root) = a_repo("https://example.com/acme/Not A Name.git");
    assert_eq!(first_clone_name(&root), "plane");
}

/// A level's local skills stay on this machine (ADR 0099 §1, #2767): a fresh plane ignores the
/// plane root's and every workspace's `skills.local/`.
#[test]
fn a_fresh_plane_keeps_every_levels_local_skills_on_this_machine() {
    for line in skills_local_ignores() {
        assert!(
            GITIGNORE_BASELINE.lines().any(|one| one == line),
            "{line}: {GITIGNORE_BASELINE}"
        );
    }
    assert_eq!(
        skills_local_ignores(),
        [
            "/skills.local/",
            "/workspaces/*/skills.local/",
            "/personas/*/skills.local/",
            "/skills.local.lock"
        ]
    );
}

/// ...and `reinit` adds them to a plane made before them, once.
#[test]
fn reinit_keeps_every_levels_local_skills_on_this_machine_and_adds_the_lines_once() {
    let (_dir, root) = empty_plane();
    init(&at(&root, false), &plain());
    let mut old = GITIGNORE_BASELINE.to_owned();
    for line in skills_local_ignores() {
        old = old.replace(&format!("{line}\n"), "");
    }
    std::fs::write(root.join(".gitignore"), &old).expect("an old .gitignore");

    let outcome = reinit(&at(&root, true));

    assert_eq!(outcome.code, 0, "{:?}", outcome.said);
    let text = std::fs::read_to_string(root.join(".gitignore")).expect("a .gitignore");
    assert!(
        text.ends_with(
            "\n\n# added by `charter reinit` — local skills stay on this machine\n\
             /skills.local/\n/workspaces/*/skills.local/\n/personas/*/skills.local/\n\
             /skills.local.lock\n"
        ),
        "{text}"
    );
    reinit(&at(&root, true));
    assert_eq!(
        std::fs::read_to_string(root.join(".gitignore")).expect("a .gitignore"),
        text,
        "a second reinit adds nothing"
    );
}

/// What `init` and `reinit` append to a CRLF `.gitignore` is CRLF too (#2875): their one
/// appender ends each added line as the file's first line is, so no bare `\n` is left in it.
#[test]
fn init_and_reinit_keep_a_crlf_gitignore_crlf() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".gitignore");
    std::fs::write(&path, "build/\r\n!/workspaces/.gitkeep\r\n").unwrap();

    assert_eq!(ensure_gitignore(&path), Ok(true));

    let body = std::fs::read_to_string(&path).unwrap();
    assert!(
        body.starts_with("build/\r\n!/workspaces/.gitkeep\r\n"),
        "{body:?}"
    );
    assert!(body.contains("/personas/*/skills.local/\r\n"), "{body:?}");
    assert!(
        !body.replace("\r\n", "").contains('\n'),
        "a bare \\n in {body:?}"
    );
    assert_eq!(
        ensure_gitignore(&path),
        Ok(false),
        "and it is read back as present"
    );
}

/// A line is present only as git reads it: an indented copy ignores nothing, and one a lone
/// `\r` joins to the line before is no line of its own, so `init` and `reinit` add each again.
#[test]
fn a_line_git_does_not_read_is_added_again() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".gitignore");
    let body = "build/\n!/workspaces/.gitkeep\n/.charter/\n/.purlis/\n\
                /.claude/settings.local.json\n/charter.local.toml\n/purlis.local.toml\n\
                /sessions/\n  /skills.local/\n/workspaces/*/skills.local/\r/personas/*/skills.local/\n\
                /skills.local.lock\n";
    std::fs::write(&path, body).unwrap();

    assert_eq!(ensure_gitignore(&path), Ok(true));

    let after = std::fs::read_to_string(&path).unwrap();
    let added = &after[body.len()..];
    for line in [
        "/skills.local/\n",
        "/workspaces/*/skills.local/\n",
        "/personas/*/skills.local/\n",
    ] {
        assert!(added.contains(line), "{line:?} added: {after:?}");
    }
    assert!(!added.contains("/skills.local.lock"), "{after:?}");
}
