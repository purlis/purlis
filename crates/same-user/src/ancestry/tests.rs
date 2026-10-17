// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

#[cfg(any(target_os = "linux", target_os = "android"))]
#[test]
fn a_pidfd_names_its_process_until_it_ends() {
    assert_eq!(
        pidfd_pid("pos:\t0\nflags:\t02000002\nmnt_id:\t15\nino:\t1057\nPid:\t4242\nNSpid:\t4242\n"),
        Some(4242)
    );
    assert_eq!(pidfd_pid("pos:\t0\nPid:\t-1\nNSpid:\t-1\n"), Some(-1));
    assert_eq!(pidfd_pid("pos:\t0\n"), None);
}

/// Where [`a_listener_that_has_ended_is_told_as_gone`] has this test program listen, when
/// it starts it again as the listener.
#[cfg(any(target_os = "linux", target_os = "android"))]
const LISTEN_AT: &str = "PURLIS_SAME_USER_TEST_LISTEN_AT";

/// Not a test of its own: this test program, started again by
/// [`a_listener_that_has_ended_is_told_as_gone`], listening at [`LISTEN_AT`] until it is
/// ended.
#[cfg(any(target_os = "linux", target_os = "android"))]
#[test]
#[ignore = "started by a_listener_that_has_ended_is_told_as_gone"]
fn listens_until_ended() {
    let Some(path) = std::env::var_os(LISTEN_AT) else {
        return;
    };
    let _listening = std::os::unix::net::UnixListener::bind(path).expect("a socket");
    println!("up");
    std::thread::sleep(std::time::Duration::from_secs(30));
}

