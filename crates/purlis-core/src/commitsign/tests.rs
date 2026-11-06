// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::secrets::keyring::Store as _;
use crate::testgit;

fn a_keyring(dir: &Path) -> keyring::FileStore {
    keyring::FileStore::at(dir.join("keyring.json"))
}

/// A repository with one commit by `who@example.invalid`, and that commit's id.
fn a_repo(at: &Path) -> String {
    testgit::run(at, &["init", "-q", "-b", "main"]);
    for (key, value) in [("user.name", "Who"), ("user.email", "who@example.invalid")] {
        testgit::run(at, &["config", key, value]);
    }
    std::fs::write(at.join("a.txt"), "a\n").unwrap();
    testgit::run(at, &["add", "a.txt"]);
    testgit::run(at, &["commit", "-q", "-m", "first"]);
    head(at)
}

fn head(at: &Path) -> String {
    testgit::run(at, &["rev-parse", "HEAD"]).line().to_owned()
}

fn commit(at: &Path, file: &str, message: &str) -> String {
    std::fs::write(at.join(file), format!("{file}\n")).unwrap();
    testgit::run(at, &["add", file]);
    testgit::run(at, &["commit", "-q", "-m", message]);
    head(at)
}

/// What `git verify-commit` says of `id`, trusting `signer` for `who@example.invalid`: git's own
/// check, through `ssh-keygen -Y verify`, as a forge's is. Plain git, as a test's fixture git is.
fn verified(at: &Path, id: &str, signer: &Signer) -> std::process::Output {
    let allowed = at.join("allowed_signers");
    std::fs::write(
        &allowed,
        format!(
            "who@example.invalid namespaces=\"git\" {}\n",
            signer.public_line()
        ),
    )
    .unwrap();
    crate::forklock::output(
        std::process::Command::new("git")
            .arg("-C")
            .arg(at)
            .args(["-c", "gpg.format=ssh", "-c"])
            .arg(format!("gpg.ssh.allowedSignersFile={}", allowed.display()))
            .args(["verify-commit", id])
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1"),
    )
    .expect("git runs")
}

fn raw(at: &Path, id: &str) -> Vec<u8> {
    git::run_with_input(at, &["cat-file", "commit", id], Vec::new(), git::READ)
        .unwrap()
        .out
}

#[test]
fn a_signed_copy_is_verified_by_git_and_keeps_the_tree_author_committer_and_message() {
    let dir = tempfile::tempdir().unwrap();
    let at = dir.path();
    let id = a_repo(at);
    let signer = Signer::generate().unwrap();

    let signed = signed_copy(at, &id, &HashMap::new(), &signer).unwrap();

    assert_ne!(signed, id);
    let checked = verified(at, &signed, &signer);
    assert!(
        checked.status.success(),
        "git verify-commit: {}",
        String::from_utf8_lossy(&checked.stderr)
    );
    // The commit git made is unsigned, so git's own check of it fails: the check is not vacuous.
    assert!(!verified(at, &id, &signer).status.success());
    // Taken off its signature, the copy is the commit git made, byte for byte.
    assert_eq!(
        unsigned(&raw(at, &signed), &HashMap::new()).unwrap(),
        raw(at, &id)
    );
}

#[test]
fn another_key_does_not_verify_a_copy() {
    let dir = tempfile::tempdir().unwrap();
    let at = dir.path();
    let id = a_repo(at);
    let signed = signed_copy(at, &id, &HashMap::new(), &Signer::generate().unwrap()).unwrap();
    assert!(
        !verified(at, &signed, &Signer::generate().unwrap())
            .status
            .success()
    );
}

#[test]
fn a_message_that_is_not_utf8_is_signed_as_its_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let at = dir.path();
    a_repo(at);
    testgit::run(at, &["config", "i18n.commitEncoding", "ISO-8859-1"]);
    std::fs::write(at.join("b.txt"), "b\n").unwrap();
    testgit::run(at, &["add", "b.txt"]);
    let message = at.join("message");
    std::fs::write(&message, b"caf\xe9\n").unwrap();
    testgit::run(at, &["commit", "-q", "-F", &message.display().to_string()]);
    let id = head(at);
    let signer = Signer::generate().unwrap();

    let signed = signed_copy(at, &id, &HashMap::new(), &signer).unwrap();

    assert!(verified(at, &signed, &signer).status.success());
    assert!(
        raw(at, &signed).ends_with(b"\n\ncaf\xe9\n"),
        "the message's bytes are kept"
    );
}

