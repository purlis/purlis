// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::forge::recorded::Recorded;
use crate::forge::signingkeys::{recording, recording_with};
use crate::secrets::keyring;

fn on_account(signer: &Signer) -> Asked {
    Asked::KeyOnAccount {
        key: signer.public_line(),
        on: None,
    }
}

/// No key, then made (in the fenced file keychain), then not on the account, then added by
/// the window's sign-in and listed, then a pushed commit Verified: the whole of setup that does
/// not wait on the person, as the forge answers it.
#[test]
fn no_key_then_made_then_added_then_listed_then_verified() {
    let config = tempfile::tempdir().unwrap();
    let keys = tempfile::tempdir().unwrap();
    crate::machine::device_id(config.path()).unwrap();
    let store = keyring::FileStore::at(keys.path().join("keyring.json"));

    assert_eq!(
        crate::commitsign::kept(config.path(), &store).unwrap(),
        None
    );
    let (signer, had) = key_made(config.path(), &store).unwrap();
    assert!(!had);
    assert_eq!(
        crate::commitsign::kept(config.path(), &store).unwrap(),
        Some(signer.clone())
    );
    let (again, had) = key_made(config.path(), &store).unwrap();
    assert!(had);
    assert_eq!(again, signer, "a second setup keeps the key it made");

    // Whether the account has it is read from GitHub's public list of its signing keys.
    let none = recording("publicly_none", &signer);
    assert_eq!(
        answered_over(&none, &github(), "octocat", &on_account(&signer), None).unwrap(),
        Heard::KeyOnAccount { on: false }
    );

    let added = recording("added_then_listed", &signer);
    signingkeys::add_over(&added, &github(), &signer, "purlis · studio").unwrap();
    assert!(
        signingkeys::listed_over(&added, &github())
            .unwrap()
            .this_device(&signer)
            .is_some()
    );
    assert!(added.unspent().is_empty());
    let public = recording("publicly_listed", &signer);
    assert_eq!(
        answered_over(&public, &github(), "octocat", &on_account(&signer), None).unwrap(),
        Heard::KeyOnAccount { on: true }
    );

    let verified = recording("commit_verified", &signer);
    let asked = Asked::Check {
        repo: "octo/hello".to_owned(),
        commit: "0123abc".to_owned(),
        code: "A1B2C3".to_owned(),
        on: None,
    };
    let Heard::Checked(checked) =
        answered_over(&verified, &github(), "octocat", &asked, None).unwrap()
    else {
        panic!("a check is answered with what the forge said of the commit");
    };
    assert!(checked.verified);
}

