// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `charter hook pretooluse` — the Bash guard, answered rather than blocked.
//!
//! **This file is the switch M3.1 was built in front of.** Until it existed, `main.rs`'s
//! `is_a_tool_hook` answered every word in the `pretooluse`/`posttooluse` namespace with exit 2
//! — *block* — because refusing a tool call is the only safe thing a program that has checked
//! nothing can do. The eight arms are ported and assembled ([`purlis_core::toolgate`]), and so
//! now is the rest of the tool-hook namespace (`hooks.rs`); `is_a_tool_hook` is left guarding
//! only words charter has not invented yet.
//!
//! # What this does
//!
//! `charter/hooks.py:pretooluse` is its bookkeeping, eight refusals, and one ALLOW, in that
//! order, and so is this:
//!
//! 1. **Bookkeeping** ([`purlis_core::toolhooks::pretooluse_bookkeeping`]), in a plane only:
//!    the heartbeat of the piece the command runs in, and the record-memory cadence reset a
//!    `charter … remember` earns. The Python's guard sighting, turn marker and trace row are
//!    not kept — nothing in charter-app reads them.
//! 2. **The refusals** ([`purlis_core::toolgate::verdict`]): one JSON object on stdout, or
//!    nothing.
//! 3. **The persona tool gate** ([`purlis_core::personagate`]), in a plane only and only when
//!    nothing refused: `allow`, so the harness does not prompt, when the active persona's
//!    `tools:` declares the program. It never denies; the worst it can do is leave the prompt.
//!
//! # Why a denial is exit 0
//!
//! The verdict IS the JSON. A `PreToolUse` hook refuses by printing
//! `permissionDecision: "deny"` and exiting cleanly; exit 2 is the *fallback* for when that
//! print could not be delivered, and every other non-zero status is a non-blocking error the
//! harness logs while the tool call proceeds. So the one direction this must not fail in is a
//! decided denial that never reached stdout, which is charter#438 and is why the write is
//! checked, the stream is flushed inside the check, and a failure there exits 2 with the reason
//! on stderr. A write is not a delivery: `print` to a pipe buffers, and the first version of
//! that fix upstream returned 0 on a real broken pipe.
//!
//! # Where the plane comes from
//!
//! The PROCESS's directory, not the payload's. That is what the Python does — `charter.config`
//! resolves once at import, from `$CHARTER_ROOT` and the interpreter's own cwd — and the two
//! are genuinely different questions: the plane is where charter's files are, and the payload's
//! `cwd` is where the command being judged would run. Both are passed, each to the arms that
//! ask for it.

use std::io::Write;
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use purlis_core::enforcement::{Family, Mode};
use purlis_core::handoffguard::Caller;
use purlis_core::toolgate::{self, Call, Launched, Plane};
use purlis_core::toolhooks::Answer;

/// What a harness reads as "block" — and, here, only as the fallback for an undelivered
/// denial. `hooks.py:DENY_EXIT`.
const DENY_EXIT: u8 = 2;

/// The plane's facts, owned, because [`Plane`] borrows them.
struct Found {
    root: String,
    forges: Vec<purlis_core::forge::Forge>,
}

