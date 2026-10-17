// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The `theme_preview` tool, as the MCP server serves it** (#2330): one
//! [`hookwire::Ask::ThemePreview`] handed to the app that started the chat over the chat's hook
//! socket, on the framed stream, so each theme specimen's picture comes back whole as an image.
//! What the tool takes and says is [`purlis_core::themepreview`]; the app draws the specimen in
//! a hidden window of its own. With no app there is no specimen.

use std::time::Duration;

use purlis_core::hookwire::{self, Answer, Ask, ThemePreviewAsk};
use purlis_core::themepreview;

use crate::brokered::NotTaken;

/// How long the tool waits on the app: each specimen's budget, and a little for the reply.
const ASKED_WITHIN: Duration =
    Duration::from_secs(themepreview::BUDGET.as_secs() * themepreview::AT_MOST as u64 + 5);

/// The tool called with `args`: what to tell the model, and the pictures the app attached.
pub fn tool(
    args: &serde_json::Map<String, serde_json::Value>,
) -> (Result<String, String>, Vec<hookwire::framed::Attached>) {
    let specimens = match themepreview::asked(args) {
        Ok(specimens) => specimens,
        Err(why) => return (Err(why), Vec::new()),
    };
    what_the_app_said(crate::brokered::replied(
        move |chat| Ask::ThemePreview(ThemePreviewAsk { chat, specimens }),
        ASKED_WITHIN,
    ))
}

/// What the app's reply, or the failure to get one, tells the model.
fn what_the_app_said(
    replied: Result<std::io::Result<hookwire::framed::Reply>, NotTaken>,
) -> (Result<String, String>, Vec<hookwire::framed::Attached>) {
    match replied {
        Ok(Ok(reply)) => match reply.answer {
            Answer::Previewed { painted } if !reply.attached.is_empty() => {
                (Ok(themepreview::said(&painted)), reply.attached)
            }
            Answer::No { why } if why == hookwire::UNKNOWN_LINE => {
                (Err(OLDER_APP.to_owned()), Vec::new())
            }
            Answer::No { why } => (Err(why), Vec::new()),
            _ => (Err(OLDER_APP.to_owned()), Vec::new()),
        },
        Ok(Err(e)) => (
            Err(format!(
                "the theme specimen did not reach this tool ({e}): ask for it again"
            )),
            Vec::new(),
        ),
        Err(NotTaken::Dropped) => (Err(OLDER_APP.to_owned()), Vec::new()),
        Err(NotTaken::NoConnection) => (Err(NO_CONNECTION.to_owned()), Vec::new()),
        Err(NotTaken::NotAChat | NotTaken::AppGone) => (Err(NO_APP.to_owned()), Vec::new()),
    }
}

const OLDER_APP: &str = "The purlis app that started this chat draws no theme specimen: it may \
                         be older than this tool.";

const NO_CONNECTION: &str = "This harness hands purlis's tools no connection to the app that \
                             started the chat, so no theme specimen can be drawn.";

const NO_APP: &str = "No purlis app started this chat, or it is not running, so no theme \
                      specimen can be drawn: the purlis app draws it, for the chats it started.";

#[cfg(test)]
mod tests {
    use super::*;
    use purlis_core::hookwire::framed::{Attached, Reply};
    use purlis_core::theme::Appearance;
    use purlis_core::themepreview::Painted;

    #[test]
    fn the_pictures_the_app_drew_reach_the_model_after_what_they_are() {
        let reply = Reply {
            answer: Answer::Previewed {
                painted: vec![Painted {
                    theme: "purlis-classic".into(),
                    appearance: Appearance::Light,
                    width: 2560,
                    height: 1600,
                    millis: 700,
                    png: Vec::new(),
                }],
            },
            attached: vec![Attached {
                media_type: "image/png".into(),
                bytes: b"png".to_vec(),
            }],
        };
        let (said, attached) = what_the_app_said(Ok(Ok(reply)));
        let said = said.expect("drawn");
        assert!(said.contains("purlis-classic (light)"), "{said}");
        assert_eq!(attached.len(), 1);
    }

    #[test]
    fn with_no_app_the_tool_refuses_saying_the_app_draws_it() {
        for not in [NotTaken::NotAChat, NotTaken::AppGone] {
            let (said, attached) = what_the_app_said(Err(not));
            assert_eq!(said, Err(NO_APP.to_owned()));
            assert!(attached.is_empty());
        }
        let (said, _) = what_the_app_said(Err(NotTaken::NoConnection));
        assert_eq!(said, Err(NO_CONNECTION.to_owned()));
    }

    #[test]
    fn an_app_older_than_the_tool_is_said_to_be_one() {
        let older = Reply::from(Answer::No {
            why: hookwire::UNKNOWN_LINE.to_owned(),
        });
        assert_eq!(
            what_the_app_said(Ok(Ok(older))).0,
            Err(OLDER_APP.to_owned())
        );
        assert_eq!(
            what_the_app_said(Err(NotTaken::Dropped)).0,
            Err(OLDER_APP.to_owned())
        );
    }

    #[test]
    fn the_app_s_refusal_is_its_own_words() {
        let no = Reply::from(Answer::No {
            why: "the specimen was not drawn within 5 s".to_owned(),
        });
        assert_eq!(
            what_the_app_said(Ok(Ok(no))).0,
            Err("the specimen was not drawn within 5 s".to_owned())
        );
    }
}