#[test]
fn what_is_not_one_public_key_is_refused_before_anything_is_sent() {
    let recorded = Recorded::parse(r#"{"source": "nothing", "exchanges": []}"#).unwrap();
    for key in [
        "",
        "ssh-ed25519",
        "not a key at all",
        "ssh-ed25519 AAAA\nssh-rsa BBBB",
    ] {
        let asked = Asked::KeyOnAccount {
            key: key.to_owned(),
            on: None,
        };
        assert!(
            answered_over(&recorded, &github(), "octocat", &asked, None).is_err(),
            "{key:?}"
        );
    }
}

#[test]
fn the_key_is_removed_from_the_keychain() {
    let config = tempfile::tempdir().unwrap();
    let keys = tempfile::tempdir().unwrap();
    crate::machine::device_id(config.path()).unwrap();
    let store = keyring::FileStore::at(keys.path().join("keyring.json"));
    key_made(config.path(), &store).unwrap();
    assert!(key_removed(config.path(), &store).unwrap());
    assert_eq!(
        crate::commitsign::kept(config.path(), &store).unwrap(),
        None
    );
    assert!(!key_removed(config.path(), &store).unwrap());
}

#[cfg(unix)]
mod over_the_window_line {
    use std::time::Duration;

    use super::*;
    use crate::windowline::{self, Answered, Said};

    /// The command asks over the line; the window answers with its own transport (here a
    /// recording); the command hears what the forge said, and holds no sign-in of its own.
    #[test]
    fn the_command_hears_what_the_window_asked_github() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("purlis").join("window.sock");
        let signer = Signer::generate().unwrap();
        let line = signer.public_line();
        let recorded = std::sync::Arc::new(recording("publicly_listed", &signer));
        let theirs = std::sync::Arc::clone(&recorded);
        windowline::serve_on(&socket, move |asked| match asked {
            windowline::Asked::Signing(asked) => {
                match answered_over(theirs.as_ref(), &github(), "octocat", &asked, None) {
                    Ok(heard) => Said::Signing(heard),
                    Err(why) => Said::Refused { why },
                }
            }
            _ => Said::Refused {
                why: "not this test's".to_owned(),
            },
        })
        .unwrap();
        let within = Duration::from_secs(10);
        let reached = reach(
            Asked::KeyOnAccount {
                key: line,
                on: None,
            },
            |asked| windowline::ask_on(&socket, asked, within, &mut |_| {}),
        );
        assert_eq!(reached, Reached::Heard(Heard::KeyOnAccount { on: true }));
    }

    #[test]
    fn no_window_no_answer_and_another_questions_answer_are_told_apart() {
        let asked = || Asked::Check {
            repo: "octo/hello".to_owned(),
            commit: "0123abc".to_owned(),
            code: "A1B2C3".to_owned(),
            on: None,
        };
        assert_eq!(reach(asked(), |_| Answered::NoWindow), Reached::NoWindow);
        assert_eq!(reach(asked(), |_| Answered::NoAnswer), Reached::NoAnswer);
        assert_eq!(
            reach(asked(), |_| Answered::Said(Said::Asked { note: None })),
            Reached::OtherAnswer
        );
        assert_eq!(
            reach(asked(), |_| Answered::Said(Said::Refused {
                why: "not signed in".to_owned()
            })),
            Reached::Refused("not signed in".to_owned())
        );
    }

    /// The profile lines' answer never takes a signing ask for a profile's.
    #[test]
    fn a_signing_ask_is_no_profiles() {
        let dir = tempfile::tempdir().unwrap();
        let said = windowline::answer(
            dir.path(),
            windowline::Asked::Signing(Asked::KeyOnAccount {
                key: "k".into(),
                on: None,
            }),
            |_| panic!("no forget"),
            |_, _| panic!("no keep"),
            |_, _, _| panic!("no approve"),
            |_, _, _| panic!("no take-off"),
        );
        assert!(matches!(said, Said::Refused { .. }));
    }
}

mod the_check {
    use super::*;

    const TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
    const WHEN: i64 = 1_794_067_200;

    fn who() -> Who {
        Who {
            name: "Octo Cat".to_owned(),
            email: "octocat@example.invalid".to_owned(),
        }
    }

    /// The recording `case`, this test's own signature of the test commit put in.
    fn signed(case: &str, signer: &Signer) -> Recorded {
        let signature = signer.sign(&test_commit(TREE, &who(), WHEN)).unwrap();
        recording_with(case, &[("@SIGNATURE@", &signature)])
    }

    #[test]
    fn a_test_commit_github_verifies_is_verified() {
        let signer = Signer::generate().unwrap();
        let recorded = signed("test_commit_verified", &signer);
        let checked =
            check_with_a_test_commit(&recorded, &github(), &signer, "octo/hello", &who(), WHEN)
                .unwrap();
        assert!(checked.verified);
        assert!(recorded.unspent().is_empty());
    }

    #[test]
    fn a_test_commit_whose_email_github_does_not_know_is_not_verified_and_says_why() {
        let signer = Signer::generate().unwrap();
        let recorded = signed("test_commit_email_unverified", &signer);
        let checked =
            check_with_a_test_commit(&recorded, &github(), &signer, "octo/hello", &who(), WHEN)
                .unwrap();
        assert!(!checked.verified);
        assert_eq!(checked.reason, "unverified_email");
    }

