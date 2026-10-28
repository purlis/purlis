// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

//! **A chat's router** (SK-15b, #3230; ADR 0102 §5, §6): which router its workspace or its
//! persona chose ([`chosen`]), the question an extension router is asked at the chat's start
//! ([`at_start`]), and the routing record that says what came of it ([`Record`]).
//!
//! - **The persona's choice wins** (`skill-router:` in its own definition), then its workspace's
//!   (`settings.skills.router`), then the built-in router. A choice that names no router is the
//!   built-in router, and the record says why.
//! - **An extension router is asked once, at the chat's start**, which of the skills the built-in
//!   router may hide to hide, each with its own line ([`purlis_skills::SkillSet::candidates`]). It
//!   is handed those skills' names and descriptions, the pins and the budget: never a prompt, the
//!   chat's id, its prompt salt (SK-38) or anything else of the chat.
//! - **It has 100 ms**, its confined program's start inside them
//!   ([`crate::extension::studio::ROUTER_DEADLINE`]). The chat's start waits no longer whatever
//!   the extension does: past that, on an error, or on an answer the set will not take (one that
//!   hides a pin, names a skill it may not hide, or leaves the list over the budget,
//!   [`purlis_skills::SkillSet::routed_by`]), the chat is routed by the built-in router.
//! - **The routing record** goes in the chat's event log as `skill.routed`, and every skill run
//!   the chat opens names it ([`crate::eventlog::Recorder::skill_routed`]). It names skills and
//!   says why each is hidden, and holds nothing of a prompt or a description.
//!
//! A chat routed by the built-in router asks nothing and writes no record: what that router
//! hides is worked out the same way wherever the set is routed again.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use purlis_skills::{Budget, Line, Router, SkillSet};

use crate::extension::studio::{Ask, ROUTER_DEADLINE, RouteBudget, RouteSkill};

/// The word that chooses the built-in router.
pub const BUILT_IN: &str = "built-in";

/// The persona definition's key that chooses its chats' router.
pub(crate) const PERSONA_KEY: &str = "skill-router";

/// The word a record says a skill the built-in router hid over the budget with.
const OVER_BUDGET: &str = "over-budget";

/// The most characters of an error a record keeps.
const MOST_ERROR_CHARS: usize = 300;

/// **The router a workspace or a persona chose** (ADR 0102 §5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chosen {
    BuiltIn,
    /// An extension's, by its id.
    Extension(String),
}

/// **Which router a chat in `workspace` as `persona` is routed by** (ADR 0102 §5): the persona's
/// own `skill-router:` where it writes one, else the workspace's `settings.skills.router`, else
/// the built-in router. `Err` where the one in force names no router, with the sentence why:
/// such a chat is routed by the built-in router.
pub fn chosen(
    plane: &Path,
    workspace: Option<&str>,
    persona: Option<&str>,
) -> Result<Chosen, String> {
    if let Some(persona) = persona
        && let Some(written) = crate::personaverbs::skill_router(plane, persona)
    {
        return parse(&written, &format!("the persona {persona}"));
    }
    match workspace.and_then(|ws| written_in(plane, ws)) {
        Some(Ok(written)) => parse(
            &written,
            &format!("the workspace {}", workspace.unwrap_or_default()),
        ),
        Some(Err(why)) => Err(why),
        None => Ok(Chosen::BuiltIn),
    }
}

/// The workspace `workspace`'s `settings.skills.router`: `None` where it writes none, `Err`
/// where it is no string.
fn written_in(plane: &Path, workspace: &str) -> Option<Result<String, String>> {
    use crate::settings::workspace::{SKILLS, SKILLS_ROUTER};
    let table = crate::settings::workspace::read(plane, workspace)?;
    let value = table.get(SKILLS)?.get(SKILLS_ROUTER)?;
    Some(value.as_str().map(str::to_owned).ok_or_else(|| {
        format!(
            "the workspace {workspace}'s settings.{SKILLS}.{SKILLS_ROUTER} is not a router's \
             name, so its chats are routed by the built-in router"
        )
    }))
}

