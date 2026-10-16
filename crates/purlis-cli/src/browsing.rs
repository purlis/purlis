// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The chat's browser tools, as the MCP server serves them** (#2013): each call is one
//! [`hookwire::Ask::Browse`] handed to the app that started the chat over the chat's hook
//! socket, on the framed stream, so a screenshot comes back whole as an image. What the tools
//! take and say is [`purlis_core::browsertools`]; whether the chat may have a browser, and what
//! it may reach, is the app's and the person's. With no app there is no browser.

use std::time::Duration;

use purlis_core::browsertools;
use purlis_core::hookwire::{self, Answer, Ask, BrowseAsk};

use crate::brokered::NotTaken;

/// How long a tool waits on the app: a wait for text of a minute, and the page settling after.
const ASKED_WITHIN: Duration = Duration::from_secs(90);

/// Tool `name` called with `args`: what to tell the model, and what the app attached.
pub fn tool(
    name: &str,
    args: &serde_json::Map<String, serde_json::Value>,
) -> (Result<String, String>, Vec<hookwire::framed::Attached>) {
    let asked = match browsertools::asked(name, args) {
        Ok(asked) => asked,
        Err(why) => return (Err(why), Vec::new()),
    };
    // A call the agent gave no id is given one of its own, so no two calls share one.
    let call_id = asked.call_id.unwrap_or_else(browsertools::fresh_call_id);
    what_the_app_said(crate::brokered::replied(
        move |chat| {
            Ask::Browse(Box::new(BrowseAsk {
                chat,
                browser: asked.browser,
                call_id,
                asked: asked.asked,
            }))
        },
        ASKED_WITHIN,
    ))
}

/// **`ask_operator` pointed at one of the chat's browsers** (#2015): handed to the app as one
/// [`Ask::Browse`], which raises the Ask under Needs you and answers at once. What the person
/// does in the browser is never told: the chat hears only that they handed it back.
pub fn needs_you(args: &serde_json::Map<String, serde_json::Value>) -> Result<String, String> {
    let asked = browsertools::needs_you(args)?;
    let call_id = browsertools::fresh_call_id();
    the_person_was_asked(crate::brokered::replied(
        move |chat| {
            Ask::Browse(Box::new(BrowseAsk {
                chat,
                browser: asked.browser,
                call_id,
                asked: asked.asked,
            }))
        },
        ASKED_WITHIN,
    ))
}

/// What the app's answer to a browser question tells the model: its own words alone.
fn the_person_was_asked(
    replied: Result<std::io::Result<hookwire::framed::Reply>, NotTaken>,
) -> Result<String, String> {
    if let Ok(Ok(reply)) = &replied
        && let Answer::Browsed { browsed, .. } = &reply.answer
    {
        return browsed.note.clone().ok_or_else(|| OLDER_APP.to_owned());
    }
    // Any other answer is the app's refusal in its own words, or an app too old to raise it.
    Err(what_the_app_said(replied)
        .0
        .err()
        .unwrap_or_else(|| OLDER_APP.to_owned()))
}

/// What the app's reply, or the failure to get one, tells the model.
fn what_the_app_said(
    replied: Result<std::io::Result<hookwire::framed::Reply>, NotTaken>,
) -> (Result<String, String>, Vec<hookwire::framed::Attached>) {
    match replied {
        Ok(Ok(reply)) => match reply.answer {
            Answer::Browsed { browser, browsed } => {
                (Ok(browsertools::said(&browser, &browsed)), reply.attached)
            }
            Answer::No { why } if why == hookwire::UNKNOWN_LINE => {
                (Err(OLDER_APP.to_owned()), Vec::new())
            }
            Answer::No { why } => (Err(why), Vec::new()),
            _ => (Err(OLDER_APP.to_owned()), Vec::new()),
        },
        Ok(Err(e)) => (
            Err(format!(
                "the browser's answer did not reach this tool ({e}): take a snapshot to see \
                 where the page is"
            )),
            Vec::new(),
        ),
        Err(NotTaken::Dropped) => (Err(OLDER_APP.to_owned()), Vec::new()),
        Err(NotTaken::NoConnection) => (Err(NO_CONNECTION.to_owned()), Vec::new()),
        Err(NotTaken::NotAChat | NotTaken::AppGone) => (Err(NO_APP.to_owned()), Vec::new()),
    }
}

const OLDER_APP: &str = "The purlis app that started this chat gives chats no browser: it may be \
                         older than these tools.";

const NO_CONNECTION: &str = "This harness hands purlis's tools no connection to the app that \
                             started the chat, so there is no browser to drive.";

const NO_APP: &str = "No purlis app started this chat, or it is not running, so there is no \
                      browser to drive: browsers run only for chats purlis started.";

#[cfg(test)]
mod tests;
