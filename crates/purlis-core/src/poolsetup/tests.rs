// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::dockerpool::tests::{FakeDocker, inspected};

fn facts(engine: Engine) -> Facts {
    Facts {
        engine,
        connect: None,
        pool: None,
        identity: Identity::None,
        repos: true,
    }
}

fn on_a_pool(runner: Runner, sign_ins: SignIns) -> Facts {
    Facts {
        pool: Some(PoolFacts {
            name: "dev-box".into(),
            runner,
            sign_ins,
        }),
        ..facts(Engine::Ready)
    }
}

fn states(setup: &Setup) -> Vec<StepState> {
    setup.steps.iter().map(|step| step.state).collect()
}

fn answer(facts: &Facts) -> Setup {
    of(Kind::Docker, facts, &Choices::default())
}

fn next(setup: &Setup, id: StepId) -> ActionId {
    setup
        .step(id)
        .and_then(|step| step.next.as_ref())
        .expect("a next action")
        .id
}

fn signed_in() -> SignIns {
    SignIns::Read(vec![("work".into(), Profile::Standing(Standing::SignedIn))])
}

use StepState::{Attention, Current, Done, Later, Skipped};

#[test]
fn docker_has_the_five_steps_of_the_ruling_in_order() {
    let names: Vec<_> = Kind::Docker.steps().iter().map(|s| s.name()).collect();
    assert_eq!(
        names,
        [
            "Where chats run",
            "Pool",
            "Sign-ins",
            "Agent identity",
            "Try it"
        ]
    );
}

#[test]
fn without_docker_where_needs_attention_and_says_where_to_get_it() {
    let setup = answer(&facts(Engine::Missing));
    assert_eq!(setup.at, Some(StepId::Where));
    assert_eq!(states(&setup), [Attention, Later, Later, Later, Later]);
    assert_eq!(next(&setup, StepId::Where), ActionId::GetDocker);
}

#[test]
fn a_stopped_docker_asks_to_start_it() {
    let setup = answer(&facts(Engine::NotAnswering));
    assert_eq!(states(&setup)[0], Attention);
    assert_eq!(next(&setup, StepId::Where), ActionId::StartDocker);
}

#[test]
fn a_ready_docker_is_recommended_and_nothing_is_chosen_until_the_person_chooses() {
    let setup = answer(&facts(Engine::Ready));
    assert_eq!(states(&setup), [Current, Later, Later, Later, Later]);
    assert_eq!(next(&setup, StepId::Where), ActionId::UseDocker);

    let chose = Choices {
        kind: Some(Kind::Docker),
        ..Choices::default()
    };
    let setup = of(Kind::Docker, &facts(Engine::Ready), &chose);
    assert_eq!(states(&setup), [Done, Current, Later, Later, Later]);
    assert_eq!(next(&setup, StepId::Pool), ActionId::CreatePool);
}

#[test]
fn a_later_step_says_nothing_yet() {
    let setup = answer(&facts(Engine::Ready));
    let later = setup.step(StepId::Identity).unwrap();
    assert!(later.findings.is_empty());
    assert!(later.next.is_none());
}

#[test]
fn a_pool_made_elsewhere_answers_where_and_pool_with_no_choice_stored() {
    let setup = answer(&on_a_pool(Runner::Running, SignIns::Read(Vec::new())));
    assert_eq!(states(&setup), [Done, Done, Current, Later, Later]);
    assert_eq!(
        setup.step(StepId::Pool).unwrap().line.as_deref(),
        Some("dev-box")
    );
    assert_eq!(next(&setup, StepId::Signins), ActionId::ChooseProfiles);
}

#[test]
fn a_runner_removed_by_hand_needs_attention_on_the_pool_step_alone() {
    let setup = answer(&on_a_pool(Runner::Gone, SignIns::NotAsked));
    assert_eq!(states(&setup), [Done, Attention, Later, Later, Later]);
    assert_eq!(next(&setup, StepId::Pool), ActionId::MakeRunnerAgain);
}

#[test]
fn a_pool_with_no_runner_yet_says_so_and_checks_again() {
    let setup = answer(&on_a_pool(Runner::None, SignIns::NotAsked));
    assert_eq!(states(&setup)[1], Attention);
    assert_eq!(next(&setup, StepId::Pool), ActionId::CheckAgain);
}

#[test]
fn a_runner_on_another_docker_says_so() {
    let setup = answer(&on_a_pool(Runner::OnAnotherEngine, SignIns::NotAsked));
    assert_eq!(states(&setup)[1], Attention);
    assert_eq!(next(&setup, StepId::Pool), ActionId::UseOtherDocker);
}