/// `charter hook pretooluse` — refuse this tool call, or get out of the way.
///
/// `payload` is the harness's JSON on stdin. A payload that will not parse is `{}`, exactly as
/// `_read_stdin` makes it: every arm is then asked about the empty command and none fires. That
/// is deliberate and is not a fail-open — a guard with no command to judge has nothing to
/// refuse, and refusing anyway would block every tool call on a harness whose payload charter
/// does not understand.
pub fn pretooluse(payload: &str, now: Option<&str>) -> crate::hooks::Answered {
    // Bookkeeping first, as the Python does: a refusal below still means the session was here.
    crate::hooks::with_hook(payload, now, purlis_core::toolhooks::pretooluse_bookkeeping);
    let data: serde_json::Value = serde_json::from_str(payload).unwrap_or(serde_json::Value::Null);
    fn text(at: &serde_json::Value) -> String {
        at.as_str().unwrap_or_default().to_string()
    }

    // Python's `ti.get("command", "") or ""` and `data.get("cwd") or ""`: a null, a number or an
    // absent field are all the empty string, and the arms are still asked.
    let command = text(&data["tool_input"]["command"]);
    let cwd = text(&data["cwd"]);
    let agent_id = data["agent_id"].as_str().map(str::to_owned);
    let permission_mode = data["permission_mode"].as_str().map(str::to_owned);
    // Which harness this session runs under: A7 reads it to decide whether an `agent_id` on the
    // payload means a sub-agent.
    let harness = purlis_core::hookwire::chat_harness(&purlis_core::envvar::var);

    // `config.ROOT`: `$CHARTER_ROOT`, then the marker walk up from the process's directory,
    // then — when nothing is found — the directory itself, which is what Python falls back to
    // and what makes `STATE_DIR` exist outside a plane.
    let here = std::env::current_dir().unwrap_or_default();
    let root = purlis_core::plane::resolve(&here).unwrap_or(here);
    // `config.STATE_DIR`, which exists with or without a plane: the leak guard is ungated and
    // still needs a `.charter/` to refuse a walk into. `$CHARTER_HOME` overrides it, here as
    // everywhere else.
    let state_dir = purlis_core::plane::state_dir(&root);
    // **`_in_a_plane()` is the MARKER, not a successful resolve** — `config.HAS_CONTROL_PLANE`
    // is `(ROOT / "charter.toml").is_file()` and nothing else. The difference is real and the
    // differential found it: `$CHARTER_ROOT` is taken on trust by `plane::resolve`, so a
    // session pinned at a directory that is not a plane resolved to it happily, and the five
    // gated arms then denied in a directory with no control plane — which is charter#852
    // exactly, reintroduced one level down.
    let found = purlis_core::names::has_manifest(&root).then(|| Found {
        root: root.display().to_string(),
        forges: purlis_core::forge::known_ordered(&root),
    });
    // Where Claude Code loaded its settings: the session's start folder, which a `cd` in the
    // shell does not move (RN-7).
    let session_dir = std::env::var("CLAUDE_PROJECT_DIR").unwrap_or_default();
    // What the app told this chat at its start: whether it was given a sandbox, and the folder
    // it was started in (#1345). A harness the app did not start has neither.
    let chat_dir = std::env::var(purlis_core::sandboxblock::CHAT_DIR_ENV).ok();
    // Whether a person is at the chat, by the fact the app holds for it (R-109): asked only of
    // a line A4 would refuse unattended, and only in a chat the app started, which has a fact.
    let asked_attendance = || attendance_of_this_chat(permission_mode.as_deref());
    let launched = Launched {
        sandboxed: purlis_core::sandbox::chat_is_sandboxed(),
        chat_dir: chat_dir.as_deref(),
        attendance: crate::session::chat_number().map(|_| toolgate::Attending(&asked_attendance)),
    };
    let plane = found.as_ref().map(|found| Plane {
        root: &found.root,
        forges: &found.forges,
        session_dir: &session_dir,
        launched,
    });
    let call = Call {
        command: &command,
        cwd: &cwd,
        state_dir: &state_dir,
        caller: Caller {
            agent_id: agent_id.as_deref(),
            harness: harness.as_deref(),
            permission_mode: permission_mode.as_deref(),
        },
    };
    // How each family of refusals runs for this chat (Q308, Q308a): as its start resolved the
    // project's `[policy]`, made stricter by what the project says now.
    let policy = purlis_core::enforcement::in_force(&root);
    let chat = purlis_core::enforcement::this_chat();
    let judged = toolgate::judged(&call, plane.as_ref(), &|family| policy.mode(family));
    for logged in &judged.logged {
        if let Some(family) = logged.arm.and_then(toolgate::Arm::family) {
            log_verdict(&root, chat.as_deref(), family, logged);
        }
    }
    if let Some(verdict) = judged.refused {
        return crate::hooks::Answered::of(Answer::Deny(verdict));
    }
    // The merge hold (R-114, R-115), judged by the project policy (M75 slice 04): in a chat the
    // app started, the session host decides a merge, and an ask hands its request to the app,
    // which asks the person. After the refusals, and ahead of A4, which leaves a merge the hold
    // stands for to it, attended or not (R-145).
    match judged_merge(&command, &cwd) {
        Some(MergeJudged::Refused(verdict)) => {
            return crate::hooks::Answered::of(Answer::Deny(verdict));
        }
        Some(MergeJudged::Logged(verdict)) => {
            log_verdict(&root, chat.as_deref(), Family::GuardsMerge, &verdict)
        }
        None => {}
    }
    // A11: plain git that writes a branch folder another live chat holds, from a chat that runs
    // git itself (#3931). After every refusal, because it is the one that asks the app.
    // `guards.git` (Q308).
    let git = policy.mode(Family::GuardsGit);
    if git != Mode::Off
        && let Some(found) = &found
        && !launched.sandboxed
        && let Some(verdict) = held_branch_folder(Path::new(&found.root), &command, &cwd)
    {
        if git == Mode::Enforce {
            return crate::hooks::Answered::of(Answer::Deny(verdict));
        }
        log_verdict(&root, chat.as_deref(), Family::GuardsGit, &verdict);
    }
    // Nothing refused: a dispatch or a handoff that writes and shares its call with another
    // command was refused above (#3785), and nothing here asks the person.
    // Then the persona tool gate is asked. Its answer is an allow or nothing;
    // one it could not print is simply the ordinary prompt. It reads the command too, so a
    // reading of it nested too deep to read whole is the refusal the guards give (#1355).
    let (allowed, too_deep) = purlis_core::shellseg::too_deep_within(|| {
        crate::hooks::with_hook(payload, now, purlis_core::toolhooks::persona_allow)
    });
    if too_deep {
        return crate::hooks::Answered::of(Answer::Deny(toolgate::too_deep()));
    }
    crate::hooks::Answered::of(allowed.map_or(Answer::Nothing, Answer::Say))
}

