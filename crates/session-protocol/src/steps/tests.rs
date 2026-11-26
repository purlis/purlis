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
        let step = begin("link_handshake");
        std::thread::sleep(std::time::Duration::from_millis(30));
        step.done();
    });
    assert!(out.contains("INFO"), "{out}");
    assert!(out.contains("step=\"link_handshake\"") || out.contains("step=link_handshake"));
    let ms: u64 = out
        .split("elapsed_ms=")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("no elapsed_ms in {out}"));
    assert!((30..2000).contains(&ms), "{ms} ms in {out}");
}

#[test]
fn a_step_in_a_pool_names_the_pool_by_its_alias() {
    let out = said_while(|| begin_in("ssh_forward_open", "lab").done());
    assert!(out.contains("(pool: lab)"), "{out}");
}

#[test]
fn a_step_dropped_without_being_done_says_refused() {
    // An early return or a `?` drops the guard: that is the failing way out.
    fn leaves_early() -> Result<(), &'static str> {
        let _step = begin("harness_spawn");
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
    let out = said_while(|| begin("link_check").done());
    assert!(out.contains("link_check done after"), "{out}");
}

#[test]
fn a_step_ended_by_an_answer_is_done_on_ok_and_refused_on_err() {
    let out = said_while(|| {
        let answer: Result<(), &str> = begin("link_check").ended(Err("no"));
        assert!(answer.is_err());
    });
    assert!(out.contains("link_check refused after"), "{out}");
    let out = said_while(|| {
        let _ = begin("link_check").ended(Ok::<_, &str>(()));
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
    let out = said_while(|| begin("runner_start").under("container_check").done());
    assert!(out.contains("(parent: container_check)"), "{out}");
    let out = said_while(|| begin_in("runner_start", "lab").under("x").done());
    assert!(out.contains("(pool: lab, parent: x)"), "{out}");
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
    let out = said_while(|| begin("start_program_call").cancelled());
    assert!(out.contains("start_program_call cancelled after"), "{out}");
    assert!(!out.contains("refused"), "{out}");
}

#[test]
fn a_wait_that_ran_out_says_gave_up_not_refused() {
    let out = said_while(|| begin("start_program_call").gave_up());
    assert!(out.contains("start_program_call gave_up after"), "{out}");
    assert!(!out.contains("refused"), "{out}");
}

#[test]
fn a_quiet_step_writes_nothing() {
    let out = said_while(|| begin("browser_engine_prepare").quiet());
    assert!(out.is_empty(), "{out}");
}

#[test]
fn a_step_begun_inside_another_names_it_without_being_told() {
    let out = said_while(|| {
        let wrapper = begin("link_check");
        wrapper.inside(|| begin("container_check").done());
        wrapper.done();
        begin("after").done();
    });
    assert!(out.contains("container_check done after"), "{out}");
    let line = |name: &str| out.lines().find(|l| l.contains(name)).unwrap().to_owned();
    assert!(
        line("container_check").contains("(parent: link_check)"),
        "{out}"
    );
    assert!(!line("link_check done").contains("(parent:"), "{out}");
    assert!(!line("step=\"after\"").contains("(parent:"), "{out}");
}

#[test]
fn the_innermost_running_step_is_the_parent() {
    let out = said_while(|| {
        let outer = begin("first_link");
        outer.inside(|| {
            let middle = begin("link_check");
            middle.inside(|| begin("engine_pin_check").done());
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
        let wrapper = begin("link_check");
        let carried = wrapper.context();
        let dispatch = tracing::dispatcher::get_default(Clone::clone);
        std::thread::spawn(move || {
            tracing::dispatcher::with_default(&dispatch, || {
                carried.run(|| begin("container_check").done());
                begin("not_inside").done();
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
    assert!(!line("not_inside").contains("(parent:"), "{out}");
}

#[test]
fn a_context_carries_the_parent_into_a_task() {
    let out = said_while(|| {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        runtime.block_on(async {
            let wrapper = begin("first_link");
            let task = tokio::spawn(wrapper.context().future(async {
                tokio::task::yield_now().await;
                begin("link_handshake").done();
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
        let wrapper = begin("first_link");
        let kept = wrapper.context();
        wrapper.done();
        kept.run(|| begin("link_handshake").done());
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
        let wrapper = begin("container_check");
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