/// A listener that has ended is no listener: the pidfd of the far end names it while it
/// runs, and nobody once it has gone.
#[cfg(any(target_os = "linux", target_os = "android"))]
#[test]
fn a_listener_that_has_ended_is_told_as_gone() {
    use std::io::BufRead;
    let dir = tempfile::tempdir().expect("a folder");
    let path = dir.path().join("s");
    let mut listener = Command::new(std::env::current_exe().expect("this test program"))
        .args([
            "--exact",
            "ancestry::tests::listens_until_ended",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(LISTEN_AT, &path)
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("it listens");
    let mut out = std::io::BufReader::new(listener.stdout.take().expect("its output"));
    let mut line = String::new();
    // The test runner prints the test's name first, on the same line.
    while !line.trim_end().ends_with(" up") && line.trim() != "up" {
        line.clear();
        assert!(
            out.read_line(&mut line).expect("it says") > 0,
            "it never listened"
        );
    }
    let socket = std::os::unix::net::UnixStream::connect(&path).expect("connected");
    assert!(listener_is(&socket, listener.id()).expect("read"));
    listener.kill().expect("ended");
    listener.wait().expect("reaped");
    assert!(!listener_is(&socket, listener.id()).expect("read"));
}

/// The listener of a socket this process connected to is told by its pidfd: this test's
/// own process, listening, and nobody once it has gone.
#[cfg(any(target_os = "linux", target_os = "android"))]
#[test]
fn the_far_end_of_a_socket_is_told_by_its_pidfd() {
    let dir = tempfile::tempdir().expect("a folder");
    let path = dir.path().join("s");
    let _listening = std::os::unix::net::UnixListener::bind(&path).expect("a socket");
    let socket = std::os::unix::net::UnixStream::connect(&path).expect("connected");
    assert!(listener_is(&socket, std::process::id()).expect("read"));
    assert!(!listener_is(&socket, 1).expect("read"));
}

/// 50 → 40 → 30 (a chat's program) → 20 → 1; 60 → 1, in session 30, its parent gone.
fn tree(pid: u32) -> Option<u32> {
    [(50, 40), (40, 30), (30, 20), (20, 1), (60, 1), (70, 1)]
        .into_iter()
        .find(|(child, _)| *child == pid)
        .map(|(_, parent)| parent)
}

fn session(pid: u32) -> io::Result<u32> {
    Ok(if matches!(pid, 50 | 40 | 30 | 60) {
        30
    } else {
        1
    })
}

#[test]
fn a_process_below_a_chat_s_program_is_inside_it() {
    assert_eq!(inside_a_chat(50, &[30], tree, session).unwrap(), Some(30));
}

#[test]
fn a_chat_s_program_itself_is_inside_it() {
    assert_eq!(inside_a_chat(30, &[30], tree, session).unwrap(), Some(30));
}

#[test]
fn a_process_orphaned_from_a_chat_is_inside_it_by_its_session() {
    assert_eq!(inside_a_chat(60, &[30], tree, session).unwrap(), Some(30));
}

#[test]
fn a_process_in_no_chat_s_tree_or_session_is_outside() {
    assert_eq!(inside_a_chat(70, &[30], tree, session).unwrap(), None);
    assert_eq!(inside_a_chat(20, &[30], tree, session).unwrap(), None);
}

#[test]
fn a_process_whose_parent_or_session_cannot_be_read_is_never_vouched_outside() {
    assert!(
        inside_a_chat(99, &[30], tree, session).is_err(),
        "no parent"
    );
    assert!(
        inside_a_chat(70, &[30], tree, |_| Err(io::Error::other("gone"))).is_err(),
        "no session"
    );
}

#[test]
fn a_parent_table_that_loops_is_never_vouched_outside() {
    let looping = |pid: u32| Some(if pid == 5 { 6 } else { 5 });
    assert!(inside_a_chat(5, &[30], looping, session).is_err());
}

#[test]
fn this_process_s_session_is_read_from_the_kernel() {
    let ours = session_of(std::process::id()).unwrap();
    assert!(ours > 0);
}

#[test]
fn an_ancestor_that_holds_the_socket_is_the_host() {
    assert!(judge(10, &[30, 20, 10], true).is_ok());
}

#[test]
fn an_ancestor_that_reused_a_dead_listener_s_pid_holds_no_socket_and_is_refused() {
    // A stand-in listened, left its socket with a child and exited; a chat's harness
    // started later and was given the same pid. It is an ancestor, and holds nothing.
    assert!(matches!(
        judge(30, &[30, 20, 10], false),
        Err(NotOurHost::HoldsNoSocketThere { pid: 30 })
    ));
}

#[test]
fn a_process_that_is_no_ancestor_is_never_the_host() {
    assert!(matches!(
        judge(99, &[30, 20, 10], true),
        Err(NotOurHost::NotAnAncestor { pid: 99 })
    ));
}

#[test]
fn a_process_holding_a_socket_at_the_path_is_told_apart_from_one_that_holds_none() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("s.sock");
    let _listener = std::os::unix::net::UnixListener::bind(&path).expect("bound");
    let run = |command: &mut Command| command.output();

    assert!(holds_a_socket_at(std::process::id(), &path, run).expect("readable"));
    // The test's parent (cargo, or whatever ran it) holds nothing there.
    assert!(!holds_a_socket_at(std::os::unix::process::parent_id(), &path, run).expect("readable"));
}

/// #1542: the app that holds a project's hook socket is found by connecting to it, and only
/// while it holds the socket there. Binds a socket, so this first runs on CI.
#[test]
fn the_holder_of_a_socket_is_the_process_listening_there_now_and_nothing_else_is() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    let run = |command: &mut Command| command.output();
    assert_eq!(socket_holder(&path, run), None, "nothing there");

    let listener = std::os::unix::net::UnixListener::bind(&path).expect("bound");
    assert_eq!(
        socket_holder(&path, run),
        Some(std::process::id()),
        "this process listens there"
    );
    let link = dir.path().join("link.sock");
    std::os::unix::fs::symlink(&path, &link).expect("a link");
    assert_eq!(socket_holder(&link, run), None, "never through a link");

    drop(listener);
    assert!(path.exists(), "a listener that has gone leaves its file");
    assert_eq!(
        socket_holder(&path, run),
        None,
        "a crashed app's leftover answers nothing"
    );

    let file = dir.path().join("plain");
    std::fs::write(&file, "").expect("a file");
    assert_eq!(socket_holder(&file, run), None, "not a socket");
}