/// **One quiet line for a refusal a family under `log` would have made** (Q308): by its arm's id,
/// or, where it has no arm, the id of the hold that made it, and never its sentence, which may
/// quote the command.
fn log_verdict(root: &Path, chat: Option<&str>, family: Family, verdict: &toolgate::Verdict) {
    use purlis_core::enforcement::{RULE_BRANCH_FOLDER, RULE_MERGE_HOLD};
    let rule: &str = match (verdict.arm, family) {
        (Some(arm), _) => arm.id(),
        (None, Family::GuardsMerge) => RULE_MERGE_HOLD,
        (None, _) => RULE_BRANCH_FOLDER,
    };
    let chat = chat.unwrap_or(purlis_core::enforcement::UNKNOWN_CHAT);
    purlis_core::enforcement::record(root, chat, family, rule, None);
}

/// How long the guard waits for the app to say whether this chat is attended (R-109): the app
/// answers from what it holds, so this is the time a slow app may take, inside the hook's
/// [`BUDGET`].
const ATTENDANCE_IS_ANSWERED_WITHIN: Duration = Duration::from_millis(500);

/// **This chat's attendance fact, as the app holds it** (R-109, T1-18,
/// [`purlis_core::attendance::Fact`]), as it would stand once the app heard what this line's
/// payload says (`permission_mode`). **Unattended unless the app says attended**: no app, an
/// app older than the ask, a refusal and a wait past [`ATTENDANCE_IS_ANSWERED_WITHIN`] all read
/// as unattended, so A4 never takes a chat for attended where its fact may say otherwise.
pub(crate) fn attendance_of_this_chat(
    permission_mode: Option<&str>,
) -> purlis_core::dispatchunattended::Attendance {
    use purlis_core::dispatchunattended::Attendance;
    use purlis_core::hookwire::{Answer, Ask, AttendanceAsk};
    let answer = crate::brokered::asked(
        |chat| {
            Ask::Attendance(AttendanceAsk::read(
                chat,
                permission_mode,
                &purlis_core::envvar::var,
            ))
        },
        ATTENDANCE_IS_ANSWERED_WITHIN,
    );
    match answer {
        Ok(Ok(Answer::Attended { attended: true })) => Attendance::Attended,
        _ => Attendance::Unattended,
    }
}

/// How long the guard waits for the session host to decide a merge: it loads the plane's
/// committed policy and decides, which is a few git reads. With [`A_MERGE_IS_TAKEN_WITHIN`] after
/// it, inside the hook's [`BUDGET`] (`a_merge_decided_and_then_taken_fits_the_budget`).
const A_MERGE_IS_DECIDED_WITHIN: Duration = Duration::from_millis(700);

/// What the policy left of a merge.
enum MergeJudged {
    /// Refused to the chat: held for the person, denied, or not decided.
    Refused(toolgate::Verdict),
    /// Let through under a mode that logs what it would have refused.
    Logged(toolgate::Verdict),
}

