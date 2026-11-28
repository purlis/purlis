// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct Said(Arc<Mutex<Vec<u8>>>);
impl std::io::Write for Said {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Said {
    type Writer = Said;
    fn make_writer(&'a self) -> Said {
        self.clone()
    }
}

fn said_while(then: impl FnOnce()) -> String {
    let said = Said::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(said.clone())
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::with_default(subscriber, then);
    String::from_utf8(said.0.lock().unwrap().clone()).unwrap()
}

#[test]
fn a_step_says_its_name_and_how_many_milliseconds_it_took_at_info() {
    let out = said_while(|| {
        let step = begin(Name::LinkHandshake);
        std::thread::sleep(std::time::Duration::from_millis(30));
        step.done();
    });
    assert!(out.contains("INFO"), "{out}");
    assert!(out.contains("step=\"link_handshake\"") || out.contains("step=link_handshake"));
    let ms: u64 = out
        .split(" ms=")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("no ms in {out}"));
    assert!((30..2000).contains(&ms), "{ms} ms in {out}");
}

#[test]
fn a_step_in_a_pool_names_the_pool_by_its_alias() {
    let out = said_while(|| begin_in(Name::SshForwardOpen, "lab").done());
    assert!(out.contains("(pool: lab)"), "{out}");
}

#[test]
fn a_step_dropped_without_being_done_says_refused() {
    // An early return or a `?` drops the guard: that is the failing way out.
    fn leaves_early() -> Result<(), &'static str> {
        let _step = begin(Name::HarnessSpawn);
        Err("no")?;
        Ok(())
    }
    let out = said_while(|| {
        let _ = leaves_early();
    });
    assert!(out.contains("harness_spawn refused after"), "{out}");
}

#[test]
fn a_step_closed_with_done_says_done() {
    let out = said_while(|| begin(Name::LinkCheck).done());
    assert!(out.contains("link_check done after"), "{out}");
}

#[test]
fn a_step_ended_by_an_answer_is_done_on_ok_and_refused_on_err() {
    let out = said_while(|| {
        let answer: Result<(), &str> = begin(Name::LinkCheck).ended(Err("no"));
        assert!(answer.is_err());
    });
    assert!(out.contains("link_check refused after"), "{out}");
    let out = said_while(|| {
        let _ = begin(Name::LinkCheck).ended(Ok::<_, &str>(()));
    });
    assert!(out.contains("link_check done after"), "{out}");
}

#[test]
fn a_miss_that_is_the_answer_is_a_step_done_not_refused() {
    // `docker container inspect` finding nothing on a normal start is `Ok(ran)` with a
    // non-zero exit: the run was made, and its caller reads the answer.
    let out = said_while(|| {
        let ran: Result<i32, &str> = Ok(1);
        let _ = begin_debug("docker_container").ended(ran);
    });
    assert!(!out.contains("refused"), "{out}");
}

#[test]
fn a_step_inside_another_names_its_parent() {
    let out = said_while(|| begin(Name::RunnerStart).under(Name::ContainerCheck).done());
    assert!(out.contains("(parent: container_check)"), "{out}");
    let out = said_while(|| {
        begin_in(Name::RunnerStart, "lab")
            .under(Name::ProfileReady)
            .done()
    });
    assert!(out.contains("(pool: lab, parent: profile_ready)"), "{out}");
}

#[test]
fn a_docker_call_is_written_at_debug_and_a_top_level_step_at_info() {
    let at_info = said_while(|| begin_debug("docker_exec").done());
    assert!(at_info.is_empty(), "{at_info}");
    let said = Said::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(said.clone())
        .with_ansi(false)
        .with_max_level(tracing::Level::DEBUG)
        .finish();
    tracing::subscriber::with_default(subscriber, || begin_debug("docker_exec").done());
    let out = String::from_utf8(said.0.lock().unwrap().clone()).unwrap();
    assert!(
        out.contains("DEBUG") && out.contains("docker_exec done"),
        "{out}"
    );
}

#[test]
fn a_step_the_person_cancelled_says_cancelled_not_refused() {
    let out = said_while(|| begin(Name::StartProgramCall).cancelled());
    assert!(out.contains("start_program_call cancelled after"), "{out}");
    assert!(!out.contains("refused"), "{out}");
}