#[test]
fn docker_stopping_affects_where_alone_and_keeps_the_pool_done() {
    let mut stopped = on_a_pool(Runner::NotAsked, SignIns::NotAsked);
    stopped.engine = Engine::NotAnswering;
    let setup = answer(&stopped);
    assert_eq!(states(&setup), [Attention, Done, Later, Later, Later]);
    assert_eq!(setup.at, Some(StepId::Where));
}

#[test]
fn an_asleep_runner_is_not_called_signed_out() {
    let setup = answer(&on_a_pool(Runner::Asleep, SignIns::NotAsked));
    assert_eq!(states(&setup), [Done, Done, Current, Later, Later]);
    assert_eq!(next(&setup, StepId::Signins), ActionId::WakeRunner);
}

#[test]
fn a_signed_out_profile_offers_its_sign_in_and_the_command_for_it() {
    let setup = answer(&on_a_pool(
        Runner::Running,
        SignIns::Read(vec![
            ("work".into(), Profile::Standing(Standing::SignedIn)),
            ("personal".into(), Profile::Standing(Standing::SignedOut)),
        ]),
    ));
    let step = setup.step(StepId::Signins).unwrap();
    assert_eq!(step.state, Current);
    let action = step.next.as_ref().unwrap();
    assert_eq!(action.id, ActionId::SignIn);
    assert_eq!(
        action.command.as_deref(),
        Some("purlis profile sign-in personal --pool dev-box")
    );
}

#[test]
fn a_profile_whose_harness_cannot_say_counts_because_a_chat_on_it_starts() {
    let setup = answer(&on_a_pool(
        Runner::Running,
        SignIns::Read(vec![(
            "work".into(),
            Profile::Standing(Standing::CannotTell),
        )]),
    ));
    let step = setup.step(StepId::Signins).unwrap();
    assert_eq!(step.state, Done);
    assert_eq!(step.findings.len(), 1, "{step:?}");
}

#[test]
fn a_runner_that_could_not_answer_needs_attention_with_something_to_try() {
    let setup = answer(&on_a_pool(
        Runner::Running,
        SignIns::CannotTell("it did not answer in time".into()),
    ));
    assert_eq!(states(&setup)[2], Attention);
    assert_eq!(next(&setup, StepId::Signins), ActionId::CheckAgain);
}

#[test]
fn every_step_a_person_is_on_says_what_to_do_next() {
    let cases = [
        facts(Engine::Missing),
        facts(Engine::NotAnswering),
        facts(Engine::Ready),
        on_a_pool(Runner::None, SignIns::NotAsked),
        on_a_pool(Runner::Gone, SignIns::NotAsked),
        on_a_pool(Runner::OnAnotherEngine, SignIns::NotAsked),
        on_a_pool(Runner::OnlyDraining, SignIns::NotAsked),
        on_a_pool(Runner::Running, SignIns::Tainted),
        on_a_pool(Runner::Running, SignIns::NeedsProject),
        Facts {
            identity: Identity::KeptOff {
                persona: "steward".into(),
                tool: "gh".into(),
            },
            ..on_a_pool(Runner::Running, signed_in())
        },
        Facts {
            identity: Identity::Changed {
                persona: "steward".into(),
                tool: "gh".into(),
            },
            ..on_a_pool(Runner::Running, signed_in())
        },
        facts(Engine::Elsewhere),
        Facts {
            identity: Identity::Elsewhere {
                host: "git.acme.internal".into(),
                program: Some("gh".into()),
                persona: None,
            },
            ..on_a_pool(Runner::Running, signed_in())
        },
        on_a_pool(Runner::CannotTell("x".into()), SignIns::NotAsked),
        on_a_pool(Runner::Asleep, SignIns::NotAsked),
        on_a_pool(Runner::Running, SignIns::Read(Vec::new())),
        on_a_pool(Runner::Running, signed_in()),
    ];
    for case in cases {
        let setup = answer(&case);
        let at = setup.at.expect("somewhere to be");
        let step = setup.step(at).unwrap();
        assert!(step.next.is_some(), "{case:?} leaves {at:?} with no way on");
        assert!(!step.findings.is_empty(), "{case:?} says nothing on {at:?}");
    }
}

#[test]
fn an_identity_read_from_the_plane_finishes_its_step_as_what_it_acts_through() {
    let mut set = on_a_pool(Runner::Running, signed_in());
    set.identity = Identity::Set {
        persona: "steward".into(),
        tool: "gh".into(),
    };
    let setup = answer(&set);
    assert_eq!(states(&setup), [Done, Done, Done, Done, Current]);
    assert_eq!(
        setup.step(StepId::Identity).unwrap().line.as_deref(),
        Some("gh through steward")
    );
    assert_eq!(next(&setup, StepId::Try), ActionId::StartTestChat);
}

