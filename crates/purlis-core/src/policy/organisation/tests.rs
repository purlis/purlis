// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! The Organisation's folder: found, judged on every OS, and `policy.json` translated (M75-06).

use super::*;
use crate::enforcement::SandboxLevel;
use crate::policy::FAMILIES;
use crate::policy::levels::Loaded;
use crate::sandbox::policy::{AclEntry, Seen, acl_lets_others_write, judged_as, set_for_this_test};

/// A folder of the test's own, which the test is the administrator of, holding `files`.
#[cfg(unix)]
fn folder(mode: u32, files: &[(&str, &str, u32)]) -> (tempfile::TempDir, u32) {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let dir = tempfile::tempdir().expect("a folder");
    for (name, text, file_mode) in files {
        let path = dir.path().join(name);
        std::fs::write(&path, text).expect("a file");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(*file_mode))
            .expect("its mode");
    }
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(mode)).expect("its mode");
    let me = std::fs::metadata(dir.path()).expect("it is there").uid();
    (dir, me)
}

/// Whether `found` reads as `strict` plus every lock, under the full sandbox.
fn is_strict_plus_every_lock(found: &Found) -> bool {
    let loaded = Loaded::shipped_strict_under(Some(&found.as_sources()));
    FAMILIES
        .iter()
        .all(|family| loaded.organisation_locks.families.contains(*family))
        && loaded.organisation_locks.keep.as_deref() == Some("once")
        && loaded.organisation_locks.timed.as_deref() == Some("off")
        && loaded.sandbox_at_least == Some(SandboxLevel::Full)
}

const RULES: &str = "// purlis-policy 1\n@id(\"skills-organisation-none\") @level(\"organisation\")\n\
                     forbid (principal, action == Action::\"install\", resource);\n";

#[cfg(unix)]
#[test]
fn an_organisation_folder_a_standard_user_can_write_reads_as_strict_plus_every_lock() {
    // The same folder, with its rules inside, where the group, anyone else, or a user who is not
    // the administrator could write it or own it.
    for (mode, file_mode, who_admin, check) in [
        (0o775, 0o644, 0, Check::Bits),
        (0o757, 0o644, 0, Check::Bits),
        (0o755, 0o664, 0, Check::Bits),
        (0o755, 0o646, 0, Check::Bits),
        (0o755, 0o644, 1, Check::Owner),
    ] {
        let (dir, me) = folder(mode, &[(RULES_FILE, RULES, file_mode)]);
        let found = read_folder(dir.path(), me + who_admin).expect("the folder is there");
        assert!(
            is_strict_plus_every_lock(&found),
            "{mode:o}/{file_mode:o}: {found:?}"
        );
        let said = found.failed.as_deref().expect("it failed");
        assert!(said.contains(check.name()), "{said}");
        assert!(found.rules.is_none(), "no file of a failed folder is read");
    }
    // The control: a folder only its administrator writes is read, and its rules apply.
    let (dir, me) = folder(0o755, &[(RULES_FILE, RULES, 0o644)]);
    let found = read_folder(dir.path(), me).expect("the folder is there");
    assert_eq!(found.failed, None);
    assert_eq!(found.rules.as_deref(), Some(RULES));
    assert!(!is_strict_plus_every_lock(&found));
}

#[cfg(unix)]
#[test]
fn an_untrusted_organisation_folder_with_no_file_still_reads_as_strict_plus_every_lock() {
    // Whoever could write it could take the file out of it (D-1343-11).
    let (dir, me) = folder(0o777, &[]);
    let found = read_folder(dir.path(), me).expect("the folder is there");
    assert!(is_strict_plus_every_lock(&found), "{found:?}");
}

#[cfg(unix)]
#[test]
fn no_organisation_folder_is_no_organisation() {
    let dir = tempfile::tempdir().expect("a folder");
    assert_eq!(read_folder(&dir.path().join("purlis"), 0), None);
}

#[test]
fn an_organisation_file_whose_macos_acl_grants_write_reads_as_strict_plus_every_lock() {
    let seen = |acl: &[AclEntry]| Seen {
        link: false,
        expected_type: true,
        uid: 0,
        mode: 0o100_644,
        acl_writes: acl_lets_others_write(acl),
    };
    let folder = Seen {
        expected_type: true,
        mode: 0o40_755,
        ..seen(&[])
    };
    let allows_write = AclEntry {
        allow: true,
        writes: true,
        root: false,
    };
    // An ACL that lets someone but root write the file, or the folder, is refused as the ACL.
    let refused = judged_as(0, Some(seen(&[allows_write])), folder).expect_err("a write ACL");
    assert_eq!(refused.check, Check::Acl);
    let refused = judged_as(
        0,
        Some(seen(&[])),
        Seen {
            acl_writes: acl_lets_others_write(&[allows_write]),
            ..folder
        },
    )
    .expect_err("a write ACL on the folder");
    assert_eq!(refused.check, Check::Acl);
    // Read-only entries, denies and root's own entries change nothing.
    for fine in [
        AclEntry {
            writes: false,
            ..allows_write
        },
        AclEntry {
            allow: false,
            ..allows_write
        },
        AclEntry {
            root: true,
            ..allows_write
        },
    ] {
        assert_eq!(
            judged_as(0, Some(seen(&[fine])), folder),
            Ok(()),
            "{fine:?}"
        );
    }
    // And the folder that carries it reads as strict plus every lock.
    let found = Found::failed(Path::new("/etc/purlis"), &refused);
    assert!(is_strict_plus_every_lock(&found));
}