#[test]
fn a_wait_that_ran_out_says_gave_up_not_refused() {
    let out = said_while(|| begin(Name::StartProgramCall).gave_up());
    assert!(out.contains("start_program_call gave_up after"), "{out}");
    assert!(!out.contains("refused"), "{out}");
}

#[test]
fn a_quiet_step_writes_nothing() {
    let out = said_while(|| begin(Name::BrowserEnginePrepare).quiet());
    assert!(out.is_empty(), "{out}");
}

#[test]
fn a_step_begun_inside_another_names_it_without_being_told() {
    let out = said_while(|| {
        let wrapper = begin(Name::LinkCheck);
        wrapper.inside(|| begin(Name::ContainerCheck).done());
        wrapper.done();
        begin(Name::CodeSend).done();
    });
    assert!(out.contains("container_check done after"), "{out}");
    let line = |name: &str| out.lines().find(|l| l.contains(name)).unwrap().to_owned();
    assert!(
        line("container_check").contains("(parent: link_check)"),
        "{out}"
    );
    assert!(!line("link_check done").contains("(parent:"), "{out}");
    assert!(!line("step=\"code_send\"").contains("(parent:"), "{out}");
}

#[test]
fn the_innermost_running_step_is_the_parent() {
    let out = said_while(|| {
        let outer = begin(Name::FirstLink);
        outer.inside(|| {
            let middle = begin(Name::LinkCheck);
            middle.inside(|| begin(Name::EnginePinCheck).done());
            middle.done();
        });
        outer.done();
    });
    let line = |name: &str| out.lines().find(|l| l.contains(name)).unwrap().to_owned();
    assert!(
        line("engine_pin_check").contains("(parent: link_check)"),
        "{out}"
    );
    assert!(
        line("link_check done").contains("(parent: first_link)"),
        "{out}"
    );
}

#[test]
fn a_context_carries_the_parent_to_another_thread() {
    let out = said_while(|| {
        let wrapper = begin(Name::LinkCheck);
        let carried = wrapper.context();
        let dispatch = tracing::dispatcher::get_default(Clone::clone);
        std::thread::spawn(move || {
            tracing::dispatcher::with_default(&dispatch, || {
                carried.run(|| begin(Name::ContainerCheck).done());
                begin(Name::HoldProject).done();
            });
        })
        .join()
        .unwrap();
        wrapper.done();
    });
    let line = |name: &str| out.lines().find(|l| l.contains(name)).unwrap().to_owned();
    assert!(
        line("container_check").contains("(parent: link_check)"),
        "{out}"
    );
    assert!(!line("hold_project").contains("(parent:"), "{out}");
}

#[test]
fn a_context_carries_the_parent_into_a_task() {
    let out = said_while(|| {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        runtime.block_on(async {
            let wrapper = begin(Name::FirstLink);
            let task = tokio::spawn(wrapper.context().future(async {
                tokio::task::yield_now().await;
                begin(Name::LinkHandshake).done();
            }));
            task.await.unwrap();
            wrapper.done();
        });
    });
    let line = |name: &str| out.lines().find(|l| l.contains(name)).unwrap().to_owned();
    assert!(
        line("link_handshake").contains("(parent: first_link)"),
        "{out}"
    );
}

#[test]
fn a_step_that_has_ended_is_not_named_by_a_context_kept_past_it() {
    // A kept link made inside a start dials again an hour later: that is not inside it.
    let out = said_while(|| {
        let wrapper = begin(Name::FirstLink);
        let kept = wrapper.context();
        wrapper.done();
        kept.run(|| begin(Name::LinkHandshake).done());
    });
    let line = out.lines().find(|l| l.contains("link_handshake")).unwrap();
    assert!(!line.contains("(parent:"), "{out}");
}