/// `written`, `whose` choice, as a router.
fn parse(written: &str, whose: &str) -> Result<Chosen, String> {
    let written = written.trim();
    if written == BUILT_IN {
        return Ok(Chosen::BuiltIn);
    }
    if crate::extension::an_id(written) {
        return Ok(Chosen::Extension(written.to_owned()));
    }
    Err(format!(
        "{whose} chooses {:?} as its router, which is neither {BUILT_IN} nor an extension's id, \
         so its chats are routed by the built-in router",
        crate::shown::readable(written, 64)
    ))
}

/// **What asks an extension router its question**: the app's or the host's executor, with this
/// machine's record of extensions and the project's choices ([`OnThisMachine`]), or a test's own.
/// Asked on a thread of its own, which the chat's start never waits on past the deadline.
pub trait AskRouter: Send + Sync {
    /// The answer of the router `extension` to `ask`, read as far as every studio answer is, or
    /// why it gave none.
    fn ask(
        &self,
        extension: &str,
        ask: &Ask<'_>,
    ) -> Result<serde_json::Map<String, serde_json::Value>, String>;
}

/// **The production [`AskRouter`]**: `executor`, gated by this machine's record of extensions and
/// the choices of the project at `project`, as every studio question is
/// ([`crate::executor::Executor::studio`]). Both are read only when a router is asked, so a start
/// whose chat chose none reads neither.
pub struct OnThisMachine {
    pub executor: Arc<crate::executor::Executor>,
    pub project: std::path::PathBuf,
}

impl OnThisMachine {
    /// Asking with this program's own executor, one for the whole process, which knows the
    /// built-in extensions of the bundle it shipped in: for a session host, which holds none of
    /// the window's.
    pub fn with_this_programs(project: &Path) -> Self {
        static EXECUTOR: std::sync::OnceLock<Arc<crate::executor::Executor>> =
            std::sync::OnceLock::new();
        let executor = EXECUTOR.get_or_init(|| {
            Arc::new(crate::executor::Executor::with_built_in(
                crate::extension::BuiltIn::of_this_program(),
            ))
        });
        Self {
            executor: Arc::clone(executor),
            project: project.to_path_buf(),
        }
    }
}

impl AskRouter for OnThisMachine {
    fn ask(
        &self,
        extension: &str,
        ask: &Ask<'_>,
    ) -> Result<serde_json::Map<String, serde_json::Value>, String> {
        let config = crate::machine::config_root().ok_or_else(|| {
            format!("'{extension}' was not asked: this machine has no config home to read it from")
        })?;
        let choices = crate::extension::project::Choices::read(&self.project);
        self.executor.studio(&config, &choices, extension, ask)
    }
}

/// **Why a chat was routed by the built-in router instead of the one it chose** (ADR 0102 §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FellBack {
    /// It did not answer within its deadline.
    Slow,
    /// It could not be asked, or answered with an error.
    Failed,
    /// Its answer was one the set does not take.
    Wrong,
    /// What was chosen names no router.
    Unchosen,
}

/// One skill a routing hid, as its record says it: by name, with the router's own line, or
/// `over-budget` where the built-in router hid it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RecordHidden {
    pub skill: String,
    pub why: String,
}

/// **The routing record** (ADR 0102 §6): the body of a chat's `skill.routed`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Record {
    /// Its ULID, which every skill run the chat opens names.
    pub routing: String,
    /// The router asked, by its extension's id, or what was written where it names none.
    pub router: String,
    /// How long its question took, in milliseconds.
    pub ms: u64,
    /// Why the chat was routed by the built-in router instead, where it was.
    pub fell_back: Option<FellBack>,
    /// The sentence why, where it fell back.
    pub error: Option<String>,
    /// Every skill the routing hid, by name.
    pub hidden: Vec<RecordHidden>,
}

