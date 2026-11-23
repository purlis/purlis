// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The policy's levels and their files** (M75 slice 05, #7013; spec §4 and §5).
//!
//! A chat's policy is the shipped default, the Organisation's two files, the project's file and
//! `policy.cedar` as committed at the plane root's `HEAD` (with the Workspace and Persona
//! defaults in that same file), and the person's own tighten-only local file. These tests load
//! real planes, made with git in a temporary folder, and judge what each level may and may not
//! do: the read is of `HEAD` as written, a level writes only about its own chats, a lock drops
//! a lower permit, a same-`@id` rule replaces only at its own level or below, and a broken file
//! falls back without ever being skipped.

use std::path::Path;

use purlis_core::enforcement::{Mode, SandboxLevel, Template};
use purlis_core::policy::levels::{self, Accepted, Chat, Loaded, Notice, Organisation, Sources};
use purlis_core::policy::{self, Decision, Level, Question, World};

const WORKSPACES: [&str; 3] = ["web", "api", "infra"];
const PERSONAS: [&str; 3] = ["builder", "reviewer", "deployer"];
const VERSION: &str = "// purlis-policy 1\n";

/// git, with no global or system config, in `dir`.
fn git(dir: &Path, args: &[&str]) {
    let ran = purlis_core::forklock::output(
        std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "init.defaultBranch=main",
            ])
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1"),
    )
    .expect("git runs");
    assert!(ran.status.success(), "git {args:?}: {ran:?}");
}

/// git's answer, trimmed.
fn git_out(dir: &Path, args: &[&str]) -> String {
    let ran = purlis_core::forklock::output(
        std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1"),
    )
    .expect("git runs");
    assert!(ran.status.success(), "git {args:?}: {ran:?}");
    String::from_utf8(ran.stdout)
        .expect("text")
        .trim()
        .to_owned()
}

fn write(dir: &Path, file: &str, text: &str) {
    let path = dir.join(file);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("a folder");
    }
    std::fs::write(path, text).expect("a file");
}

/// A plane whose root commits `files`, one commit.
fn plane(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a folder");
    git(dir.path(), &["init", "-q"]);
    commit(dir.path(), files);
    dir
}

/// Commits `files` at `dir`.
fn commit(dir: &Path, files: &[(&str, &str)]) {
    for (file, text) in files {
        write(dir, file, text);
    }
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "--allow-empty", "-m", "c"]);
}

fn sources(root: &Path) -> Sources<'_> {
    Sources {
        root,
        workspaces: &WORKSPACES,
        personas: &PERSONAS,
        organisation: None,
        accepted: None,
    }
}

const ORG: &str = "/etc/purlis";
const ORG_OWNER: &str = "Platform team";

fn organisation<'a>(rules: Option<&'a str>, settings: Option<&'a str>) -> Organisation<'a> {
    Organisation {
        folder: ORG,
        owner: ORG_OWNER,
        rules,
        settings,
        json: None,
        failed: None,
    }
}

fn world() -> World {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/policy/world.json");
    let text = std::fs::read_to_string(path).expect("the world");
    World::from_json(serde_json::from_str(&text).expect("JSON")).expect("the world loads")
}

fn context() -> serde_json::Value {
    serde_json::json!({ "sandbox": "floor", "attended": true, "harness_mode": "default", "depth": 0 })
}

/// What `loaded` decides for `chat` installing the skill `tdd`, which the shipped default allows.
fn install(loaded: &Loaded, chat: &str) -> Decision {
    loaded.policy.decide(
        &Question {
            chat,
            action: "install",
            resource: ("Skill", "tdd"),
            context: context(),
        },
        &world(),
    )
}

fn merge(loaded: &Loaded) -> Decision {
    let mut context = context();
    context["via"] = "".into();
    loaded.policy.decide(
        &Question {
            chat: "web/builder",
            action: "merge",
            resource: ("Branch", "acme/web#main"),
            context,
        },
        &world(),
    )
}

fn allowed() -> Decision {
    Decision::Allow {
        permits: vec!["policy-everything-else".into()],
    }
}

fn denied(id: &str) -> Decision {
    Decision::Deny {
        forbids: vec![id.into()],
    }
}

/// A project's rule forbidding every install.
const NO_INSTALLS: &str = "// purlis-policy 1\n\
     @id(\"skills-none\")\n\
     forbid (principal, action == Action::\"install\", resource);\n";

/// The notice whose sentence holds `words`, or a panic listing them all.
fn notice<'a>(loaded: &'a Loaded, words: &str) -> &'a Notice {
    loaded
        .notices
        .iter()
        .find(|notice| notice.what.contains(words))
        .unwrap_or_else(|| panic!("no notice says {words:?}: {:#?}", loaded.notices))
}

const ANYONE: Chat<'static> = Chat {
    workspace: None,
    persona: None,
};

#[test]
fn the_policy_is_read_from_the_plane_roots_head_never_a_branch_folder() {
    purlis_core::unsteered!();
    let root = plane(&[
        ("policy.cedar", NO_INSTALLS),
        ("charter.toml", "[policy]\ntemplate = \"strict\"\n"),
    ]);
    // A branch folder of the same plane, whose committed and working copies say otherwise.
    let parent = tempfile::tempdir().expect("a folder");
    let branch = parent.path().join("feature");
    git(
        root.path(),
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "feature",
            branch.to_str().unwrap(),
        ],
    );
    commit(
        &branch,
        &[
            ("policy.cedar", VERSION),
            ("charter.toml", "[policy]\ntemplate = \"open\"\n"),
        ],
    );
    write(&branch, "purlis.toml", "[policy]\ntemplate = \"open\"\n");

    let loaded = levels::load(&sources(root.path()));
    assert_eq!(install(&loaded, "web/builder"), denied("skills-none"));
    assert_eq!(loaded.template_for(ANYONE), Template::Strict);
}

// A graft changes only a commit's parents, never the tree `HEAD:<file>` reads, so no test of
// the HEAD read can see one; the read still passes `GIT_GRAFT_FILE` (spec §5) for the author
// check of §9.2, which walks history.
#[test]
fn a_replace_ref_does_not_change_what_head_reads() {
    purlis_core::unsteered!();
    let root = plane(&[("policy.cedar", NO_INSTALLS)]);
    let written = git_out(root.path(), &["rev-parse", "HEAD"]);
    // Another commit, with no rule, made off to the side and put in HEAD's place.
    commit(root.path(), &[("policy.cedar", VERSION)]);
    let stand_in = git_out(root.path(), &["rev-parse", "HEAD"]);
    git(root.path(), &["reset", "-q", "--hard", &written]);
    git(root.path(), &["replace", &written, &stand_in]);
    git(root.path(), &["config", "core.useReplaceRefs", "true"]);
    assert_eq!(
        git_out(root.path(), &["show", "HEAD:policy.cedar"]),
        VERSION.trim(),
        "git itself reads the stand-in"
    );

    let loaded = levels::load(&sources(root.path()));
    assert_eq!(install(&loaded, "web/builder"), denied("skills-none"));
}

