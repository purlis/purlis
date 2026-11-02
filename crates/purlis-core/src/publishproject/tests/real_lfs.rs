// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A project's first publish against a real git-lfs** (#4516): the window's own hands, with no
//! stand-in repository, so the push and its upload take the road the product takes. A rewrite in
//! the project's config lands the forge's HTTPS address on a bare repository, and an upload server
//! on a loopback port stands in for its LFS address. The helper is the one the window names, a
//! program of a [`crate::lfsupload::testserver::ThisMachine`] standing for the app. A fake `gh` is
//! there too: a first publish signs with the person's account or not at all (#752), so it must
//! not run. Run by the CI job `real git-lfs`, by this module's name.

use std::time::Duration;

use super::*;
use crate::forge::gitcredential::as_the_window;
use crate::forge::gitcredential::tests::Machine;
use crate::forge::route::HostScope;
use crate::lfsupload::testserver::{Object, Taking, Uploads, Watching, named, on_a_machine};
use crate::lfsupload::{STAND_IN, StandIn};

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_a_first_publish_uploads_first_signed_by_the_windows_own_helper() {
    on_a_machine(
        &named(
            module_path!(),
            "real_git_lfs_a_first_publish_uploads_first_signed_by_the_windows_own_helper",
        ),
        |machine| {
            let dir = tempfile::tempdir().unwrap();
            let top = std::fs::canonicalize(dir.path()).unwrap();
            let bare = top.join("forge.git");
            a_bare(&bare);
            let root = top.join("notebook");
            a_repository_with_a_commit(&root);
            let ran = |args: &[&str]| {
                let done = crate::testgit::run(&root, args);
                assert!(done.ok(), "{args:?}: {done:?}");
                done.out
            };
            let rewrite = format!("url.file://{}.insteadOf", bare.display());
            ran(&[
                "config",
                &rewrite,
                "https://github.com/octocat/notebook.git",
            ]);
            let object = Object::of(2, 61);
            std::fs::write(
                root.join(".gitattributes"),
                "*.bin filter=lfs diff=lfs merge=lfs -text\n",
            )
            .unwrap();
            std::fs::write(root.join("large.bin"), &object.bytes).unwrap();
            ran(&["add", ".gitattributes", "large.bin"]);
            ran(&["commit", "-q", "-m", "a large file"]);
            assert_eq!(
                ran(&["cat-file", "blob", "HEAD:large.bin"]),
                object.pointer()
            );
            let account = Machine::signed_in_on("github.com");
            let server = Uploads::serving(
                Taking::Everything,
                Some(Watching {
                    forge: bare.clone(),
                    reference: format!("refs/heads/{BRANCH}"),
                }),
            );
            STAND_IN.with(|held| {
                *held.borrow_mut() = Some(StandIn {
                    endpoint: server.endpoint(),
                    helper: None,
                    stall: Duration::from_secs(60),
                });
            });
            let (backend, _) = over(Kind::GitHub, "made");

            let answer = as_the_window(
                &HostScope::for_a_test(),
                &machine.window_exe(),
                account.config(),
                || {
                    publish(
                        &plan(Kind::GitHub, &root, "made"),
                        backend.as_ref(),
                        &Caller::window(),
                        &OnThisMachine { push_to: None },
                        &AtomicBool::new(false),
                        &mut |_| {},
                    )
                },
            );

            STAND_IN.with(|held| *held.borrow_mut() = None);
            assert!(matches!(answer, Ok(Created::Made { .. })), "{answer:?}");
            let (bytes, moved) = server.holds(&object.oid).expect("the object arrived");
            assert!(
                bytes == object.bytes,
                "the object's bytes changed on the way"
            );
            assert!(!moved, "the ref had moved before the object arrived");
            assert!(!in_the_remote(&bare, &["for-each-ref"]).trim().is_empty());
            assert!(server.signed() >= 1, "never signed");
            assert!(
                machine.window_helper_ran(),
                "the window's helper was not asked"
            );
            assert_eq!(machine.gh_ran(), Vec::<String>::new());
            assert!(!machine.persons_helper_ran());
        },
    );
}