    /// GitHub picks the account to verify an SSH signature against by the commit's email, so a
    /// key on the account and an email that isn't one of its verified ones reads as a key
    /// GitHub doesn't know (#5916).
    #[test]
    fn an_unknown_key_listed_on_the_account_under_an_unverified_email_is_the_emails_doing() {
        let signer = Signer::generate().unwrap();
        for reason in ["unknown_key", "not_signing_key"] {
            let recorded = recording("key_listed_then_emails", &signer);
            let why = email_is_why(
                &recorded,
                &github(),
                &signer,
                "octocat",
                reason,
                "someone@else.example",
            );
            assert!(why, "{reason}");
            assert!(recorded.unspent().is_empty());
        }
    }

    #[test]
    fn an_unknown_key_under_a_verified_email_is_not_the_emails_doing() {
        let signer = Signer::generate().unwrap();
        let recorded = recording("key_listed_then_emails", &signer);
        assert!(!email_is_why(
            &recorded,
            &github(),
            &signer,
            "octocat",
            "unknown_key",
            "Work@Example.org",
        ));
    }

    #[test]
    fn an_unknown_key_the_account_does_not_list_is_the_keys_doing() {
        let signer = Signer::generate().unwrap();
        let recorded = recording("key_not_listed", &signer);
        assert!(!email_is_why(
            &recorded,
            &github(),
            &signer,
            "octocat",
            "unknown_key",
            "someone@else.example",
        ));
        assert!(recorded.unspent().is_empty());
    }

    /// Unread emails make no claim about the email: the window says only what it has seen.
    #[test]
    fn an_unknown_key_whose_accounts_emails_cant_be_read_is_not_put_down_to_the_email() {
        let signer = Signer::generate().unwrap();
        let recorded = recording("key_listed_emails_refused", &signer);
        assert!(!email_is_why(
            &recorded,
            &github(),
            &signer,
            "octocat",
            "unknown_key",
            "someone@else.example",
        ));
    }

    #[test]
    fn an_unknown_key_whose_key_list_cant_be_read_is_not_put_down_to_the_email() {
        let signer = Signer::generate().unwrap();
        let recorded = recording("key_list_refused", &signer);
        assert!(!email_is_why(
            &recorded,
            &github(),
            &signer,
            "octocat",
            "unknown_key",
            "someone@else.example",
        ));
        assert!(recorded.unspent().is_empty());
    }

    /// GitLab finds a key by its fingerprint, and its public list holds sign-in keys too: its
    /// `unknown_key` is the key's, and nothing more is asked.
    #[test]
    fn gitlabs_unknown_key_is_never_put_down_to_the_email() {
        let signer = Signer::generate().unwrap();
        let recorded = gitlab_recording("auth_key_listed_then_emails", &signer);
        assert!(!email_is_why(
            &recorded,
            &gitlab(),
            &signer,
            "sam",
            "unknown_key",
            "someone@else.example",
        ));
        assert_eq!(recorded.unspent().len(), 3);
    }

    /// Another reason asks GitHub nothing more: the recording has no answer to give.
    #[test]
    fn another_reason_is_never_the_emails_doing_and_asks_nothing() {
        let signer = Signer::generate().unwrap();
        let recorded = recording("key_not_listed", &signer);
        for reason in ["valid", "unverified_email", "expired_key", "other_key"] {
            assert!(!email_is_why(
                &recorded,
                &github(),
                &signer,
                "octocat",
                reason,
                "someone@else.example",
            ));
        }
        assert_eq!(recorded.unspent().len(), 1);
    }

