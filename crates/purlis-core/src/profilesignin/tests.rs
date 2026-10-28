// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

fn key_sign_in(variable: &str, reference: &str) -> SignIn {
    SignIn {
        kind: "api-key".to_owned(),
        label: Some("work key".to_owned()),
        credentials: [(variable.to_owned(), reference.to_owned())].into(),
        rest: Default::default(),
    }
}

#[test]
fn a_claude_key_profile_names_its_key_by_a_vault_reference() {
    assert_eq!(
        key_sign_in("ANTHROPIC_API_KEY", "vault:providers/anthropic").check("claude"),
        Ok(())
    );
}

#[test]
fn a_credential_given_as_anything_but_a_vault_reference_is_refused() {
    let why = key_sign_in("ANTHROPIC_API_KEY", "written-here-by-hand")
        .check("claude")
        .unwrap_err();
    assert!(why.contains("vault:<vault>/<key>"), "{why}");
    assert!(
        !why.contains("written-here-by-hand"),
        "a refusal never repeats what it refused: {why}"
    );
}

#[test]
fn a_variable_the_harness_is_not_handed_for_its_kind_is_refused() {
    for (harness, variable) in [
        ("claude", "OPENAI_API_KEY"),
        ("claude", "PATH"),
        ("codex", "ANTHROPIC_API_KEY"),
    ] {
        let why = key_sign_in(variable, "vault:providers/a")
            .check(harness)
            .unwrap_err();
        assert!(why.contains(variable), "{harness} {variable}: {why}");
    }
}

#[test]
fn a_cloud_profile_names_its_clouds_variables() {
    let bedrock = SignIn {
        kind: "cloud".to_owned(),
        label: Some("team account".to_owned()),
        credentials: [
            ("CLAUDE_CODE_USE_BEDROCK", "vault:aws/use"),
            ("AWS_REGION", "vault:aws/region"),
            ("AWS_ACCESS_KEY_ID", "vault:aws/id"),
            ("AWS_SECRET_ACCESS_KEY", "vault:aws/secret"),
        ]
        .map(|(v, r)| (v.to_owned(), r.to_owned()))
        .into(),
        rest: Default::default(),
    };
    assert_eq!(bedrock.check("claude"), Ok(()));
    let mut keyed = bedrock.clone();
    keyed.kind = "api-key".to_owned();
    assert!(
        keyed.check("claude").is_err(),
        "a cloud's variables are no key"
    );
}

#[test]
fn a_kind_the_harness_does_not_sign_in_with_is_refused() {
    let local = SignIn {
        kind: "local".to_owned(),
        label: None,
        credentials: Default::default(),
        rest: Default::default(),
    };
    assert!(local.check("claude").is_err(), "Claude Code has no local");
    assert_eq!(local.check("codex"), Ok(()));
    let unknown = SignIn {
        kind: "password".to_owned(),
        ..local
    };
    assert!(unknown.check("codex").is_err());
}

#[test]
fn a_key_or_cloud_profile_names_at_least_one_credential_and_the_others_none() {
    let mut empty = key_sign_in("ANTHROPIC_API_KEY", "vault:providers/anthropic");
    empty.credentials.clear();
    assert!(empty.check("claude").is_err());
    let mut plan = key_sign_in("ANTHROPIC_API_KEY", "vault:providers/anthropic");
    plan.kind = "subscription".to_owned();
    assert!(
        plan.check("claude").is_err(),
        "a subscription is the harness's own and names no vault entry"
    );
}

#[test]
fn a_harness_purlis_does_not_ship_is_handed_nothing() {
    assert!(
        key_sign_in("ANTHROPIC_API_KEY", "vault:providers/anthropic")
            .check("shelly")
            .is_err()
    );
}

mod at_start {
    use super::*;
    use crate::secrets::cmd::tests::Plane;
    use serde_json::json;
    use std::ffi::OsString;

    /// A plane with a plain-file vault `providers` holding the work key, open to `dev`.
    fn plane() -> Plane {
        let plane = Plane::new(&[]);
        let file = plane.plain(
            "providers",
            json!({"anthropic": "the-work-anthropic-value"}),
        );
        plane.register(
            "providers",
            "plain-file",
            json!({"file": file.to_string_lossy()}),
            Some("dev"),
        );
        plane
    }