#[test]
fn a_repo_config_key_does_not_change_what_head_reads() {
    purlis_core::unsteered!();
    let parent = tempfile::tempdir().expect("a folder");
    let root = parent.path().join("plane");
    std::fs::create_dir(&root).expect("the plane");
    git(&root, &["init", "-q"]);
    // The plane's own rule, and a decoy committed one folder down under the plane's name.
    commit(
        &root,
        &[
            ("policy.cedar", NO_INSTALLS),
            ("plane/policy.cedar", VERSION),
            ("charter.toml", "[policy]\ntemplate = \"strict\"\n"),
            ("plane/charter.toml", "[policy]\ntemplate = \"open\"\n"),
        ],
    );
    // A work tree one folder up makes `./` the decoy's folder for git.
    git(
        &root,
        &["config", "core.worktree", parent.path().to_str().unwrap()],
    );
    assert_eq!(
        git_out(&root, &["show", "HEAD:./policy.cedar"]),
        VERSION.trim(),
        "git itself reads the decoy"
    );

    let loaded = levels::load(&sources(&root));
    assert_eq!(install(&loaded, "web/builder"), denied("skills-none"));
    assert_eq!(loaded.template_for(ANYONE), Template::Strict);
}

#[test]
fn a_plane_with_no_head_runs_its_accepted_copy_or_the_shipped_default() {
    purlis_core::unsteered!();
    let unborn = tempfile::tempdir().expect("a folder");
    git(unborn.path(), &["init", "-q"]);
    let no_repository = tempfile::tempdir().expect("a folder");
    for root in [unborn.path(), no_repository.path()] {
        // Working files that are never the project's.
        write(root, "policy.cedar", NO_INSTALLS);
        write(root, "charter.toml", "[policy]\ntemplate = \"strict\"\n");

        let shipped = levels::load(&sources(root));
        assert_eq!(install(&shipped, "web/builder"), allowed());
        assert_eq!(shipped.template_for(ANYONE), Template::Watch);
        let said = notice(&shipped, "no commit");
        assert_eq!(said.file, "policy.cedar");

        let accepted_rules = "// purlis-policy 1\n\
             @id(\"skills-accepted\")\n\
             forbid (principal, action == Action::\"install\", resource);\n";
        let accepted = levels::load(&Sources {
            accepted: Some(Accepted {
                rules: Some(accepted_rules),
                settings: Some("[policy]\ntemplate = \"team\"\n"),
            }),
            ..sources(root)
        });
        assert_eq!(install(&accepted, "web/builder"), denied("skills-accepted"));
        assert_eq!(accepted.template_for(ANYONE), Template::Team);
        notice(&accepted, "no commit");
        assert_eq!(accepted.sandbox_at_least, None, "nothing here is broken");
    }
}

#[test]
fn an_uncommitted_change_to_the_project_file_changes_no_decision() {
    purlis_core::unsteered!();
    let root = plane(&[
        ("policy.cedar", NO_INSTALLS),
        ("charter.toml", "[policy]\ntemplate = \"strict\"\n"),
    ]);
    write(root.path(), "policy.cedar", VERSION);
    write(
        root.path(),
        "charter.toml",
        "[policy]\ntemplate = \"open\"\n",
    );
    write(
        root.path(),
        "purlis.toml",
        "[policy]\ntemplate = \"open\"\n",
    );

    let loaded = levels::load(&sources(root.path()));
    assert_eq!(install(&loaded, "web/builder"), denied("skills-none"));
    assert_eq!(loaded.template_for(ANYONE), Template::Strict);
    assert_eq!(loaded.mode_for("guards.merge", ANYONE), Mode::Enforce);
}

/// A plane whose committed `policy.cedar` holds `rules` after its version line.
fn with_rules(rules: &str) -> tempfile::TempDir {
    plane(&[("policy.cedar", &format!("{VERSION}{rules}"))])
}

#[test]
fn an_at_level_above_the_files_place_is_refused_at_load() {
    purlis_core::unsteered!();
    let root = with_rules(
        "\n@id(\"skills-org-claim\") @level(\"organisation\")\n\
         forbid (principal, action == Action::\"install\", resource);\n",
    );
    let loaded = levels::load(&sources(root.path()));
    let said = notice(&loaded, "@level(\"organisation\") is above this file");
    assert_eq!((said.file.as_str(), said.line), ("policy.cedar", 3));
    assert!(loaded.policy.rule("skills-org-claim").is_none());
    assert_eq!(install(&loaded, "web/builder"), allowed());
}

#[test]
fn an_at_level_you_in_a_committed_file_is_refused_at_load() {
    purlis_core::unsteered!();
    let root = with_rules(
        "@id(\"skills-mine\") @level(\"you\")\n\
         forbid (principal, action == Action::\"install\", resource);\n",
    );
    let loaded = levels::load(&sources(root.path()));
    let said = notice(&loaded, "@level(\"you\") in a shared file");
    assert_eq!((said.file.as_str(), said.line), ("policy.cedar", 2));
    assert!(loaded.policy.rule("skills-mine").is_none());
}

#[test]
fn the_lock_level_clash_and_cross_level_id_probe_policies_decide_as_the_oracle_does() {
    purlis_core::unsteered!();
    // The oracle, until slice 09's equality cells: each probe's answer, written by hand.
    // A lock: deny-by-default with `skills` locked, so a workspace's permit on it is dropped.
    let lock = plane(&[
        (
            "charter.toml",
            "[policy.rules]\n\"policy-everything-else\" = \"off\"\n\n\
             [policy.locks]\nfamilies = [\"skills\"]\n",
        ),
        (
            "policy.cedar",
            "// purlis-policy 1\n\
             @id(\"skills-web-install\") @level(\"workspace\")\n\
             permit (principal in Workspace::\"web\", action == Action::\"install\", resource);\n",
        ),
    ]);
    let probe = levels::load(&sources(lock.path()));
    assert_eq!(install(&probe, "web/builder"), Decision::NoPermit, "lock");

    // A level clash: the Organisation's forbid binds a project's permit.
    let clash = with_rules(
        "@id(\"skills-project-install\")\n\
         permit (principal, action == Action::\"install\", resource);\n",
    );
    let org_rules = "// purlis-policy 1\n\
         @id(\"skills-org-none\")\n\
         forbid (principal, action == Action::\"install\", resource);\n";
    let probe = levels::load(&Sources {
        organisation: Some(organisation(Some(org_rules), None)),
        ..sources(clash.path())
    });
    assert_eq!(
        install(&probe, "web/builder"),
        denied("skills-org-none"),
        "level clash"
    );

    // A cross-level `@id`: the project's rule of the Organisation's name replaces nothing.
    let cross = with_rules(
        "@id(\"skills-org-none\")\n\
         permit (principal, action == Action::\"install\", resource);\n",
    );
    let probe = levels::load(&Sources {
        organisation: Some(organisation(Some(org_rules), None)),
        ..sources(cross.path())
    });
    assert_eq!(
        install(&probe, "web/builder"),
        denied("skills-org-none"),
        "cross-level id"
    );
}

#[test]
fn both_charter_toml_and_purlis_toml_are_read_as_the_project_file() {
    purlis_core::unsteered!();
    for file in ["charter.toml", "purlis.toml"] {
        let root = plane(&[(file, "[policy]\ntemplate = \"strict\"\n")]);
        let loaded = levels::load(&sources(root.path()));
        assert_eq!(loaded.template_for(ANYONE), Template::Strict, "{file}");
    }
}