/// **What came of asking a chat's router at its start**: the record, and the answer the set took,
/// which every later routing of the chat's set applies again ([`super::PlaneSkills::routed`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Routed {
    pub record: Record,
    /// The skills the router hid, each with its line, where its answer was taken.
    pub answer: Option<Vec<(String, Line)>>,
}

impl Routed {
    /// **What a person is told of it** on the chat's tab and in the studio: which router, how
    /// long it took, and, where it fell back, why.
    pub fn said(&self) -> String {
        let record = &self.record;
        match (&record.fell_back, &record.error) {
            (None, _) => format!(
                "Routed by {} in {} ms.",
                Router::Extension(record.router.clone()).shown(),
                record.ms
            ),
            (Some(FellBack::Slow), _) => format!(
                "{} did not answer within {} ms, so this chat lists what the built-in router \
                 shows.",
                capitalised(&Router::Extension(record.router.clone()).shown()),
                ROUTER_DEADLINE.as_millis()
            ),
            (Some(_), why) => format!(
                "This chat lists what the built-in router shows: {}",
                why.as_deref()
                    .unwrap_or("its router gave no answer purlis takes.")
            ),
        }
    }

    /// **The router's answer, applied to `set`** within `budget` with `pins`, where it was taken:
    /// each skill it hid that `set` still has for a router to hide, so one pinned or gone since
    /// the chat's start is listed or gone as the set says, and the rest stay hidden. Else, or
    /// where what is left of it leaves the list over the budget, `set` routed by the built-in
    /// router.
    pub(crate) fn apply(&self, set: SkillSet, budget: Budget, pins: &[String]) -> SkillSet {
        let Some(hidden) = &self.answer else {
            return set.routed(budget, pins);
        };
        let still: std::collections::HashSet<String> = set
            .candidates(budget, pins)
            .skills
            .into_iter()
            .filter(|one| !one.pinned)
            .map(|one| one.name)
            .collect();
        let hidden: Vec<(String, Line)> = hidden
            .iter()
            .filter(|(name, _)| still.contains(name))
            .cloned()
            .collect();
        // Kept to fall back on: what is left of an answer can still leave the list over the
        // budget where the set grew since the chat's start.
        let fallback = set.clone();
        set.routed_by(
            Router::Extension(self.record.router.clone()),
            budget,
            pins,
            &hidden,
        )
        .unwrap_or_else(|why| {
            tracing::warn!(
                "purlis: this chat's router's answer no longer fits its skills ({why}); the \
                 built-in router's list is used"
            );
            fallback.routed(budget, pins)
        })
    }
}

/// **How a chat's start routed its set, kept for its reattach** (SK-15f, #3578): the reopen
/// record's `chats[].routing`. A chat reattached from the session host still holds the listing
/// its harness was handed at its real start, so its reattach routes its set as that start did,
/// from this, and never asks its router again.
///
/// Read back as data, never trusted: the answer kept is applied by [`Routed::apply`], which hides
/// only what the set still has for a router to hide, so a kept answer can never hide a pin, a
/// built-in skill or anything outside the chat's set. A value that does not read whole reads as
/// absent, and that chat's router is asked again, as before this was kept.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "KeptOnDisk", into = "KeptOnDisk")]
pub enum Kept {
    /// The built-in router, which was asked nothing; and a chat the plane's skills did not reach.
    BuiltIn,
    /// An extension router was asked, and this came of it: its answer, or the built-in router's
    /// fallback.
    Asked(Routed),
}

impl Kept {
    /// What a start that routed its set as `routed` keeps
    /// ([`crate::hoststart::ChatSkills::routed`]).
    pub fn of(routed: Option<&Routed>) -> Self {
        routed.map_or(Self::BuiltIn, |routed| Self::Asked(routed.clone()))
    }

    /// What a reattach routes its set by: `None` for the built-in router.
    pub fn routed(self) -> Option<Routed> {
        match self {
            Self::BuiltIn => None,
            Self::Asked(routed) => Some(routed),
        }
    }
}