#[test]
fn repos_on_another_forge_host_are_said_plainly_with_the_tool_line_to_add_by_hand() {
    let mut set = on_a_pool(Runner::Running, signed_in());
    set.identity = Identity::Elsewhere {
        host: "git.acme.internal".into(),
        program: Some("gh".into()),
        persona: Some("steward".into()),
    };
    let setup = answer(&set);
    let step = setup.step(StepId::Identity).unwrap();
    assert_eq!(step.state, Current);
    assert_eq!(next(&setup, StepId::Identity), ActionId::CheckAgain);
    assert!(
        step.findings[0].contains("can't set up git.acme.internal by itself"),
        "{:?}",
        step.findings
    );
    let see = step.see.as_ref().expect("the line to add by hand");
    assert_eq!(
        see.text,
        "purlis vault add agent-forge --provider keyring\n\
         purlis secret set agent-forge token --stdin\n\n\
         [sandbox.personas.steward.tools]\n\
         gh = { vault = \"agent-forge\", as = \"GH_ENTERPRISE_TOKEN env\", key = \"token\", host = \"git.acme.internal\" }"
    );
    assert!(
        see.note.contains("Never use your own token"),
        "a vault of the agent's own, never the person's: {}",
        see.note
    );
    assert!(step.skippable, "it never dead-ends: it can be skipped");
}

#[test]
fn repos_only_on_a_host_purlis_knows_no_forge_at_say_what_to_declare_and_give_no_line() {
    let mut set = on_a_pool(Runner::Running, signed_in());
    set.identity = Identity::Elsewhere {
        host: "bitbucket.org".into(),
        program: None,
        persona: Some("steward".into()),
    };
    let setup = answer(&set);
    let step = setup.step(StepId::Identity).unwrap();
    assert!(
        step.findings[0].contains("Settings › Forges"),
        "{:?}",
        step.findings
    );
    assert_eq!(step.see, None, "no line for a forge purlis doesn't know");
    assert_eq!(next(&setup, StepId::Identity), ActionId::CheckAgain);
}

#[test]
fn a_gitlab_host_is_given_glab_s_line() {
    let mut set = on_a_pool(Runner::Running, signed_in());
    set.identity = Identity::Elsewhere {
        host: "gitlab.com".into(),
        program: Some("glab".into()),
        persona: None,
    };
    let setup = answer(&set);
    let see = setup.step(StepId::Identity).unwrap().see.clone().unwrap();
    assert!(
        see.text.ends_with(
            "[sandbox.personas.<persona>.tools]\n\
             glab = { vault = \"agent-forge\", as = \"GITLAB_TOKEN env\", key = \"token\" }"
        ),
        "{}",
        see.text
    );
}

#[test]
fn an_identity_not_allowed_here_asks_to_allow_it() {
    let mut set = on_a_pool(Runner::Running, signed_in());
    set.identity = Identity::NotAllowed {
        persona: "steward".into(),
        tool: "gh".into(),
    };
    let setup = answer(&set);
    assert_eq!(states(&setup)[3], Current);
    assert_eq!(next(&setup, StepId::Identity), ActionId::AllowTool);
}

#[test]
fn a_skipped_step_moves_on_and_what_is_found_done_wins_over_the_skip() {
    let mut chose = Choices::default();
    chose
        .skipped
        .insert("dev-box".into(), vec![StepId::Identity, StepId::Signins]);
    let setup = of(
        Kind::Docker,
        &on_a_pool(Runner::Running, SignIns::Read(Vec::new())),
        &chose,
    );
    assert_eq!(states(&setup), [Done, Done, Skipped, Skipped, Current]);

    let setup = of(
        Kind::Docker,
        &on_a_pool(Runner::Running, signed_in()),
        &chose,
    );
    assert_eq!(states(&setup), [Done, Done, Done, Skipped, Current]);
}

#[test]
fn a_skip_on_one_pool_says_nothing_of_another() {
    let mut chose = Choices::default();
    chose.skipped.insert("other".into(), vec![StepId::Identity]);
    let setup = of(
        Kind::Docker,
        &on_a_pool(Runner::Running, signed_in()),
        &chose,
    );
    assert_eq!(states(&setup)[3], Current);
}