#[test]
fn a_workspace_rule_about_another_workspaces_chats_is_refused_at_load() {
    purlis_core::unsteered!();
    for (scope, line) in [
        ("principal in Plane::\"acme\"", 2),
        ("principal in Workspace::\"nowhere\"", 2),
        ("principal in Persona::\"builder\"", 2),
        ("principal", 2),
    ] {
        let root = with_rules(&format!(
            "@id(\"skills-ws-none\") @level(\"workspace\")\n\
             forbid ({scope}, action == Action::\"install\", resource);\n"
        ));
        let loaded = levels::load(&sources(root.path()));
        let said = notice(&loaded, "a workspace rule is about one workspace's chats");
        assert_eq!(
            (said.file.as_str(), said.line),
            ("policy.cedar", line),
            "{scope}"
        );
        // The rule it names is given its own level, never its file's place.
        assert_eq!(said.rules[0].level, Level::Workspace, "{scope}");
        assert!(loaded.policy.rule("skills-ws-none").is_none(), "{scope}");
    }
    // Its own workspace's chats: loaded, and binding there only.
    let root = with_rules(
        "@id(\"skills-ws-none\") @level(\"workspace\")\n\
         forbid (principal in Workspace::\"web\", action == Action::\"install\", resource);\n",
    );
    let loaded = levels::load(&sources(root.path()));
    assert!(loaded.notices.is_empty(), "{:#?}", loaded.notices);
    assert_eq!(install(&loaded, "web/builder"), denied("skills-ws-none"));
    assert_eq!(install(&loaded, "infra/deployer"), allowed());
}

#[test]
fn a_project_lock_drops_a_lower_permit_with_a_notice() {
    purlis_core::unsteered!();
    let root = plane(&[
        (
            "charter.toml",
            "[policy]\ntemplate = \"team\"\n\n[policy.locks]\nfamilies = [\"skills\"]\n",
        ),
        (
            "policy.cedar",
            "// purlis-policy 1\n\
             @id(\"skills-web-install\") @level(\"workspace\")\n\
             permit (principal in Workspace::\"web\", action == Action::\"install\", resource);\n",
        ),
    ]);
    let loaded = levels::load(&sources(root.path()));
    assert!(loaded.policy.rule("skills-web-install").is_none());
    let said = notice(&loaded, "is locked");
    assert_eq!((said.file.as_str(), said.line), ("charter.toml", 5));
    let named = &said.rules[0];
    assert_eq!(named.id, "skills-web-install");
    assert_eq!(named.level, Level::Workspace);
    assert_eq!(named.owner, "workspace web");
    assert_eq!((named.file.as_str(), named.line), ("policy.cedar", 2));
}

#[test]
fn a_rule_with_a_shipped_id_replaces_it_and_an_identical_one_restates() {
    purlis_core::unsteered!();
    // Replaced: the merge hold, without its ask, refuses.
    let root = with_rules(
        "@id(\"guards-merge-hold\")\n\
         forbid (principal, action == Action::\"merge\", resource);\n",
    );
    let loaded = levels::load(&sources(root.path()));
    assert_eq!(merge(&loaded), denied("guards-merge-hold"));
    let said = notice(&loaded, "replaces");
    assert_eq!((said.file.as_str(), said.line), ("policy.cedar", 2));
    assert_eq!(
        said.rules[0].owner,
        format!("purlis default {}", env!("CARGO_PKG_VERSION"))
    );

    // Restated: the same rule, word for word, is reported as such and changes nothing.
    let root = with_rules(
        "@id(\"guards-merge-hold\") @level(\"project\") @ask(\"you\") @keep(\"once\")\n\
         forbid (principal, action in [Action::\"merge\", Action::\"land\"], resource);\n",
    );
    let loaded = levels::load(&sources(root.path()));
    assert_eq!(
        merge(&loaded),
        Decision::Ask {
            asks: vec!["guards-merge-hold".into()]
        }
    );
    notice(&loaded, "restates");
    assert!(!loaded.notices.iter().any(|n| n.what.contains("replaces")));
}

#[test]
fn a_project_rule_cannot_replace_an_organisation_rule_by_id() {
    purlis_core::unsteered!();
    let root = with_rules(
        "@id(\"skills-org-none\")\n\
         permit (principal, action == Action::\"install\", resource);\n",
    );
    let org_rules = "// purlis-policy 1\n\
         @id(\"skills-org-none\")\n\
         forbid (principal, action == Action::\"install\", resource);\n";
    let loaded = levels::load(&Sources {
        organisation: Some(organisation(Some(org_rules), None)),
        ..sources(root.path())
    });
    let held = loaded
        .policy
        .rule("skills-org-none")
        .expect("the Organisation's rule");
    assert_eq!(held.level, Level::Organisation);
    assert_eq!(install(&loaded, "web/builder"), denied("skills-org-none"));
    let said = notice(&loaded, "can't replace");
    assert_eq!((said.file.as_str(), said.line), ("policy.cedar", 2));
    assert!(said.what.contains("an Organisation rule"), "{}", said.what);
    assert_eq!(said.rules[0].level, Level::Organisation);
    assert_eq!(said.rules[0].owner, ORG_OWNER);
}

#[test]
fn a_lower_permit_that_can_never_pass_a_higher_forbid_is_warned_about() {
    purlis_core::unsteered!();
    let root = with_rules(
        "@id(\"skills-allow-install\")\n\
         permit (principal, action == Action::\"install\", resource);\n",
    );
    let org_rules = "// purlis-policy 1\n\
         @id(\"skills-org-none\")\n\
         forbid (principal, action in [Action::\"install\", Action::\"load\"], resource);\n";
    let loaded = levels::load(&Sources {
        organisation: Some(organisation(Some(org_rules), None)),
        ..sources(root.path())
    });
    let said = notice(&loaded, "can never pass");
    assert_eq!((said.file.as_str(), said.line), ("policy.cedar", 2));
    let ids: Vec<(&str, Level)> = said
        .rules
        .iter()
        .map(|r| (r.id.as_str(), r.level))
        .collect();
    assert_eq!(
        ids,
        [
            ("skills-allow-install", Level::Project),
            ("skills-org-none", Level::Organisation)
        ]
    );
    // A forbid with a condition may let it pass: no warning.
    let org_rules = "// purlis-policy 1\n\
         @id(\"skills-org-deep\")\n\
         forbid (principal, action == Action::\"install\", resource) when { context.depth > 1 };\n";
    let loaded = levels::load(&Sources {
        organisation: Some(organisation(Some(org_rules), None)),
        ..sources(root.path())
    });
    assert!(loaded.notices.is_empty(), "{:#?}", loaded.notices);
}