    fn work_key() -> Vec<(String, Reference)> {
        vec![(
            "ANTHROPIC_API_KEY".to_owned(),
            Reference::parse("vault:providers/anthropic").unwrap(),
        )]
    }

    #[test]
    fn a_chat_whose_persona_may_use_the_vault_is_handed_each_value_in_its_variable() {
        let plane = plane();

        let handed = read(&plane.ctx, Some("dev"), &work_key()).expect("it is read");

        assert_eq!(handed.names(), ["ANTHROPIC_API_KEY"]);
        let env = handed_onto(Vec::new(), &handed);
        assert_eq!(
            env,
            [(
                OsString::from("ANTHROPIC_API_KEY"),
                OsString::from("the-work-anthropic-value")
            )]
        );
    }

    #[test]
    fn a_chat_on_no_persona_or_one_the_vault_is_not_open_to_is_handed_nothing() {
        let plane = plane();

        for persona in [None, Some("ops")] {
            let why = read(&plane.ctx, persona, &work_key()).unwrap_err();
            assert!(why.contains("providers"), "{persona:?}: {why}");
            assert!(!why.contains("the-work-anthropic-value"), "{why}");
        }
    }

    #[test]
    fn a_missing_entry_starts_nothing_and_says_which() {
        let plane = plane();
        let gone = vec![(
            "ANTHROPIC_API_KEY".to_owned(),
            Reference::parse("vault:providers/elsewhere").unwrap(),
        )];

        let why = read(&plane.ctx, Some("dev"), &gone).unwrap_err();

        assert!(why.contains("ANTHROPIC_API_KEY"), "{why}");
    }

    #[test]
    fn what_is_handed_never_shows_its_values() {
        let plane = plane();
        let handed = read(&plane.ctx, Some("dev"), &work_key()).unwrap();

        let shown = format!("{handed:?}");

        assert!(shown.contains("ANTHROPIC_API_KEY"), "{shown}");
        assert!(!shown.contains("the-work-anthropic-value"), "{shown}");
    }

    #[test]
    fn the_read_is_recorded_by_name_and_never_by_value() {
        let plane = plane();
        read(&plane.ctx, Some("dev"), &work_key()).unwrap();

        let trace = serde_json::to_string(&plane.trace()).unwrap();

        assert!(trace.contains("providers"), "{trace}");
        assert!(!trace.contains("the-work-anthropic-value"), "{trace}");
    }

    #[test]
    fn a_handed_value_wins_over_one_the_chat_would_inherit_and_nothing_else_moves() {
        let plane = plane();
        let handed = read(&plane.ctx, Some("dev"), &work_key()).unwrap();
        let inherited = vec![
            (OsString::from("ANTHROPIC_API_KEY"), OsString::from("older")),
            (OsString::from("HOME"), OsString::from("/home/a")),
        ];

        let env = handed_onto(inherited.clone(), &handed);

        assert_eq!(
            env,
            [
                (OsString::from("HOME"), OsString::from("/home/a")),
                (
                    OsString::from("ANTHROPIC_API_KEY"),
                    OsString::from("the-work-anthropic-value")
                ),
            ]
        );
        assert_eq!(
            handed_onto(inherited.clone(), &Handed::default()),
            inherited,
            "a chat with nothing handed, a shell tab's among them, is started as it was"
        );
    }
}

mod billed {
    use super::*;
    use crate::profiles::{Profile, Source};
    use crate::profilestore::{Folders, MachineProfile};

    fn profile(name: &str, source: Source) -> Profile {
        Profile {
            name: name.to_owned(),
            kind: "claude".to_owned(),
            harness: "claude-code".to_owned(),
            command: vec!["claude".to_owned()],
            env: Vec::new(),
            source,
        }
    }

    fn record(name: &str, sign_in: Option<SignIn>) -> MachineProfile {
        MachineProfile {
            name: name.to_owned(),
            harness: "claude".to_owned(),
            title: name.to_owned(),
            folders: Folders {
                config: "/profiles/work/config".into(),
                data: None,
            },
            adopted: None,
            imported: None,
            sign_in,
            pinned: None,
            sandbox_only: false,
            kept_for: None,
            rest: Default::default(),
        }
    }