#[test]
fn the_answer_keeps_the_prototypes_names_as_json() {
    let setup = answer(&on_a_pool(Runner::Running, SignIns::Read(Vec::new())));
    let json = serde_json::to_value(&setup).unwrap();
    assert_eq!(json["kind"], "docker");
    assert_eq!(json["at"], "signins");
    let ids: Vec<_> = json["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["id"].clone())
        .collect();
    assert_eq!(
        ids,
        ["where", "pool", "signins", "identity", "try"].map(serde_json::Value::from)
    );
    assert_eq!(json["steps"][2]["state"], "current");
    assert_eq!(json["steps"][2]["next"]["id"], "choose-profiles");
}

/* ---------- choices ---------- */

#[test]
fn choices_are_kept_and_taken_back() {
    let config = tempfile::tempdir().unwrap();
    assert_eq!(read_choices(config.path()).unwrap(), Choices::default());
    choose_kind(config.path(), Kind::Docker).unwrap();
    a_pool_kept(config.path(), "dev-box");
    skip(config.path(), "dev-box", StepId::Identity).unwrap();
    skip(config.path(), "dev-box", StepId::Identity).unwrap();
    let held = read_choices(config.path()).unwrap();
    assert_eq!(held.kind, Some(Kind::Docker));
    assert_eq!(held.skipped["dev-box"], [StepId::Identity]);
    unskip(config.path(), "dev-box", StepId::Identity).unwrap();
    assert!(read_choices(config.path()).unwrap().skipped.is_empty());
}

#[test]
fn a_pool_this_machine_doesnt_keep_is_never_skipped_on() {
    let config = tempfile::tempdir().unwrap();
    assert!(skip(config.path(), "nowhere", StepId::Identity).is_err());
    assert_eq!(read_choices(config.path()).unwrap(), Choices::default());
}

#[test]
fn a_step_later_ones_need_cannot_be_skipped() {
    let config = tempfile::tempdir().unwrap();
    a_pool_kept(config.path(), "dev-box");
    for step in [StepId::Where, StepId::Pool, StepId::Try] {
        assert!(skip(config.path(), "dev-box", step).is_err(), "{step:?}");
    }
    assert_eq!(read_choices(config.path()).unwrap(), Choices::default());
}

#[test]
fn a_file_purlis_did_not_write_is_refused_and_not_read_as_no_choice() {
    let config = tempfile::tempdir().unwrap();
    a_pool_kept(config.path(), "dev-box");
    skip(config.path(), "dev-box", StepId::Identity).unwrap();
    let path = crate::machine::dir(config.path()).join(CHOICES_FILE);
    let keep_mode = std::fs::metadata(&path).unwrap().permissions();
    std::fs::write(&path, "{\"version\":2}").unwrap();
    std::fs::set_permissions(&path, keep_mode).unwrap();
    assert!(read_choices(config.path()).is_err());
}

#[test]
fn forgetting_a_pool_clears_its_skips_so_a_new_pool_of_its_name_starts_clean() {
    let config = tempfile::tempdir().unwrap();
    a_pool_kept(config.path(), "dev-box");
    a_pool_kept(config.path(), "other");
    skip(config.path(), "dev-box", StepId::Identity).unwrap();
    skip(config.path(), "other", StepId::Identity).unwrap();
    let locked = runners::lock(config.path());
    runners::forget_pool_locked(config.path(), "dev-box", &locked).unwrap();
    drop(locked);
    let held = read_choices(config.path()).unwrap();
    assert!(!held.skipped.contains_key("dev-box"));
    assert!(held.skipped.contains_key("other"));
}

/* ---------- reading ---------- */

/// A `docker` that fails whatever it is asked, as one with no engine behind it does.
fn a_docker_that_does_not_answer(folder: &Path) -> Docker {
    use std::os::unix::fs::PermissionsExt as _;
    let program = folder.join("docker");
    std::fs::write(&program, "#!/bin/sh\necho 'Cannot connect' >&2\nexit 1\n").unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    Docker::at(program).with_settings_in(folder.join("settings"))
}

#[test]
fn read_tells_docker_missing_from_docker_not_answering_from_docker_ready() {
    let config = tempfile::tempdir().unwrap();
    let scripts = tempfile::tempdir().unwrap();
    let silent = a_docker_that_does_not_answer(scripts.path());
    let fake = FakeDocker::new(&inspected());
    let ready = fake.docker();
    let cases = [
        (None, ActionId::GetDocker),
        (Some(&silent), ActionId::StartDocker),
        (Some(&ready), ActionId::UseDocker),
    ];
    for (docker, expected) in cases {
        let here = Here {
            config: config.path(),
            plane: None,
            docker,
            engine: Ok("/var/run/docker.sock".into()),
        };
        let setup = read(&here, None).unwrap();
        assert_eq!(next(&setup, StepId::Where), expected);
        assert_eq!(setup.pool, None);
    }
}

#[test]
fn read_refuses_a_pool_this_machine_has_not() {
    let config = tempfile::tempdir().unwrap();
    let here = Here {
        config: config.path(),
        plane: None,
        docker: None,
        engine: Err("none".into()),
    };
    let why = read(&here, Some("nowhere")).unwrap_err().to_string();
    assert!(why.contains("no pool named nowhere"), "{why}");
}

#[test]
fn read_picks_up_a_choice_made_since_the_last_read() {
    let config = tempfile::tempdir().unwrap();
    let fake = FakeDocker::new(&inspected());
    let docker = fake.docker();
    let here = Here {
        config: config.path(),
        plane: None,
        docker: Some(&docker),
        engine: Ok("/var/run/docker.sock".into()),
    };
    assert_eq!(read(&here, None).unwrap().at, Some(StepId::Where));
    choose_kind(config.path(), Kind::Docker).unwrap();
    assert_eq!(read(&here, None).unwrap().at, Some(StepId::Pool));
}

#[test]
fn with_no_project_open_identity_says_to_open_one() {
    assert_eq!(identity_in(None, &[]), Identity::NoProject);
}

#[test]
fn a_docker_elsewhere_is_said_and_its_runner_is_not_asked_about() {
    let mut there = on_a_pool(Runner::NotAsked, SignIns::NotAsked);
    there.engine = Engine::Elsewhere;
    let setup = answer(&there);
    assert_eq!(states(&setup), [Attention, Done, Later, Later, Later]);
    assert_eq!(next(&setup, StepId::Where), ActionId::UseLocalDocker);
}

#[test]
fn a_docker_that_answers_and_is_not_on_this_machine_reads_as_elsewhere() {
    let config = tempfile::tempdir().unwrap();
    let fake = FakeDocker::new(&inspected());
    let docker = fake.docker();
    let here = Here {
        config: config.path(),
        plane: None,
        docker: Some(&docker),
        engine: Err("tcp://user:secret@host".into()),
    };
    let setup = read(&here, None).unwrap();
    assert_eq!(next(&setup, StepId::Where), ActionId::UseLocalDocker);
    let said = serde_json::to_string(&setup).unwrap();
    assert!(!said.contains("secret"), "{said}");
}

#[test]
fn a_tainted_pool_needs_attention_and_offers_a_new_runner() {
    let setup = answer(&on_a_pool(Runner::Running, SignIns::Tainted));
    assert_eq!(states(&setup)[2], Attention);
    assert_eq!(next(&setup, StepId::Signins), ActionId::RemovePool);
}

#[test]
fn a_profile_being_fetched_says_so_and_a_chat_can_start() {
    let setup = answer(&on_a_pool(
        Runner::Running,
        SignIns::Read(vec![("work".into(), Profile::Standing(Standing::Fetching))]),
    ));
    let step = setup.step(StepId::Signins).unwrap();
    assert_eq!(step.state, Done);
    assert!(step.findings[0].contains("fetching"), "{step:?}");
}

#[test]
fn a_skip_does_not_hide_what_needs_attention() {
    let mut chose = Choices::default();
    chose
        .skipped
        .insert("dev-box".into(), vec![StepId::Signins]);
    let setup = of(
        Kind::Docker,
        &on_a_pool(Runner::Running, SignIns::Tainted),
        &chose,
    );
    assert_eq!(states(&setup)[2], Attention);
    assert_eq!(next(&setup, StepId::Signins), ActionId::RemovePool);
}

#[test]
fn coming_back_finds_the_name_given_before_the_pool_is_made() {
    let config = tempfile::tempdir().unwrap();
    let here = Here {
        config: config.path(),
        plane: None,
        docker: None,
        engine: Err("none".into()),
    };
    let found = resumed(&here).unwrap();
    assert_eq!((found.setup.pool, found.name), (None, None));

    choose_name(config.path(), Some("dev-box")).unwrap();
    let found = resumed(&here).unwrap();
    assert_eq!(found.setup.pool, None, "not made yet");
    assert_eq!(found.name.as_deref(), Some("dev-box"));
}

#[test]
fn a_name_typed_never_picks_a_pool_and_the_pool_made_is_gone_on_with() {
    use crate::runners::Set;
    let config = tempfile::tempdir().unwrap();
    let here = Here {
        config: config.path(),
        plane: None,
        docker: None,
        engine: Err("none".into()),
    };
    runners::record_pool(
        config.path(),
        &crate::runners::tests::pool("dev-box", Set::default()),
    )
    .unwrap();

    choose_name(config.path(), Some("dev-box")).unwrap();
    assert_eq!(
        resumed(&here).unwrap().setup.pool,
        None,
        "typed, not chosen"
    );

    choose_pool(config.path(), "dev-box").unwrap();
    let found = resumed(&here).unwrap();
    assert_eq!(found.setup.pool.as_deref(), Some("dev-box"));
    assert_eq!(found.name, None);

    let locked = runners::lock(config.path());
    runners::forget_pool_locked(config.path(), "dev-box", &locked).unwrap();
    drop(locked);
    assert_eq!(read_choices(config.path()).unwrap().pool, None);
}

#[test]
fn a_pool_this_machine_has_not_cant_be_gone_on_with() {
    let config = tempfile::tempdir().unwrap();
    assert!(choose_pool(config.path(), "nowhere").is_err());
}

#[test]
fn without_docker_the_next_action_names_the_official_page() {
    let setup = answer(&facts(Engine::Missing));
    let next = setup.step(StepId::Where).unwrap().next.as_ref().unwrap();
    assert!(
        next.url
            .as_deref()
            .unwrap()
            .starts_with("https://docs.docker.com/"),
        "{next:?}"
    );
}

#[test]
fn a_name_no_pool_could_have_is_refused_and_not_kept() {
    let config = tempfile::tempdir().unwrap();
    assert!(choose_name(config.path(), Some(&"x".repeat(70_000))).is_err());
    assert_eq!(read_choices(config.path()).unwrap().name, None);
}

fn a_pool_kept(config: &Path, name: &str) {
    runners::record_pool(
        config,
        &crate::runners::tests::pool(name, runners::Set::default()),
    )
    .unwrap();
}

fn here(config: &Path) -> Here<'_> {
    Here {
        config,
        plane: None,
        docker: None,
        engine: Err("no engine here".to_owned()),
    }
}

