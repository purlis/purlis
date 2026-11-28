// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The session host's decisions** (M75 slice 04, #7012; spec §6): what the app answers a
//! hook's [`Ask::Decide`](purlis_core::hookwire::Ask::Decide).
//!
//! The app alone loads, verifies and evaluates the policy. A hook names the action and its
//! resource; the chat is the one whose token the line carried, which the listener checked before
//! this is asked, and everything the policy reads of it is this app's record: its workspace and
//! persona, whether it runs sandboxed, whether the person is at it. The policy is the project's
//! as committed at the plane root's `HEAD` ([`purlis_core::policy::decide::in_plane`]), never a
//! file the line names.

use purlis_core::hookwire::{Answer, DecideAsk, PersonWhyAsk, WhyAsk};
use purlis_core::policy::decide;

use crate::planes::Held;

/// **Answers chat `asked.chat`'s ask for one decision**: refused for a chat this app does not
/// have open, since nothing of it is known to decide by. A deny is written to the event log here,
/// as the host decided it.
pub fn asked(held: &Held, asked: &DecideAsk) -> Answer {
    let chat = asked.chat;
    // Only what an enforcement point is wired for: a chat names nothing else for the host to
    // decide, record as a block or pause itself on (M75-04 round 2).
    if !decide::is_asked_by_an_enforcement_point(&asked.action, &asked.kind) {
        return Answer::No {
            why: format!(
                "nothing asks the host to decide {} on a {}.",
                purlis_core::shown::readable(&asked.action, 40),
                purlis_core::shown::readable(&asked.kind, 40)
            ),
        };
    }
    let id = with_its_repo(
        held,
        asked.chat,
        &asked.action,
        &asked.id,
        asked.cwd.as_deref(),
    );
    let Some(decision) = decision(held, chat, &asked.action, (&asked.kind, &id)) else {
        return Answer::No {
            why: format!("chat {chat} is not one this app has open, so nothing was decided."),
        };
    };
    if decision.after_mode == decide::Outcome::Deny {
        blocked(held, chat, &asked.tool, &decision);
    }
    Answer::Decided(Box::new(decision))
}

/// **The resource's id with its repo found**, as the merge the host then raises decides it: a
/// merge the command gave no repo (`#12`) is of the repo of the clone the hook ran in
/// ([`crate::mergeholds::found`]). The id as it came where it cannot be found: the merge is
/// refused later all the same, and nothing here decides on a guess.
fn with_its_repo(held: &Held, chat: u32, action: &str, id: &str, cwd: Option<&str>) -> String {
    use purlis_core::hookwire::MergeAsk;
    let number = id.strip_prefix('#').and_then(|n| n.parse::<u64>().ok());
    let (Some(number), Some(cwd), "merge") = (number, cwd, action) else {
        return id.to_owned();
    };
    let ask = MergeAsk {
        chat,
        request: purlis_core::mergehold::Request { repo: None, number },
        cwd: cwd.to_owned(),
    };
    match crate::mergeholds::found(held.root(), &ask) {
        Ok((_, repo)) => format!("{}#{number}", repo.as_str()),
        Err(_) => id.to_owned(),
    }
}

/// **What the app records of chat `chat`** that a decision about it is made from: none for a chat
/// this app does not have open.
pub(crate) fn facts_of(held: &Held, chat: u32) -> Option<decide::ChatFacts> {
    let chats = held.chats();
    chats.workspace_of(chat).map(|workspace| decide::ChatFacts {
        workspace,
        persona: chats
            .persona_of(chat)
            .and_then(|persona| persona.started_as),
        sandboxed: chats.runs_sandboxed(chat),
        level: chats.applied_sandbox_level(chat),
        attended: chats.attendance_of(chat).is_some_and(|fact| {
            fact.hearing(false, None) == purlis_core::dispatchunattended::Attendance::Attended
        }),
        // The mode the host started the harness in, from its own start record: never a report
        // of the chat's hook and never a word of its line (Q320e R6-1).
        harness_mode: chats
            .attendance_of(chat)
            .and_then(purlis_core::attendance::Fact::start_mode),
    })
}

/// **What the app's sealed store says now, for chat `chat`**: the deny state refuses every action
/// but `start`, and `context.machine` is the device id the person confirmed (M75-07b). `chat` is
/// the chat's ULID, which a Once, timed or This-chat grant is keyed on (M75-20, M75-21): without
/// it no grant of those scopes serves.
fn sealed_for(held: &Held, chat: u32) -> decide::Sealed {
    decide::Sealed {
        chat: held.chats().id_of(chat),
        ..decide::Sealed::of(held.root())
    }
}