#[test]
fn a_socket_with_no_file_or_a_plain_file_has_no_holder_and_nothing_is_run() {
    let dir = tempfile::tempdir().expect("a directory");
    let file = dir.path().join("hooks.sock");
    let never = |_: &mut Command| -> io::Result<Output> { panic!("nothing to ask lsof") };
    assert_eq!(socket_holder(&file, never), None, "no file");
    std::fs::write(&file, "").expect("a file");
    assert_eq!(socket_holder(&file, never), None, "a plain file");
}

#[test]
fn proc_net_unix_names_only_its_listening_sockets_at_the_path() {
    let dir = tempfile::tempdir().expect("a directory");
    let path = dir.path().join("hooks.sock");
    std::fs::write(&path, "").expect("a file to resolve");
    let target = std::fs::canonicalize(&path).expect("resolves");
    let shown = path.display();
    let table = format!(
        "Num       RefCount Protocol Flags    Type St Inode Path\n\
         0000000000000000: 00000002 00000000 00010000 0001 01 4242 {shown}\n\
         0000000000000000: 00000003 00000000 00000000 0001 03 4343 {shown}\n\
         0000000000000000: 00000002 00000000 00010000 0001 01 4444 /elsewhere.sock\n\
         0000000000000000: 00000002 00000000 00010000 0001 01 4545\n"
    );

    assert_eq!(listening_at(&table, &target), [4242]);
    assert_eq!(socket_inode(Path::new("socket:[4242]")), Some(4242));
    assert_eq!(socket_inode(Path::new("pipe:[4242]")), None);
}

/// #1146: a path the kernel writes with its own spaces, or with a newline in it, is still
/// the socket's path.
#[test]
fn proc_net_unix_reads_a_path_with_spaces_or_a_newline_whole() {
    let dir = tempfile::tempdir().expect("a directory");
    for folder in ["two  spaces", "a\nnewline", "both  \n0000: and more"] {
        let at = dir.path().join(folder);
        std::fs::create_dir(&at).expect("a folder");
        let path = at.join("hooks.sock");
        std::fs::write(&path, "").expect("a file to resolve");
        let target = std::fs::canonicalize(&path).expect("resolves");
        let shown = path.display();
        let table = format!(
            "Num       RefCount Protocol Flags    Type St Inode Path\n\
             0000000000000000: 00000002 00000000 00010000 0001 01  4242 {shown}\n\
             0000000000000000: 00000002 00000000 00010000 0001 01 4444 /elsewhere.sock\n"
        );

        assert_eq!(listening_at(&table, &target), [4242], "{folder:?}");
    }
}

/// #1146: `lsof` escapes what is not printable ASCII in a name, so a hook socket under a
/// folder with a newline, or with letters past ASCII, is listed as its escaped spelling,
/// and that is the socket; the longest path the app binds a socket at is listed whole.
#[test]
fn lsof_lists_a_socket_whose_path_it_escaped_and_nothing_near_it() {
    let dir = tempfile::tempdir().expect("a directory");
    let base = std::fs::canonicalize(dir.path()).expect("resolves");
    let long = "l".repeat(
        103usize
            .saturating_sub(base.as_os_str().len() + "/hooks.sock".len() + 1)
            .max(1),
    );
    for folder in [
        "a\nnewline",
        "tab\there",
        "Проекты",
        "del\u{7f}",
        long.as_str(),
    ] {
        let at = base.join(folder);
        std::fs::create_dir(&at).expect("a folder");
        let path = at.join("hooks.sock");
        std::fs::write(&path, "").expect("a file to resolve");
        let target = std::fs::canonicalize(&path).expect("resolves");
        let printed = format!("p1\nf3\nn{}\n", lsof_spelling(&path));
        assert!(
            printed.is_ascii(),
            "lsof prints ASCII in the C locale: {printed}"
        );
        assert!(
            lsof_lists(&printed, &path, &target),
            "{folder:?}: {printed}"
        );

        let shorter = lsof_spelling(&path);
        let near = format!("p1\nf3\nn{}\n", &shorter[..shorter.len() - 1]);
        assert!(!lsof_lists(&near, &path, &target), "{folder:?}: {near}");
    }
    assert!(base.join(long).join("hooks.sock").as_os_str().len() <= 103);
    assert_eq!(lsof_spelling(Path::new("/a\u{1}\u{ff}")), "/a^A\\xc3\\xbf");
}