#[test]
fn a_pool_made_by_hand_under_the_name_typed_is_noticed() {
    let config = tempfile::tempdir().unwrap();
    choose_name(config.path(), Some("dev-box")).unwrap();
    a_pool_kept(config.path(), "dev-box");
    let found = resumed(&here(config.path())).unwrap();
    assert_eq!(found.setup.pool.as_deref(), Some("dev-box"));
    assert!(found.noticed);
}

#[test]
fn a_name_typed_never_picks_a_pool_that_was_there_before_it() {
    let config = tempfile::tempdir().unwrap();
    a_pool_kept(config.path(), "dev-box");
    choose_name(config.path(), Some("dev-box")).unwrap();
    let found = resumed(&here(config.path())).unwrap();
    assert_eq!(found.setup.pool, None);
    assert!(!found.noticed);
    assert_eq!(found.setup.others, ["dev-box"]);
}

#[test]
fn a_pool_this_setup_went_on_with_is_not_a_notice() {
    let config = tempfile::tempdir().unwrap();
    a_pool_kept(config.path(), "dev-box");
    choose_pool(config.path(), "dev-box").unwrap();
    let found = resumed(&here(config.path())).unwrap();
    assert_eq!(found.setup.pool.as_deref(), Some("dev-box"));
    assert!(!found.noticed);
    assert!(found.setup.others.is_empty());
}