    /// The test commit is the object git itself writes for the same fields, byte for byte: what
    /// GitHub rebuilds from them and checks the signature over.
    #[test]
    fn the_test_commit_is_what_git_writes_for_the_same_fields() {
        let repo = tempfile::tempdir().unwrap();
        crate::testgit::run(repo.path(), &["init", "-q"]);
        let date = format!("{WHEN} +0000");
        let made = crate::forklock::output(
            std::process::Command::new("git")
                .arg("-C")
                .arg(repo.path())
                .args(["commit-tree", TREE, "-m", TEST_MESSAGE.trim_end()])
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_AUTHOR_NAME", "Octo Cat")
                .env("GIT_AUTHOR_EMAIL", "octocat@example.invalid")
                .env("GIT_AUTHOR_DATE", &date)
                .env("GIT_COMMITTER_NAME", "Octo Cat")
                .env("GIT_COMMITTER_EMAIL", "octocat@example.invalid")
                .env("GIT_COMMITTER_DATE", &date),
        )
        .unwrap();
        assert!(
            made.status.success(),
            "{}",
            String::from_utf8_lossy(&made.stderr)
        );
        let id = String::from_utf8(made.stdout).unwrap();
        let raw = crate::forklock::output(
            std::process::Command::new("git")
                .arg("-C")
                .arg(repo.path())
                .args(["cat-file", "commit", id.trim()]),
        )
        .unwrap()
        .stdout;
        assert_eq!(
            String::from_utf8(raw).unwrap(),
            String::from_utf8(test_commit(TREE, &who(), WHEN)).unwrap()
        );
    }

    #[test]
    fn with_no_repository_named_the_accounts_own_latest_is_checked_in() {
        let signer = Signer::generate().unwrap();
        let recorded = recording("own_repo", &signer);
        assert_eq!(
            a_repo_to_check_in(&recorded, &github()).unwrap().as_deref(),
            Some("octo/hello")
        );
    }
}

// GitLab (#5629).

fn gitlab() -> Forge {
    Forge::default_of(Kind::GitLab)
}

fn gitlab_recording(case: &str, signer: &Signer) -> Recorded {
    crate::forge::signingkeys::recording_of("gitlab", case, &[("@KEY@", &signer.public_line())])
}

fn pushed(kind: Kind, host: &str, repo: &str, commit: &str, at: i64) -> SignedPush {
    SignedPush {
        kind,
        host: host.to_owned(),
        repo: repo.to_owned(),
        commit: commit.to_owned(),
        at,
    }
}

/// GitLab's Check asks about the newest signed chat commit the app pushed to that GitLab, since
/// GitLab makes no signed commit of its own.
#[test]
fn gitlabs_check_reads_the_newest_signed_push_to_that_gitlab() {
    let config = tempfile::tempdir().unwrap();
    assert_eq!(last_signed_push(config.path(), &gitlab()), None);
    note_signed_push(
        config.path(),
        pushed(Kind::GitLab, "gitlab.com", "platform/billing", "0aa", 1),
    )
    .unwrap();
    note_signed_push(
        config.path(),
        pushed(Kind::GitHub, "github.com", "octo/hello", "0bb", 2),
    )
    .unwrap();
    note_signed_push(
        config.path(),
        pushed(Kind::GitLab, "git.example.org", "acme/api", "0cc", 3),
    )
    .unwrap();
    note_signed_push(
        config.path(),
        pushed(Kind::GitLab, "gitlab.com", "platform/billing", "0dd", 4),
    )
    .unwrap();
    let last = last_signed_push(config.path(), &gitlab()).unwrap();
    assert_eq!(
        (last.repo.as_str(), last.commit.as_str()),
        ("platform/billing", "0dd")
    );
    // One per repository: the push it replaced is gone.
    assert_eq!(signed_pushes(config.path()).len(), 3);
}

#[test]
fn a_pushed_commit_gitlab_verifies_with_this_devices_key_is_verified() {
    let signer = Signer::generate().unwrap();
    let recorded = gitlab_recording("commit_verified", &signer);
    let checked =
        check_a_pushed_commit(&recorded, &gitlab(), &signer, "platform/billing", "0123abc")
            .unwrap();
    assert!(checked.verified);
}

