// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! `purlis persona where`, the `persona_where` tool, and the two things a chat's hooks tell it
//! unasked: where it is working at its start, and what changed at a later turn (#1450).
//!
//! All four ask the app that started the chat, over the chat's hook socket
//! ([`purlis_core::hookwire::Ask::WhereWorking`]), and print what it answers with
//! [`purlis_core::awareness`]'s words. Nothing here reads a file: the answer is the app's own
//! record, so it is the same from a sandboxed chat, and there is nothing to answer with where
//! no app is.

use std::process::ExitCode;
use std::time::Duration;

use purlis_core::awareness::{self, Tell, Working};
use purlis_core::hookwire::{self, Answer, Ask};

use crate::brokered::NotTaken;

/// How long the command and the tool wait on the app.
const ASKED_WITHIN: Duration = Duration::from_secs(10);

/// How long a hook waits on the app, for each of the ask and the answer: it holds a chat's
/// start or a turn, and the harness gives `sessionstart` five seconds in all, so an app that
/// is slow costs the line and never the briefing or the turn.
const A_HOOK_WAITS: Duration = Duration::from_millis(750);

/// What came of asking the app where this chat is working.
enum Asked {
    Working(Box<Working>),
    /// This process is in no chat the app started.
    NotAChat,
    /// The app read the ask and will not answer it: its sentence.
    Refused(String),
    /// No app answered, and why not.
    Unanswered(&'static str),
}

/// The ask for where this chat is working, said as `tell`.
fn where_working(tell: Tell) -> impl FnOnce(u32) -> Ask {
    move |chat| Ask::WhereWorking(hookwire::WhereWorking { chat, tell })
}

fn asked(tell: Tell, within: Duration) -> Asked {
    what_the_app_said(crate::brokered::asked(where_working(tell), within))
}

/// [`asked`], over the framed stream where the app speaks it (#1996): the largest answer the
/// app gives comes back whole however long it grows, with anything the app attaches to it.
fn replied(tell: Tell, within: Duration) -> (Asked, Vec<hookwire::framed::Attached>) {
    match crate::brokered::replied(where_working(tell), within) {
        Ok(Ok(reply)) => (what_the_app_said(Ok(Ok(reply.answer))), reply.attached),
        Ok(Err(e)) => (what_the_app_said(Ok(Err(e))), Vec::new()),
        Err(not) => (what_the_app_said(Err(not)), Vec::new()),
    }
}

/// What the app's answer, or the failure to get one, says.
fn what_the_app_said(answer: Result<std::io::Result<Answer>, NotTaken>) -> Asked {
    match answer {
        Ok(Ok(Answer::Working(working))) => Asked::Working(working),
        Ok(Ok(Answer::No { why })) if why == hookwire::NOTHING_ANSWERS => {
            Asked::Unanswered(NO_ANSWER)
        }
        Ok(Ok(Answer::No { why })) => Asked::Refused(why),
        Ok(Ok(_) | Err(_)) | Err(NotTaken::Dropped) => Asked::Unanswered(NO_ANSWER),
        Err(NotTaken::NotAChat) => Asked::NotAChat,
        Err(NotTaken::NoConnection) => Asked::Unanswered(NO_CONNECTION),
        Err(NotTaken::AppGone) => Asked::Unanswered(APP_GONE),
    }
}

const NO_ANSWER: &str = "The purlis app that started this chat did not answer where it is \
                         working: it may be older than this command. Its window lists every \
                         open chat.";

const NO_CONNECTION: &str = "This process has no connection to the purlis app that started \
                             the chat, so where it is working is not known here: run `purlis \
                             persona where` in the chat itself.";

const APP_GONE: &str = "The purlis app that started this chat is not running, so there is no \
                        record to read of where it is working.";

/// The instant times are said against, in this machine's zone.
fn local(now: chrono::DateTime<chrono::Utc>) -> chrono::DateTime<chrono::FixedOffset> {
    now.with_timezone(&chrono::Local).fixed_offset()
}

/// `purlis persona where`: the picture on stdout; outside a chat, a sentence saying so.
pub fn run(now: Option<&str>) -> ExitCode {
    match asked(Tell::Asked, ASKED_WITHIN) {
        Asked::Working(working) => {
            println!(
                "{}",
                awareness::listing(&working.picture, local(crate::hooks::instant(now)))
            );
            ExitCode::SUCCESS
        }
        Asked::NotAChat => {
            crate::voice::info(awareness::NOT_A_CHAT);
            ExitCode::SUCCESS
        }
        Asked::Refused(why) => {
            crate::voice::err(format!("purlis persona where: {why}"));
            ExitCode::FAILURE
        }
        Asked::Unanswered(why) => {
            crate::voice::warn(why);
            ExitCode::FAILURE
        }
    }
}

/// The `persona_where` tool: what the command prints, or why there is nothing to print, and
/// what the app attached to its answer.
pub fn tool() -> (Result<String, String>, Vec<hookwire::framed::Attached>) {
    let (asked, attached) = replied(Tell::Asked, ASKED_WITHIN);
    let said = match asked {
        Asked::Working(working) => Ok(awareness::listing(
            &working.picture,
            local(chrono::Utc::now()),
        )),
        Asked::NotAChat => Ok(awareness::NOT_A_CHAT.to_owned()),
        Asked::Refused(why) => Err(why),
        Asked::Unanswered(why) => Err(why.to_owned()),
    };
    (said, attached)
}

/// What a chat's start briefing says of where it is working, where the app has anything to
/// say. From then that is what the app counts the chat as told.
pub fn briefing(now: chrono::DateTime<chrono::Utc>) -> Option<String> {
    match asked(Tell::Start, A_HOOK_WAITS) {
        Asked::Working(working) => awareness::briefing(&working.picture, local(now)),
        _ => None,
    }
}

/// The one line a turn is told when where the chat is working has changed since it was last
/// told, and nothing when it has not.
pub fn update(now: chrono::DateTime<chrono::Utc>) -> Option<String> {
    match asked(Tell::Turn, A_HOOK_WAITS) {
        Asked::Working(working) => awareness::update(&working, local(now)),
        _ => None,
    }
}