#[test]
fn a_docker_call_names_no_parent_even_inside_a_step() {
    let said = Said::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(said.clone())
        .with_ansi(false)
        .with_max_level(tracing::Level::DEBUG)
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        let wrapper = begin(Name::ContainerCheck);
        wrapper.inside(|| begin_debug("docker_container_inspect").done());
        wrapper.done();
    });
    let out = String::from_utf8(said.0.lock().unwrap().clone()).unwrap();
    let line = out
        .lines()
        .find(|l| l.contains("docker_container_inspect"))
        .unwrap();
    assert!(!line.contains("(parent:"), "{out}");
}

/// The line of `name` in `out`.
fn line_of<'a>(out: &'a str, name: &str) -> &'a str {
    out.lines()
        .find(|l| l.contains(&format!("step {name} ")))
        .unwrap_or_else(|| panic!("no line for {name} in {out}"))
}

#[test]
fn the_contract_names_every_top_level_step_and_a_rename_fails_here() {
    // #7864: the step names are a contract. The baseline's top-level steps (#7835's table) and
    // the five #7189 added, and a pool sign-in's three (#7939). Renaming one is a breaking change, with a changelog line: this list
    // changes on purpose only.
    let named: Vec<&str> = Name::ALL.iter().map(|name| name.as_str()).collect();
    assert_eq!(
        named,
        [
            "ssh_forward_open",
            "engine_pin_check",
            "container_check",
            "runner_start",
            "link_dial",
            "link_check",
            "link_prepare",
            "link_exec_spawn",
            "link_handshake",
            "first_link",
            "profile_ready",
            "hold_project",
            "code_send",
            "start_program_call",
            "harness_spawn",
            "browser_engine_prepare",
            "browser_engine_download",
            "browser_engine_start",
            "browser_first_page",
            "view_first_frame",
            "profile_sign_in_start",
            "profile_sign_in_address",
            "profile_sign_in_done",
        ]
    );
    // Each is a name of its own, in the shape a filter or a grep finds.
    let mut unique = named.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), named.len(), "{named:?}");
    assert!(
        named
            .iter()
            .all(|n| n.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')),
        "{named:?}"
    );
}

#[test]
fn every_cause_has_a_word_of_its_own() {
    let words: Vec<&str> = Cause::ALL.iter().map(|cause| cause.as_str()).collect();
    assert_eq!(
        words,
        [
            "unreachable",
            "refused",
            "host_key_changed",
            "key_changed",
            "gone",
            "handshake",
            "timed_out",
            "ended",
        ]
    );
}

#[test]
fn done_cancelled_and_gave_up_are_info_and_refused_is_warn() {
    let out = said_while(|| {
        begin(Name::LinkCheck).done();
        begin(Name::StartProgramCall).cancelled();
        begin(Name::HoldProject).gave_up();
        let _refused = begin(Name::HarnessSpawn);
    });
    assert!(line_of(&out, "link_check").contains(" INFO "), "{out}");
    assert!(
        line_of(&out, "start_program_call").contains(" INFO "),
        "{out}"
    );
    assert!(line_of(&out, "hold_project").contains(" INFO "), "{out}");
    assert!(line_of(&out, "harness_spawn").contains(" WARN "), "{out}");
}

#[test]
fn a_refused_step_is_written_where_only_warnings_are_kept() {
    let said = Said::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(said.clone())
        .with_ansi(false)
        .with_max_level(tracing::Level::WARN)
        .finish();
    tracing::subscriber::with_default(subscriber, || {
        begin(Name::LinkCheck).done();
        let _refused = begin(Name::LinkDial);
    });
    let out = String::from_utf8(said.0.lock().unwrap().clone()).unwrap();
    assert!(out.contains("link_dial refused after"), "{out}");
    assert!(!out.contains("link_check"), "{out}");
}

#[test]
fn every_line_carries_its_step_its_outcome_and_its_ms() {
    let out = said_while(|| begin(Name::CodeSend).done());
    let line = line_of(&out, "code_send");
    assert!(
        line.contains("step=\"code_send\"") || line.contains("step=code_send"),
        "{line}"
    );
    assert!(
        line.contains("outcome=\"done\"") || line.contains("outcome=done"),
        "{line}"
    );
    assert!(line.contains(" ms="), "{line}");
    // What is not known is not written, not even empty.
    for absent in ["pool=", "parent=", "cause=", "attempt=", "start="] {
        assert!(!line.contains(absent), "{absent} in {line}");
    }
}