#[cfg(target_os = "macos")]
#[test]
fn a_real_macos_acl_that_grants_write_is_read_and_refused() {
    use exacl::{AclEntry as Entry, Perm};
    let (dir, me) = folder(0o755, &[(RULES_FILE, RULES, 0o644)]);
    let file = dir.path().join(RULES_FILE);
    assert!(
        read_folder(dir.path(), me).expect("there").failed.is_none(),
        "no ACL, no refusal"
    );
    exacl::setfacl(
        &[&file],
        &[Entry::allow_group(
            "everyone",
            Perm::WRITE | Perm::READ,
            None,
        )],
        None,
    )
    .expect("an ACL");
    let found = read_folder(dir.path(), me).expect("there");
    let said = found.failed.as_deref().expect("the ACL is refused");
    assert!(said.contains(Check::Acl.name()), "{said}");
    assert!(is_strict_plus_every_lock(&found));
}

fn admin(who: Principal, aces: &[(Principal, bool, bool)]) -> Described {
    Described {
        link: false,
        expected_type: true,
        owner: who,
        aces: aces
            .iter()
            .map(|(who, allow, writes)| Ace {
                who: who.clone(),
                allow: *allow,
                writes: *writes,
            })
            .collect(),
    }
}

fn user() -> Principal {
    Principal::Other("S-1-5-21-1-2-3-1001".to_owned())
}

#[test]
fn a_windows_folder_owned_by_a_standard_user_is_judged_failed() {
    let good = admin(
        Principal::Administrators,
        &[
            (Principal::Administrators, true, true),
            (Principal::System, true, true),
            (Principal::Other("Users".to_owned()), true, false),
        ],
    );
    assert_eq!(judge_windows(&good, Some(&good)), Ok(()));
    // A standard user can create %ProgramData%\purlis, and then owns it.
    let owned = admin(user(), &[(Principal::Administrators, true, true)]);
    let refused = judge_windows(&owned, None).expect_err("a user owns it");
    assert_eq!(refused.check, Check::Owner);
    // Owned by Administrators, but an entry lets a user, or Everyone, write.
    for who in [user(), Principal::Other("Everyone".to_owned())] {
        let open = admin(Principal::System, &[(who, true, true)]);
        assert_eq!(
            judge_windows(&open, None)
                .expect_err("it is writable")
                .check,
            Check::Acl
        );
        // And its file, whatever the folder says.
        assert_eq!(
            judge_windows(&good, Some(&open))
                .expect_err("its file")
                .check,
            Check::Acl
        );
    }
    // A link, or a file that is not one.
    let link = Described {
        link: true,
        ..good.clone()
    };
    assert_eq!(
        judge_windows(&link, None).expect_err("a link").check,
        Check::Link
    );
    let not_a_file = Described {
        expected_type: false,
        ..good.clone()
    };
    assert_eq!(
        judge_windows(&good, Some(&not_a_file))
            .expect_err("a folder")
            .check,
        Check::Kind
    );
    // A folder that fails reads as strict plus every lock.
    assert!(is_strict_plus_every_lock(&Found::failed(
        Path::new("C:\\ProgramData\\purlis"),
        &refused
    )));
}

#[test]
fn the_windows_organisation_folder_is_found_by_the_known_folder_api_not_the_environment() {
    // The lookup is the API's, handed in: what it answers is the folder, whatever
    // `%ProgramData%` says, and where it does not answer there is no folder to look in.
    let found = windows_folder(|| Some(PathBuf::from("C:\\ProgramData")));
    assert_eq!(found, Some(PathBuf::from("C:\\ProgramData").join("purlis")));
    assert_eq!(windows_folder(|| None), None);
    // And nothing in this module reads the environment: a chat could set a variable.
    let source = include_str!("../organisation.rs");
    for line in source.lines().filter(|l| !l.trim_start().starts_with("//")) {
        for read in ["env::var", "var_os", "env!(", "option_env"] {
            assert!(!line.contains(read), "the environment is read: {line}");
        }
    }
    assert!(source.contains("FOLDERID_ProgramData"));
}

