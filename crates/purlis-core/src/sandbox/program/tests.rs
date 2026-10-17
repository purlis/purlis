// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

use super::*;

/// #1040: purlis's own program, which wraps a chat on Linux, is refused where the chat may
/// write it, and once it was replaced while it ran; elsewhere it is its real path.
#[test]
fn purlis_s_own_program_is_held_as_the_harness_s_is() {
    let f = Fixture::new();
    let cwd = f.cwd();
    let chat = Chat {
        cwd: &cwd,
        writable: &[],
        env: &[],
    };
    let inside = f.plane.path().join("purlis");
    std::fs::write(&inside, "").expect("a program");
    assert!(matches!(
        checked_wrap_at(&inside, f.plane.path(), chat),
        Err(NotStarted::ProgramWritable(_))
    ));
    let replaced = f.outside.path().join("purlis (deleted)");
    assert_eq!(
        checked_wrap_at(&replaced, f.plane.path(), chat),
        Err(NotStarted::WrapReplaced)
    );
    let outside = f.outside.path().join("purlis");
    std::fs::write(&outside, "").expect("a program");
    assert_eq!(
        checked_wrap_at(&outside, f.plane.path(), chat),
        Ok(outside.canonicalize().expect("real"))
    );
    assert!(
        NotStarted::WrapReplaced
            .to_string()
            .contains("Restart purlis, then start the chat again.")
    );
}

/// A plane, a chat folder in it, and a place outside both that the test counts as not
/// writable by the chat (the temp roots are the caller's to add, and left out here).
struct Fixture {
    plane: tempfile::TempDir,
    outside: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let plane = tempfile::tempdir().expect("a plane");
        std::fs::create_dir_all(plane.path().join("workspaces/w")).expect("a workspace");
        Self {
            plane,
            outside: tempfile::tempdir().expect("outside"),
        }
    }

    fn cwd(&self) -> PathBuf {
        self.plane.path().join("workspaces/w")
    }

    fn check(
        &self,
        harness: Harness,
        program: &Path,
        writable: &[PathBuf],
        probe: Option<&Probe<'_>>,
    ) -> Result<Vec<String>, NotStarted> {
        let cwd = self.cwd();
        checked(
            harness,
            &[program.display().to_string()],
            self.plane.path(),
            Chat {
                cwd: &cwd,
                writable,
                env: &[],
            },
            probe,
        )
    }
}

const CLAUDE: &str = "#!/bin/sh\necho '2.1.288 (Claude Code)'\n";

#[test]
fn the_claude_code_version_a_start_heard_is_kept_in_its_shape_alone() {
    // #1353: a Report's draft names it. Other tests answer too, so only the shape is
    // asserted of what is kept, and that a harness never asked has none.
    let fx = Fixture::new();
    let claude = stand_in::program(fx.outside.path(), "my-claude", CLAUDE);
    let probe = |_: &[String]| Some("2.1.299 (Claude Code)\n/Users/dev/x".to_owned());
    assert!(
        fx.check(Harness::ClaudeCode, &claude, &[], Some(&probe))
            .is_ok()
    );
    let kept = version_heard(Harness::ClaudeCode).expect("kept");
    let parts: Vec<&str> = kept.split('.').collect();
    assert_eq!(parts.len(), 3, "{kept}");
    assert!(
        parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit())),
        "{kept}"
    );
    assert_eq!(version_heard(Harness::Codex), None);
    assert_eq!(version_heard(Harness::Opencode), None);
}

#[test]
fn a_claude_code_profile_starts_sandboxed_only_on_claude_code_where_no_chat_writes() {
    let fx = Fixture::new();
    let claude = stand_in::program(fx.outside.path(), "my-claude", CLAUDE);
    let renamed = stand_in::program(
        fx.outside.path(),
        "claude",
        "#!/bin/sh\necho 'codex-cli 0.147.0'\n",
    );
    let real = claude.canonicalize().expect("real");
    // A legitimate wrapper outside, answering as Claude Code: it runs by its real path.
    assert_eq!(
        fx.check(Harness::ClaudeCode, &claude, &[], None),
        Ok(vec![real.display().to_string()])
    );
    // A program named `claude` that is something else: its sandbox would bind nothing.
    assert!(matches!(
        fx.check(Harness::ClaudeCode, &renamed, &[], None),
        Err(NotStarted::NotTheHarness(Harness::ClaudeCode, _, Some(ref said)))
            if said == "codex-cli 0.147.0"
    ));
}