#[test]
fn a_dial_says_its_attempt_and_its_typed_cause() {
    let out = said_while(|| {
        let _ = begin_in(Name::LinkDial, "lab")
            .attempt(3)
            .because(Cause::Refused);
    });
    let line = line_of(&out, "link_dial");
    assert!(line.contains("refused after"), "{line}");
    assert!(line.contains("attempt=3"), "{line}");
    assert!(
        line.contains("cause=\"refused\"") || line.contains("cause=refused"),
        "{line}"
    );
    assert!(
        line.contains("(pool: lab, cause: refused, attempt: 3)"),
        "{line}"
    );
}

#[test]
fn a_cause_is_not_written_on_a_step_that_was_done() {
    let out = said_while(|| {
        begin(Name::LinkDial)
            .attempt(1)
            .because(Cause::Unreachable)
            .done()
    });
    let line = line_of(&out, "link_dial");
    assert!(!line.contains("cause"), "{line}");
    assert!(line.contains("attempt=1"), "{line}");
}

#[test]
fn a_start_names_its_id_on_every_step_begun_inside_it_on_any_thread() {
    let id = StartId::parse("01J9ZQ5000000000000000000A").unwrap();
    let out = said_while(|| {
        let starting = Context::of_start(id.clone());
        starting.run(|| {
            begin(Name::HoldProject).done();
            let carried = current();
            let dispatch = tracing::dispatcher::get_default(Clone::clone);
            std::thread::spawn(move || {
                tracing::dispatcher::with_default(&dispatch, || {
                    carried.run(|| begin(Name::StartProgramCall).done());
                });
            })
            .join()
            .unwrap();
            assert_eq!(start_id(), Some(id.clone()));
        });
        // Outside the start, no id.
        begin(Name::CodeSend).done();
        assert_eq!(start_id(), None);
    });
    assert!(
        line_of(&out, "hold_project").contains("start: 01J9ZQ5000000000000000000A"),
        "{out}"
    );
    assert!(
        line_of(&out, "start_program_call").contains("start=\"01J9ZQ5000000000000000000A\"")
            || line_of(&out, "start_program_call").contains("start=01J9ZQ5000000000000000000A"),
        "{out}"
    );
    assert!(!line_of(&out, "code_send").contains("start"), "{out}");
}

#[test]
fn a_step_told_its_start_names_it_where_no_context_carries_one() {
    // The host's `harness_spawn`: the id came over the link, in the start it was asked for.
    let id = StartId::parse("01J9ZQ5000000000000000000A").unwrap();
    let out = said_while(|| begin(Name::HarnessSpawn).of_start(Some(&id)).done());
    assert!(
        line_of(&out, "harness_spawn").contains("start: 01J9ZQ5000000000000000000A"),
        "{out}"
    );
}

#[test]
fn a_minted_start_id_reads_back_as_itself_and_two_differ() {
    let one = StartId::mint();
    let other = StartId::mint();
    assert_ne!(one, other);
    assert_eq!(StartId::parse(one.as_str()), Some(one.clone()));
}

#[test]
fn a_line_never_carries_a_host_a_user_a_path_a_docker_argument_or_a_token() {
    // Whatever a caller or a peer hands in where a pool's alias or a start's id goes, the line
    // keeps only what has the shape of one: a pool's name (lower-case letters, digits, dashes)
    // and an id of letters and digits.
    let leaks = [
        "aharon@build-1.example.com",
        "ssh://aharon@10.0.0.5:22",
        "/Users/aharon/.ssh/id_ed25519",
        "--host=tcp://10.0.0.5:2375",
        "-H unix:///var/run/docker.sock",
        // A forge token's shape, spelled in two halves so no scan of this file mistakes it
        // for one.
        concat!("gh", "p_0123456789abcdefghijklmnopqrstuvwxyzAB"),
        "lab\nINFO purlis: step forged done after 1 ms",
        "lab, parent: forged",
        "10.0.0.5",
    ];
    let out = said_while(|| {
        for leak in leaks {
            begin_in(Name::SshForwardOpen, leak).done();
            begin(Name::HarnessSpawn)
                .of_start(StartId::parse(leak).as_ref())
                .done();
            Context::of_start_said(leak).run(|| begin(Name::HoldProject).done());
        }
    });
    for leak in leaks {
        assert!(!out.contains(leak), "{leak:?} is in {out}");
    }
    for needle in [
        "@",
        "/",
        "ghp_",
        "--host",
        "10.0.0.5",
        "forged",
        "unix:",
        "aharon",
        "docker.sock",
        "id_ed25519",
    ] {
        assert!(!out.contains(needle), "{needle:?} is in {out}");
    }
    assert_eq!(out.lines().count(), leaks.len() * 3, "{out}");
}