    #[test]
    fn every_profile_says_what_it_is_billed_to_even_one_that_cannot_start_yet() {
        let key = key_sign_in("ANTHROPIC_API_KEY", "vault:providers/anthropic");
        let machine = [record("work", Some(key))];

        assert_eq!(
            billed(&profile("work", Source::Machine), &machine, &[]).as_deref(),
            Some("your key · work key · waits for the key proxy")
        );
        assert_eq!(
            billed(&profile("claude", Source::BuiltIn), &machine, &[]).as_deref(),
            Some("whatever Claude Code is signed in with · claude"),
            "a sign-in purlis cannot see is said as the harness's own"
        );
        assert_eq!(
            billed(&profile("work", Source::Local), &machine, &[]).as_deref(),
            Some("whatever Claude Code is signed in with · work"),
            "a profile the project declares under the name is not the machine's"
        );
    }
}

/// The operator's ruling of 2026-10-15 (#2151): a cloud's credentials reach its chat's harness
/// in its process environment now; a key waits for the key proxy (#822).
mod handing {
    use super::*;
    use crate::profiles::{Profile, Source};
    use crate::profilestore::{Folders, MachineProfile};

    fn signed_in(harness: &str, sign_in: SignIn) -> (Profile, Vec<MachineProfile>) {
        let profile = Profile {
            name: "work".to_owned(),
            kind: harness.to_owned(),
            harness: harness.to_owned(),
            command: vec![harness.to_owned()],
            env: Vec::new(),
            source: Source::Machine,
        };
        let record = MachineProfile {
            name: "work".to_owned(),
            harness: harness.to_owned(),
            title: "work".to_owned(),
            folders: Folders {
                config: "/profiles/work/config".into(),
                data: None,
            },
            adopted: None,
            imported: None,
            sign_in: Some(sign_in),
            pinned: None,
            sandbox_only: false,
            kept_for: None,
            rest: Default::default(),
        };
        (profile, vec![record])
    }

    fn cloud(credentials: &[(&str, &str)]) -> SignIn {
        SignIn {
            kind: "cloud".to_owned(),
            label: Some("team account".to_owned()),
            credentials: credentials
                .iter()
                .map(|(v, r)| ((*v).to_owned(), (*r).to_owned()))
                .collect(),
            rest: Default::default(),
        }
    }

    fn bedrock() -> (Profile, Vec<MachineProfile>) {
        signed_in(
            "claude",
            cloud(&[
                ("CLAUDE_CODE_USE_BEDROCK", "vault:aws/use"),
                ("AWS_REGION", "vault:aws/region"),
                ("AWS_ACCESS_KEY_ID", "vault:aws/id"),
                ("AWS_SECRET_ACCESS_KEY", "vault:aws/secret"),
            ]),
        )
    }

    #[test]
    fn a_bedrock_profile_is_refused_in_words_while_this_version_hands_no_cloud_credentials() {
        let (profile, machine) = bedrock();

        let why = at_start(&profile, &machine, &[]).unwrap_err();

        assert!(why.contains("#2151"), "{why}");
        assert_eq!(
            billed(&profile, &machine, &[]).as_deref(),
            Some(
                "your cloud account · team account · cloud credentials are not handed to a chat in this version"
            ),
            "the picker says it before the start does"
        );
    }

    #[test]
    fn where_the_cloud_path_is_open_the_row_says_its_commands_can_read_the_credentials() {
        let (profile, machine) = bedrock();

        assert_eq!(
            billed_in(&profile, &machine, &[], true).as_deref(),
            Some(
                "your cloud account · team account · the commands it runs can read these credentials"
            )
        );
    }

    #[test]
    fn a_bedrock_profiles_chat_starts_with_each_reference_and_no_value() {
        let (profile, machine) = bedrock();

        let at = at_start_in(&profile, &machine, &[], true)
            .expect("a cloud profile starts where the cloud path is open")
            .expect("its sign-in is known");

        assert_eq!(at.kind, SignInKind::Cloud);
        let names: Vec<&str> = at.credentials.iter().map(|(v, _)| v.as_str()).collect();
        assert_eq!(
            names,
            [
                "AWS_ACCESS_KEY_ID",
                "AWS_REGION",
                "AWS_SECRET_ACCESS_KEY",
                "CLAUDE_CODE_USE_BEDROCK"
            ]
        );
        assert_eq!(at.credentials[0].1.vault, "aws");
    }