/// **A merge, judged by the project policy** (M75 slice 04, spec §6): the session host decides
/// it ([`purlis_core::hookwire::Ask::Decide`]) and the outcome after the mode is enforced: an
/// ask holds it for the person ([`held_merge`]), a deny refuses it naming the rules, an allow
/// lets it run, and one the mode let through is logged. `None` for a command that merges
/// nothing, and one the policy lets run.
///
/// **No host is a refusal**: a chat inside a plane whose host cannot be asked, or does not
/// answer, is refused, and never decides here (§6). Asked only of a chat the app started, as
/// before: a process the app did not start keeps today's guards until slice 07b says which
/// processes are outside any plane (Q327; the shipped `strict` such a process gets is
/// [`purlis_core::policy::decide::decide_locally`]).
fn judged_merge(command: &str, cwd: &str) -> Option<MergeJudged> {
    use purlis_core::hookwire::{Answer, Ask, DecideAsk, UNKNOWN_LINE};
    use purlis_core::mergehold;
    use purlis_core::policy::decide::Outcome;
    let held = mergehold::held(command)?;
    let (action, kind, id) = mergehold::policy_request(&held);
    let asked = crate::brokered::asked(
        |chat| {
            Ask::Decide(DecideAsk {
                chat,
                action: action.to_owned(),
                kind: kind.to_owned(),
                id,
                tool: "Bash".to_owned(),
                cwd: Some(cwd.to_owned()),
            })
        },
        A_MERGE_IS_DECIDED_WITHIN,
    );
    let refused = |reason: &str, denial: String| {
        MergeJudged::Refused(toolgate::Verdict {
            arm: None,
            reason: reason.to_owned(),
            shape: None,
            denial,
        })
    };
    let decision = match asked {
        Ok(Ok(Answer::Decided(decision))) => decision,
        // An app of another build could not read the ask: refused in the words that say so and
        // name what to do (#6960), never decided here.
        Ok(Ok(Answer::No { why })) if why == UNKNOWN_LINE => {
            // The hook is the app's own binary, so it is the newer one: a bundle replaced under
            // the running app (#7177). Another hook is a plugin of another build.
            let app = if is_the_apps_own_binary() {
                mergehold::AppSaid::OlderApp
            } else {
                mergehold::AppSaid::OtherBuild
            };
            return Some(refused(
                mergehold::reason(&held, &app),
                mergehold::refusal(&held, &app),
            ));
        }
        Ok(Ok(Answer::No { why })) => {
            return Some(refused(
                mergehold::REASON_UNDECIDED,
                mergehold::undecided(&why),
            ));
        }
        Ok(Ok(_)) => {
            return Some(refused(
                mergehold::REASON_UNDECIDED,
                mergehold::undecided("the app answered something else"),
            ));
        }
        Ok(Err(e)) => {
            return Some(refused(
                mergehold::REASON_UNDECIDED,
                mergehold::undecided(&format!("no answer: {e}")),
            ));
        }
        // Whether this is a chat's is the app's to say, by its ancestry, as well as its
        // environment's (#6995): a process that is no chat's keeps today's guards (Q327).
        Err(crate::brokered::NotTaken::NotAChat) => return None,
        Err(_) => {
            return Some(refused(
                mergehold::REASON_UNDECIDED,
                mergehold::undecided("no session host answered"),
            ));
        }
    };
    match decision.after_mode {
        Outcome::Allow if decision.would_have_blocked() => {
            Some(MergeJudged::Logged(toolgate::Verdict {
                arm: None,
                reason: mergehold::REASON.to_owned(),
                shape: None,
                denial: mergehold::held_by(&decision),
            }))
        }
        Outcome::Allow => None,
        Outcome::Deny => Some(refused(
            mergehold::REASON_DENIED,
            mergehold::denied(&decision),
        )),
        Outcome::Ask => Some(MergeJudged::Refused(held_merge(&held, cwd, &decision))),
    }
}

/// How long the guard waits for the app to take a held merge: the app reads the request from
/// the forge before it answers, within [`purlis_core::mergehold::THE_FORGE_IS_READ_WITHIN`], so
/// the chat is told the person is asked only where an ask waits (Q239). Inside the hook's
/// [`BUDGET`].
const A_MERGE_IS_TAKEN_WITHIN: Duration = Duration::from_millis(1_000);

