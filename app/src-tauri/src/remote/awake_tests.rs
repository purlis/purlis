//! The keep-awake assertion (iOS design §6.4): wanted only with the setting and a working
//! chat together, held by one child for as long as it is wanted and gone when released, and
//! the platform's own program, bound to this process or to the pipe it reads.

use std::process::Command;

use super::awake::{Holder, Program, wanted};

#[test]
fn the_setting_and_a_working_chat_are_both_needed() {
    assert!(!wanted(false, false));
    assert!(!wanted(false, true));
    assert!(!wanted(true, false));
    assert!(wanted(true, true));
}

/// Whether a process with `pid` is still there (`kill -0`).
fn alive(pid: u32) -> bool {
    purlis_core::forklock::status(Command::new("kill").args(["-0", &pid.to_string()]))
        .is_ok_and(|status| status.success())
}

#[test]
fn the_assertion_follows_what_is_wanted_and_holds_one_child_that_ends_on_release() {
    let holder = Holder::new(Program::test_double());
    assert!(!holder.held());
    holder.set(true);
    assert!(holder.held());
    let pid = holder.child_pid().unwrap();
    assert!(alive(pid));
    holder.set(true);
    assert_eq!(
        holder.child_pid(),
        Some(pid),
        "the same answer twice is one child"
    );
    holder.set(false);
    assert!(!holder.held());
    assert!(holder.child_pid().is_none());
    assert!(!alive(pid), "the child is gone with the pipe");
    holder.set(false);
    assert!(!holder.held(), "released twice is released");
}

#[test]
fn the_real_program_is_the_platforms_and_bound_to_this_process() {
    let real = Program::real();
    #[cfg(target_os = "macos")]
    {
        assert_eq!(real.argv()[0], "caffeinate");
        assert!(
            real.argv().contains(&std::process::id().to_string()),
            "{:?}",
            real.argv()
        );
    }
    #[cfg(target_os = "linux")]
    {
        assert_eq!(real.argv()[0], "systemd-inhibit");
        assert_eq!(real.argv().last().map(String::as_str), Some("cat"));
    }
    assert!(real.argv().len() > 1);
}