#[test]
fn a_failed_organisation_folder_notice_offers_the_admin_a_way_out() {
    let (check, why) = (
        Check::Bits,
        "its folder can be written by others than the administrator",
    );
    let found = Found::failed(
        Path::new("/etc/purlis"),
        &Refused {
            check,
            why: why.to_owned(),
        },
    );
    let loaded = Loaded::shipped_strict_under(Some(&found.as_sources()));
    let notice = loaded
        .notices
        .iter()
        .find(|notice| notice.file == "/etc/purlis")
        .expect("a Notice names the folder");
    // The check, by name, what was found, what it reads as, and both ways out.
    for said in [
        check.name(),
        why,
        "strict with every lock",
        "fix it",
        "remove the folder",
    ] {
        assert!(notice.what.contains(said), "{said}: {}", notice.what);
    }
}

#[test]
fn the_organisations_policy_json_is_translated_and_adds_to_its_policy_cedar() {
    let json = r#"{
        "owner": "Platform team",
        "sandbox": {
            "hosts": ["api.example.com"],
            "never-hosts": ["evil.example"],
            "opt-out": false,
            "write-grants": false,
            "allow-scopes": ["once", "chat"]
        },
        "dispatch": { "running-per-chat": 4 },
        "enforcement": ["guards.merge"]
    }"#;
    let translated = translate(json, Path::new("/etc/purlis/policy.json")).expect("it translates");
    assert_eq!(translated.owner, "Platform team");
    let found = Found {
        folder: "/etc/purlis".to_owned(),
        owner: translated.owner.clone(),
        rules: Some(RULES.to_owned()),
        settings: Some(
            "[policy.locks]\nfamilies = [\"vaults\"]\nkeep = \"this workspace\"\n".to_owned(),
        ),
        json: Some(Ok(translated)),
        failed: None,
    };
    let loaded = Loaded::shipped_strict_under(Some(&found.as_sources()));
    assert_eq!(loaded.notices, [], "nothing is broken");
    // policy.cedar's rule, and the translation's, both stand: it adds.
    for rule in [
        "skills-organisation-none",
        "network-organisation-host-0",
        "network-organisation-fixed-hosts",
        "network-organisation-never-0",
    ] {
        assert!(loaded.policy.rule(rule).is_some(), "{rule}");
    }
    // policy.toml's locks and the translation's, together, and the narrower scope wins.
    let locks = &loaded.organisation_locks;
    for family in ["vaults", "sandbox", "network", "guards.merge"] {
        assert!(locks.families.contains(family), "{family}: {locks:?}");
    }
    assert!(locks.actions.contains("write"));
    assert_eq!(locks.keep.as_deref(), Some("this chat"));
}

#[test]
fn an_unknown_policy_json_key_reads_as_strict_plus_every_lock() {
    for json in [
        r#"{"sandbox": {"made-up": true}}"#,
        r#"{"made-up": 1}"#,
        r#"{"dispatch": {"made-up": 1}}"#,
        r#"{"sandbox": {"opt-out": "no"}}"#,
        "not json",
    ] {
        let found = Found {
            folder: "/etc/purlis".to_owned(),
            owner: "the Organisation".to_owned(),
            rules: Some(RULES.to_owned()),
            settings: None,
            json: Some(translate(json, Path::new("/etc/purlis/policy.json"))),
            failed: None,
        };
        assert!(found.json.as_ref().expect("read").is_err(), "{json}");
        assert!(is_strict_plus_every_lock(&found), "{json}");
        let loaded = Loaded::shipped_strict_under(Some(&found.as_sources()));
        assert!(
            loaded
                .notices
                .iter()
                .any(|n| n.file == "/etc/purlis/policy.json"),
            "a Notice names the file: {json}"
        );
        // Its own rules are not read in part: the broken file is never skipped.
        assert!(loaded.policy.rule("skills-organisation-none").is_none());
    }
}

#[test]
fn a_test_build_reads_the_organisation_level_from_test_locks_never_etc_purlis() {
    // Empty by default, whatever this machine's /etc/purlis holds.
    assert_eq!(in_force(), None);
    // The seam's locks are the Organisation, translated.
    set_for_this_test(Locks::parse(
        r#"{"owner": "Seam", "sandbox": {"never-hosts": ["evil.example"]}}"#,
        Path::new("/etc/purlis/policy.json"),
    ));
    let found = in_force().expect("the seam's Organisation");
    assert_eq!(found.owner, "Seam");
    let loaded = Loaded::shipped_strict_under(Some(&found.as_sources()));
    assert!(loaded.policy.rule("network-organisation-never-0").is_some());
    // A refused seam reads as strict plus every lock.
    set_for_this_test(Locks::parse(
        "not json",
        Path::new("/etc/purlis/policy.json"),
    ));
    assert!(is_strict_plus_every_lock(&in_force().expect("refused")));
    // Another thread has none: the seam is the test's own.
    assert_eq!(std::thread::spawn(in_force).join().expect("a thread"), None);
    // And the test build never reaches for the folder or the environment.
    let source = include_str!("../organisation.rs");
    let test_build = source
        .split("pub fn in_force() -> Option<Found> {")
        .nth(2)
        .and_then(|rest| rest.split("\n}\n").next())
        .expect("the test build's in_force");
    assert!(!test_build.contains("read_folder") && !test_build.contains("folder()"));
}