#[test]
fn a_pushed_commit_signed_with_another_key_says_nothing_of_this_one() {
    let signer = Signer::generate().unwrap();
    let other = Signer::generate().unwrap();
    // GitLab verified it, with `other`'s key.
    let recorded = gitlab_recording("commit_verified", &other);
    let checked =
        check_a_pushed_commit(&recorded, &gitlab(), &signer, "platform/billing", "0123abc")
            .unwrap();
    assert!(!checked.verified);
    assert_eq!(checked.reason, "other_key");
}

/// #5788: the email chosen for chat commits is kept per forge account, so a person whose GitHub
/// and GitLab addresses differ is Verified on both.
#[test]
fn the_email_chosen_for_chat_commits_is_kept_per_forge_account_until_it_is_cleared() {
    let config = tempfile::tempdir().unwrap();
    let octocat = AccountRef::new(Kind::GitHub, "github.com", "octocat");
    let sam = AccountRef::new(Kind::GitLab, "GitLab.Example.org", "sam");
    assert_eq!(commit_email_of(config.path(), &octocat), None);
    set_commit_email_of(
        config.path(),
        &octocat,
        Some("1+octocat@users.noreply.github.com"),
    )
    .unwrap();
    set_commit_email_of(config.path(), &sam, Some("sam@example.org")).unwrap();
    assert_eq!(
        commit_email_of(config.path(), &octocat).as_deref(),
        Some("1+octocat@users.noreply.github.com")
    );
    assert_eq!(
        commit_email_of(
            config.path(),
            &AccountRef::new(Kind::GitLab, "gitlab.example.org", "sam")
        )
        .as_deref(),
        Some("sam@example.org"),
        "a host is the same in any case"
    );
    // Another login on the same host chose nothing.
    assert_eq!(
        commit_email_of(
            config.path(),
            &AccountRef::new(Kind::GitHub, "github.com", "hubot")
        ),
        None
    );
    for bad in [
        "",
        "sam",
        "sam@local",
        "a b@example.com",
        "x@y.z\nuser.name=evil",
    ] {
        assert!(
            set_commit_email_of(config.path(), &sam, Some(bad)).is_err(),
            "{bad:?}"
        );
    }
    assert_eq!(
        commit_email_of(config.path(), &sam).as_deref(),
        Some("sam@example.org")
    );
    set_commit_email_of(config.path(), &sam, None).unwrap();
    assert_eq!(commit_email_of(config.path(), &sam), None);
    assert!(
        commit_email_of(config.path(), &octocat).is_some(),
        "the other stays"
    );
    set_commit_email_of(config.path(), &sam, None).unwrap();
    forget_records(config.path()).unwrap();
    assert_eq!(commit_email_of(config.path(), &octocat), None);
}

/// #5788: an address chosen before choices were per account was chosen for the device: it stays
/// in use for any account that has not chosen its own since.
#[test]
fn an_email_chosen_for_the_whole_device_before_stands_for_an_account_with_none_of_its_own() {
    let config = tempfile::tempdir().unwrap();
    let folder = config.path().join("signing");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("commit-email"), "old@example.com\n").unwrap();
    let octocat = AccountRef::new(Kind::GitHub, "github.com", "octocat");
    let sam = AccountRef::new(Kind::GitLab, "gitlab.com", "sam");
    assert_eq!(
        commit_email_of(config.path(), &sam).as_deref(),
        Some("old@example.com")
    );
    set_commit_email_of(config.path(), &octocat, Some("new@example.com")).unwrap();
    assert_eq!(
        commit_email_of(config.path(), &octocat).as_deref(),
        Some("new@example.com")
    );
    assert_eq!(
        commit_email_of(config.path(), &sam).as_deref(),
        Some("old@example.com")
    );
    forget_records(config.path()).unwrap();
    assert_eq!(commit_email_of(config.path(), &sam), None);
}