#[test]
fn a_removed_pool_s_name_doesn_t_pick_the_next_pool_of_that_name() {
    let config = tempfile::tempdir().unwrap();
    choose_name(config.path(), Some("dev-box")).unwrap();
    a_pool_kept(config.path(), "dev-box");
    forget_pool_locked(config.path(), "dev-box").unwrap();
    let chose = read_choices(config.path()).unwrap();
    assert_eq!(chose.name, None);
    assert!(!chose.name_free);
}

#[test]
fn signing_in_by_hand_is_the_command_the_step_shows() {
    let setup = answer(&on_a_pool(
        Runner::Running,
        SignIns::Read(vec![
            ("work".into(), Profile::Standing(Standing::SignedIn)),
            ("oss".into(), Profile::Standing(Standing::SignedOut)),
        ]),
    ));
    let see = setup
        .step(StepId::Signins)
        .and_then(|step| step.see.clone())
        .expect("commands");
    assert_eq!(see.text, "purlis profile sign-in oss --pool dev-box");
}

/// **#4718**: a pool with no profiles yet shows the line that records one, to fill in.
#[test]
fn a_pool_with_no_profiles_yet_shows_how_to_record_one_by_hand() {
    let setup = answer(&on_a_pool(Runner::Running, SignIns::Read(Vec::new())));
    let see = setup
        .step(StepId::Signins)
        .and_then(|step| step.see.clone())
        .expect("commands");
    assert_eq!(see.text, "purlis pool add-profile dev-box PROFILE");
}

#[test]
fn where_says_how_it_asks_and_that_it_only_reads() {
    let setup = answer(&facts(Engine::Ready));
    let see = setup
        .step(StepId::Where)
        .and_then(|step| step.see.clone())
        .expect("commands");
    assert_eq!(see.text, "docker version");
    assert!(see.note.contains("only reads"));
}

fn tried_on(pool: &str) -> Choices {
    let mut chose = Choices::default();
    chose.tried.insert(pool.into(), 1);
    chose
}

fn try_lines(setup: &Setup) -> Vec<(Verdict, String)> {
    setup
        .step(StepId::Try)
        .expect("Try it")
        .checks
        .iter()
        .map(|check| (check.verdict, check.title.clone()))
        .collect()
}