/// **The merge hold** (R-114, R-115, [`purlis_core::mergehold`]): the refusal of `held`, run in
/// `cwd` by a chat the app started, which the project policy's `decision` holds for the person.
/// The refusal ends naming the rule that holds it, with its level and owner.
///
/// **No answer lets the merge through**: where no app takes the hand-over, the chat is refused
/// all the same. **It is a hold only where the app confirms it holds the ask**
/// ([`Answer::MergeHeld`], Q239); any other answer, or none, is refused with a block's reason,
/// which the event log records.
fn held_merge(
    held: &purlis_core::mergehold::Held,
    cwd: &str,
    decision: &purlis_core::policy::decide::Decision,
) -> toolgate::Verdict {
    use purlis_core::hookwire::{Answer, Ask, MergeAsk, NOTHING_ANSWERS, UNKNOWN_LINE};
    use purlis_core::mergehold::{self, AppSaid, Held};
    let answer = match held {
        Held::Request(request) => crate::brokered::asked(
            |chat| {
                Ask::Merge(MergeAsk {
                    chat,
                    request: request.clone(),
                    cwd: cwd.to_owned(),
                })
            },
            A_MERGE_IS_TAKEN_WITHIN,
        )
        .ok()
        .and_then(Result::ok),
        Held::Unreadable | Held::Land => None,
    };
    let app = match &answer {
        Some(Answer::MergeHeld { said }) => AppSaid::Held(said),
        // An app of another build could not read the hand-over: no refusal of its own (#6960).
        // The hook is the app's own binary, so it is the newer one: a bundle replaced under the
        // running app (#7177). Another hook is a plugin of another build.
        Some(Answer::No { why }) if why == UNKNOWN_LINE && is_the_apps_own_binary() => {
            AppSaid::OlderApp
        }
        Some(Answer::No { why }) if why == UNKNOWN_LINE => AppSaid::OtherBuild,
        Some(Answer::No { why }) if why != NOTHING_ANSWERS => AppSaid::Refused(why),
        _ => AppSaid::Unheard,
    };
    toolgate::Verdict {
        // No arm of the gate: the hold is its own rule, after the gate's.
        arm: None,
        reason: mergehold::reason(held, &app).to_owned(),
        shape: None,
        denial: format!(
            "{} {}",
            mergehold::refusal(held, &app),
            mergehold::held_by(decision)
        ),
    }
}

/// Whether this hook is the program the app hands its chats as their hook binary
/// ([`purlis_core::plugin::BINARY_ENV`]), by its real path.
fn is_the_apps_own_binary() -> bool {
    let (Some(named), Ok(own)) = (
        purlis_core::envvar::var_os(purlis_core::plugin::BINARY_ENV),
        std::env::current_exe(),
    ) else {
        return false;
    };
    match (std::fs::canonicalize(named), std::fs::canonicalize(own)) {
        (Ok(named), Ok(own)) => named == own,
        _ => false,
    }
}

/// How long the guard waits, in all, for the app to judge a line's branch folder claims
/// (#3931): each is a check and one appended line, and the whole wait stays well inside the
/// hook's [`BUDGET`], whatever the arms before it took.
const CLAIMS_ARE_ANSWERED_WITHIN: Duration = Duration::from_millis(900);