#[test]
fn a_broken_project_file_falls_back_to_the_accepted_copy_else_strict_with_the_full_sandbox() {
    purlis_core::unsteered!();
    let root = plane(&[
        ("charter.toml", "[policy]\n\"guards.merj\" = \"log\"\n"),
        ("policy.cedar", NO_INSTALLS),
    ]);
    let alone = levels::load(&sources(root.path()));
    let said = notice(&alone, "broken");
    assert_eq!((said.file.as_str(), said.line), ("charter.toml", 2));
    assert_eq!(alone.template_for(ANYONE), Template::Strict);
    assert_eq!(alone.sandbox_at_least, Some(SandboxLevel::Full));
    // Strict: the broken level's own rules are not read.
    assert_eq!(install(&alone, "web/builder"), allowed());

    let accepted = levels::load(&Sources {
        accepted: Some(Accepted {
            settings: Some("[policy]\ntemplate = \"team\"\n"),
            rules: Some(NO_INSTALLS),
        }),
        ..sources(root.path())
    });
    assert_eq!(accepted.template_for(ANYONE), Template::Team);
    assert_eq!(install(&accepted, "web/builder"), denied("skills-none"));
    assert_eq!(accepted.sandbox_at_least, Some(SandboxLevel::Full));
    notice(&accepted, "the copy you last accepted");
}

#[test]
fn a_broken_organisation_file_reads_as_strict_plus_every_lock() {
    purlis_core::unsteered!();
    let root = plane(&[("charter.toml", "[policy]\ntemplate = \"open\"\n")]);
    let loaded = levels::load(&Sources {
        organisation: Some(organisation(
            Some("@id(\"skills-x\") forbid (principal, action, resource);"),
            None,
        )),
        ..sources(root.path())
    });
    let said = notice(&loaded, "broken");
    assert_eq!(
        (said.file.as_str(), said.line),
        ("/etc/purlis/policy.cedar", 1)
    );
    // The project's `open` is the nearest template, and every family is locked enforced over it.
    assert_eq!(loaded.template_for(ANYONE), Template::Open);
    for family in purlis_core::policy::FAMILIES
        .iter()
        .filter(|f| **f != "sandbox")
    {
        assert!(
            loaded.organisation_locks.families.contains(*family),
            "{family}"
        );
        assert_eq!(loaded.mode_for(family, ANYONE), Mode::Enforce, "{family}");
    }
    assert_eq!(loaded.sandbox_at_least, Some(SandboxLevel::Full));
}

#[test]
fn a_broken_file_runs_the_full_sandbox_even_with_an_accepted_copy() {
    purlis_core::unsteered!();
    let root = plane(&[(
        "policy.cedar",
        "@id(\"skills-x\") forbid (principal, action, resource);",
    )]);
    let loaded = levels::load(&Sources {
        accepted: Some(Accepted {
            settings: Some("[policy]\ntemplate = \"open\"\n"),
            rules: Some(NO_INSTALLS),
        }),
        ..sources(root.path())
    });
    assert_eq!(install(&loaded, "web/builder"), denied("skills-none"));
    assert_eq!(loaded.sandbox_at_least, Some(SandboxLevel::Full));
}

#[test]
fn a_you_level_loosening_is_ignored_with_a_notice() {
    purlis_core::unsteered!();
    let root = plane(&[(
        "charter.toml",
        "[policy]\ntemplate = \"team\"\n\n[policy.rules]\n\"policy-everything-else\" = \"enforce\"\n",
    )]);
    write(
        root.path(),
        "charter.local.toml",
        "[policy]\ntemplate = \"open\"\n\"guards.merge\" = \"off\"\n\"guards.git\" = \"enforce\"\n\n\
         [policy.rules]\n\"guards-merge-hold\" = \"off\"\n\"policy-everything-else\" = \"off\"\n",
    );
    git(root.path(), &["config", "core.excludesFile", "/dev/null"]);
    write(root.path(), ".git/info/exclude", "charter.local.toml\n");

    let loaded = levels::load(&sources(root.path()));
    assert_eq!(loaded.template_for(ANYONE), Template::Team);
    assert_eq!(loaded.mode_for("guards.merge", ANYONE), Mode::Enforce);
    assert_eq!(loaded.mode_for("guards.git", ANYONE), Mode::Enforce);
    for (words, line) in [
        ("template = \"open\"", 2),
        ("\"guards.merge\" = \"off\"", 3),
        ("\"guards-merge-hold\" = \"off\"", 7),
    ] {
        let said = notice(&loaded, words);
        assert!(said.what.contains("ignored"), "{said:#?}");
        assert_eq!(
            (said.file.as_str(), said.line),
            ("charter.local.toml", line),
            "{words}"
        );
    }
    // The merge hold still asks; deny-by-default, a tightening, applies at once.
    assert_eq!(merge(&loaded), Decision::NoPermit);
    assert_eq!(install(&loaded, "web/builder"), Decision::NoPermit);
    // The tightenings, for slice 14 to seal into the accepted floor.
    let tightenings: Vec<(&str, &str)> = loaded
        .tightenings
        .iter()
        .map(|t| (t.key.as_str(), t.value.as_str()))
        .collect();
    assert_eq!(
        tightenings,
        [
            ("guards.git", "enforce"),
            ("rules.policy-everything-else", "off")
        ]
    );
}

#[test]
fn a_workspace_default_in_the_project_file_sets_that_workspaces_template() {
    purlis_core::unsteered!();
    let root = plane(&[(
        "charter.toml",
        "[policy]\ntemplate = \"watch\"\n\n[policy.workspace.infra]\ntemplate = \"strict\"\n",
    )]);
    let loaded = levels::load(&sources(root.path()));
    let infra = Chat {
        workspace: Some("infra"),
        persona: None,
    };
    let web = Chat {
        workspace: Some("web"),
        persona: None,
    };
    assert_eq!(loaded.template_for(infra), Template::Strict);
    assert_eq!(loaded.template_for(web), Template::Watch);
    assert_eq!(loaded.mode_for("guards.merge", infra), Mode::Enforce);
    assert_eq!(loaded.mode_for("guards.merge", web), Mode::Log);
}

#[test]
fn a_workspace_and_a_persona_default_for_one_key_take_the_stricter() {
    purlis_core::unsteered!();
    let root = plane(&[(
        "charter.toml",
        "[policy]\ntemplate = \"watch\"\n\n\
         [policy.workspace.web]\n\"guards.merge\" = \"off\"\ntemplate = \"team\"\n\n\
         [policy.persona.builder]\n\"guards.merge\" = \"enforce\"\ntemplate = \"hold\"\n",
    )]);
    let loaded = levels::load(&sources(root.path()));
    let both = Chat {
        workspace: Some("web"),
        persona: Some("builder"),
    };
    assert_eq!(loaded.mode_for("guards.merge", both), Mode::Enforce);
    assert_eq!(loaded.template_for(both), Template::Team);
    let workspace_alone = Chat {
        workspace: Some("web"),
        persona: Some("reviewer"),
    };
    assert_eq!(loaded.mode_for("guards.merge", workspace_alone), Mode::Off);
}

#[test]
fn a_workspace_table_naming_no_workspace_fails_validation() {
    purlis_core::unsteered!();
    for (table, words) in [
        ("[policy.workspace.nowhere]", "no workspace \"nowhere\""),
        ("[policy.persona.nobody]", "no persona \"nobody\""),
    ] {
        let root = plane(&[(
            "charter.toml",
            &format!("[policy]\ntemplate = \"open\"\n\n{table}\ntemplate = \"strict\"\n"),
        )]);
        let loaded = levels::load(&sources(root.path()));
        let said = notice(&loaded, words);
        assert_eq!(
            (said.file.as_str(), said.line),
            ("charter.toml", 4),
            "{table}"
        );
        // Broken, with no accepted copy: strict.
        assert_eq!(loaded.template_for(ANYONE), Template::Strict, "{table}");
    }
}