#[test]
fn setup_ends_only_once_a_real_chat_has_started_on_the_pool() {
    let facts = on_a_pool(Runner::Running, signed_in());
    let before = of(Kind::Docker, &facts, &Choices::default());
    assert_ne!(before.step(StepId::Try).unwrap().state, Done);
    let after = of(Kind::Docker, &facts, &tried_on("dev-box"));
    assert_eq!(after.step(StepId::Try).unwrap().state, Done);
    assert_eq!(
        try_lines(&after)[0],
        (Verdict::Ok, "A chat started on dev-box".into())
    );
}

#[test]
fn a_chat_tried_on_another_pool_proves_nothing_here() {
    let facts = on_a_pool(Runner::Running, signed_in());
    let setup = of(Kind::Docker, &facts, &tried_on("other"));
    assert_ne!(setup.step(StepId::Try).unwrap().state, Done);
}

#[test]
fn a_skipped_step_is_a_check_not_made_and_never_reads_as_done() {
    let facts = on_a_pool(Runner::Running, signed_in());
    let mut chose = tried_on("dev-box");
    chose
        .skipped
        .insert("dev-box".into(), vec![StepId::Identity]);
    let lines = try_lines(&of(Kind::Docker, &facts, &chose));
    assert_eq!(
        lines[2],
        (
            Verdict::Skip,
            "gh and git push weren't checked: agent identity was skipped".into()
        )
    );
    assert!(lines.iter().all(|(verdict, _)| *verdict != Verdict::Todo));
}

#[test]
fn an_identity_set_up_is_a_check_that_passed_and_one_not_set_up_is_not() {
    let mut set = on_a_pool(Runner::Running, signed_in());
    set.identity = Identity::Set {
        persona: "steward".into(),
        tool: "gh".into(),
    };
    let lines = try_lines(&of(Kind::Docker, &set, &tried_on("dev-box")));
    assert_eq!(
        lines[2],
        (
            Verdict::Ok,
            "The agent's identity is set: gh through steward".into()
        )
    );
    let unset = try_lines(&of(
        Kind::Docker,
        &on_a_pool(Runner::Running, signed_in()),
        &tried_on("dev-box"),
    ));
    assert_ne!(unset[2].0, Verdict::Ok);
}

#[test]
fn with_no_repo_gh_and_git_push_are_skipped_and_the_end_reads_works_except_for_this() {
    // C on #4469: nothing to check them against, so a check not made, never one passed.
    let mut set = on_a_pool(Runner::Running, signed_in());
    set.identity = Identity::Set {
        persona: "steward".into(),
        tool: "gh".into(),
    };
    set.repos = false;
    let lines = try_lines(&of(Kind::Docker, &set, &tried_on("dev-box")));
    assert_eq!(
        lines[2],
        (
            Verdict::Skip,
            "gh and git push weren't checked: this project has no repo yet".into()
        )
    );
}

#[test]
fn the_profiles_signed_in_are_said_by_name() {
    let facts = on_a_pool(Runner::Running, signed_in());
    let lines = try_lines(&of(Kind::Docker, &facts, &tried_on("dev-box")));
    assert_eq!(lines[1], (Verdict::Ok, "work signed in".into()));
}

#[test]
fn a_proof_is_kept_for_a_pool_kept_and_goes_with_the_pool() {
    let config = tempfile::tempdir().unwrap();
    assert!(tried(config.path(), "dev-box", 7).is_err(), "no such pool");
    a_pool_kept(config.path(), "dev-box");
    tried(config.path(), "dev-box", 7).unwrap();
    assert_eq!(
        read_choices(config.path()).unwrap().tried.get("dev-box"),
        Some(&7)
    );
    forget_pool_locked(config.path(), "dev-box").unwrap();
    assert!(read_choices(config.path()).unwrap().tried.is_empty());
}

#[test]
fn only_a_runner_asleep_is_offered_to_be_woken() {
    let asleep = answer(&on_a_pool(Runner::Asleep, SignIns::NotAsked));
    assert_eq!(next(&asleep, StepId::Signins), ActionId::WakeRunner);
    let unread = answer(&on_a_pool(Runner::Running, SignIns::NotAsked));
    assert_eq!(next(&unread, StepId::Signins), ActionId::CheckAgain);
}

#[test]
fn a_pool_whose_chats_have_no_identity_is_offered_as_setup_not_finished() {
    assert_eq!(
        in_the_picker_by(&Identity::None),
        InThePicker::NotFinished(NO_IDENTITY_IN_THE_PICKER.into())
    );
    assert_eq!(
        in_the_picker_by(&Identity::NotAllowed {
            persona: "steward".into(),
            tool: "gh".into()
        }),
        InThePicker::NotFinished(NO_IDENTITY_IN_THE_PICKER.into())
    );
    assert_eq!(
        in_the_picker_by(&Identity::Set {
            persona: "steward".into(),
            tool: "gh".into()
        }),
        InThePicker::Offered
    );
}