#[test]
fn a_program_where_the_chat_can_write_is_refused_by_its_path_and_by_its_real_path() {
    let fx = Fixture::new();
    let real_outside = stand_in::program(fx.outside.path(), "claude", CLAUDE);
    let refused = |result: Result<Vec<String>, NotStarted>| {
        matches!(result, Err(NotStarted::ProgramWritable(_)))
    };
    // A wrapper in the plane, and one in the chat's own folder.
    let inside = stand_in::program(fx.plane.path(), "claude-wrap", CLAUDE);
    let in_cwd = stand_in::program(&fx.cwd(), "claude-here", CLAUDE);
    // A link in the plane to a real program outside: the chat could rewrite the link.
    let link = fx.cwd().join("claude-link");
    std::os::unix::fs::symlink(&real_outside, &link).expect("a link");
    // A linked folder in the plane, to the folder outside the program is in.
    let dir_link = fx.cwd().join("bin");
    std::os::unix::fs::symlink(fx.outside.path(), &dir_link).expect("a linked folder");
    // A link outside to a program in the plane.
    let elsewhere = tempfile::tempdir().expect("elsewhere");
    let out_link = elsewhere.path().join("claude");
    std::os::unix::fs::symlink(&inside, &out_link).expect("a link out");
    for (what, program) in [
        ("in the plane", inside.clone()),
        ("in the chat's folder", in_cwd),
        ("a link in the plane", link),
        (
            "through a linked folder in the plane",
            dir_link.join("claude"),
        ),
        ("a link outside to the plane", out_link),
    ] {
        for harness in [Harness::ClaudeCode, Harness::Opencode] {
            assert!(
                refused(fx.check(harness, &program, &[], None)),
                "{what}, {harness:?}"
            );
        }
    }
    // Anywhere else the sandbox lets the chat write: a temp folder, opencode's own data.
    let writable = tempfile::tempdir().expect("a writable place");
    let in_writable = stand_in::program(writable.path(), "claude", CLAUDE);
    assert!(refused(fx.check(
        Harness::ClaudeCode,
        &in_writable,
        &[writable.path().to_path_buf()],
        None
    )));
    // The refusal names where.
    assert_eq!(
        NotStarted::ProgramWritable(PathBuf::from("/p/node_modules/.bin/claude")).to_string(),
        "this project runs every chat sandboxed, and the program lives where this chat can \
         write: /p/node_modules/.bin/claude, so it was not started sandboxed. Keep the \
         program outside the project and outside what a chat may write."
    );
}

