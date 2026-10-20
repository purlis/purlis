// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **How a browser's engine ended** (#2745): who ended it, its process's exit, and why its
//! DevTools connection closed, said in words for the host's log that hold nothing a page showed.

use std::fmt;

use crate::cdp::Closed;

/// How an engine's own process ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineExit {
    /// It exited with this code.
    Code(i32),
    /// A signal ended it, named as the system names it (`SIGKILL`).
    Signal(String),
}

impl fmt::Display for EngineExit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineExit::Code(code) => write!(f, "exit code {code}"),
            EngineExit::Signal(signal) => write!(f, "signal {signal}"),
        }
    }
}

/// Who ended an engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndedBy {
    /// purlis stopped it: the browser was stopped, closed, or its host ended.
    Stopped,
    /// It ended by itself: it exited, or a signal purlis did not send ended it.
    Itself,
    /// purlis ended it because its DevTools connection closed while it still ran.
    LostConnection,
}

/// How a browser's engine ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ending {
    /// Who ended it.
    pub by: EndedBy,
    /// Its process's exit, where purlis saw it.
    pub exit: Option<EngineExit>,
    /// Why its DevTools connection closed, where it closed before purlis stopped the engine.
    pub closed: Option<Closed>,
}

impl fmt::Display for Ending {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let exit = match &self.exit {
            Some(exit) => format!(" ({exit})"),
            None => String::new(),
        };
        match self.by {
            EndedBy::Stopped => write!(f, "purlis stopped its engine{exit}"),
            EndedBy::Itself => {
                write!(f, "its engine ended by itself{exit}")?;
                match &self.closed {
                    Some(closed) if *closed != Closed::PipeEnded => write!(f, "; {closed}"),
                    _ => Ok(()),
                }
            }
            EndedBy::LostConnection => {
                write!(
                    f,
                    "purlis ended its engine{exit} because its DevTools connection closed"
                )?;
                match &self.closed {
                    Some(closed) => write!(f, ": {closed}"),
                    None => Ok(()),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