#[test]
fn the_organisations_policy_toml_can_go_deny_by_default() {
    purlis_core::unsteered!();
    let root = plane(&[("charter.toml", "[policy]\ntemplate = \"team\"\n")]);
    let loaded = levels::load(&Sources {
        organisation: Some(organisation(
            None,
            Some(
                "[policy]\ntemplate = \"strict\"\n\n\
                 [policy.rules]\n\"policy-everything-else\" = \"off\"\n\n\
                 [policy.sets]\nproduction-hosts = [\"api.acme.com\"]\n",
            ),
        )),
        ..sources(root.path())
    });
    assert!(loaded.notices.is_empty(), "{:#?}", loaded.notices);
    assert_eq!(install(&loaded, "web/builder"), Decision::NoPermit);
    assert_eq!(loaded.sets["production-hosts"], ["api.acme.com"]);
    // Nearest wins for a default: the project's template.
    assert_eq!(loaded.template_for(ANYONE), Template::Team);
}

#[test]
fn an_organisation_policy_toml_with_a_bad_value_reads_as_strict_plus_every_lock() {
    purlis_core::unsteered!();
    let root = plane(&[("charter.toml", "[policy]\ntemplate = \"open\"\n")]);
    for bad in [
        "[policy]\n\"guards.merj\" = \"log\"\n",
        "[policy.locks]\nfamilies = [\"guards.merj\"]\n",
        "[policy.sets]\nproduction-hosts = \"api.acme.com\"\n",
    ] {
        let loaded = levels::load(&Sources {
            organisation: Some(organisation(None, Some(bad))),
            ..sources(root.path())
        });
        let said = notice(&loaded, "broken");
        assert_eq!(
            (said.file.as_str(), said.line),
            ("/etc/purlis/policy.toml", 2),
            "{bad}"
        );
        for family in purlis_core::policy::FAMILIES
            .iter()
            .filter(|f| **f != "sandbox")
        {
            assert_eq!(
                loaded.mode_for(family, ANYONE),
                Mode::Enforce,
                "{bad}: {family}"
            );
        }
        assert_eq!(
            loaded.organisation_locks.families.len(),
            purlis_core::policy::FAMILIES.len(),
            "{bad}"
        );
        assert_eq!(loaded.sandbox_at_least, Some(SandboxLevel::Full), "{bad}");
    }
}

#[test]
fn a_lock_on_one_action_and_a_per_family_keep_are_read() {
    purlis_core::unsteered!();
    let root = plane(&[(
        "charter.toml",
        "[policy.locks]\nactions = [\"push\"]\n\
         keep = { \"guards.merge\" = \"once\", vaults = \"this chat\" }\ntimed = \"off\"\n",
    )]);
    let loaded = levels::load(&sources(root.path()));
    assert!(loaded.notices.is_empty(), "{:#?}", loaded.notices);
    let locks = &loaded.project_locks;
    assert_eq!(locks.actions.iter().collect::<Vec<_>>(), ["push"]);
    assert_eq!(locks.keep, None);
    assert_eq!(locks.keep_per_family["guards.merge"], "once");
    assert_eq!(locks.keep_per_family["vaults"], "this chat");
    assert_eq!(locks.timed.as_deref(), Some("off"));

    let everything = plane(&[("charter.toml", "[policy.locks]\nkeep = \"once\"\n")]);
    let loaded = levels::load(&sources(everything.path()));
    assert_eq!(loaded.project_locks.keep.as_deref(), Some("once"));

    let no_such_action = plane(&[("charter.toml", "[policy.locks]\nactions = [\"shove\"]\n")]);
    let loaded = levels::load(&sources(no_such_action.path()));
    let said = notice(&loaded, "\"shove\"");
    assert_eq!((said.file.as_str(), said.line), ("charter.toml", 2));
}

/// The path of the loose object git names `oid` by, in `root`'s repository.
fn loose(root: &Path, oid: &str) -> std::path::PathBuf {
    root.join(".git/objects").join(&oid[..2]).join(&oid[2..])
}

#[test]
fn a_committed_file_whose_object_is_gone_is_unread_never_absent() {
    purlis_core::unsteered!();
    let root = plane(&[
        ("charter.toml", "[policy]\ntemplate = \"open\"\n"),
        ("policy.cedar", NO_INSTALLS),
    ]);
    let blob = git_out(root.path(), &["rev-parse", "HEAD:charter.toml"]);
    std::fs::remove_file(loose(root.path(), &blob)).expect("the loose blob goes");

    let loaded = levels::load(&sources(root.path()));
    // Not "the project says nothing": the file is there and does not read, so strict.
    let said = notice(&loaded, "missing");
    assert_eq!(
        said.file, "charter.toml",
        "the Notice names the file that did not read"
    );
    assert_eq!(loaded.template_for(ANYONE), Template::Strict);
    assert_eq!(loaded.sandbox_at_least, Some(SandboxLevel::Full));
}

#[test]
fn a_committed_file_rewritten_in_place_under_its_name_is_unread() {
    purlis_core::unsteered!();
    let root = plane(&[("policy.cedar", NO_INSTALLS)]);
    let blob = git_out(root.path(), &["rev-parse", "HEAD:policy.cedar"]);
    // Another blob's object, with no rule, written over the committed one's under its name.
    write(root.path(), "other.cedar", VERSION);
    let other = git_out(root.path(), &["hash-object", "-w", "other.cedar"]);
    let path = loose(root.path(), &blob);
    std::fs::remove_file(&path).expect("the loose blob goes");
    std::fs::copy(loose(root.path(), &other), &path).expect("the stand-in is put in its place");
    assert_eq!(
        git_out(root.path(), &["show", "HEAD:policy.cedar"]),
        VERSION.trim(),
        "git itself reads the stand-in"
    );

    let loaded = levels::load(&sources(root.path()));
    let said = notice(&loaded, "does not hash");
    assert_eq!(said.file, "policy.cedar");
    assert_eq!(loaded.template_for(ANYONE), Template::Strict);
    assert_eq!(loaded.sandbox_at_least, Some(SandboxLevel::Full));
}

#[test]
fn a_dot_git_whose_head_does_not_read_is_unread_never_no_repository() {
    purlis_core::unsteered!();
    let root = plane(&[("charter.toml", "[policy]\ntemplate = \"strict\"\n")]);
    // A working file that says otherwise, and a HEAD that names nothing.
    write(
        root.path(),
        "charter.toml",
        "[policy]\ntemplate = \"open\"\n",
    );
    write(root.path(), ".git/HEAD", "garbage\n");

    let loaded = levels::load(&sources(root.path()));
    assert!(
        !loaded
            .notices
            .iter()
            .any(|n| n.what.contains("no git repository")),
        "{:#?}",
        loaded.notices
    );
    assert_eq!(notice(&loaded, "does not read").file, ".git");
    assert_eq!(loaded.template_for(ANYONE), Template::Strict);
    assert_eq!(loaded.sandbox_at_least, Some(SandboxLevel::Full));
    // ADR 0125's reader takes the same answer: never the working file.
    assert_eq!(
        purlis_core::enforcement::Settings::read(root.path()).template,
        Template::Strict
    );
}