/// [`Kept`] as the reopen record spells it.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "by", rename_all = "kebab-case")]
enum KeptOnDisk {
    BuiltIn,
    Extension {
        record: Record,
        /// The router's answer, where it was taken: absent where the chat fell back.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        answer: Option<Vec<RecordHidden>>,
    },
}

impl From<Kept> for KeptOnDisk {
    fn from(kept: Kept) -> Self {
        match kept {
            Kept::BuiltIn => Self::BuiltIn,
            Kept::Asked(Routed { record, answer }) => Self::Extension {
                record,
                answer: answer.map(|hidden| {
                    hidden
                        .into_iter()
                        .map(|(skill, why)| RecordHidden {
                            skill,
                            why: why.to_string(),
                        })
                        .collect()
                }),
            },
        }
    }
}

impl TryFrom<KeptOnDisk> for Kept {
    type Error = String;

    /// Held to what a start writes: a routing's ULID, a router that is an extension's id or none,
    /// an error a reader sees whole, and each line of the answer a [`Line`].
    fn try_from(disk: KeptOnDisk) -> Result<Self, Self::Error> {
        let (record, answer) = match disk {
            KeptOnDisk::BuiltIn => return Ok(Self::BuiltIn),
            KeptOnDisk::Extension { record, answer } => (record, answer),
        };
        if ulid::Ulid::from_string(&record.routing).is_err() {
            return Err("a routing that is not a ULID".to_owned());
        }
        if !record.router.is_empty() && !crate::extension::an_id(&record.router) {
            return Err("a router that is not an extension's id".to_owned());
        }
        if let Some(bad) = record
            .hidden
            .iter()
            .find(|one| one.why != OVER_BUDGET && Line::new(&one.why).is_err())
        {
            return Err(format!("a reason for {:?} that is not one line", bad.skill));
        }
        let answer = answer
            .map(|hidden| {
                hidden
                    .into_iter()
                    .map(|one| Line::new(&one.why).map(|why| (one.skill, why)))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        let record = Record {
            error: record.error.as_deref().map(clipped),
            ..record
        };
        Ok(Self::Asked(Routed { record, answer }))
    }
}

/// `why` on one line a reader sees whole, cut at [`MOST_ERROR_CHARS`] characters with `…`.
fn clipped(why: &str) -> String {
    let line = purlis_unicode::marked(why.trim(), purlis_unicode::Breaks::Marked);
    if line.chars().count() <= MOST_ERROR_CHARS {
        return line;
    }
    let mut cut: String = line.chars().take(MOST_ERROR_CHARS).collect();
    cut.push('…');
    cut
}

fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// **Asks the router `chosen` which of `set`'s skills to hide**, at a chat's start (ADR 0102 §5,
/// §6), within `deadline` however `asker` behaves, and routes `set` by its answer within `budget`
/// with `pins`, or by the built-in router where it gave none the set takes. `None` for the
/// built-in router, which is asked nothing.
pub fn at_start(
    set: &SkillSet,
    chosen: Result<Chosen, String>,
    budget: Budget,
    pins: &[String],
    asker: Option<&Arc<dyn AskRouter>>,
    deadline: Duration,
) -> Option<Routed> {
    let began = Instant::now();
    let id = match chosen {
        Ok(Chosen::BuiltIn) => return None,
        Ok(Chosen::Extension(id)) => id,
        Err(why) => {
            return Some(fell_back(
                set,
                budget,
                pins,
                "",
                FellBack::Unchosen,
                why,
                began,
            ));
        }
    };
    let Some(asker) = asker else {
        let why = format!("'{id}' was not asked: this start asks no router extension");
        return Some(fell_back(
            set,
            budget,
            pins,
            &id,
            FellBack::Failed,
            why,
            began,
        ));
    };
    let candidates = set.candidates(budget, pins);
    let asked = candidates.skills.len();
    let skills: Vec<RouteSkill> = candidates
        .skills
        .into_iter()
        .map(|one| RouteSkill {
            name: one.name,
            description: one.description,
            pinned: one.pinned,
        })
        .collect();
    let spent = |skills: usize, chars: usize| RouteBudget { skills, chars };
    let question = Question {
        skills,
        budget: spent(budget.skills, budget.chars),
        besides: spent(candidates.besides.skills, candidates.besides.chars),
    };
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    let asking = Arc::clone(asker);
    let extension = id.clone();
    // A thread nobody joins: one still asking at the deadline is left to the executor, which
    // stops its program at the same deadline, so nothing it does holds the chat's start.
    let spawned = std::thread::Builder::new()
        .name(format!("router {id}"))
        .spawn(move || {
            let ask = question.ask();
            let _ = send.send(asking.ask(&extension, &ask));
        });
    if let Err(why) = spawned {
        let why = format!("purlis could not start asking '{id}': {why}");
        return Some(fell_back(
            set,
            budget,
            pins,
            &id,
            FellBack::Failed,
            why,
            began,
        ));
    }
    let answered = match receive.recv_timeout(deadline.saturating_sub(began.elapsed())) {
        Ok(answered) => answered,
        Err(_) => {
            let why = format!("'{id}' did not answer within {} ms", deadline.as_millis());
            return Some(fell_back(
                set,
                budget,
                pins,
                &id,
                FellBack::Slow,
                why,
                began,
            ));
        }
    };
    let doc = match answered {
        Ok(doc) => doc,
        Err(why) => {
            return Some(fell_back(
                set,
                budget,
                pins,
                &id,
                FellBack::Failed,
                why,
                began,
            ));
        }
    };
    let taken = crate::extension::studio::routed_of(&id, asked, &doc).and_then(|hidden| {
        set.clone()
            .routed_by(Router::Extension(id.clone()), budget, pins, &hidden)
            .map(|routed| (routed, hidden))
            .map_err(|why| format!("'{id}' answered a routing purlis does not take: {why}"))
    });
    match taken {
        Ok((routed, hidden)) => Some(Routed {
            record: record(&routed, &id, None, None, began),
            answer: Some(hidden),
        }),
        Err(why) => Some(fell_back(
            set,
            budget,
            pins,
            &id,
            FellBack::Wrong,
            why,
            began,
        )),
    }
}

/// The question a router is asked, owned, for the thread that asks it.
struct Question {
    skills: Vec<RouteSkill>,
    budget: RouteBudget,
    besides: RouteBudget,
}

impl Question {
    fn ask(&self) -> Ask<'_> {
        Ask::Route {
            skills: &self.skills,
            budget: self.budget,
            besides: self.besides,
        }
    }
}

/// The record of a routing of `set` that fell back on the built-in router, `why`.
fn fell_back(
    set: &SkillSet,
    budget: Budget,
    pins: &[String],
    router: &str,
    how: FellBack,
    why: String,
    began: Instant,
) -> Routed {
    let routed = set.clone().routed(budget, pins);
    Routed {
        record: record(&routed, router, Some(how), Some(why), began),
        answer: None,
    }
}

fn record(
    routed: &SkillSet,
    router: &str,
    fell_back: Option<FellBack>,
    error: Option<String>,
    began: Instant,
) -> Record {
    let hidden = routed
        .routing
        .iter()
        .flat_map(|routing| &routing.hidden)
        .map(|one| RecordHidden {
            skill: one.name.clone(),
            why: match &one.why {
                purlis_skills::HiddenWhy::OverBudget => OVER_BUDGET.to_owned(),
                purlis_skills::HiddenWhy::ByRouter(line) => line.to_string(),
            },
        })
        .collect();
    Record {
        routing: ulid::Ulid::generate().to_string(),
        // An extension's id as `chosen` held it (`extension::an_id`), or none for a choice
        // that names no router: never a string that needs escaping.
        router: router.to_owned(),
        ms: u64::try_from(began.elapsed().as_millis()).unwrap_or(u64::MAX),
        fell_back,
        error: error.map(|why| clipped(&why)),
        hidden,
    }
}

#[cfg(test)]
mod tests;