#[test]
fn lsof_names_are_the_paths_and_never_a_client_s_arrow() {
    assert_eq!(
        lsof_names("p85072\nf3\nn/tmp/a.sock\nf4\nn->0x4f16079b877a2aa\nf5\nn/tmp/a.sock\n"),
        ["/tmp/a.sock", "/tmp/a.sock"]
    );
}

/// Now, in microseconds since 1970: the clock a macOS start time is read against.
#[cfg(target_os = "macos")]
fn micros_now() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("after 1970")
        .as_micros()
}

/// A macOS start time, `<seconds>.<microseconds>`, in microseconds since 1970.
#[cfg(target_os = "macos")]
fn micros_of(started: &str) -> u128 {
    let (seconds, micros) = started.split_once('.').expect("seconds.microseconds");
    assert_eq!(micros.len(), 6, "microseconds, zero-padded: {started}");
    seconds.parse::<u128>().expect("seconds") * 1_000_000
        + micros.parse::<u128>().expect("microseconds")
}

#[cfg(target_os = "macos")]
#[test]
fn a_child_s_start_is_a_time_of_day_between_before_and_after_it_was_started() {
    let before = std::time::SystemTime::now();
    let mut child = Command::new("/bin/sleep")
        .arg("30")
        .stdin(std::process::Stdio::null())
        .spawn()
        .expect("a child");
    let after = std::time::SystemTime::now();
    let at = Parents::started_at(child.id()).expect("the child's start");
    let _ = child.kill();
    let _ = child.wait();
    // The kernel's start is in microseconds; the clock read here may be finer.
    let slack = std::time::Duration::from_micros(1);
    assert!(at + slack >= before && at <= after + slack, "{at:?}");
    assert_eq!(Parents::started_at(child.id()), None, "a reaped child");
}

#[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
#[test]
fn this_process_and_a_child_it_starts_are_read_as_the_kernel_has_them() {
    let me = std::process::id();
    assert_eq!(
        Parents::of(me),
        Some(std::os::unix::process::parent_id()),
        "this process's parent"
    );
    let mine = Parents::started(me).expect("this process's start");
    assert_eq!(
        Parents::started(me).as_deref(),
        Some(mine.as_str()),
        "a start is read the same every time"
    );

    #[cfg(target_os = "macos")]
    let before = micros_now();
    let mut child = Command::new("/bin/sleep")
        .arg("30")
        .stdin(std::process::Stdio::null())
        .spawn()
        .expect("a child");
    #[cfg(target_os = "macos")]
    let after = micros_now();

    assert_eq!(Parents::of(child.id()), Some(me), "the child's parent");
    assert_eq!(Parents::chain(child.id()).first(), Some(&me));
    let theirs = Parents::started(child.id()).expect("the child's start");
    assert_eq!(
        Parents::started(child.id()).as_deref(),
        Some(theirs.as_str()),
        "the child's start is read the same every time"
    );
    // Linux counts clock ticks since boot, which no wall clock here can be held against:
    // the child's is a count, and no earlier than this process's own.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        let ticks = |started: &str| started.parse::<u64>().expect("clock ticks since boot");
        assert!(
            ticks(&mine) <= ticks(&theirs),
            "the child started at {theirs}, before this process did at {mine}"
        );
    }
    // Exact, and inside this test's run: after this process began, and between the two
    // readings of the clock around the spawn.
    #[cfg(target_os = "macos")]
    {
        let (mine, theirs) = (micros_of(&mine), micros_of(&theirs));
        assert!(mine < before, "this process began before the test did");
        assert!(
            before <= theirs && theirs <= after,
            "the child started at {theirs}, outside {before}..={after}"
        );
    }

    // Killed and not yet waited for: a zombie, which macOS answers nothing about.
    let _ = child.kill();
    #[cfg(target_os = "macos")]
    {
        let patience = std::time::Instant::now();
        while Parents::started(child.id()).is_some()
            && patience.elapsed() < std::time::Duration::from_secs(10)
        {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(
            Parents::started(child.id()),
            None,
            "exited and unreaped reads as gone"
        );
        assert_eq!(Parents::of(child.id()), None, "and so has no parent");
    }
    let _ = child.wait();
    assert_eq!(
        Parents::started(child.id()),
        None,
        "gone, so nothing is read"
    );
    assert_eq!(Parents::of(child.id()), None, "gone, so no parent");
}