#[test]
fn a_project_rule_mode_cannot_loosen_what_the_organisation_set() {
    purlis_core::unsteered!();
    let root = plane(&[(
        "charter.toml",
        "[policy.rules]\n\"policy-everything-else\" = \"enforce\"\n",
    )]);
    let loaded = levels::load(&Sources {
        organisation: Some(organisation(
            None,
            Some("[policy.rules]\n\"policy-everything-else\" = \"off\"\n"),
        )),
        ..sources(root.path())
    });
    assert_eq!(install(&loaded, "web/builder"), Decision::NoPermit);
    let said = notice(&loaded, "would loosen the rule");
    assert_eq!((said.file.as_str(), said.line), ("charter.toml", 2));
    assert_eq!(loaded.rule_modes["policy-everything-else"], Mode::Off);
}

#[test]
fn a_project_rule_mode_cannot_loosen_a_shipped_rule_on_a_locked_family() {
    purlis_core::unsteered!();
    let root = plane(&[(
        "charter.toml",
        "[policy.rules]\n\"guards-merge-hold\" = \"off\"\n",
    )]);
    let loaded = levels::load(&Sources {
        organisation: Some(organisation(
            None,
            Some("[policy.locks]\nfamilies = [\"guards.merge\"]\n"),
        )),
        ..sources(root.path())
    });
    assert_eq!(
        merge(&loaded),
        Decision::Ask {
            asks: vec!["guards-merge-hold".into()]
        }
    );
    let said = notice(&loaded, "is locked by Platform team");
    assert_eq!((said.file.as_str(), said.line), ("charter.toml", 2));

    // Unlocked, the project may turn its shipped rule off (a loosening the press accepts).
    let loaded = levels::load(&sources(root.path()));
    assert_eq!(merge(&loaded), allowed());
}

#[test]
fn a_local_file_git_would_carry_keeps_its_tightenings_with_a_notice() {
    purlis_core::unsteered!();
    let root = plane(&[("charter.toml", "[policy]\ntemplate = \"team\"\n")]);
    git(root.path(), &["config", "core.excludesFile", "/dev/null"]);
    write(
        root.path(),
        "charter.local.toml",
        "[policy]\ntemplate = \"strict\"\n",
    );

    let loaded = levels::load(&sources(root.path()));
    assert_eq!(loaded.template_for(ANYONE), Template::Strict);
    let said = notice(&loaded, "read all the same");
    assert_eq!(said.file, "charter.local.toml");
    assert_eq!(loaded.tightenings.len(), 1, "{:#?}", loaded.tightenings);
}

#[cfg(unix)]
#[test]
fn a_local_file_that_cannot_be_read_is_told_never_lost_silently() {
    purlis_core::unsteered!();
    use std::os::unix::fs::PermissionsExt as _;
    let root = plane(&[("charter.toml", "[policy]\ntemplate = \"team\"\n")]);
    let local = root.path().join("charter.local.toml");
    write(
        root.path(),
        "charter.local.toml",
        "[policy]\ntemplate = \"strict\"\n",
    );
    std::fs::set_permissions(&local, std::fs::Permissions::from_mode(0o000)).expect("chmod");
    if std::fs::read_to_string(&local).is_ok() {
        return; // root reads anything: there is nothing to see here.
    }

    let loaded = levels::load(&sources(root.path()));
    let said = notice(&loaded, "could not be read");
    assert_eq!(said.file, "charter.local.toml");
}

#[test]
fn a_locked_family_never_runs_below_enforce() {
    purlis_core::unsteered!();
    let root = plane(&[("charter.toml", "[policy]\ntemplate = \"open\"\n")]);
    let loaded = levels::load(&Sources {
        organisation: Some(organisation(
            None,
            Some(
                "[policy]\n\"guards.merge\" = \"off\"\n\n\
                 [policy.locks]\nfamilies = [\"guards.merge\"]\n",
            ),
        )),
        ..sources(root.path())
    });
    assert_eq!(loaded.mode_for("guards.merge", ANYONE), Mode::Enforce);
    let said = notice(&loaded, "never runs below enforce");
    assert_eq!(
        (said.file.as_str(), said.line),
        ("/etc/purlis/policy.toml", 2)
    );
}

#[test]
fn an_unknown_template_reads_strict_and_an_unknown_mode_enforce_and_the_file_still_reads() {
    purlis_core::unsteered!();
    let root = plane(&[("charter.toml", "[policy]\ntemplate = \"open\"\n")]);
    for (bad, words, line) in [
        ("[policy]\ntemplate = \"lenient\"\n", "reads as strict", 2),
        (
            "[policy]\n\"guards.git\" = \"sometimes\"\n",
            "so it is enforced",
            2,
        ),
        (
            "[policy.rules]\n\"guards-merge-hold\" = 1\n",
            "so it is enforced",
            2,
        ),
    ] {
        let loaded = levels::load(&Sources {
            organisation: Some(organisation(Some(NO_INSTALLS), Some(bad))),
            ..sources(root.path())
        });
        let said = notice(&loaded, words);
        assert_eq!(
            (said.file.as_str(), said.line),
            ("/etc/purlis/policy.toml", line),
            "{bad}"
        );
        // Not broken: no lock on everything, no full sandbox, and its rules still bind.
        assert!(loaded.organisation_locks.families.is_empty(), "{bad}");
        assert_eq!(loaded.sandbox_at_least, None, "{bad}");
        assert_eq!(
            install(&loaded, "web/builder"),
            denied("skills-none"),
            "{bad}"
        );
    }
    let loaded = levels::load(&Sources {
        organisation: Some(organisation(
            None,
            Some("[policy]\n\"guards.git\" = \"sometimes\"\n"),
        )),
        ..sources(root.path())
    });
    assert_eq!(
        loaded.mode_for("guards.git", ANYONE),
        Mode::Off,
        "the project's open is nearer"
    );
}

#[test]
fn a_project_cannot_widen_an_organisations_set_that_lets_through() {
    purlis_core::unsteered!();
    let root = plane(&[(
        "charter.toml",
        "[policy.sets]\nknown-hosts = [\"evil.example\"]\nproduction-hosts = [\"db.acme.com\"]\n",
    )]);
    let loaded = levels::load(&Sources {
        organisation: Some(organisation(
            None,
            Some(
                "[policy.sets]\nknown-hosts = [\"api.acme.com\"]\n\
                 production-hosts = [\"api.acme.com\"]\n",
            ),
        )),
        ..sources(root.path())
    });
    assert_eq!(loaded.sets["known-hosts"], ["api.acme.com"]);
    let said = notice(&loaded, "\"evil.example\" is left out");
    assert_eq!((said.file.as_str(), said.line), ("charter.toml", 2));
    // A set whose rules forbid what is in it only tightens as it grows.
    assert_eq!(
        loaded.sets["production-hosts"],
        ["api.acme.com", "db.acme.com"]
    );
}