#[test]
fn a_chain_is_signed_oldest_first_each_on_the_signed_copy_of_its_parent() {
    let dir = tempfile::tempdir().unwrap();
    let at = dir.path();
    let base = a_repo(at);
    let one = commit(at, "b.txt", "second");
    let two = commit(at, "c.txt", "third");
    let signer = Signer::generate().unwrap();

    let signed = sign_branch(at, "refs/heads/main", &two, &[&base], &signer).unwrap();

    assert_eq!(head(at), signed, "the branch is moved to the signed copy");
    let listed = testgit::run(at, &["rev-list", &signed, "--not", &base]);
    let ids: Vec<&str> = listed.out.lines().collect();
    assert_eq!(ids.len(), 2);
    for id in &ids {
        assert!(verified(at, id, &signer).status.success(), "{id}");
        assert_ne!(*id, one);
        assert_ne!(*id, two);
    }
    // The base is not signed, and is still the chain's root.
    assert_eq!(
        testgit::run(at, &["rev-parse", &format!("{signed}~2")]).line(),
        base
    );
    // The same tree at the end as git made.
    assert_eq!(
        testgit::run(at, &["rev-parse", &format!("{signed}^{{tree}}")]).line(),
        testgit::run(at, &["rev-parse", &format!("{two}^{{tree}}")]).line()
    );
}

#[test]
fn a_branch_that_moved_while_it_was_signed_is_left_where_it_is() {
    let dir = tempfile::tempdir().unwrap();
    let at = dir.path();
    let base = a_repo(at);
    let made = commit(at, "b.txt", "second");
    let moved = commit(at, "c.txt", "third, by someone else");
    let signer = Signer::generate().unwrap();

    let refused = sign_branch(at, "refs/heads/main", &made, &[&base], &signer);

    assert!(refused.is_err(), "{refused:?}");
    assert_eq!(head(at), moved);
}

#[test]
fn nothing_made_since_is_nothing_signed() {
    let dir = tempfile::tempdir().unwrap();
    let at = dir.path();
    let base = a_repo(at);
    let signer = Signer::generate().unwrap();
    assert_eq!(
        sign_branch(at, "refs/heads/main", &base, &[&base], &signer).unwrap(),
        base
    );
}

#[test]
fn a_commit_signed_before_carries_only_the_new_signature() {
    let dir = tempfile::tempdir().unwrap();
    let at = dir.path();
    let id = a_repo(at);
    let (old, new) = (Signer::generate().unwrap(), Signer::generate().unwrap());
    let once = signed_copy(at, &id, &HashMap::new(), &old).unwrap();

    let twice = signed_copy(at, &once, &HashMap::new(), &new).unwrap();

    assert!(verified(at, &twice, &new).status.success());
    let text = String::from_utf8_lossy(&raw(at, &twice)).into_owned();
    assert_eq!(text.matches("BEGIN SSH SIGNATURE").count(), 1, "{text}");
}

#[test]
fn what_is_signed_drops_every_signature_line_and_maps_the_parents() {
    let raw = b"tree 1111111111111111111111111111111111111111\n\
parent aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n\
parent bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\n\
author A <a@x> 1 +0000\n\
committer A <a@x> 1 +0000\n\
gpgsig -----BEGIN SSH SIGNATURE-----\n \
U1NIU0lH\n \
-----END SSH SIGNATURE-----\n\
\n\
message\n\
 indented line\n";
    let parents = HashMap::from([(
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
        "cccccccccccccccccccccccccccccccccccccccc".to_owned(),
    )]);
    let out = String::from_utf8(unsigned(raw, &parents).unwrap()).unwrap();
    assert_eq!(
        out,
        "tree 1111111111111111111111111111111111111111\n\
parent cccccccccccccccccccccccccccccccccccccccc\n\
parent bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\n\
author A <a@x> 1 +0000\n\
committer A <a@x> 1 +0000\n\
\n\
message\n\
 indented line\n"
    );
    assert!(unsigned(b"not a commit\n\nbody", &parents).is_none());
    assert!(unsigned(b"tree 1\nno end", &parents).is_none());
}