/// **What the project's policy decides of chat `chat` doing `action` to `resource`**, by this
/// app's record of the chat: none for a chat this app does not have open.
///
/// **The timed grants this decision found ended, or lost with their rule, are taken out of the
/// store here**, each after its Journal line (M75-21): such a grant serves nothing either way, so
/// the sweep only keeps the store and the Journal true.
pub fn decision(
    held: &Held,
    chat: u32,
    action: &str,
    resource: (&str, &str),
) -> Option<decide::Decision> {
    let facts = facts_of(held, chat);
    // The accepted floor and the machine's locks, until slices 06 and 14 read them in the loader.
    let floor = purlis_core::enforcement::in_force(held.root());
    let sealed = sealed_for(held, chat);
    crate::timedgrants::sweep(held.root(), held.hooks(), &sealed.standing, sealed.at());
    let decided = decide::for_chat(
        held.root(),
        chat,
        facts.as_ref(),
        &floor,
        &sealed,
        action,
        resource,
    );
    crate::timedgrants::end_quietly(held.root(), held.hooks(), None, &sealed.take_retired());
    decided
}

/// **What the policy decides of chat `chat`'s ask before any answer is applied**, read without
/// spending a Once grant: what the Allowed list asks to tell whether a timed grant still serves.
pub fn unanswered(
    held: &Held,
    chat: u32,
    action: &str,
    resource: (&str, &str),
) -> Option<decide::Decision> {
    let facts = facts_of(held, chat);
    let floor = purlis_core::enforcement::in_force(held.root());
    let sealed = sealed_for(held, chat);
    let decided = decide::unanswered_for_chat(
        held.root(),
        chat,
        facts.as_ref(),
        &floor,
        &sealed,
        action,
        resource,
    );
    crate::timedgrants::end_quietly(held.root(), held.hooks(), None, &sealed.take_retired());
    decided
}

/// **What the person is offered at chat `chat`'s ask** ([`grants::offer`]): the scopes, the timed
/// grant's lengths, Never and Don't allow, or, at a deny, the one way out. None for a chat this app
/// does not have open.
pub fn offered(
    held: &Held,
    chat: u32,
    action: &str,
    resource: (&str, &str),
) -> Option<purlis_core::policy::grants::Offer> {
    decision(held, chat, action, resource)
        .map(|decision| purlis_core::policy::grants::offer(&decision))
}

/// **The timed grants that answer chat `chat`'s ask**, for the person's `reason`, made by the host
/// from what it decides now ([`decide::timed_for_chat`]). Nothing is sealed here: the caller holds
/// the person's press.
pub fn timed_grants(
    held: &Held,
    chat: u32,
    action: &str,
    resource: (&str, &str),
    given: (purlis_core::policy::grants::Span, &str),
) -> Option<Result<Vec<purlis_core::policy::grants::Grant>, purlis_core::policy::grants::Refused>> {
    let facts = facts_of(held, chat);
    let floor = purlis_core::enforcement::in_force(held.root());
    decide::timed_for_chat(
        held.root(),
        chat,
        facts.as_ref(),
        &floor,
        &sealed_for(held, chat),
        action,
        resource,
        given,
    )
}

/// **Answers chat `asked.chat`'s ask why an action is decided as it is** (M75 slice 18): the
/// decision the host would give that chat, named in full, by this app's record of it. Refused for
/// a chat this app does not have open. Nothing is sealed, spent, blocked or written: a chat that
/// asks why learns what it would be told, and moves nothing by asking.
pub fn why(held: &Held, asked: &WhyAsk) -> Answer {
    let chat = asked.chat;
    let Some(facts) = facts_of(held, chat) else {
        return Answer::No {
            why: format!("chat {chat} is not one this app has open, so nothing was decided."),
        };
    };
    // As the merge the host raises is decided: an id with no repo is of the clone the command ran
    // in (F6), so a `#12` is asked about the repo it would be decided for.
    let id = with_its_repo(held, chat, &asked.action, &asked.id, asked.cwd.as_deref());
    let floor = purlis_core::enforcement::in_force(held.root());
    Answer::Why(Box::new(purlis_core::policy::why::for_chat(
        held.root(),
        chat,
        &facts,
        &floor,
        &sealed_for(held, chat),
        (&asked.action, (&asked.kind, &id)),
    )))
}