#[test]
fn githubs_verified_addresses_are_offered_primary_first_and_the_noreply_last() {
    let signer = Signer::generate().unwrap();
    let recorded = recording("emails", &signer);
    let found = verified_emails(&recorded, &github()).unwrap();
    let emails: Vec<_> = found.iter().map(|one| one.email.as_str()).collect();
    assert_eq!(
        emails,
        [
            "octocat@example.com",
            "work@example.org",
            "583231+octocat@users.noreply.github.com"
        ]
    );
    assert!(found[0].primary && found[2].private);
    assert!(recorded.unspent().is_empty());
}

#[test]
fn gitlabs_confirmed_addresses_are_offered_and_an_unconfirmed_one_is_not() {
    let signer = Signer::generate().unwrap();
    let recorded = gitlab_recording("auth_key_listed_then_emails", &signer);
    let found = verified_emails(&recorded, &gitlab()).unwrap();
    let emails: Vec<_> = found.iter().map(|one| one.email.as_str()).collect();
    assert_eq!(emails, ["sam@example.org", "sam@billing.example.org"]);
}

#[test]
fn an_account_with_no_verified_address_offers_none_and_its_settings_page_is_named() {
    let signer = Signer::generate().unwrap();
    let recorded = recording("emails_none", &signer);
    assert!(verified_emails(&recorded, &github()).unwrap().is_empty());
    assert_eq!(
        email_settings(&github()),
        "https://github.com/settings/emails"
    );
    assert_eq!(
        email_settings(&forge_of(Kind::GitLab, "git.example.org")),
        "https://git.example.org/-/user_settings/profile"
    );
}

#[test]
fn a_chat_commit_is_made_as_the_email_chosen_for_it() {
    let isolation = crate::worktree::git::Isolated::identity(Some("Sam"), Some("sam@laptop.local"))
        .as_email("sam@example.com");
    assert_eq!(
        isolation,
        crate::worktree::git::Isolated::identity(Some("Sam"), Some("sam@example.com"))
    );
    // A value git would not read as one line changes nothing.
    let kept = crate::worktree::git::Isolated::identity(Some("Sam"), Some("sam@laptop.local"));
    assert_eq!(kept.clone().as_email("a\nuser.name=x"), kept);
}

#[test]
fn a_commit_gitlab_verified_without_naming_this_devices_key_gets_no_tick() {
    let signer = Signer::generate().unwrap();
    let recorded = Recorded::parse(
        r#"{"source": "GitLab's answer for a commit it signed itself, naming no key", "exchanges": [
        {"call": {"endpoint": {"rest": {"method": null, "path": "projects/platform%2Fbilling/repository/commits/0123abc/signature"}}, "fields": []},
         "reply": {"code": 0, "out": "{\"signature_type\": \"SSH\", \"verification_status\": \"verified_system\"}", "err": "", "status": 200}}]}"#,
    )
    .unwrap();
    let checked =
        check_a_pushed_commit(&recorded, &gitlab(), &signer, "platform/billing", "0123abc")
            .unwrap();
    assert!(!checked.verified);
}