    #[test]
    fn a_key_profiles_chat_waits_for_the_key_proxy_and_says_so() {
        let (profile, machine) = signed_in(
            "claude",
            key_sign_in("ANTHROPIC_API_KEY", "vault:providers/anthropic"),
        );

        let why = at_start(&profile, &machine, &[]).unwrap_err();

        assert!(why.contains("waits for the key proxy"), "{why}");
        assert_eq!(
            billed(&profile, &machine, &[]).as_deref(),
            Some("your key · work key · waits for the key proxy"),
            "the picker says it before the start does"
        );
    }

    #[test]
    fn a_cloud_that_signs_in_with_more_than_variables_is_refused_in_words() {
        for (harness, credentials, said) in [
            (
                "claude",
                vec![
                    ("CLAUDE_CODE_USE_VERTEX", "vault:gcp/use"),
                    ("ANTHROPIC_VERTEX_PROJECT_ID", "vault:gcp/project"),
                ],
                "credentials file",
            ),
            (
                "opencode",
                vec![("GOOGLE_CLOUD_PROJECT", "vault:gcp/project")],
                "credentials file",
            ),
            (
                "codex",
                vec![("AZURE_OPENAI_API_KEY", "vault:azure/key")],
                "configuration",
            ),
        ] {
            let (profile, machine) = signed_in(harness, cloud(&credentials));

            let why = at_start_in(&profile, &machine, &[], true).unwrap_err();

            assert!(why.contains(said), "{harness}: {why}");
            assert!(why.contains("#2151"), "{harness}: {why}");
            let row = billed_in(&profile, &machine, &[], true).unwrap();
            assert!(
                row.contains("is not signed in from a vault"),
                "{harness}: {row}"
            );
        }
    }
}

#[test]
fn no_refusal_or_row_has_a_double_space() {
    use crate::profiles::{Profile, Source};
    let profile = |harness: &str| Profile {
        name: "work".to_owned(),
        kind: harness.to_owned(),
        harness: harness.to_owned(),
        command: vec![harness.to_owned()],
        env: Vec::new(),
        source: Source::Machine,
    };
    let mut said = Vec::new();
    for (harness, kind, variable) in [
        ("claude", "api-key", "ANTHROPIC_API_KEY"),
        ("claude", "cloud", "AWS_REGION"),
        ("claude", "cloud", "CLAUDE_CODE_USE_VERTEX"),
        ("codex", "cloud", "AZURE_OPENAI_API_KEY"),
        ("claude", "local", "AWS_REGION"),
        ("claude", "api-key", "PATH"),
        ("shelly", "api-key", "ANTHROPIC_API_KEY"),
    ] {
        let sign_in = SignIn {
            kind: kind.to_owned(),
            label: Some("work key".to_owned()),
            credentials: [(variable.to_owned(), "vault:v/k".to_owned())].into(),
            rest: Default::default(),
        };
        if let Err(why) = sign_in.check(harness) {
            said.push(why);
        }
        let record = crate::profilestore::MachineProfile {
            name: "work".to_owned(),
            harness: harness.to_owned(),
            title: "work".to_owned(),
            folders: crate::profilestore::Folders {
                config: "/profiles/work/config".into(),
                data: None,
            },
            adopted: None,
            imported: None,
            sign_in: Some(sign_in),
            pinned: None,
            sandbox_only: false,
            kept_for: None,
            rest: Default::default(),
        };
        for open in [false, true] {
            let machine = [record.clone()];
            if let Err(why) = at_start_in(&profile(harness), &machine, &[], open) {
                said.push(why);
            }
            said.extend(billed_in(&profile(harness), &machine, &[], open));
        }
    }
    let mut bad = key_sign_in("ANTHROPIC_API_KEY", "written-by-hand");
    said.push(bad.check("claude").unwrap_err());
    bad.credentials.clear();
    said.push(bad.check("claude").unwrap_err());
    assert!(said.len() > 10, "{said:?}");
    for one in &said {
        assert!(!one.contains("  "), "{one:?}");
    }
}

#[test]
fn a_sign_in_keeps_every_field_this_build_does_not_know() {
    let text = r#"{"kind":"cloud","label":"team","credentials":{"AWS_REGION":"vault:aws/region"},"since":"later"}"#;

    let sign_in: SignIn = serde_json::from_str(text).unwrap();

    assert_eq!(sign_in.rest.get("since"), Some(&serde_json::json!("later")));
    let again: serde_json::Value = serde_json::to_value(&sign_in).unwrap();
    assert_eq!(
        again,
        serde_json::from_str::<serde_json::Value>(text).unwrap()
    );
}