#[test]
fn the_key_is_made_once_and_read_back_the_same() {
    let (config, keys) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let store = a_keyring(keys.path());
    assert!(kept(config.path(), &store).unwrap().is_none());

    let first = make(config.path(), &store).unwrap();
    let again = make(config.path(), &store).unwrap();

    assert_eq!(first, again);
    assert_eq!(kept(config.path(), &store).unwrap(), Some(first.clone()));
    assert!(first.public_line().starts_with("ssh-ed25519 "));
    assert!(first.fingerprint().starts_with("SHA256:"));
    assert!(!format!("{first:?}").contains("PRIVATE"), "{first:?}");
    // Another device keeps its own.
    let other = make(tempfile::tempdir().unwrap().path(), &store).unwrap();
    assert_ne!(other, first);
}

#[test]
fn the_persons_own_key_is_taken_with_its_passphrase_and_kept_without_one() {
    let (config, keys) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let store = a_keyring(keys.path());
    // A key as the person has one: made by `ssh-keygen`, with a passphrase.
    let file = keys.path().join("id_ed25519");
    let made = crate::forklock::output(
        std::process::Command::new("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", "open sesame", "-C", "me", "-f"])
            .arg(&file),
    )
    .expect("ssh-keygen runs");
    assert!(made.status.success(), "{made:?}");
    let locked = std::fs::read_to_string(&file).unwrap();
    let public = std::fs::read_to_string(file.with_extension("pub")).unwrap();

    assert!(take(config.path(), &store, &locked, None).is_err());
    assert!(take(config.path(), &store, &locked, Some("wrong")).is_err());
    let taken = take(config.path(), &store, &locked, Some("open sesame")).unwrap();

    assert_eq!(
        taken.public_line().split(' ').nth(1),
        public.split(' ').nth(1),
        "the key taken is the person's"
    );
    let service = service(config.path()).unwrap();
    let held = store.get(&service, ACCOUNT).unwrap().unwrap().into_inner();
    assert!(
        !PrivateKey::from_openssh(held.as_bytes())
            .unwrap()
            .is_encrypted()
    );
    assert!(take(config.path(), &store, "not a key", None).is_err());
}

#[test]
fn an_item_that_is_not_a_key_is_refused() {
    let (config, keys) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let store = a_keyring(keys.path());
    let service = service(config.path()).unwrap();
    store.set(&service, ACCOUNT, "not a key").unwrap();
    assert!(kept(config.path(), &store).is_err());
    assert!(make(config.path(), &store).is_err(), "never made over it");
    assert!(forget(config.path(), &store).unwrap());
    assert!(kept(config.path(), &store).unwrap().is_none());
}

#[test]
fn github_lists_the_key_by_its_type_and_data_whatever_its_title() {
    let signer = Signer::generate().unwrap();
    let data = signer.public_line();
    let data: Vec<&str> = data.split(' ').collect();
    let listed = format!(
        r#"[{{"id": 1, "key": "ssh-ed25519 AAAAother", "title": "laptop"}},
            {{"id": 2, "key": "{} {}", "title": "purlis"}}]"#,
        data[0], data[1]
    );
    assert!(listed_on_github(&listed, &signer).unwrap());
    assert!(!listed_on_github("[]", &signer).unwrap());
    assert!(listed_on_github("not json", &signer).is_err());
}

#[test]
fn githubs_check_of_a_commit_is_read_with_its_reason_in_plain_words() {
    let answer =
        r#"{"sha": "abc", "commit": {"verification": {"verified": true, "reason": "valid"}}}"#;
    assert_eq!(verification(answer).unwrap(), (true, "valid".to_owned()));
    let answer = r#"{"commit": {"verification": {"verified": false, "reason": "unknown_key"}}}"#;
    let (verified, reason) = verification(answer).unwrap();
    assert!(!verified);
    assert!(
        reason_said(&reason).contains(SETUP),
        "{}",
        reason_said(&reason)
    );
    assert!(reason_said("unverified_email").contains("Settings › Emails"));
    assert!(verification(r#"{"commit": {}}"#).is_err());
}