/// **Answers the person's own `purlis policy why`** ([`PersonWhyAsk`]), asked in their terminal
/// and in no chat: the listener has refused a sender that may be inside one. The app is the one
/// that reads the sealed side, so the answer is made from the accepted copy, the accepted floor,
/// the person's grants and nevers, and the dry run diffs against them (spec §10.2).
///
/// `--chat N` is **the app's own record of chat N**, never a word of the line: a chat this app
/// does not have open is refused, and a flag that would also describe a chat is refused with it.
/// Without `--chat` the flags describe the context. Nothing is sealed, spent or written.
pub fn why_for_person(held: &Held, asked: &PersonWhyAsk) -> Answer {
    use purlis_core::policy::decide::{Asker, Sealed};
    use purlis_core::policy::levels::Chat;
    use purlis_core::policy::why;
    let root = held.root();
    let floor = purlis_core::enforcement::in_force(root);
    let no = |why: String| Answer::No { why };
    let said = |answered: Result<why::Why, String>| match answered {
        Ok(why) => Answer::Why(Box::new(why)),
        Err(why) => no(why),
    };
    if let Some(chat) = asked.chat {
        let context_flags = asked.workspace.is_some()
            || asked.persona.is_some()
            || asked.unattended
            || asked.sandbox.is_some()
            || asked.depth.is_some();
        if context_flags {
            return no(format!(
                "--chat {chat} asks as that chat, whose facts are this app's own record: leave \
                 --workspace, --persona, --unattended, --sandbox and --depth off"
            ));
        }
        let Some(facts) = facts_of(held, chat) else {
            return no(format!(
                "chat {chat} is not one this app has open, so its facts are not known and \
                 nothing was decided."
            ));
        };
        let id = with_its_repo(held, chat, &asked.action, &asked.id, asked.cwd.as_deref());
        let request = (asked.action.as_str(), (asked.kind.as_str(), id.as_str()));
        let sealed = sealed_for(held, chat);
        return match &asked.policy {
            None => Answer::Why(Box::new(why::for_chat(
                root, chat, &facts, &floor, &sealed, request,
            ))),
            Some(policy) => said(why::dry_run_for_chat(
                root, chat, &facts, &floor, &sealed, request, policy,
            )),
        };
    }
    let id = with_its_repo(held, 0, &asked.action, &asked.id, asked.cwd.as_deref());
    let request = (asked.action.as_str(), (asked.kind.as_str(), id.as_str()));
    let asker = Asker {
        chat: 0,
        of: Chat {
            workspace: asked.workspace.as_deref(),
            persona: asked.persona.as_deref(),
        },
        sandbox: asked.sandbox.as_deref().unwrap_or("floor"),
        attended: !asked.unattended,
        harness_mode: "default",
        depth: asked.depth.unwrap_or(0),
        via: "hook",
        start: None,
        git: None,
    };
    let sealed = Sealed::of(root);
    match &asked.policy {
        None => Answer::Why(Box::new(why::in_plane(
            root,
            &asker,
            request,
            &sealed,
            Some(&floor),
        ))),
        Some(policy) => said(why::dry_run(
            root,
            &asker,
            request,
            &sealed,
            Some(&floor),
            policy,
        )),
    }
}

/// **Writes the block a deny is**, as the host decided it (Q313.3): source `policy`, aimed at
/// the first deciding rule, with every deciding rule, its level, the family and both outcomes.
/// Written here, not from the chat's own line, which a chat at the sandbox level `off` can reach.
fn blocked(held: &Held, chat: u32, tool: &str, decision: &decide::Decision) {
    use purlis_core::eventlog::stops::{Block, Decided, Source, Stop, TARGET_CHARS, Target};
    let stop = Stop::Block(Block {
        tool: purlis_core::shown::readable(tool, TARGET_CHARS),
        source: Source::Policy,
        qualifier: None,
        target: decision
            .deciding
            .first()
            .and_then(|rule| Target::guard_arm(&rule.id).ok()),
        args: None,
        policy: Some(Decided::from(decision)),
    });
    if let Err(why) = held.hooks().write_stop(chat, stop.clone()) {
        tracing::warn!("purlis: chat {chat}'s policy block was not recorded: {why}");
    }
    crate::pausing::stopped(held, chat, &stop, None);
}
