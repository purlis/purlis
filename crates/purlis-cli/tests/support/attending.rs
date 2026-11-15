// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **An app that answers a chat's attendance ask** (T1-22, #6413): a listener on a socket of its
//! own, holding the token of one chat it started, which answers [`Ask::Attendance`] as a test
//! sets it and refuses every other ask. What the app holds and how it answers are in
//! `app/src-tauri/src/handoff/tests.rs`; this is what a hook of a chat the app started is
//! answered.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use purlis_core::hookwire::{self, Answer, Ask, CHAT_ENV, Listener, SOCKET_ENV, TOKEN_ENV};

/// The stand-in app, listening while it is held.
pub struct AttendingApp {
    _reading: hookwire::Reading,
    socket: PathBuf,
    chat: u32,
    token: hookwire::ChatToken,
    asked: Arc<Mutex<usize>>,
}

impl AttendingApp {
    /// An app listening under `within`, with chat `chat` open, which says the chat is attended
    /// where `attended`.
    pub fn answering(within: &Path, chat: u32, attended: bool) -> Self {
        let socket = within.join("app").join("hooks.sock");
        let listener = Listener::bind(within, &socket).unwrap_or_else(|e| panic!("a socket: {e}"));
        let token = listener
            .tokens()
            .issue_to_this_process(chat)
            .expect("a token");
        let asked = Arc::new(Mutex::new(0));
        let counted = Arc::clone(&asked);
        let reading = listener.each_answering(
            Box::new(|_| {}),
            Box::new(move |_, ask| match ask {
                Ask::Attendance(_) => {
                    *counted.lock().unwrap() += 1;
                    Answer::Attended { attended }
                }
                _ => Answer::No {
                    why: "not asked here".to_owned(),
                },
            }),
        );
        Self {
            _reading: reading,
            socket,
            chat,
            token,
            asked,
        }
    }

    /// The environment the app gave its chat: its socket, its number and its token.
    pub fn chat_env(&self) -> Vec<(String, String)> {
        vec![
            (SOCKET_ENV.to_owned(), self.socket.display().to_string()),
            (CHAT_ENV.to_owned(), self.chat.to_string()),
            (TOKEN_ENV.to_owned(), self.token.expose().to_owned()),
        ]
    }

    /// How many times a hook asked whether the chat is attended.
    pub fn attendance_asked(&self) -> usize {
        *self.asked.lock().unwrap()
    }
}