#[test]
fn a_project_lock_on_one_action_drops_a_lower_permit_on_it() {
    purlis_core::unsteered!();
    let root = plane(&[
        ("charter.toml", "[policy.locks]\nactions = [\"install\"]\n"),
        (
            "policy.cedar",
            "// purlis-policy 1\n\
             @id(\"skills-web-install\") @level(\"workspace\")\n\
             permit (principal in Workspace::\"web\", action == Action::\"install\", resource);\n\
             @id(\"skills-web-load\") @level(\"workspace\")\n\
             permit (principal in Workspace::\"web\", action == Action::\"load\", resource);\n",
        ),
    ]);
    let loaded = levels::load(&sources(root.path()));
    assert!(loaded.policy.rule("skills-web-install").is_none());
    assert!(
        loaded.policy.rule("skills-web-load").is_some(),
        "another action is not locked"
    );
    let said = notice(&loaded, "\"install\" is locked");
    assert_eq!((said.file.as_str(), said.line), ("charter.toml", 2));
    assert_eq!(said.rules[0].id, "skills-web-install");
}

// ── M75 slice 10 (#7019): modes and rule modes (spec §6.4, §7.4) ─────────────────────────────

/// **Spec §7.4's table**, written out apart from `Template::mode`: how `template` runs `family`.
/// The `sandbox` family's is its level's (slice 11): off at the level `off`, else enforced.
fn the_table(template: Template, family: &str) -> Mode {
    let secret = ["secrets", "vaults", "guards.credential"].contains(&family);
    let q308b = [
        "guards.forge",
        "guards.shape",
        "guards.plane",
        "mode",
        "tools",
        "trust",
        "runners",
        "consents",
        "limits",
    ];
    match (family, template) {
        // The record is the same in every template and is never off or log.
        ("audit", _) => Mode::Enforce,
        // The person's own credential files at the floor: `team` and `strict` (Q325).
        ("secrets.credential_files", Template::Team | Template::Strict) => Mode::Enforce,
        ("secrets.credential_files", _) => Mode::Off,
        ("sandbox", Template::Open) => Mode::Off,
        ("sandbox", _) => Mode::Enforce,
        (_, Template::Open) => Mode::Off,
        (_, Template::Strict) => Mode::Enforce,
        // `vaults` takes off or enforce only, so where the table says log it enforces.
        (_, Template::Watch) if secret => Mode::Enforce,
        (_, Template::Hold) if secret || family == "guards.merge" => Mode::Enforce,
        (_, Template::Team)
            if secret
                || q308b.contains(&family)
                || ["guards.merge", "guards.release", "guards.save"].contains(&family) =>
        {
            Mode::Enforce
        }
        // An unapproved profile or declaration still asks (Q329).
        (_, Template::Watch | Template::Hold) if family == "trust" => Mode::Enforce,
        // `ports` takes off or enforce only (Q328), so where the table says log it enforces.
        _ if family == "ports" => Mode::Enforce,
        _ => Mode::Log,
    }
}

#[test]
fn each_template_gives_its_mode_table() {
    purlis_core::unsteered!();
    for template in Template::ALL {
        let root = plane(&[(
            "charter.toml",
            &format!("[policy]\ntemplate = \"{}\"\n", template.word()),
        )]);
        let loaded = levels::load(&sources(root.path()));
        assert_eq!(loaded.template_for(ANYONE), template);
        for family in policy::FAMILIES {
            assert_eq!(
                loaded.mode_for(family, ANYONE),
                the_table(template, family),
                "{} under {}",
                family,
                template.word()
            );
        }
    }
    // The sandbox level each template sets (slice 11 makes it `[policy] sandbox` for `strict`).
    use purlis_core::enforcement::SandboxLevel::{Floor, Full, Off};
    let levels: Vec<_> = Template::ALL.map(Template::sandbox_level).into();
    assert_eq!(
        levels,
        [Some(Off), Some(Floor), Some(Floor), Some(Full), None]
    );
}

#[test]
fn log_on_a_permit_reads_as_enforce_with_a_notice() {
    purlis_core::unsteered!();
    let root = plane(&[
        (
            "charter.toml",
            "[policy.rules]\n\"skills-allow-pdf\" = \"log\"\n",
        ),
        (
            "policy.cedar",
            "// purlis-policy 1\n@id(\"skills-allow-pdf\")\n\
             permit (principal, action == Action::\"install\", resource == Skill::\"pdf\");\n",
        ),
    ]);
    let loaded = levels::load(&sources(root.path()));
    assert_eq!(loaded.rule_modes["skills-allow-pdf"], Mode::Enforce);
    let said = notice(&loaded, "a permit is off or enforced");
    assert_eq!((said.file.as_str(), said.line), ("charter.toml", 2));
    assert_eq!(said.rules[0].id, "skills-allow-pdf");
    // A forbid keeps `log`.
    let root = plane(&[(
        "charter.toml",
        "[policy.rules]\n\"guards-merge-hold\" = \"log\"\n",
    )]);
    let loaded = levels::load(&sources(root.path()));
    assert_eq!(loaded.rule_modes["guards-merge-hold"], Mode::Log);
    assert!(
        loaded
            .notices
            .iter()
            .all(|n| !n.what.contains("is off or enforced")),
        "{:#?}",
        loaded.notices
    );
}

#[test]
fn log_on_a_vaults_rule_reads_as_enforce_with_a_notice() {
    purlis_core::unsteered!();
    let root = plane(&[(
        "charter.toml",
        "[policy.rules]\n\"vaults-other-persona\" = \"log\"\n\"secrets-reveal\" = \"log\"\n",
    )]);
    let loaded = levels::load(&sources(root.path()));
    assert_eq!(loaded.rule_modes["vaults-other-persona"], Mode::Enforce);
    let said = notice(&loaded, "a vaults rule is off or enforced");
    assert_eq!((said.file.as_str(), said.line), ("charter.toml", 2));
    assert_eq!(said.rules[0].id, "vaults-other-persona");
    // A rule of another family keeps `log`.
    assert_eq!(loaded.rule_modes["secrets-reveal"], Mode::Log);
    // `off` is still allowed on a vaults rule.
    let root = plane(&[(
        "charter.toml",
        "[policy.rules]\n\"vaults-other-persona\" = \"off\"\n",
    )]);
    let loaded = levels::load(&sources(root.path()));
    assert_eq!(loaded.rule_modes["vaults-other-persona"], Mode::Off);
}

