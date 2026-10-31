// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The rules that stay** (#2039, spec #2030 stories 31 and 32, ADR 0095): a teammate makes a
//! profile on their machine under the name a persona's definition names (`profile: qa-bot`),
//! and the persona's chats now start on it. Who may start on what does not change with it:
//!
//! - the project's dispatch list for the persona (`[dispatch.profiles]`, #1509) holds a profile
//!   made here as it holds any other;
//! - a profile the clone's own local file declares under that name is the clone's, and is still
//!   asked about on this machine before its command runs;
//! - and a command that switches the harness's permission prompts off is still refused, whoever
//!   made a profile of that name.
//!
//! A profile made here runs its harness's own program with its folder variables set, so it
//! carries no flag of its own and has no command to approve.

use std::path::{Path, PathBuf};

use purlis_core::dispatchunattended::{NamedBy, bypass_refusal};
use purlis_core::harness_declaration::Declarations;
use purlis_core::personaprofile::{self, Chosen, Named, Offer, Refused, Who};
use purlis_core::profiles::{self, ProfileSet, Source};
use purlis_core::profilestore::{Folders, Imported, MachineProfile};

/// A profile this machine made, `name`, of Claude Code, in a folder of its own.
fn made_here(name: &str, folder: &Path) -> MachineProfile {
    MachineProfile {
        name: name.to_owned(),
        harness: "claude".to_owned(),
        title: name.to_owned(),
        folders: Folders {
            config: folder.join(name),
            data: None,
        },
        adopted: None,
        imported: Some(Imported {
            at: 1,
            from: PathBuf::from("/home/dev/.claude"),
        }),
        sign_in: None,
        pinned: None,
        sandbox_only: false,
        kept_for: None,
        rest: serde_json::Map::new(),
    }
}

/// A project whose committed file says `committed`.
fn project(committed: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("charter.toml"), committed).unwrap();
    dir
}

fn built_in() -> Declarations {
    Declarations {
        declared: purlis_core::harness_declaration::builtins().to_vec(),
        refused: Vec::new(),
    }
}

/// The profiles `root` offers with `machine` made here and `local` as the clone's local file.
fn offered(
    root: &Path,
    local: Option<&str>,
    machine: &[MachineProfile],
) -> (ProfileSet, Vec<Offer>) {
    let committed = std::fs::read_to_string(root.join("charter.toml")).ok();
    let set = profiles::derive_with(
        committed.as_deref(),
        Ok(local.map(str::to_owned)),
        &built_in(),
        machine,
    );
    let offers = personaprofile::offers_of(root, &set, &built_in());
    (set, offers)
}

/// Persona `qa`, whose definition names `qa-bot`, held to what `root` lists for it.
fn qa(root: &Path) -> Named {
    Named {
        persona: "qa".to_owned(),
        profile: Some("qa-bot".to_owned()),
        listed: purlis_core::dispatchprofiles::listed_at(root, "qa"),
        ..Named::default()
    }
}

#[test]
fn a_profile_made_here_under_the_name_a_persona_names_starts_its_dispatched_chats() {
    purlis_core::unsteered!();
    let dir = project("");
    let folders = tempfile::tempdir().unwrap();
    let (_, offers) = offered(dir.path(), None, &[made_here("qa-bot", folders.path())]);

    assert_eq!(
        personaprofile::for_dispatch(&qa(dir.path()), Some("claude"), None, &offers),
        Ok(Chosen {
            profile: "qa-bot".to_owned(),
            by: Who::Persona,
            fell_back: None,
        })
    );
}

#[test]
fn the_projects_dispatch_list_holds_a_profile_made_here_as_it_holds_any_other() {
    purlis_core::unsteered!();
    let dir = project("[dispatch.profiles]\nqa = [\"claude\"]\n");
    let folders = tempfile::tempdir().unwrap();
    let (_, offers) = offered(dir.path(), None, &[made_here("qa-bot", folders.path())]);

    assert_eq!(
        personaprofile::for_dispatch(&qa(dir.path()), Some("claude"), None, &offers),
        Err(Refused::NotListed {
            profile: "qa-bot".to_owned(),
            by: Who::Persona,
            persona: "qa".to_owned(),
            listed: vec!["claude".to_owned()],
        })
    );

    // Listed, it starts.
    let dir = project("[dispatch.profiles]\nqa = [\"qa-bot\"]\n");
    let (_, offers) = offered(dir.path(), None, &[made_here("qa-bot", folders.path())]);
    assert!(personaprofile::for_dispatch(&qa(dir.path()), Some("claude"), None, &offers).is_ok());
}

#[test]
fn a_profile_made_here_has_no_command_to_approve_and_switches_no_prompts_off() {
    purlis_core::unsteered!();
    let dir = project("");
    let folders = tempfile::tempdir().unwrap();
    let (set, offers) = offered(dir.path(), None, &[made_here("qa-bot", folders.path())]);

    let made = set.get("qa-bot").unwrap();
    assert_eq!(made.source, Source::Machine);
    assert_eq!(made.command, vec!["claude".to_owned()]);
    assert!(
        !offers
            .iter()
            .find(|o| o.name == "qa-bot")
            .unwrap()
            .needs_approval
    );
    assert_eq!(
        bypass_refusal("qa-bot", &made.command, NamedBy::ThePersona("qa")),
        None
    );
}

#[test]
fn the_clones_own_profile_of_that_name_is_still_asked_about_before_it_runs() {
    purlis_core::unsteered!();
    let dir = project("");
    let folders = tempfile::tempdir().unwrap();
    let local = "[harness.qa-bot]\nkind = \"claude\"\ncommand = [\"claude\"]\n";
    let (set, offers) = offered(
        dir.path(),
        Some(local),
        &[made_here("qa-bot", folders.path())],
    );

    assert_eq!(set.get("qa-bot").unwrap().source, Source::Local);
    assert_eq!(
        personaprofile::for_dispatch(&qa(dir.path()), Some("claude"), None, &offers),
        Err(Refused::NotApproved {
            profile: "qa-bot".to_owned(),
            by: Who::Persona,
            persona: Some("qa".to_owned()),
        })
    );
}

#[test]
fn a_command_that_switches_the_prompts_off_is_still_refused_whoever_made_a_profile_of_that_name() {
    purlis_core::unsteered!();
    let dir = project("");
    let folders = tempfile::tempdir().unwrap();
    let local = "[harness.qa-bot]\nkind = \"claude\"\n\
                 command = [\"claude\", \"--dangerously-skip-permissions\"]\n";
    let (set, _) = offered(
        dir.path(),
        Some(local),
        &[made_here("qa-bot", folders.path())],
    );

    let profile = set.get("qa-bot").unwrap();
    let refused = bypass_refusal("qa-bot", &profile.command, NamedBy::ThePersona("qa"));
    assert_eq!(
        refused.as_deref(),
        Some(
            "persona 'qa' names profile 'qa-bot', which starts its harness with the permission \
             prompts off (--dangerously-skip-permissions), and purlis starts no chat for another \
             chat on such a profile. Give the persona a profile that asks, from its view, or \
             name one with --profile."
        )
    );
}
