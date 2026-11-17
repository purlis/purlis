// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! What a scenario needs RUNNING while its command runs, which no file can carry.
//!
//! Two kinds, each a stand-in for the desktop app's half of a conversation on its hook socket.
//! Both bind the socket, answer a ticket ask with a ticket, answer the ask that spends it, and
//! are gone once the command is. Like the app, each serves every connection made to it, and
//! answers a `whoami` (#6995) as an app with none of the asker's chats: the scenarios run side by
//! side in one process, and a command whose environment names no chat asks every socket that
//! process holds. They check nothing, because what is under test is the COMMAND's side; the app's
//! half is `app/src-tauri/src/handoff.rs` and its own tests.
//!
//! - `an-app-that-opens` (charter-app#204) answers the spending ask with "opened as chat 9":
//!   the differential's `_AN_APP_THAT_OPENS`, answer for answer.
//! - `an-app-that-answers` (#1436) answers it with the scenario's own `answer`, one JSON value
//!   written as one line: what a dispatch is told, whichever of the app's answers that is.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde_json::{Map, Value};

pub struct Served {
    socket: PathBuf,
    thread: JoinHandle<()>,
    /// Set once the scenario's command has finished: the stand-in stops listening.
    done: Arc<AtomicBool>,
}

/// What the stand-in answers a `{"whoami": true}` (#6995): what an app says to a process in
/// none of its chats, so the command puts its environment's chat to it, as it does to an app
/// older than the question. Another scenario's command, run beside this one, may ask it too.
const IN_NO_CHAT: &str =
    "{\"no\": {\"why\": \"this process runs inside none of this app's chats\"}}\n";

pub fn start(serve: &Map<String, Value>, side: &Path) -> Served {
    let kind = serve["kind"].as_str().expect("a kind");
    let answers = match kind {
        "an-app-that-opens" => "{\"opened\": {\"chat\": 9}}\n".to_owned(),
        "an-app-that-answers" => {
            let answer = serve.get("answer").expect("an answer");
            format!("{}\n", serde_json::to_string(answer).expect("one line"))
        }
        _ => panic!("no stand-in named {kind:?}"),
    };
    let socket = side.join(serve["socket"].as_str().expect("a socket"));
    std::fs::create_dir_all(socket.parent().expect("a parent")).expect("the socket's directory");
    let listener = UnixListener::bind(&socket).expect("the stand-in binds");
    listener.set_nonblocking(true).expect("non-blocking");
    let done = Arc::new(AtomicBool::new(false));
    let stop = Arc::clone(&done);
    let thread = std::thread::spawn(move || {
        // Thirty seconds, as the stand-in it replaces: a command that never connects does not
        // leave it waiting for ever. Every connection is served, one at a time, as an app
        // serves many: a `whoami` one (#6995) is told no chat, and the command's own follows.
        let deadline = Instant::now() + Duration::from_secs(30);
        while !stop.load(Ordering::SeqCst) && Instant::now() < deadline {
            match listener.accept() {
                Ok((connection, _)) => serve_one(connection, &answers),
                Err(_) => std::thread::sleep(Duration::from_millis(10)),
            }
        }
    });
    Served {
        socket,
        thread,
        done,
    }
}

/// One connection, each line answered: a `whoami` with [`IN_NO_CHAT`], a ticket ask with a
/// ticket, anything else with the scenario's answer.
fn serve_one(connection: std::os::unix::net::UnixStream, answers: &str) {
    connection.set_nonblocking(false).expect("blocking");
    connection
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("a timeout");
    let mut writer = connection.try_clone().expect("a second handle");
    for line in BufReader::new(connection).lines() {
        let Ok(line) = line else { break };
        let Ok(ask) = serde_json::from_str::<Value>(&line) else {
            break;
        };
        let answer = if ask.get("whoami").is_some() {
            IN_NO_CHAT.to_owned()
        } else if ask.get("ticket").is_some() {
            format!("{{\"ticket\": {{\"ticket\": \"{}\"}}}}\n", "0".repeat(64))
        } else {
            answers.to_owned()
        };
        if writer
            .write_all(answer.as_bytes())
            .and_then(|()| writer.flush())
            .is_err()
        {
            break;
        }
    }
}

impl Served {
    /// Stop the stand-in once the scenario's command is done, and take its socket away, as the
    /// stand-in it replaces did on its way out.
    pub fn finish(self) {
        self.done.store(true, Ordering::SeqCst);
        let _ = self.thread.join();
        let _ = std::fs::remove_file(&self.socket);
    }
}