#[test]
fn a_pool_named_as_a_pool_is_named() {
    let out = said_while(|| begin_in(Name::SshForwardOpen, "gpu-lab-2").done());
    assert!(out.contains("(pool: gpu-lab-2)"), "{out}");
}

#[test]
fn a_start_held_names_its_id_until_it_is_let_go_of() {
    let out = said_while(|| {
        let held = starting(StartId::parse("01JD3Q4M0R8W2ZK7A6F1B9CXYE").unwrap());
        assert_eq!(held.id().as_str(), "01JD3Q4M0R8W2ZK7A6F1B9CXYE");
        begin(Name::HoldProject).done();
        drop(held);
        begin(Name::CodeSend).done();
    });
    assert!(
        line_of(&out, "hold_project").contains("start: 01JD3Q4M0R8W2ZK7A6F1B9CXYE"),
        "{out}"
    );
    assert!(!line_of(&out, "code_send").contains("start"), "{out}");
}

#[test]
fn a_start_id_is_a_ulids_shape_and_nothing_a_token_could_be() {
    // A peer's id is written to the runner's log only where it is one the window could have
    // made: 26 characters of Crockford's base32. A hex token, a cloud key or a word is not.
    assert!(StartId::parse("01JD3Q4M0R8W2ZK7A6F1B9CXYE").is_some());
    for not_one in [
        concat!("AK", "IAIOSFODNN7EXAMPLE"),
        "0123456789abcdef0123456789abcdef01234567",
        "01jd3q4m0r8w2zk7a6f1b9cxye",
        "01JD3Q4M0R8W2ZK7A6F1B9CXYI",
        "01JD3Q4M0R8W2ZK7A6F1B9CXY",
        "buildhost",
    ] {
        assert_eq!(StartId::parse(not_one), None, "{not_one}");
    }
}

#[test]
fn the_contract_pins_every_field_a_line_may_carry() {
    let id = StartId::parse("01JD3Q4M0R8W2ZK7A6F1B9CXYE").unwrap();
    let out = said_while(|| {
        let _held = starting(id);
        let parent = begin(Name::FirstLink);
        parent.inside(|| {
            let _ = begin_in(Name::LinkDial, "lab")
                .attempt(2)
                .because(Cause::Unreachable);
        });
        parent.done();
    });
    let line = line_of(&out, "link_dial");
    // The event's own fields, after its message (the span's `step{…}` before it is not one).
    let fields: Vec<&str> = line
        .rsplit_once(") ")
        .map_or("", |(_, fields)| fields)
        .split_whitespace()
        .filter_map(|word| word.split_once('=').map(|(name, _)| name))
        .collect();
    assert_eq!(
        fields,
        [
            "step", "outcome", "ms", "pool", "parent", "cause", "attempt", "start"
        ],
        "{line}"
    );
    assert!(
        line.contains(
            "(pool: lab, parent: first_link, cause: unreachable, attempt: 2, start: 01JD3Q4M0R8W2ZK7A6F1B9CXYE)"
        ),
        "{line}"
    );
}

#[test]
fn a_context_for_later_work_names_no_start() {
    let id = StartId::parse("01JD3Q4M0R8W2ZK7A6F1B9CXYE").unwrap();
    let out = said_while(|| {
        let later = {
            let _held = starting(id);
            current().without_start()
        };
        later.run(|| begin(Name::LinkDial).done());
    });
    assert!(!line_of(&out, "link_dial").contains("start"), "{out}");
}