#[test]
fn a_docker_pool_with_no_runner_that_starts_chats_is_not_offered_and_one_not_kept_is() {
    let config = tempfile::tempdir().unwrap();
    a_pool_kept(config.path(), "dev-box");
    let names = ["team".to_owned(), "dev-box".to_owned()];
    assert_eq!(
        in_the_picker(config.path(), None, &names),
        vec![
            ("team".to_owned(), InThePicker::Offered),
            (
                "dev-box".to_owned(),
                InThePicker::NotOffered(CANNOT_START_IN_THE_PICKER.into())
            ),
        ],
        "a pool not kept here is offered; a Docker pool kept with no runner is not"
    );
}

/// A setup that ended on Try it, on pool `name`, as Pool setup leaves it.
fn a_setup_finished_on(config: &Path, name: &str) {
    a_pool_kept(config, name);
    choose_kind(config, Kind::Docker).unwrap();
    choose_pool(config, name).unwrap();
    tried(config, name, 1).unwrap();
}

#[test]
fn adding_a_pool_after_a_finished_setup_starts_a_new_one_with_nothing_filled_in() {
    let config = tempfile::tempdir().unwrap();
    a_setup_finished_on(config.path(), "dev-box");

    assert_eq!(start_adding(config.path()).unwrap(), Adding::New);

    let found = resumed(&here(config.path())).unwrap();
    assert_eq!((found.setup.pool, found.name), (None, None));
    assert!(!found.noticed);
    assert_eq!(found.setup.others, ["dev-box"]);
    assert_eq!(
        read_choices(config.path()).unwrap().kind,
        None,
        "Where is asked again"
    );
}

#[test]
fn a_finished_pool_keeps_its_proof_and_skips_when_a_new_setup_starts() {
    let config = tempfile::tempdir().unwrap();
    a_setup_finished_on(config.path(), "dev-box");
    skip(config.path(), "dev-box", StepId::Identity).unwrap();

    start_adding(config.path()).unwrap();

    let chose = read_choices(config.path()).unwrap();
    assert_eq!(chose.tried.get("dev-box"), Some(&1));
    assert_eq!(chose.skipped.get("dev-box"), Some(&vec![StepId::Identity]));
}

#[test]
fn adding_a_pool_while_a_new_one_is_unfinished_continues_it() {
    let config = tempfile::tempdir().unwrap();
    a_pool_kept(config.path(), "dev-box");
    choose_pool(config.path(), "dev-box").unwrap();

    assert_eq!(start_adding(config.path()).unwrap(), Adding::Continuing);
    assert_eq!(
        resumed(&here(config.path())).unwrap().setup.pool.as_deref(),
        Some("dev-box")
    );
}

#[test]
fn adding_a_pool_with_only_a_name_typed_continues_with_the_name() {
    let config = tempfile::tempdir().unwrap();
    choose_name(config.path(), Some("dev-box")).unwrap();

    assert_eq!(start_adding(config.path()).unwrap(), Adding::Continuing);
    assert_eq!(
        resumed(&here(config.path())).unwrap().name.as_deref(),
        Some("dev-box")
    );
}

#[test]
fn adding_a_pool_with_only_docker_chosen_starts_at_where() {
    let config = tempfile::tempdir().unwrap();
    choose_kind(config.path(), Kind::Docker).unwrap();

    assert_eq!(start_adding(config.path()).unwrap(), Adding::New);
    assert_eq!(read_choices(config.path()).unwrap().kind, None);
}

#[test]
fn starting_over_forgets_the_unfinished_setup_and_keeps_its_pool() {
    let config = tempfile::tempdir().unwrap();
    a_pool_kept(config.path(), "dev-box");
    choose_kind(config.path(), Kind::Docker).unwrap();
    choose_pool(config.path(), "dev-box").unwrap();

    start_over(config.path()).unwrap();

    let found = resumed(&here(config.path())).unwrap();
    assert_eq!((found.setup.pool, found.name), (None, None));
    assert_eq!(found.setup.others, ["dev-box"], "the pool stays");
    assert_eq!(start_adding(config.path()).unwrap(), Adding::New);
}

#[test]
fn a_new_pool_is_recommended_a_name_no_pool_has() {
    let config = tempfile::tempdir().unwrap();
    assert_eq!(resumed(&here(config.path())).unwrap().recommended, "docker");

    a_pool_kept(config.path(), "docker");
    a_pool_kept(config.path(), "docker-2");
    let recommended = resumed(&here(config.path())).unwrap().recommended;
    assert_eq!(recommended, "docker-3");
    assert!(runners::usable_pool_name(&recommended).is_ok());
}
