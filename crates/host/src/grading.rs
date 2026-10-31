// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **The grades a terminal asked a window for** (SK-61, #3241): who asked, who was asked, and
//! what is owed to each once one of them goes. The host only carries a grade: the window runs it,
//! and a terminal's ask takes the one check a press of Grade takes there.

use std::collections::HashMap;
use std::time::Duration;

use purlis_session_protocol::session::GradeAnswer;
use tokio::sync::oneshot;

/// How long a window has to answer: it reads the skill's copy and checks a chat could start.
pub(crate) const ANSWERED_WITHIN: Duration = Duration::from_secs(60);

/// One ask, unanswered or running.
struct One {
    asker: u64,
    window: u64,
    /// Who waits for the answer, until it comes.
    waiting: Option<oneshot::Sender<GradeAnswer>>,
    /// The grade it started, once it did.
    grade: Option<String>,
    /// Whether it starts a grade, whose steps follow its answer.
    starts: bool,
    /// Whether its grade's last step came before its answer: a grade's thread and the answer race.
    over: bool,
}

/// What is owed once a link has gone.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Gone {
    /// `(window, grade)`: a grade nobody follows any more, to stop.
    pub to_stop: Vec<(u64, String)>,
    /// `(terminal, grade)`: a grade whose window went, to say so.
    pub to_tell: Vec<(u64, String)>,
}

/// The asks a host holds, by the question.
#[derive(Default)]
pub(crate) struct GradeAsks(HashMap<u64, One>);

impl GradeAsks {
    /// `asker` asked `window` question `question`: what its answer comes on.
    pub fn asked(
        &mut self,
        question: u64,
        asker: u64,
        window: u64,
        starts: bool,
    ) -> oneshot::Receiver<GradeAnswer> {
        let (answer, answered) = oneshot::channel();
        self.0.insert(
            question,
            One {
                asker,
                window,
                waiting: Some(answer),
                grade: None,
                starts,
                over: false,
            },
        );
        answered
    }

    /// The window `from` answered `question`. An answer from any other link is not one. The
    /// answer is the grade to stop, where it started for an asker that is no longer waiting.
    pub fn answered(&mut self, question: u64, from: u64, answer: GradeAnswer) -> Option<String> {
        let started = match &answer {
            GradeAnswer::Started { grade } => Some(grade.clone()),
            _ => None,
        };
        let Some(one) = self.0.get_mut(&question).filter(|one| one.window == from) else {
            // Nobody follows it: an ask given up on, or an asker gone. Only the link that was
            // asked is held to that, and any other link was told nothing to start.
            return started;
        };
        let waiting = one.waiting.take();
        let told = waiting.is_some_and(|waiting| waiting.send(answer).is_ok());
        match started {
            Some(_) if told && one.over => {
                self.0.remove(&question);
                None
            }
            Some(grade) if told && one.starts => {
                one.grade = Some(grade);
                None
            }
            started => {
                self.0.remove(&question);
                started
            }
        }
    }

    /// One step of `question` from the window `from`: the link to push it to, and, at its `end`,
    /// nothing more is held of it.
    pub fn step(&mut self, question: u64, from: u64, end: bool) -> Option<u64> {
        let one = self.0.get_mut(&question).filter(|one| one.window == from)?;
        let asker = one.asker;
        if end {
            if one.waiting.is_some() {
                one.over = true;
            } else {
                self.0.remove(&question);
            }
        }
        Some(asker)
    }

    /// An ask whose answer did not come in time, or that nobody waits for now. A grade it did
    /// start is stopped as its answer arrives.
    pub fn not_answered(&mut self, question: u64) {
        self.0.remove(&question);
    }

    /// Forgets what `link` asked or was asked: what is owed for it.
    pub fn link_gone(&mut self, link: u64) -> Gone {
        let mut gone = Gone::default();
        self.0.retain(|_, one| {
            if one.asker == link {
                if let Some(grade) = &one.grade {
                    gone.to_stop.push((one.window, grade.clone()));
                }
                return false;
            }
            if one.window == link {
                if let Some(grade) = &one.grade {
                    gone.to_tell.push((one.asker, grade.clone()));
                }
                // An unanswered asker hears "did not answer" as its sender goes.
                return false;
            }
            true
        });
        gone
    }
}

#[cfg(test)]
mod tests;