/// **The one-writer guard on a chat's plain git** (#3931): the refusal of `command`, run in
/// `cwd` in the project at `root` by a chat that runs git itself, where any git on the line
/// commits, updates or pushes in a branch folder another live chat holds
/// ([`purlis_core::piececlaim`]), in the brokered refusal's words; or `None`.
///
/// Asked only of a chat the app started, as the app names it by the asker's ancestry or the
/// environment says (#6995): the person's own terminal runs no hook, and a harness they start
/// themselves is theirs. The app judges each write's claim against the chats it has, and the
/// chat takes a folder no live chat holds.
///
/// **The app's silence lets the call through** (the review of #4129). This guards against
/// accidents and is not a wall: where the app is older than the ask, or does not answer within
/// [`CLAIMS_ARE_ANSWERED_WITHIN`], the git runs. A stale claim of a chat that has ended can then
/// never stop another, and a slow app can never cost the hook its budget. **No app at all, or
/// something that is not the app, refuses it** (#6995): a chat's process cannot unset or
/// replace the app to have its git judged by no one.
fn held_branch_folder(root: &Path, command: &str, cwd: &str) -> Option<toolgate::Verdict> {
    use purlis_core::hookwire::{ClaimAct, GitWork};
    let ends = Instant::now() + CLAIMS_ARE_ANSWERED_WITHIN;
    let mut asked = Vec::new();
    let writes = purlis_core::branchgit::branch_write(command, cwd, &root.display().to_string());
    // Whether this is a chat's is the app's to say, by its ancestry, as well as its
    // environment's (#6995), asked only of a line that writes a branch at all.
    if (writes.unplaced.is_none() && writes.found.is_empty()) || !crate::brokered::in_a_chat() {
        return None;
    }
    // A git that may write a branch where the line cannot be followed: with no sandbox behind
    // it, it is refused with the shape that places it, where the project has a branch folder
    // it could be in. Before any wait on the app, so no wait lets it through.
    if let Some(sub) = writes.unplaced
        && purlis_core::branchgit::any_branch_folder(root)
    {
        return Some(toolgate::Verdict {
            arm: None,
            reason: purlis_core::piececlaim::REASON.to_owned(),
            denial: purlis_core::branchgit::unplaced_said(&sub),
            shape: Some(sub),
        });
    }
    for write in writes.found {
        let Some(at) = purlis_core::worktree::locate(root, &write.folder) else {
            continue;
        };
        let Some(act) = ClaimAct::of(write.what) else {
            continue;
        };
        // One ask per folder and act: two commits in one folder on one line ask once.
        if asked.contains(&(at.clone(), act)) {
            continue;
        }
        asked.push((at.clone(), act));
        let left = ends.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return None;
        }
        let work = GitWork::WorktreeClaim {
            repo: at.repo.clone(),
            piece: at.piece.clone(),
            act,
        };
        // Refused where the app says another live chat holds the folder, and where no app, or
        // something that is not the app, answers for this chat (#6995); any other answer, a
        // `No` of the app's included, lets the git run, as the app's silence does.
        let forwarded = crate::gitask::forwarded(&at.workspace, work, left);
        if let crate::gitask::Forwarded::Refused(why) = &forwarded
            && why.starts_with(purlis_core::hookwire::reach::NO_APP_ANSWERED)
        {
            return Some(toolgate::Verdict {
                arm: None,
                reason: purlis_core::piececlaim::REASON.to_owned(),
                shape: Some(write.sub),
                denial: why.clone(),
            });
        }
        if let crate::gitask::Forwarded::Said { code, lines } = forwarded
            && code == purlis_core::hookwire::CLAIM_HELD
        {
            use purlis_core::repocmd::Say;
            let why: Vec<String> = lines
                .iter()
                .map(|line| match line {
                    Say::Info(s) | Say::Done(s) | Say::Warn(s) | Say::Fail(s) | Say::Plain(s) => {
                        s.to_string()
                    }
                    Say::Out(s) => s.clone(),
                })
                .collect();
            return Some(toolgate::Verdict {
                arm: None,
                reason: purlis_core::piececlaim::REASON.to_owned(),
                shape: Some(write.sub),
                denial: why.join(" "),
            });
        }
    }
    None
}

/// The stack the guards run on: far more than any of them needs.
///
/// A stack overflow is not a panic: it aborts the process before any hook can answer, and a
/// guard that dies without answering is an allow. Each guard bounds how deep it reads (the
/// floor's `MAX_DEPTH`), and this is the backstop beneath them all. Only the pages a guard
/// touches are ever committed, so the reservation costs nothing a person could notice.
const GUARD_STACK: usize = 256 * 1024 * 1024;

/// The time every `PreToolUse` hook decides within, from the moment it starts: reading the
/// payload, then every guard and gate it runs (#1355).
///
/// A harness gives a hook a timeout of its own, and whether the tool call runs when it passes is
/// the harness's choice: Codex and Claude Code run it (see `docs/hooks.md` → When a hook runs
/// out of time). So purlis never lets a harness time a guard out. It decides inside this budget,
/// and a decision not made in time is a refusal. Two seconds, well inside the smallest
/// `PreToolUse` timeout purlis registers (`the_budget_is_well_inside_every_harness_timeout`),
/// and far more than an ordinary command takes. The caps in front of the guards
/// ([`purlis_core::guardcaps`]) bound how slow a command inside them can be, and purlis-core's
/// `the_whole_verdict_at_the_largest_and_deepest_command_stays_well_inside_the_budget` holds
/// the slowest shapes known under this; the budget is the backstop for one nobody has found.
///
/// It starts in `hook`, not when the process does: the time to start the process is outside it
/// (`docs/hooks.md` → When a hook runs out of time).
pub(crate) const BUDGET: Duration = Duration::from_secs(2);