/// A walk from a person's terminal passes through root's processes (`login`, `launchd`):
/// their parents are read, though when they started is theirs alone.
#[cfg(target_os = "macos")]
#[test]
fn another_user_s_process_has_a_parent_read_and_no_start() {
    assert_eq!(
        Parents::started(1),
        None,
        "root's, so not this user's to read"
    );
    match Parents::of(1) {
        Some(parent) => assert_eq!(parent, 0, "launchd, under the kernel"),
        // A suite run inside a sandbox whose profile lets a process read only its own
        // sandbox's processes (`process-info* (target same-sandbox)`) is refused launchd
        // by the profile, not by the kernel's own rule: nothing to tell here.
        None => eprintln!(
            "skipped: this sandbox does not let the test read launchd's parent, so only \
             \"no start for root's process\" was checked"
        ),
    }
    assert_eq!(Parents::of(0), None, "pid 0 names no process to ask about");
    assert_eq!(Parents::of(u32::MAX), None, "not a pid at all");
}

/// What stamps a commit a chat's agent makes (`purlis_core::process::descends_from`): the
/// process making it is below the chat's recorded program, however far.
#[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
#[test]
fn a_process_two_below_this_one_walks_up_to_it_and_not_to_one_beside_it() {
    use std::io::BufRead;
    // This process -> sh -> sh -> sleep, the innermost saying its own pid.
    let mut below = Command::new("/bin/sh")
        .args(["-c", "/bin/sh -c 'echo $$; exec /bin/sleep 30' & wait"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("a child");
    let mut beside = Command::new("/bin/sleep")
        .arg("30")
        .stdin(std::process::Stdio::null())
        .spawn()
        .expect("another child");
    let mut said = String::new();
    std::io::BufReader::new(below.stdout.take().expect("its output"))
        .read_line(&mut said)
        .expect("a line");
    let deepest: u32 = said.trim().parse().expect("a pid");

    let reaches_us = walk(deepest, std::process::id(), Parents::of);
    let reaches_a_sibling = walk(deepest, beside.id(), Parents::of);
    let chain = Parents::chain(deepest);

    let _ = Command::new("/bin/kill").arg(deepest.to_string()).status();
    for child in [&mut below, &mut beside] {
        let _ = child.kill();
        let _ = child.wait();
    }
    assert!(reaches_us, "two below this process: {chain:?}");
    assert_eq!(chain.first(), Some(&below.id()), "its parent first");
    assert!(!reaches_a_sibling, "a process beside it is not above it");
}

/// A sandboxed chat (D-1407-8b): a seatbelt profile lets a process read only the processes
/// of its own sandbox, and the chat's program is the topmost of those. The walk stops ON
/// the recorded pid, so nothing above the sandbox is ever asked about.
#[test]
fn a_walk_reaches_the_recorded_program_without_reading_it_or_anything_above_it() {
    // 50 -> 40 -> 30 (the chat's program) -> 20 (the app, outside the sandbox) -> 1.
    let inside_the_sandbox = |pid| match pid {
        50 => Some(40),
        40 => Some(30),
        30 | 20 => panic!("process {pid} was read, and the sandbox would have refused it"),
        _ => None,
    };
    assert!(walk(50, 30, inside_the_sandbox));
    // The app, above the sandbox, is reached only through a reading the sandbox refuses.
    let refused_above = |pid| match pid {
        50 => Some(40),
        40 => Some(30),
        _ => None,
    };
    assert!(!walk(50, 20, refused_above), "a doubt answers no");
}

#[test]
fn a_walk_stops_at_the_top_at_a_gap_and_at_a_loop() {
    let tree = |pid| match pid {
        30 => Some(20),
        20 => Some(10),
        10 => Some(1),
        1 => Some(0),
        7 => Some(7),
        8 => Some(9),
        9 => Some(8),
        _ => None,
    };
    assert!(walk(30, 10, tree));
    assert!(walk(30, 1, tree));
    assert!(!walk(30, 99, tree), "the chain ends at the top");
    assert!(!walk(20, 30, tree), "a child is not an ancestor");
    assert!(!walk(55, 10, tree), "no parent known");
    assert!(!walk(7, 99, tree), "its own parent");
    assert!(!walk(8, 10, tree), "a loop");
}

#[test]
fn the_boot_time_is_the_btime_line_of_proc_stat_and_nothing_else() {
    let stat = "cpu  1 2 3 4\nintr 9 9\nctxt 77\nbtime 1760000000\nprocesses 4242\n";
    assert_eq!(boot_time_in(stat), Some(1_760_000_000));
    assert_eq!(boot_time_in("cpu 1 2\nbtimes 5\n"), None, "another word");
    assert_eq!(boot_time_in("btime soon\n"), None, "not a number");
    assert_eq!(boot_time_in(""), None);
}

#[test]
fn a_start_in_clock_ticks_is_a_time_of_day_after_the_boot() {
    let boot = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_760_000_000);
    assert_eq!(
        time_of_day(1_760_000_000, 12_345, 100),
        Some(boot + std::time::Duration::from_millis(123_450)),
        "123.45 s after the boot, at 100 ticks a second"
    );
    assert_eq!(
        time_of_day(1_760_000_000, 7, 1000),
        Some(boot + std::time::Duration::from_millis(7))
    );
    assert_eq!(time_of_day(1_760_000_000, 7, 0), None, "no tick rate");
    assert_eq!(
        time_of_day(u64::MAX, u64::MAX, 1),
        None,
        "past what a time can hold"
    );
}

/// Linux reads a start as the boot's time of day plus the start's clock ticks (#1542): the
/// boot time is whole seconds and the kernel may step it with the clock, so a start is
/// held to within a second or two either way.
#[cfg(any(target_os = "linux", target_os = "android"))]
#[test]
fn a_child_s_start_on_linux_is_a_time_of_day_around_when_it_was_started() {
    let before = std::time::SystemTime::now();
    let mut child = Command::new("/bin/sleep")
        .arg("30")
        .stdin(std::process::Stdio::null())
        .spawn()
        .expect("a child");
    let after = std::time::SystemTime::now();
    let at = Parents::started_at(child.id()).expect("the child's start");
    let _ = child.kill();
    let _ = child.wait();
    let slack = std::time::Duration::from_secs(2);
    assert!(at + slack >= before && at <= after + slack, "{at:?}");
    assert_eq!(Parents::started_at(child.id()), None, "a reaped child");
    assert!(
        Parents::started_at(std::process::id()).is_some_and(|mine| mine <= at),
        "this process started before its child"
    );
}

#[test]
fn stat_parent_reads_after_the_last_parenthesis() {
    assert_eq!(stat_parent("123 (a (b) c) S 45 6 7"), Some(45));
    assert_eq!(stat_parent("garbage"), None);
}