#[test]
fn every_word_of_the_command_is_held_to_where_the_chat_can_write() {
    // D-88g: an interpreter outside, handed a file a chat can write, runs that file.
    let fx = Fixture::new();
    let claude = stand_in::program(fx.outside.path(), "claude", CLAUDE);
    let in_cwd = stand_in::program(&fx.cwd(), "c.sh", CLAUDE);
    let at_root = stand_in::program(fx.plane.path(), "c.sh", CLAUDE);
    let temp = tempfile::tempdir().expect("a temp folder");
    let in_temp = stand_in::program(temp.path(), "c.sh", CLAUDE);
    let config = fx.plane.path().join("x.json");
    std::fs::write(&config, "{}").expect("a config");
    let not_yet = fx.cwd().join("later.json").display().to_string();
    let lib = fx.plane.path().join("lib");
    std::fs::create_dir_all(&lib).expect("a lib");
    std::fs::create_dir_all(fx.cwd().join("lib")).expect("a lib in the chat's folder");
    std::fs::write(fx.cwd().join("lib/x.mjs"), "").expect("a module in the chat's folder");
    let module = lib.join("x.mjs");
    std::fs::write(&module, "").expect("a module");
    let url = format!("file://{}", module.display());
    let url_flag = format!("--import={url}");
    let short_url = format!("--import=file:{}", module.display());
    // The plane's first letter written as `%XX`: only a URL's reader sees the plane in it.
    let plane_text = fx.plane.path().display().to_string();
    let first = plane_text.chars().nth(1).expect("a letter");
    let encoded_url = format!(
        "--import=file:///%{:02X}{}/lib/x.mjs",
        u32::from(first),
        &plane_text[2..]
    );
    let at_file = format!("@{}", config.display());
    let second_eq = format!("--a=b={}", config.display());
    let colon = format!("--settings:{}", config.display());
    let inline = format!(". {}/c.sh", fx.plane.path().display());
    let json = format!(
        r#"{{"mcpServers":{{"x":{{"command":"/bin/sh","args":["{}"]}}}}}}"#,
        at_root.display().to_string().replace('/', "\\/")
    );
    // Two siblings in one folder: a file the chat has yet to make, reached from outside.
    let plane_name = fx.plane.path().file_name().expect("a name").to_owned();
    let dotdot = format!(
        "--out={}/../{}/later.json",
        fx.outside.path().display(),
        plane_name.to_string_lossy()
    );
    let linked_dir = fx.outside.path().join("to-plane");
    std::os::unix::fs::symlink(fx.plane.path(), &linked_dir).expect("a link to the plane");
    let linked = format!("--out={}/later.json", linked_dir.display());
    let nope_abs = format!(
        "{}/nope/../../{}/lib/x.mjs",
        fx.outside.path().display(),
        plane_name.to_string_lossy()
    );
    let nope_url = format!("--import=file://{nope_abs}");
    let u = |text: &str, from: char| {
        text.chars()
            .map(|c| {
                if c == from {
                    format!("\\u{:04X}", u32::from(c))
                } else {
                    c.to_string()
                }
            })
            .collect::<String>()
    };
    let json_u = format!(
        r#"{{"command":"/bin/sh","args":["{}"]}}"#,
        u(&at_root.display().to_string(), '/')
    );
    let json_letter = format!(
        r#"{{"command":"/bin/sh","args":["{}"]}}"#,
        u(&at_root.display().to_string(), first)
    );
    let json_pct = format!(
        r#"{{"args":["--import=file:///\u0025{:02X}{}/lib/x.mjs"]}}"#,
        u32::from(first),
        &plane_text[2..]
    );
    let json_url = format!(
        r#"{{"args":["--import=file:\/\/\/%{:02X}{}\/lib\/x.mjs"]}}"#,
        u32::from(first),
        plane_text[2..].replace('/', "\\/")
    );
    let s = |path: &Path| path.display().to_string();
    let writable = [temp.path().to_path_buf()];
    let probed = std::sync::atomic::AtomicBool::new(false);
    let probe = |_: &[String]| {
        probed.store(true, std::sync::atomic::Ordering::SeqCst);
        Some("2.1.288 (Claude Code)".to_owned())
    };
    let cwd = fx.cwd();
    let check = |words: &[String]| {
        checked(
            Harness::ClaudeCode,
            words,
            fx.plane.path(),
            Chat {
                cwd: &cwd,
                writable: &writable,
                env: &[],
            },
            Some(&probe),
        )
    };
    let words = |list: &[&str]| list.iter().map(|w| (*w).to_owned()).collect::<Vec<_>>();
    for (what, command, named) in [
        (
            "a shell on a script in the chat's folder",
            words(&["/bin/sh", &s(&in_cwd)]),
            s(&in_cwd),
        ),
        (
            "env on a script at the plane's root",
            words(&["/usr/bin/env", &s(&at_root)]),
            s(&at_root),
        ),
        (
            "a shell on a relative script",
            words(&["/bin/sh", "c.sh"]),
            "c.sh".to_owned(),
        ),
        (
            "a shell on a script in temp",
            words(&["/bin/sh", &s(&in_temp)]),
            s(&in_temp),
        ),
        (
            "a flag's value naming a file in the plane",
            words(&[&s(&claude), &format!("--mcp-config={}", s(&config))]),
            format!("--mcp-config={}", s(&config)),
        ),
        (
            "a flag's argument naming a file in the plane",
            words(&[&s(&claude), "--mcp-config", &s(&config)]),
            s(&config),
        ),
        (
            "a file a chat could make before it runs",
            words(&[&s(&claude), &format!("--settings={not_yet}")]),
            format!("--settings={not_yet}"),
        ),
        (
            "the chat's folder itself",
            words(&[&s(&claude), "--add-dir", "."]),
            ".".to_owned(),
        ),
        (
            "a short flag's attached path",
            words(&[&s(&claude), &format!("-I{}", s(&lib))]),
            format!("-I{}", s(&lib)),
        ),
        (
            "a short flag's attached file",
            words(&[&s(&claude), &format!("-r{}", s(&config))]),
            format!("-r{}", s(&config)),
        ),
        (
            "a short flag's attached relative path",
            words(&[&s(&claude), "-Ilib"]),
            "-Ilib".to_owned(),
        ),
        (
            "a file URL as a flag's value",
            words(&[&s(&claude), &url_flag.clone()]),
            url_flag.clone(),
        ),
        (
            "a file URL as its own word",
            words(&[&s(&claude), &url.clone()]),
            url.clone(),
        ),
        (
            "a short file URL",
            words(&[&s(&claude), &short_url.clone()]),
            short_url.clone(),
        ),
        (
            "a percent-encoded file URL",
            words(&[&s(&claude), &encoded_url.clone()]),
            encoded_url.clone(),
        ),
        (
            "a relative file URL",
            words(&[&s(&claude), "--import=file:lib/x.mjs"]),
            "--import=file:lib/x.mjs".to_owned(),
        ),
        (
            "an argument file",
            words(&[&s(&claude), &at_file.clone()]),
            at_file.clone(),
        ),
        (
            "a value after a second `=`",
            words(&[&s(&claude), &second_eq.clone()]),
            second_eq.clone(),
        ),
        (
            "a value after a `:`",
            words(&[&s(&claude), &colon.clone()]),
            colon.clone(),
        ),
        (
            "a later file through `..`",
            words(&[&s(&claude), &dotdot.clone()]),
            dotdot.clone(),
        ),
        (
            "a later file through a link",
            words(&[&s(&claude), &linked.clone()]),
            linked.clone(),
        ),
        (
            "a path inside inline JSON, its slashes escaped",
            words(&[&s(&claude), "--mcp-config", &json]),
            json.clone(),
        ),
        (
            "`..` after a folder that is not there, in a file URL",
            words(&[&s(&claude), &nope_url.clone()]),
            nope_url.clone(),
        ),
        (
            "`..` after a folder that is not there",
            words(&[&s(&claude), &nope_abs.clone()]),
            nope_abs.clone(),
        ),
        (
            "a relative `..` after a folder that is not there",
            words(&[&s(&claude), "--import=./nope/../lib/x.mjs"]),
            "--import=./nope/../lib/x.mjs".to_owned(),
        ),
        (
            "a relative `..` after a folder that is not there, alone",
            words(&[&s(&claude), "nope/../lib/x.mjs"]),
            "nope/../lib/x.mjs".to_owned(),
        ),
        (
            "a relative path inside inline code",
            words(&[&s(&claude), r#"require("./lib/x.mjs")"#]),
            r#"require("./lib/x.mjs")"#.to_owned(),
        ),
        (
            "a relative path in quotes",
            words(&[&s(&claude), "do 'c.sh'"]),
            "do 'c.sh'".to_owned(),
        ),
        (
            "a relative path after a tab",
            words(&[&s(&claude), ".\tc.sh"]),
            ".\tc.sh".to_owned(),
        ),
        (
            "a relative path after `<`",
            words(&[&s(&claude), "cat<c.sh"]),
            "cat<c.sh".to_owned(),
        ),
        (
            "a relative path first in a list",
            words(&[&s(&claude), "c.sh:/usr/lib"]),
            "c.sh:/usr/lib".to_owned(),
        ),
        (
            "a relative path in inline JSON",
            words(&[&s(&claude), r#"{"command":"/bin/sh","args":["c.sh"]}"#]),
            r#"{"command":"/bin/sh","args":["c.sh"]}"#.to_owned(),
        ),
        (
            "a plane path in inline JSON, every slash `\\u002F`",
            words(&[&s(&claude), &json_u.clone()]),
            json_u.clone(),
        ),
        (
            "a plane path in inline JSON, one letter `\\u`-escaped",
            words(&[&s(&claude), &json_letter.clone()]),
            json_letter.clone(),
        ),
        (
            "a file URL in inline JSON whose `%` is itself escaped",
            words(&[&s(&claude), &json_pct]),
            json_pct.clone(),
        ),
        (
            "a file URL in inline JSON, escaped and percent-encoded",
            words(&[&s(&claude), &json_url.clone()]),
            json_url.clone(),
        ),
        (
            "a path inside inline code",
            words(&["/bin/sh", "-c", &inline]),
            inline.clone(),
        ),
    ] {
        assert_eq!(
            check(&command),
            Err(NotStarted::WordWritable(named)),
            "{what}"
        );
    }
    assert!(
        !probed.load(std::sync::atomic::Ordering::SeqCst),
        "a refused command was run"
    );
    // Words that name no file are left alone: flags, model names, values.
    let real = claude.canonicalize().expect("real").display().to_string();
    assert_eq!(
        check(&words(&[&s(&claude), "--model=x", "--model", "opus", "-p"])),
        Ok(words(&[&real, "--model=x", "--model", "opus", "-p"]))
    );
    assert_eq!(
        NotStarted::WordWritable("/p/c.sh".to_owned()).to_string(),
        "this project runs every chat sandboxed, and this profile's command names /p/c.sh, \
         which lies where this chat can write, so it was not started sandboxed. Keep every \
         file the command names outside the project and outside what a chat may write."
    );
}

#[test]
fn a_word_over_4_kib_is_refused_and_one_under_it_is_checked_in_bounded_time() {
    let fx = Fixture::new();
    let claude = stand_in::program(fx.outside.path(), "claude", CLAUDE);
    let cwd = fx.cwd();
    let probe = |_: &[String]| Some("2.1.288 (Claude Code)".to_owned());
    let check = |word: String| {
        checked(
            Harness::ClaudeCode,
            &[claude.display().to_string(), word],
            fx.plane.path(),
            Chat {
                cwd: &cwd,
                writable: &[],
                env: &[],
            },
            Some(&probe),
        )
    };
    assert_eq!(
        check("x".repeat(WORD_MAX + 1)),
        Err(NotStarted::WordTooLong)
    );
    // Every `/` begins a path, and every part of each is looked up: shared, not repeated.
    let began = Instant::now();
    for unit in ["/a", "/./a", "/usr/../bin/..", "/tmp/../usr", "/x/y"] {
        let word: String = unit.repeat(WORD_MAX / unit.len());
        assert!(check(word).is_ok(), "{unit}");
    }
    assert!(
        began.elapsed() < Duration::from_secs(10),
        "{:?}",
        began.elapsed()
    );
    assert_eq!(
        NotStarted::WordTooLong.to_string(),
        "this project runs every chat sandboxed, and a word of this profile's command is \
         longer than 4 KiB, which purlis does not check, so it was not started sandboxed. \
         Keep what it says in a file outside the project and name that file instead."
    );
}

#[test]
fn a_word_is_read_as_json_reads_its_strings() {
    assert_eq!(
        json_unescaped(r#"\ud83d\ude00 \u002F\/\"\\ \n\t \q \u12 \ud800x"#),
        "\u{1F600} //\"\\ \n\t \\q \\u12 \u{FFFD}x"
    );
    assert_eq!(percent_decoded("%2Fa%2fb%zz%"), "/a/b%zz%");
    assert_eq!(folded(Path::new("/a/./b//../c/..")), PathBuf::from("/a"));
}

#[test]
fn a_relative_program_is_refused() {
    let fx = Fixture::new();
    for program in ["./claude", "bin/claude", "claude"] {
        assert_eq!(
            fx.check(Harness::ClaudeCode, Path::new(program), &[], None),
            Err(NotStarted::ProgramRelative),
            "{program}"
        );
    }
}

#[test]
fn the_answer_is_asked_of_the_real_path_the_chat_then_runs() {
    let fx = Fixture::new();
    let real = stand_in::program(fx.outside.path(), "claude", CLAUDE);
    let elsewhere = tempfile::tempdir().expect("elsewhere");
    let link = elsewhere.path().join("claude");
    std::os::unix::fs::symlink(&real, &link).expect("a link");
    let asked = std::sync::Mutex::new(Vec::new());
    let probe = |words: &[String]| {
        asked.lock().expect("lock").push(words.to_vec());
        Some("2.1.288 (Claude Code)".to_owned())
    };
    let words = fx
        .check(Harness::ClaudeCode, &link, &[], Some(&probe))
        .expect("starts");
    let real = real.canonicalize().expect("real").display().to_string();
    assert_eq!(words, std::slice::from_ref(&real));
    assert_eq!(*asked.lock().expect("lock"), [vec![real]]);
}

#[test]
fn a_program_that_hangs_is_refused_and_its_whole_group_is_killed() {
    let fx = Fixture::new();
    let hang = stand_in::program(fx.outside.path(), "claude", "#!/bin/sh\nsleep 60 &\nwait\n");
    let began = Instant::now();
    let mut probe = None;
    let said = answer_of(
        &[hang.display().to_string()],
        Chat {
            cwd: &fx.cwd(),
            writable: &[],
            env: &[],
        },
        &mut |pid| probe = Some(pid),
    );
    assert_eq!(said, Err(Unanswered::TimedOut));
    assert!(
        began.elapsed() < PATIENCE + Duration::from_secs(5),
        "{:?}",
        began.elapsed()
    );
    // The probe led its own group; nothing of the group is left, its child included.
    let pid = probe.expect("the probe ran");
    let leader =
        rustix::process::Pid::from_raw(i32::try_from(pid).expect("a pid")).expect("not zero");
    // A killed child stays a zombie, still in the group, until init reaps it,
    // so give the reaping a moment rather than reading the group at once.
    let reaped = Instant::now() + Duration::from_secs(5);
    while rustix::process::test_kill_process_group(leader).is_ok() && Instant::now() < reaped {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        rustix::process::test_kill_process_group(leader).is_err(),
        "the wrapper's own child was left running"
    );
    // And the start says it timed out, not that the program is something else.
    let refused = fx.check(Harness::ClaudeCode, &hang, &[], None);
    assert_eq!(refused, Err(NotStarted::ProbeTimedOut(Harness::ClaudeCode)));
    assert!(
        refused
            .expect_err("refused")
            .to_string()
            .contains("start the chat again"),
    );
}

#[test]
fn a_child_that_leaves_the_group_holding_the_answer_does_not_hold_the_start() {
    // It answers at once and exits, but a child it started in a group of its own keeps the
    // answer's pipe open past the deadline. The probe's end does not reach that child, so
    // the test ends it: the child goes the moment a file the test writes is there, when
    // the test is done and if it fails. (By a file it reads, because the probe's own wrap
    // lets it write nothing a test could read its id from on macOS.)
    struct Told(PathBuf);
    impl Drop for Told {
        fn drop(&mut self) {
            let _ = std::fs::write(&self.0, "");
        }
    }
    let fx = Fixture::new();
    let stop = Told(fx.outside.path().join("the-test-is-over"));
    let said = fx.outside.path().join("what-left-the-group");
    let leaver = stand_in::program(
        fx.outside.path(),
        "claude",
        &format!(
            "#!/bin/sh\necho 'not claude'\n\
             perl -e 'setpgrp(0,0); for (1..200) {{ exit if -e $ARGV[0]; \
             select(undef, undef, undef, 0.1) }}' '{stop}' &\n\
             echo $! 2>/dev/null > '{said}.part' && mv '{said}.part' '{said}'\nexit 0\n",
            stop = stop.0.display(),
            said = said.display()
        ),
    );
    let began = Instant::now();
    assert!(fx.check(Harness::ClaudeCode, &leaver, &[], None).is_err());
    // A held pipe reads as no answer in time, which is asked once more with a longer wait.
    assert!(
        began.elapsed() < PATIENCE * 3 + Duration::from_secs(5),
        "{:?}",
        began.elapsed()
    );
    drop(stop);
    // Where the program could say which child it started, that child is seen to go.
    if let Some(left) = stand_in::pid_named_in(&said, Duration::from_secs(2)) {
        assert!(
            stand_in::is_gone(left, Duration::from_secs(10)),
            "the test left its own fixture running"
        );
    }
}

#[test]
fn the_answer_is_asked_in_the_chats_folder_with_its_own_environment() {
    let fx = Fixture::new();
    let teller = stand_in::program(
        fx.outside.path(),
        "claude",
        "#!/bin/sh\npwd\necho \"${CHARTER_PROBE_SEEN-unset} ${APP_ONLY-unset}\"\necho '2.1.288 (Claude Code)'\n",
    );
    let cwd = fx.cwd();
    let env = vec![("CHARTER_PROBE_SEEN".to_owned(), "chat".to_owned())];
    // SAFETY of the test: a variable only this test sets, read only by the child.
    let said = version_of(
        &[teller.display().to_string()],
        Chat {
            cwd: &cwd,
            writable: &[],
            env: &env,
        },
    )
    .expect("an answer");
    let mut lines = said.lines();
    assert_eq!(
        lines.next().map(PathBuf::from),
        Some(cwd.canonicalize().expect("real"))
    );
    assert_eq!(lines.next(), Some("chat unset"));
}

/// D-88k: the probe is the first time charter runs the program, before any sandbox of the
/// chat's, so it runs inside charter's own wrap: it writes nothing but its own temp
/// directory and reaches no network, whatever the command loads.
#[cfg(target_os = "macos")]
#[test]
fn the_answer_is_asked_inside_charters_wrap_which_writes_nothing_and_reaches_nothing() {
    let fx = Fixture::new();
    let mark = fx.outside.path().join("written");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a listener");
    let port = listener.local_addr().expect("an address").port();
    let home_dir = stand_in::NoChatWrites::new();
    let home_mark = home_dir.path().join("written");
    let program = stand_in::program(
        fx.outside.path(),
        "claude",
        &format!(
            "#!/bin/sh\necho x > '{}' && echo wrote\necho x > '{}' && echo wrote-home\n\
             /usr/bin/nc -z -w 1 127.0.0.1 {port} && echo reached\n\
             echo x > \"$TMPDIR/own\" && echo own-temp\necho '2.1.288 (Claude Code)'\n",
            mark.display(),
            home_mark.display()
        ),
    );
    let cwd = fx.cwd();
    let said = version_of(
        &[program.display().to_string()],
        Chat {
            cwd: &cwd,
            writable: &[],
            env: &[],
        },
    );
    assert_eq!(said.as_deref(), Ok("own-temp\n2.1.288 (Claude Code)"));
    assert!(!mark.exists(), "the probe wrote outside its temp directory");
    assert!(
        !home_mark.exists(),
        "the probe wrote outside its temp directory"
    );
}

#[test]
fn an_answer_is_read_only_up_to_its_cap() {
    let fx = Fixture::new();
    let noisy = stand_in::program(
        fx.outside.path(),
        "claude",
        "#!/bin/sh\nyes 'not claude' | head -c 1000000\necho '2.1.288 (Claude Code)'\n",
    );
    assert!(fx.check(Harness::ClaudeCode, &noisy, &[], None).is_err());
}

#[test]
fn only_a_line_claude_code_answers_with_counts() {
    for (said, is) in [
        ("2.1.288 (Claude Code)", true),
        ("warning\n2.1.288 (Claude Code)\n", true),
        ("2.1.288", false),
        ("2.1 (Claude Code)", false),
        ("x2.1.288 (Claude Code)", false),
        ("codex-cli 0.147.0", false),
        ("", false),
    ] {
        assert_eq!(is_claude_code(said), is, "{said:?}");
    }
}

#[test]
fn the_temp_directories_are_counted_as_writable() {
    let roots = temp_roots(&[("TMPDIR".to_owned(), "/chat/tmp".to_owned())]);
    for want in ["/tmp", "/private/tmp", "/chat/tmp"] {
        assert!(roots.contains(&PathBuf::from(want)), "{want}");
    }
    assert!(roots.contains(&std::env::temp_dir()));
}
