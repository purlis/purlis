// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **What is typed into the chat**, read the way a harness at its prompt reads it: the terminal
//! raw, a line sent on Enter, ESC CR a new line inside it (the dialects' `newline`), Esc alone an
//! interruption, Ctrl-C or Ctrl-D the end of the session. Off a terminal (a test's pipe) the same
//! bytes mean the same, and the end of input ends the session.

use std::io::{IsTerminal, Read};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    Line(String),
    Esc,
    End,
}

pub struct Keyboard {
    bytes: Receiver<Option<u8>>,
    line: Vec<u8>,
    ended: bool,
}

/// How long a byte after ESC is waited for before the ESC is read as Esc alone.
const ESC_PAIR: Duration = Duration::from_millis(40);

impl Keyboard {
    pub fn start() -> Keyboard {
        if std::io::stdin().is_terminal() {
            let _ = std::process::Command::new("stty")
                .args(["raw", "-echo"])
                .stdin(std::process::Stdio::inherit())
                .status();
        }
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let stdin = std::io::stdin();
            for byte in stdin.lock().bytes() {
                match byte {
                    Ok(b) => {
                        if tx.send(Some(b)).is_err() {
                            return;
                        }
                    }
                    Err(_) => break,
                }
            }
            let _ = tx.send(None);
        });
        Keyboard {
            bytes: rx,
            line: Vec::new(),
            ended: false,
        }
    }

    /// The next key, waiting at most until `until` (`None`: for ever).
    pub fn next(&mut self, until: Option<Instant>) -> Option<Key> {
        if self.ended {
            return Some(Key::End);
        }
        loop {
            let byte = match until {
                None => self.bytes.recv().ok().flatten(),
                Some(until) => {
                    match self
                        .bytes
                        .recv_timeout(until.saturating_duration_since(Instant::now()))
                    {
                        Ok(b) => b,
                        Err(RecvTimeoutError::Timeout) => return None,
                        Err(RecvTimeoutError::Disconnected) => None,
                    }
                }
            };
            let Some(byte) = byte else {
                self.ended = true;
                return Some(Key::End);
            };
            match byte {
                0x03 | 0x04 => {
                    self.ended = true;
                    return Some(Key::End);
                }
                0x1b => match self.bytes.recv_timeout(ESC_PAIR) {
                    Ok(Some(b'\r' | b'\n')) => self.line.push(b'\n'),
                    // An escape sequence (an arrow key): read and dropped.
                    Ok(Some(b'[' | b'O')) => {
                        while let Ok(Some(b)) = self.bytes.recv_timeout(ESC_PAIR) {
                            if b.is_ascii_alphabetic() || b == b'~' {
                                break;
                            }
                        }
                    }
                    Ok(Some(other)) => {
                        // Esc, then a byte typed after it.
                        if other != 0x1b {
                            self.line.push(other);
                        }
                        return Some(Key::Esc);
                    }
                    Ok(None) => {
                        self.ended = true;
                        return Some(Key::Esc);
                    }
                    Err(_) => return Some(Key::Esc),
                },
                b'\r' | b'\n' => {
                    if self.line.is_empty() {
                        continue;
                    }
                    let line = std::mem::take(&mut self.line);
                    return Some(Key::Line(String::from_utf8_lossy(&line).into_owned()));
                }
                0x7f | 0x08 => {
                    self.line.pop();
                }
                other => self.line.push(other),
            }
        }
    }

    /// Esc was typed since the last look, without waiting; any line typed meanwhile is kept.
    pub fn escaped(&mut self, lines: &mut Vec<String>) -> bool {
        loop {
            match self.next(Some(Instant::now())) {
                None => return false,
                Some(Key::Esc) => return true,
                Some(Key::Line(line)) => lines.push(line),
                Some(Key::End) => return true,
            }
        }
    }

    pub fn ended(&self) -> bool {
        self.ended
    }
}

impl Drop for Keyboard {
    fn drop(&mut self) {
        if std::io::stdin().is_terminal() {
            let _ = std::process::Command::new("stty")
                .arg("sane")
                .stdin(std::process::Stdio::inherit())
                .status();
        }
    }
}