/// #5793: an ask names the forge and host of the account it is about; one that names none, as
/// a command of an older build sends it, is about GitHub on github.com. The window answers a
/// GitLab account's from GitLab's public list of its keys.
#[test]
fn an_ask_names_its_forge_and_one_naming_none_is_about_github() {
    let old: Asked =
        serde_json::from_str(r#"{"key_on_account":{"key":"ssh-ed25519 AAAA"}}"#).unwrap();
    assert_eq!(old.forge(), github());
    let signer = Signer::generate().unwrap();
    let asked = Asked::KeyOnAccount {
        key: signer.public_line(),
        on: Some(OnForge {
            kind: Kind::GitLab,
            host: "GitLab.com".to_owned(),
        }),
    };
    assert_eq!(asked.forge(), gitlab());
    let recorded = gitlab_recording("publicly_listed", &signer);
    assert_eq!(
        answered_over(&recorded, &asked.forge(), "sam", &asked, None).unwrap(),
        Heard::KeyOnAccount { on: true }
    );
    assert_eq!(forge_said(&gitlab()), "GitLab");
    assert_eq!(
        forge_said(&forge_of(Kind::GitLab, "gitlab.example.org")),
        "GitLab on gitlab.example.org"
    );
    assert_eq!(
        new_key_page(&forge_of(Kind::GitLab, "gitlab.example.org")),
        "https://gitlab.example.org/-/user_settings/ssh_keys"
    );
}

/// #5793: `purlis signing check` on GitLab is held to this device's key, as the window's own
/// Check is: a commit GitLab verified with another key is not Verified for this device, and
/// with no key here nothing is asked of it.
#[test]
fn a_gitlab_check_from_the_command_is_held_to_this_devices_key() {
    let signer = Signer::generate().unwrap();
    let other = Signer::generate().unwrap();
    let asked = Asked::Check {
        repo: "platform/billing".to_owned(),
        commit: "0123abc".to_owned(),
        code: "A1B2C3".to_owned(),
        on: Some(OnForge::of(&gitlab())),
    };
    let mine = gitlab_recording("commit_verified", &signer);
    let Heard::Checked(checked) =
        answered_over(&mine, &gitlab(), "sam", &asked, Some(&signer)).unwrap()
    else {
        panic!("a verdict");
    };
    assert!(checked.verified);
    let theirs = gitlab_recording("commit_verified", &other);
    let Heard::Checked(checked) =
        answered_over(&theirs, &gitlab(), "sam", &asked, Some(&signer)).unwrap()
    else {
        panic!("a verdict");
    };
    assert!(!checked.verified);
    assert_eq!(checked.reason, "other_key");
    assert!(crate::commitsign::reason_said_on(&checked.reason, "GitLab").contains("another key"));
    let nothing = gitlab_recording("commit_verified", &signer);
    assert!(answered_over(&nothing, &gitlab(), "sam", &asked, None).is_err());
}

/// #5788: chat commits on a host carry the email of the first account signed in there, the one
/// bound to it, and never another account's on the same host in its place.
#[test]
fn a_hosts_commit_email_is_its_first_accounts_or_none() {
    let config = tempfile::tempdir().unwrap();
    let file = crate::forge::accounts::path(config.path());
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let account = |id: &str, kind: &str, host: &str, login: &str| {
        serde_json::json!({"id": id, "kind": kind, "host": host, "login": login,
            "method": "pat", "state": "signed-in"})
    };
    std::fs::write(
        &file,
        serde_json::json!({"version": 1, "accounts": [
            account("01A", "github", "github.com", "octocat"),
            account("01B", "github", "github.com", "hubot"),
            account("01C", "gitlab", "gitlab.example.org:8443", "sam"),
        ]})
        .to_string(),
    )
    .unwrap();
    let hubot = AccountRef::new(Kind::GitHub, "github.com", "hubot");
    set_commit_email_of(config.path(), &hubot, Some("hubot@example.com")).unwrap();
    let sam = AccountRef::new(Kind::GitLab, "gitlab.example.org:8443", "sam");
    set_commit_email_of(config.path(), &sam, Some("sam@example.org")).unwrap();
    assert_eq!(
        commit_emails_by_host(config.path()),
        [(
            "gitlab.example.org:8443".to_owned(),
            "sam@example.org".to_owned()
        )]
    );
    assert_eq!(
        commit_email_on(config.path(), Kind::GitHub, "github.com"),
        None
    );
    assert_eq!(
        commit_email_on(config.path(), Kind::GitLab, "gitlab.example.org:8443").as_deref(),
        Some("sam@example.org")
    );
}