#[test]
fn a_project_rule_mode_cannot_reach_an_organisation_rule() {
    purlis_core::unsteered!();
    let root = plane(&[(
        "charter.toml",
        "[policy.rules]\n\"skills-org-none\" = \"off\"\n",
    )]);
    let org_rules = "// purlis-policy 1\n\
         @id(\"skills-org-none\")\n\
         forbid (principal, action == Action::\"install\", resource);\n";
    let loaded = levels::load(&Sources {
        organisation: Some(organisation(Some(org_rules), None)),
        ..sources(root.path())
    });
    let said = notice(&loaded, "which is above this file");
    assert_eq!((said.file.as_str(), said.line), ("charter.toml", 2));
    assert_eq!(said.rules[0].level, Level::Organisation);
    // The Organisation's forbid is enforced, and still binds.
    assert_eq!(loaded.rule_modes["skills-org-none"], Mode::Enforce);
    assert_eq!(install(&loaded, "web/builder"), denied("skills-org-none"));
    // Not even `log` reaches it.
    let root = plane(&[(
        "charter.toml",
        "[policy]\ntemplate = \"open\"\n\n[policy.rules]\n\"skills-org-none\" = \"log\"\n",
    )]);
    let loaded = levels::load(&Sources {
        organisation: Some(organisation(Some(org_rules), None)),
        ..sources(root.path())
    });
    assert_eq!(loaded.rule_modes["skills-org-none"], Mode::Enforce);
    assert_eq!(install(&loaded, "web/builder"), denied("skills-org-none"));
}

#[test]
fn an_audit_mode_of_off_or_log_reads_as_enforce_with_a_notice() {
    purlis_core::unsteered!();
    for mode in ["off", "log"] {
        let root = plane(&[(
            "charter.toml",
            &format!("[policy]\ntemplate = \"open\"\naudit = \"{mode}\"\n"),
        )]);
        let loaded = levels::load(&sources(root.path()));
        assert_eq!(loaded.mode_for("audit", ANYONE), Mode::Enforce, "{mode}");
        let said = notice(&loaded, "it takes enforce only");
        assert_eq!(
            (said.file.as_str(), said.line),
            ("charter.toml", 3),
            "{mode}"
        );
        // The rest of the file reads: `open` still turns the others off.
        assert_eq!(loaded.mode_for("network", ANYONE), Mode::Off, "{mode}");
    }
    // The Organisation's file, too.
    let root = plane(&[("charter.toml", "")]);
    let loaded = levels::load(&Sources {
        organisation: Some(organisation(None, Some("[policy]\naudit = \"off\"\n"))),
        ..sources(root.path())
    });
    assert_eq!(loaded.mode_for("audit", ANYONE), Mode::Enforce);
    notice(&loaded, "it takes enforce only");
}

// ── M75-10 round 2 ───────────────────────────────────────────────────────────────────────────

#[test]
fn an_off_on_an_ask_rule_cannot_pass_an_asks_lock() {
    purlis_core::unsteered!();
    let root = plane(&[(
        "charter.toml",
        "[policy.rules]\n\"network-unknown-host\" = \"off\"\n",
    )]);
    let loaded = levels::load(&Sources {
        organisation: Some(organisation(
            None,
            Some("[policy.locks]\nfamilies = [\"asks\"]\n"),
        )),
        ..sources(root.path())
    });
    assert!(loaded.policy.rule("network-unknown-host").is_some());
    notice(&loaded, "is locked by Platform team");
}

#[test]
fn the_organisations_rule_modes_read_log_on_a_vaults_rule_as_enforce_outside_a_plane() {
    purlis_core::unsteered!();
    let org = organisation(
        None,
        Some("[policy.rules]\n\"vaults-other-persona\" = \"log\"\n"),
    );
    let loaded = Loaded::shipped_strict_under(Some(&org));
    assert_eq!(loaded.rule_modes["vaults-other-persona"], Mode::Enforce);
    notice(&loaded, "a vaults rule is off or enforced");
}

#[test]
fn a_project_cannot_replace_a_rule_the_organisation_names_in_its_rule_modes() {
    purlis_core::unsteered!();
    let root = with_rules(
        "@id(\"guards-merge-hold\")\n\
         permit (principal, action == Action::\"merge\", resource);\n",
    );
    let loaded = levels::load(&Sources {
        organisation: Some(organisation(
            None,
            Some("[policy.rules]\n\"guards-merge-hold\" = \"enforce\"\n"),
        )),
        ..sources(root.path())
    });
    let said = notice(&loaded, "in its own [policy.rules]");
    assert_eq!((said.file.as_str(), said.line), ("policy.cedar", 2));
    // The shipped forbid stands.
    assert_eq!(
        merge(&loaded),
        Decision::Ask {
            asks: vec!["guards-merge-hold".into()]
        }
    );
    // A shipped rule the Organisation does not name can still be replaced.
    let loaded = levels::load(&Sources {
        organisation: Some(organisation(None, Some("[policy]\ntemplate = \"watch\"\n"))),
        ..sources(root.path())
    });
    assert!(merge(&loaded).allows(), "{:?}", merge(&loaded));
}

/// The project's `policy.cedar` as `text`, loaded as the host loads a version.
fn load_rules(text: &str) -> Loaded {
    let rules = format!("{VERSION}{text}");
    levels::load_held(
        &sources(Path::new("/nonexistent-plane")),
        &levels::Held {
            settings: None,
            rules: Some(&rules),
            local: None,
            shipped: None,
        },
    )
}

#[test]
fn a_sandbox_rule_outside_the_subset_is_refused_at_load_under_the_full_sandbox() {
    purlis_core::unsteered!();
    let loaded = load_rules(
        "@id(\"sandbox-web-only\") @level(\"project\")\n\
         forbid (principal, action == Action::\"read\", resource) when { \
         principal.workspace == Workspace::\"web\" && resource in Workspace::\"api\" \
         && resource in Path::\"/plane/x\" };\n",
    );
    assert!(
        loaded
            .notices
            .iter()
            .any(|n| n.rules.iter().any(|r| r.id == "sandbox-web-only")),
        "{:?}",
        loaded.notices
    );
    assert_eq!(loaded.sandbox_at_least, Some(SandboxLevel::Full));
    assert!(loaded.policy.rule("sandbox-web-only").is_none());
}

#[test]
fn a_path_literal_not_spelt_normally_is_refused_at_load_on_every_action_that_takes_a_path() {
    purlis_core::unsteered!();
    for (action, family) in [
        ("read", "sandbox"),
        ("write", "sandbox"),
        ("expose", "guards-credential"),
        ("propose", "guards-plane"),
        ("upload", "browser"),
        ("download", "browser"),
    ] {
        for literal in ["/plane/a/../b", "relative/path", "/plane/a/"] {
            let id = format!("{family}-dotted");
            let loaded = load_rules(&format!(
                "@id(\"{id}\") @level(\"project\")\n\
                 forbid (principal, action == Action::\"{action}\", resource) \
                 when {{ resource == Path::\"{literal}\" }};\n"
            ));
            let said = notice(&loaded, "path literal");
            assert_eq!(
                said.rules.first().map(|r| r.id.as_str()),
                Some(id.as_str()),
                "{action} {literal}"
            );
            assert!(
                loaded.policy.rule(&id).is_none(),
                "{action} {literal}: the rule is refused"
            );
        }
    }
}

#[test]
fn a_normal_absolute_path_literal_loads_clean() {
    purlis_core::unsteered!();
    let loaded = load_rules(
        "@id(\"sandbox-normal\") @level(\"project\")\n\
         forbid (principal, action == Action::\"read\", resource) \
         when { resource == Path::\"/plane/a/b\" };\n",
    );
    assert!(loaded.notices.is_empty(), "{:?}", loaded.notices);
    assert!(loaded.policy.rule("sandbox-normal").is_some());
}
