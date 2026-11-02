// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **`purlis change push` from a wired clone against a real git-lfs** (#4516): this module's own
//! world, a clone the git policy wired, and an upload server on a loopback port on a
//! [`crate::lfsupload::testserver::ThisMachine`]. Only the LFS address stands in. The helper is
//! the one the product finds, the fake `gh` on that machine's `PATH`. Run by the CI job
//! `real git-lfs`, by this module's name.

use std::time::Duration;

use super::*;
use crate::lfsupload::testserver::{Object, Taking, Uploads, Watching, named, on_a_machine};
use crate::lfsupload::{STAND_IN, StandIn};

#[test]
#[ignore = "needs a real git-lfs: the CI job `real git-lfs` runs it"]
fn real_git_lfs_a_change_push_from_a_wired_clone_uploads_first_signed_by_the_forges_own_cli() {
    on_a_machine(
        &named(
            module_path!(),
            "real_git_lfs_a_change_push_from_a_wired_clone_uploads_first_signed_by_the_forges_own_cli",
        ),
        |machine| {
            let world = World::new("schema = 1\n");
            let widget = world.clone("widget");
            crate::gitpolicy::apply(&widget, &world.plane);
            let wired = run(&widget, &["config", "--local", "credential.helper"]);
            assert!(crate::forge::is_policy_helper(wired.trim()), "{wired}");
            let object = Object::of(3, 51);
            std::fs::write(
                widget.join(".gitattributes"),
                "*.bin filter=lfs diff=lfs merge=lfs -text\n",
            )
            .unwrap();
            std::fs::write(widget.join("large.bin"), &object.bytes).unwrap();
            run(&widget, &["add", ".gitattributes", "large.bin"]);
            run(&widget, &["commit", "-q", "-m", "a large file"]);
            assert_eq!(
                run(&widget, &["cat-file", "blob", "HEAD:large.bin"]),
                object.pointer()
            );
            world.change(&[("widget", &[])]);
            let server = Uploads::serving(
                Taking::Everything,
                Some(Watching {
                    forge: world.bare("widget"),
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

            let (code, said, _) = lone_member(&world);

            STAND_IN.with(|held| *held.borrow_mut() = None);
            assert_eq!(code, 0, "{said}");
            let (bytes, moved) = server.holds(&object.oid).expect("the object arrived");
            assert!(
                bytes == object.bytes,
                "the object's bytes changed on the way"
            );
            assert!(!moved, "the ref had moved before the object arrived");
            assert!(world.remote_has("widget", BRANCH).is_some(), "{said}");
            machine.only_gh_signed(&server, &said);
        },
    );
}