/// What sets the budget in milliseconds instead, in a debug build, for the tests that reach it.
#[cfg(debug_assertions)]
const BUDGET_ON_PURPOSE_ENV: &str = "PURLIS_TEST_GUARD_DEADLINE_MS";

/// Set in a debug build, the judging thread stalls before it judges: the tests' way to reach
/// the budget on every hook word without a command crafted for each.
#[cfg(debug_assertions)]
const STALLS_ON_PURPOSE_ENV: &str = "PURLIS_TEST_GUARD_STALLS";

/// The panic payload that says the hook ran out of its budget, rather than that it crashed.
const UNANSWERED: &str = "the guard did not answer in time";

/// One hook's budget, started when the hook is.
pub(crate) struct Budget {
    ends: Instant,
}

impl Budget {
    /// The budget, from now.
    pub(crate) fn start() -> Self {
        Self {
            ends: Instant::now() + budget(),
        }
    }

    /// What is left of it.
    pub(crate) fn left(&self) -> Duration {
        self.ends.saturating_duration_since(Instant::now())
    }
}

fn budget() -> Duration {
    #[cfg(debug_assertions)]
    if let Some(ms) = std::env::var(BUDGET_ON_PURPOSE_ENV)
        .ok()
        .and_then(|ms| ms.parse().ok())
    {
        return Duration::from_millis(ms);
    }
    BUDGET
}

/// The hook ran out of its budget: refused, by the panic hook ([`refuse_on_a_crash`]), which
/// ends the process with the sentence that says so.
pub(crate) fn out_of_time() -> ! {
    std::panic::panic_any(UNANSWERED)
}

/// Runs `judge` on a thread with [`GUARD_STACK`] of stack, and waits for its answer for what is
/// left of `budget`. A panic on that thread, a thread that cannot start, and an answer that does
/// not come in time are all the panic hook's refusal ([`refuse_on_a_crash`]), which ends the
/// process.
pub(crate) fn decided_within<T: Send>(budget: &Budget, judge: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|scope| {
        let (answer, answered) = std::sync::mpsc::channel();
        let started = std::thread::Builder::new()
            .name("guard".into())
            .stack_size(GUARD_STACK)
            .spawn_scoped(scope, move || {
                #[cfg(debug_assertions)]
                if std::env::var_os(STALLS_ON_PURPOSE_ENV).is_some() {
                    std::thread::sleep(Duration::from_secs(3600));
                }
                let _ = answer.send(judge());
            });
        if let Err(why) = started {
            panic!("the guard's thread did not start: {why}");
        }
        match answered.recv_timeout(budget.left()) {
            Ok(verdict) => verdict,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => out_of_time(),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                panic!("the guard's thread ended without answering")
            }
        }
    })
}

/// From here on, a panic anywhere in this process is a refusal: exit 2, one line on stderr.
///
/// **A crashed guard is otherwise an allow** (#349). A panic exits 101, or in a shipped build
/// (`purlis_core::panicend`) ends in SIGABRT, and a harness reads every non-zero status but
/// 2 as a non-blocking error and runs the tool. `catch_unwind` cannot help where there is no
/// unwinding, but a panic hook runs before the abort does, so the hook answers and exits
/// itself — the abort is never reached. It covers every thread, which is right: under abort a
/// panic on any thread ends the process, and it must end it as a refusal.
///
/// Exit 2 is what Claude Code and Codex both read as "block", handing stderr to the model; they
/// ignore stdout on it, so no JSON is printed. The panic's own message is not said: it can
/// carry what the guard was judging, and the model is owed only why the call did not run.
///
/// Only for a `PreToolUse` word. A crash on a reporting hook must never exit 2, which on `Stop`
/// would keep a session from ending.
pub(crate) fn refuse_on_a_crash(tell_the_host: fn(&str, bool)) {
    // The word this process answers, for the host's event log: the call is refused, and one
    // event says so (FD-9).
    // `args_os`, read before anything is installed: `args` panics on a word that is not
    // UTF-8, and nothing on the way to this hook may panic.
    let word = std::env::args_os()
        .nth(2)
        .map(|word| word.to_string_lossy().into_owned())
        .unwrap_or_default();
    std::panic::set_hook(Box::new(move |info| {
        let at = info
            .location()
            .map(|at| format!(" (at {}:{})", at.file(), at.line()))
            .unwrap_or_default();
        let mut err = std::io::stderr().lock();
        let unanswered = info.payload().downcast_ref::<&str>() == Some(&UNANSWERED);
        if unanswered {
            // The time limit's own words (#6962): a short line on a busy machine runs out of
            // time too, so this never blames the line's length or nesting, whose caps say so
            // themselves (`guardcaps::refusal`).
            let _ = writeln!(
                err,
                "purlis guard: this tool call is refused because purlis could not check it in \
                 time: the check ran out of its time limit, which a busy machine can cause, and \
                 nothing ran. Try it again; if it runs out of time again, split it into smaller \
                 commands."
            );
        } else {
            let _ = writeln!(
                err,
                "purlis guard: this tool call is refused because the guard crashed{at} before \
                 it could answer, and a guard that could not answer does not allow."
            );
        }
        let _ = err.flush();
        drop(err);
        tell_the_host(&word, unanswered);
        std::process::exit(i32::from(DENY_EXIT));
    }));
}

/// Print the verdict, or fall back to the one status a harness reads as "refused".
///
/// `hooks.py:_deny`. The flush is inside the check on purpose: a `print` to a pipe lands in a
/// userspace buffer and returns cleanly, so without it a broken stdout is discovered at exit,
/// too late for any exit status to mean anything.
pub(crate) fn deny(verdict: &toolgate::Verdict) -> ExitCode {
    let line = verdict.emitted();
    let mut out = std::io::stdout().lock();
    if writeln!(out, "{line}").is_ok() && out.flush().is_ok() {
        return ExitCode::SUCCESS;
    }
    // stderr is what exit 2 hands the model, so the reason still arrives. Best effort: the exit
    // status is the guard, and a second failed stream cannot be allowed to change it.
    let _ = writeln!(std::io::stderr(), "{}", verdict.said());
    let _ = std::io::stderr().flush();
    ExitCode::from(DENY_EXIT)
}

#[cfg(test)]
mod budget_tests {
    use super::*;

    /// The budget, and the bounded wait to tell the app after it, end well before any harness
    /// would time a `PreToolUse` hook out: the hook always decides, and is never decided for.
    #[test]
    fn the_budget_is_well_inside_every_harness_timeout() {
        for hook in purlis_core::hookreg::HANDLERS
            .iter()
            .filter(|hook| hook.event == "PreToolUse")
        {
            let harness = Duration::from_secs(u64::from(hook.timeout));
            assert!(
                BUDGET * 2 <= harness,
                "{}: {BUDGET:?} of {harness:?}",
                hook.name
            );
            // And with the app frozen: the tool call's line, then the two lines nothing
            // records (the touched file, what the chat is doing), each with its own bound.
            #[cfg(unix)]
            assert!(
                BUDGET + purlis_core::hookwire::A_TOOL_HOOK_TELLS_WITHIN <= harness,
                "{}: {BUDGET:?} and {:?} of {harness:?}",
                hook.name,
                purlis_core::hookwire::A_TOOL_HOOK_TELLS_WITHIN
            );
        }
    }

    /// M75-04 round 2: a merge asks the app twice, to decide it and then to take it, and both waits
    /// end inside the hook's [`BUDGET`] with room left for the arms before them. The second outlasts
    /// the app's read of the forge, or a hold it raised would be reported as unheard.
    #[test]
    fn a_merge_decided_and_then_taken_fits_the_budget() {
        let slack = Duration::from_millis(250);
        assert!(
            A_MERGE_IS_DECIDED_WITHIN + A_MERGE_IS_TAKEN_WITHIN + slack <= BUDGET,
            "{A_MERGE_IS_DECIDED_WITHIN:?} and {A_MERGE_IS_TAKEN_WITHIN:?} of {BUDGET:?}"
        );
        assert!(
            A_MERGE_IS_TAKEN_WITHIN > purlis_core::mergehold::THE_FORGE_IS_READ_WITHIN,
            "the app reads the forge for {:?}",
            purlis_core::mergehold::THE_FORGE_IS_READ_WITHIN
        );
    }
}
